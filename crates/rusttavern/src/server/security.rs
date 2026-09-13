//! Request security: IP whitelist, HTTP Basic authentication with HttpOnly
//! session cookies, and the CSRF header check for state-changing endpoints.
//!
//! Middleware order (per request):
//! 1. IP whitelist — peer outside the allowed CIDRs is rejected with 403.
//!    Entries may be a single IP, a CIDR range, or an upstream-style wildcard
//!    (`192.168.1.*`).
//! 2. CSRF header check — `/__tt/invoke/*` and `/__tt/stream/*` require the
//!    `x-tt-invoke` header (same-origin fetch sets it; cross-site forms cannot).
//! 3. Authentication — `authMode: basic` requires a valid session cookie or
//!    `Authorization: Basic` credentials; a successful Basic login issues a
//!    session cookie (HttpOnly + SameSite=Strict) on the response.
//! 4. `/__tt/health` is exempt from authentication (whitelist still applies).

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, State};
use axum::http::header;
use axum::http::{HeaderValue, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine;

use crate::server::config::{SecurityConfig, ServerConfig};

/// Name of the HttpOnly session cookie.
pub const SESSION_COOKIE_NAME: &str = "tt_session";
/// Session lifetime; every validated request slides the expiry forward.
const SESSION_TTL: Duration = Duration::from_secs(24 * 60 * 60);
/// Upper bound on concurrently stored session tokens. Clients that
/// re-authenticate with Basic on every request and never store the issued
/// cookie would otherwise grow the table without bound.
const MAX_SESSIONS: usize = 1024;
const SESSION_MAX_AGE_SECS: u64 = 24 * 60 * 60;
const BASIC_REALM: &str = "RustTavern";
/// Custom header required on state-changing endpoints (CSRF protection).
const CSRF_HEADER: &str = "x-tt-invoke";

/// Authentication mode resolved from `security.authMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    None,
    Basic,
}

/// A parsed IP/CIDR whitelist entry.
#[derive(Debug, Clone, PartialEq, Eq)]
enum IpCidr {
    V4 { network: u32, prefix: u8 },
    V6 { network: u128, prefix: u8 },
}

impl IpCidr {
    fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        if text.contains('*') {
            return Self::parse_wildcard(text);
        }
        let (ip_part, prefix_part) = match text.split_once('/') {
            Some((ip, prefix)) => (ip, Some(prefix)),
            None => (text, None),
        };
        let ip: IpAddr = ip_part
            .trim()
            .parse()
            .map_err(|_| format!("Invalid IP in security.whitelist entry '{text}'"))?;
        match ip {
            IpAddr::V4(v4) => {
                let prefix = match prefix_part {
                    Some(value) => value
                        .trim()
                        .parse::<u8>()
                        .map_err(|_| format!("Invalid CIDR prefix in '{text}'"))?,
                    None => 32,
                };
                if prefix > 32 {
                    return Err(format!("Invalid CIDR prefix in '{text}'"));
                }
                Ok(IpCidr::V4 {
                    network: u32::from(v4),
                    prefix,
                })
            }
            IpAddr::V6(v6) => {
                let prefix = match prefix_part {
                    Some(value) => value
                        .trim()
                        .parse::<u8>()
                        .map_err(|_| format!("Invalid CIDR prefix in '{text}'"))?,
                    None => 128,
                };
                if prefix > 128 {
                    return Err(format!("Invalid CIDR prefix in '{text}'"));
                }
                Ok(IpCidr::V6 {
                    network: u128::from(v6),
                    prefix,
                })
            }
        }
    }

    /// Parse a wildcard IPv4 entry such as `192.168.1.*`, which is equivalent
    /// to `192.168.1.0/24`. Upstream SillyTavern's `whitelist` accepts this
    /// form, so entries copied from an existing `config.yaml` keep working, and
    /// it is what most people reach for when allowing their own subnet.
    ///
    /// Trailing octets may be omitted (`192.168.*` is `192.168.0.0/16`), but a
    /// wildcard in the middle (`192.168.*.1`) has no CIDR equivalent and is
    /// rejected rather than silently widened.
    fn parse_wildcard(text: &str) -> Result<Self, String> {
        let parts: Vec<&str> = text.split('.').map(str::trim).collect();
        if parts.len() > 4 {
            return Err(format!("Invalid IP in security.whitelist entry '{text}'"));
        }

        let mut octets = [0u8; 4];
        // Number of leading octets with a concrete value; everything after them
        // must be a wildcard.
        let mut fixed = 0usize;
        for (index, part) in parts.iter().enumerate() {
            if *part == "*" {
                continue;
            }
            if index > fixed {
                return Err(format!(
                    "Invalid security.whitelist entry '{text}': only trailing octets may be '*' \
                     (for example '192.168.1.*'). Use a CIDR range for anything else."
                ));
            }
            octets[index] = part
                .parse::<u8>()
                .map_err(|_| format!("Invalid IP in security.whitelist entry '{text}'"))?;
            fixed = index + 1;
        }

        Ok(IpCidr::V4 {
            network: u32::from(Ipv4Addr::from(octets)),
            prefix: (fixed * 8) as u8,
        })
    }

    fn contains(&self, ip: IpAddr) -> bool {
        match (self, normalize_ip(ip)) {
            (IpCidr::V4 { network, prefix }, IpAddr::V4(addr)) => {
                let addr = u32::from(addr);
                let mask = if *prefix == 0 {
                    0
                } else {
                    u32::MAX << (32 - *prefix)
                };
                (addr & mask) == (*network & mask)
            }
            (IpCidr::V6 { network, prefix }, IpAddr::V6(addr)) => {
                let addr = u128::from(addr);
                let mask = if *prefix == 0 {
                    0
                } else {
                    u128::MAX << (128 - *prefix)
                };
                (addr & mask) == (*network & mask)
            }
            _ => false,
        }
    }
}

/// Normalize IPv4-mapped IPv6 addresses (`::ffff:127.0.0.1`) to plain IPv4 so
/// whitelist entries written as IPv4 keep matching on dual-stack sockets.
fn normalize_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(IpAddr::V6(v6)),
        _ => ip,
    }
}

/// Resolved security policy. Empty whitelist means "allow all peers".
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    pub auth_mode: AuthMode,
    username: String,
    password: String,
    whitelist: Vec<IpCidr>,
}

impl SecurityPolicy {
    pub fn from_config(config: &SecurityConfig) -> Result<Self, String> {
        let auth_mode = match config.auth_mode.trim() {
            "" | "none" => AuthMode::None,
            "basic" => AuthMode::Basic,
            other => {
                return Err(format!(
                    "Invalid security.authMode '{other}': expected 'none' or 'basic'"
                ));
            }
        };

        let username = config.username.clone().unwrap_or_default();
        let password = config.password.clone().unwrap_or_default();
        if auth_mode == AuthMode::Basic && (username.is_empty() || password.is_empty()) {
            return Err(
                "security.authMode is 'basic' but username/password are not configured".to_string(),
            );
        }

        let whitelist = config
            .whitelist
            .iter()
            .map(|entry| IpCidr::parse(entry))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            auth_mode,
            username,
            password,
            whitelist,
        })
    }

    /// True when the peer IP is allowed by the whitelist (or no whitelist is
    /// configured).
    pub fn is_ip_allowed(&self, ip: IpAddr) -> bool {
        if self.whitelist.is_empty() {
            return true;
        }
        let ip = normalize_ip(ip);
        self.whitelist.iter().any(|cidr| cidr.contains(ip))
    }

    fn authenticate(&self, username: &str, password: &str) -> bool {
        constant_time_eq(username, &self.username) && constant_time_eq(password, &self.password)
    }
}

/// True when the whitelist actually narrows who may connect.
///
/// An empty list means "allow all", and so does a `/0` entry, so neither counts
/// as an access control for the purpose of [`validate_bind_security`].
fn whitelist_restricts_peers(whitelist: &[String]) -> Result<bool, String> {
    let mut restricts = false;
    for entry in whitelist {
        let cidr = IpCidr::parse(entry)?;
        let allows_everything = matches!(
            cidr,
            IpCidr::V4 { prefix: 0, .. } | IpCidr::V6 { prefix: 0, .. }
        );
        if allows_everything {
            return Ok(false);
        }
        restricts = true;
    }
    Ok(restricts)
}

/// Fail-fast check: binding to a non-loopback address with no access control at
/// all would expose the server (and its data directory) to the network.
///
/// Either control is sufficient on its own:
///
/// - `authMode: basic` — every request must authenticate.
/// - a restricting `security.whitelist` — only listed peers can connect at all,
///   which is what a private LAN deployment usually wants (no password prompt on
///   every device, but the server is still unreachable from elsewhere).
///
/// A whitelist containing a `/0` entry is not a control and does not qualify.
///
/// Returns a warning for the caller to surface when the bind is allowed but has
/// no password. This runs before tracing is initialized, so the message cannot
/// be logged from here.
pub fn validate_bind_security(config: &ServerConfig) -> Result<Option<String>, String> {
    let host = config.host.trim();
    let is_loopback =
        host == "127.0.0.1" || host.eq_ignore_ascii_case("localhost") || host == "::1";
    if is_loopback {
        return Ok(None);
    }

    if !matches!(config.security.auth_mode.trim(), "" | "none") {
        return Ok(None);
    }

    if whitelist_restricts_peers(&config.security.whitelist)? {
        let count = config.security.whitelist.len();
        return Ok(Some(format!(
            "Bound to '{host}' with no password (security.authMode: none). Access is restricted \
             only by security.whitelist ({count} entr{}): anyone reaching this port from an \
             allowed address has full access to the data directory.",
            if count == 1 { "y" } else { "ies" }
        )));
    }

    Err(format!(
        "Refusing to start: binding to non-loopback address '{host}' without any access control. \
         Configure one of:\n  \
         security.authMode: basic  (plus username/password) — every request authenticates; or\n  \
         security.whitelist: [\"192.168.1.*\"] — only listed addresses may connect \
         (wildcard or CIDR, e.g. 192.168.1.0/24);\n\
         or bind to 127.0.0.1 for local-only access."
    ))
}

/// In-memory session store: token -> sliding expiry. Sessions vanish on
/// restart, which forces browsers to re-authenticate (same behavior as the
/// original SillyTavern session cookies).
#[derive(Debug, Default)]
pub struct SessionStore {
    sessions: Mutex<HashMap<String, Instant>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a session and return its token.
    pub fn create(&self) -> String {
        let token = random_hex_token();
        let mut sessions = self.sessions.lock().expect("session store poisoned");
        let now = Instant::now();
        // Opportunistic sweep: drop expired entries so repeated Basic
        // authentication (scripted clients that never store the cookie)
        // cannot grow the table without bound.
        sessions.retain(|_, expiry| *expiry > now);
        if sessions.len() >= MAX_SESSIONS {
            // Evict the entry expiring soonest to keep the table bounded.
            if let Some(oldest) = sessions
                .iter()
                .min_by_key(|(_, expiry)| **expiry)
                .map(|(token, _)| token.clone())
            {
                sessions.remove(&oldest);
            }
        }
        sessions.insert(token.clone(), now + SESSION_TTL);
        token
    }

    /// Validate a session token, sliding its expiry forward when valid.
    pub fn validate(&self, token: &str) -> bool {
        let mut sessions = self.sessions.lock().expect("session store poisoned");
        let now = Instant::now();
        let valid = match sessions.get_mut(token) {
            Some(expiry) if *expiry > now => {
                *expiry = now + SESSION_TTL;
                true
            }
            Some(_) => {
                sessions.remove(token);
                false
            }
            None => false,
        };
        // Opportunistic sweep of expired entries (same bound as create).
        sessions.retain(|_, expiry| *expiry > now);
        valid
    }
}

fn random_hex_token() -> String {
    let bytes: [u8; 32] = rand::random();
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Runtime security state shared with the auth middleware.
pub struct SecurityState {
    pub policy: SecurityPolicy,
    pub sessions: SessionStore,
}

/// Axum middleware implementing the whitelist -> CSRF -> auth chain.
pub async fn auth_middleware(
    State(security): State<Arc<SecurityState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    // 1. IP whitelist.
    let peer_ip = peer_ip(&request);
    if !security.policy.is_ip_allowed(peer_ip) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let path = request.uri().path();

    // 2. CSRF header for state-changing endpoints (invoke writes, streams
    //    subscribe). GET-only endpoints without side effects (events, files,
    //    static assets) are exempt: cross-site requests cannot attach the
    //    SameSite=Strict cookie anyway.
    if is_csrf_protected(path) && request.headers().get(CSRF_HEADER).is_none() {
        return StatusCode::FORBIDDEN.into_response();
    }

    // 3. Authentication (health endpoint is exempt).
    if security.policy.auth_mode == AuthMode::Basic && path != "/__tt/health" {
        let headers = request.headers();
        if let Some(token) = parse_cookie(headers, SESSION_COOKIE_NAME)
            && security.sessions.validate(&token)
        {
            // Authenticated by session cookie.
        } else if let Some((username, password)) = parse_basic_credentials(headers) {
            if !security.policy.authenticate(&username, &password) {
                return unauthorized_response();
            }
            // Successful Basic login: issue the session cookie on the response
            // so subsequent requests authenticate via cookie.
            let token = security.sessions.create();
            let mut response = next.run(request).await;
            response
                .headers_mut()
                .insert(header::SET_COOKIE, session_cookie_value(&token));
            return response;
        } else {
            return unauthorized_response();
        }
    }

    next.run(request).await
}

fn is_csrf_protected(path: &str) -> bool {
    path.starts_with("/__tt/invoke/") || path.starts_with("/__tt/stream/")
}

fn peer_ip(request: &Request<axum::body::Body>) -> IpAddr {
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip())
        .unwrap_or_else(|| IpAddr::V4(Ipv4Addr::UNSPECIFIED))
}

/// Parse `Authorization: Basic base64(user:pass)`.
fn parse_basic_credentials(headers: &axum::http::HeaderMap) -> Option<(String, String)> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, encoded) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("basic") {
        return None;
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .ok()?;
    let text = String::from_utf8(decoded).ok()?;
    let (username, password) = text.split_once(':')?;
    Some((username.to_string(), password.to_string()))
}

/// Extract the value of a cookie by name from the `Cookie` header.
fn parse_cookie(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in cookie.split(';') {
        if let Some((key, value)) = part.trim().split_once('=')
            && key.trim() == name
        {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn unauthorized_response() -> Response {
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(
            header::WWW_AUTHENTICATE,
            format!("Basic realm=\"{BASIC_REALM}\""),
        )
        .body(axum::body::Body::empty())
        .unwrap_or_else(|_| StatusCode::UNAUTHORIZED.into_response())
}

fn session_cookie_value(token: &str) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{SESSION_COOKIE_NAME}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={SESSION_MAX_AGE_SECS}"
    ))
    .expect("session cookie value is a valid header value")
}

/// Length-constant comparison to avoid trivial timing attacks on credentials.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use axum::body::Body;
    use axum::http::header;
    use axum::http::{HeaderMap, HeaderValue, Request};
    use tower::ServiceExt;

    use super::*;

    fn test_router(security: Arc<SecurityState>) -> axum::Router {
        axum::Router::new()
            .route("/__tt/invoke/ping", axum::routing::post(|| async { "pong" }))
            .route("/__tt/health", axum::routing::get(|| async { "ok" }))
            .route("/__tt/file", axum::routing::get(|| async { "file" }))
            .layer(axum::middleware::from_fn_with_state(
                security,
                auth_middleware,
            ))
    }

    fn request_with_peer(method: &str, uri: &str, peer: SocketAddr) -> Request<Body> {
        let mut request = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .expect("request builds");
        request
            .extensions_mut()
            .insert(ConnectInfo(peer));
        request
    }

    fn security_state(auth_mode: &str, whitelist: Vec<&str>) -> Arc<SecurityState> {
        let config = SecurityConfig {
            auth_mode: auth_mode.to_string(),
            username: Some("admin".to_string()),
            password: Some("secret".to_string()),
            whitelist: whitelist.into_iter().map(str::to_string).collect(),
        };
        let policy = SecurityPolicy::from_config(&config).expect("policy builds");
        Arc::new(SecurityState {
            policy,
            sessions: SessionStore::new(),
        })
    }

    #[test]
    fn cidr_parse_and_match_ipv4() {
        let cidr = IpCidr::parse("192.168.1.0/24").unwrap();
        assert!(cidr.contains("192.168.1.5".parse().unwrap()));
        assert!(!cidr.contains("192.168.2.5".parse().unwrap()));

        let exact = IpCidr::parse("10.0.0.7").unwrap();
        assert!(exact.contains("10.0.0.7".parse().unwrap()));
        assert!(!exact.contains("10.0.0.8".parse().unwrap()));

        let all = IpCidr::parse("0.0.0.0/0").unwrap();
        assert!(all.contains("8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn cidr_parse_and_match_ipv6() {
        let cidr = IpCidr::parse("fd00::/8").unwrap();
        assert!(cidr.contains("fd12:3456::1".parse().unwrap()));
        assert!(!cidr.contains("fe80::1".parse().unwrap()));

        let loopback = IpCidr::parse("::1/128").unwrap();
        assert!(loopback.contains("::1".parse().unwrap()));
        assert!(!loopback.contains("::2".parse().unwrap()));
    }

    #[test]
    fn ipv4_mapped_address_matches_ipv4_cidr() {
        let cidr = IpCidr::parse("127.0.0.0/8").unwrap();
        let mapped: IpAddr = "::ffff:127.0.0.1".parse().unwrap();
        assert!(cidr.contains(mapped));
    }

    #[test]
    fn invalid_cidr_is_rejected() {
        assert!(IpCidr::parse("not-an-ip").is_err());
        assert!(IpCidr::parse("1.2.3.4/33").is_err());
        assert!(IpCidr::parse("::1/129").is_err());
    }

    #[test]
    fn wildcard_entry_matches_its_subnet() {
        let cidr = IpCidr::parse("192.168.1.*").unwrap();
        assert_eq!(cidr, IpCidr::parse("192.168.1.0/24").unwrap());
        assert!(cidr.contains("192.168.1.1".parse().unwrap()));
        assert!(cidr.contains("192.168.1.254".parse().unwrap()));
        assert!(!cidr.contains("192.168.102.1".parse().unwrap()));

        let wider = IpCidr::parse("192.168.*").unwrap();
        assert_eq!(wider, IpCidr::parse("192.168.0.0/16").unwrap());
        assert!(wider.contains("192.168.102.1".parse().unwrap()));
        assert!(!wider.contains("192.169.0.1".parse().unwrap()));

        // Trailing wildcards may also be written out in full.
        assert_eq!(
            IpCidr::parse("10.*.*.*").unwrap(),
            IpCidr::parse("10.0.0.0/8").unwrap()
        );

        // A wildcard-only entry allows everything, like `0.0.0.0/0`.
        assert_eq!(
            IpCidr::parse("*").unwrap(),
            IpCidr::parse("0.0.0.0/0").unwrap()
        );
    }

    #[test]
    fn invalid_wildcard_entry_is_rejected() {
        // A wildcard in the middle has no CIDR equivalent.
        assert!(IpCidr::parse("192.168.*.1").is_err());
        assert!(IpCidr::parse("*.168.101.5").is_err());
        assert!(IpCidr::parse("192.999.*").is_err());
        assert!(IpCidr::parse("192.168.1.2.*").is_err());
    }

    #[test]
    fn wildcard_whitelist_is_enforced_by_the_policy() {
        let config = SecurityConfig {
            auth_mode: "none".to_string(),
            whitelist: vec!["192.168.1.*".to_string()],
            ..SecurityConfig::default()
        };
        let policy = SecurityPolicy::from_config(&config).expect("policy builds");
        assert!(policy.is_ip_allowed("192.168.1.42".parse().unwrap()));
        assert!(!policy.is_ip_allowed("192.168.1.42".parse().unwrap()));
    }

    #[test]
    fn policy_rejects_unknown_auth_mode() {
        let config = SecurityConfig {
            auth_mode: "token".to_string(),
            ..SecurityConfig::default()
        };
        assert!(SecurityPolicy::from_config(&config).is_err());
    }

    #[test]
    fn policy_requires_credentials_for_basic() {
        let config = SecurityConfig {
            auth_mode: "basic".to_string(),
            username: None,
            password: None,
            ..SecurityConfig::default()
        };
        assert!(SecurityPolicy::from_config(&config).is_err());
    }

    #[test]
    fn validate_bind_security_rejects_open_bind() {
        let config = ServerConfig {
            host: "0.0.0.0".to_string(),
            security: SecurityConfig {
                auth_mode: "none".to_string(),
                ..SecurityConfig::default()
            },
            ..dummy_server_config()
        };
        assert!(validate_bind_security(&config).is_err());

        let basic = ServerConfig {
            host: "0.0.0.0".to_string(),
            security: SecurityConfig {
                auth_mode: "basic".to_string(),
                username: Some("admin".to_string()),
                password: Some("secret".to_string()),
                ..SecurityConfig::default()
            },
            ..dummy_server_config()
        };
        // Authenticated: allowed, and nothing to warn about.
        assert_eq!(validate_bind_security(&basic), Ok(None));

        let loopback = ServerConfig {
            host: "127.0.0.1".to_string(),
            ..dummy_server_config()
        };
        assert_eq!(validate_bind_security(&loopback), Ok(None));
    }

    #[test]
    fn restricting_whitelist_authorizes_an_open_bind() {
        // A LAN deployment that pins the allowed subnet does not additionally
        // need a password prompt on every device, but the operator is warned
        // that there is no password.
        let lan = ServerConfig {
            host: "0.0.0.0".to_string(),
            security: SecurityConfig {
                auth_mode: "none".to_string(),
                whitelist: vec!["192.168.1.0/24".to_string(), "127.0.0.1".to_string()],
                ..SecurityConfig::default()
            },
            ..dummy_server_config()
        };
        let warning = validate_bind_security(&lan).expect("allowed").expect("warns");
        assert!(warning.contains("security.whitelist"), "{warning}");

        // The wildcard form people usually write for their own subnet.
        let wildcard = ServerConfig {
            host: "0.0.0.0".to_string(),
            security: SecurityConfig {
                auth_mode: "none".to_string(),
                whitelist: vec!["192.168.1.*".to_string()],
                ..SecurityConfig::default()
            },
            ..dummy_server_config()
        };
        assert!(validate_bind_security(&wildcard).expect("allowed").is_some());
    }

    #[test]
    fn all_addresses_whitelist_is_not_an_access_control() {
        for entry in ["0.0.0.0/0", "::/0", "*", "*.*.*.*"] {
            let config = ServerConfig {
                host: "0.0.0.0".to_string(),
                security: SecurityConfig {
                    auth_mode: "none".to_string(),
                    whitelist: vec![entry.to_string()],
                    ..SecurityConfig::default()
                },
                ..dummy_server_config()
            };
            assert!(
                validate_bind_security(&config).is_err(),
                "{entry} must not authorize an open bind"
            );
        }

        // A `/0` entry alongside real entries still opens everything up.
        let mixed = ServerConfig {
            host: "0.0.0.0".to_string(),
            security: SecurityConfig {
                auth_mode: "none".to_string(),
                whitelist: vec!["192.168.1.0/24".to_string(), "0.0.0.0/0".to_string()],
                ..SecurityConfig::default()
            },
            ..dummy_server_config()
        };
        assert!(validate_bind_security(&mixed).is_err());
    }

    #[test]
    fn invalid_whitelist_entry_fails_the_bind_check() {
        let config = ServerConfig {
            host: "0.0.0.0".to_string(),
            security: SecurityConfig {
                auth_mode: "none".to_string(),
                whitelist: vec!["not-an-ip".to_string()],
                ..SecurityConfig::default()
            },
            ..dummy_server_config()
        };
        assert!(validate_bind_security(&config).is_err());
    }

    fn dummy_server_config() -> ServerConfig {
        ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8000,
            data_root: std::path::PathBuf::from("data"),
            auto_open_browser: false,
            resources_root: std::path::PathBuf::from("resources"),
            web_root: std::path::PathBuf::from("web"),
            security: SecurityConfig::default(),
        }
    }

    #[test]
    fn basic_credentials_parse() {
        let mut headers = HeaderMap::new();
        let encoded = base64::engine::general_purpose::STANDARD.encode("admin:secret");
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Basic {encoded}")).unwrap(),
        );
        let (user, pass) = parse_basic_credentials(&headers).expect("credentials parse");
        assert_eq!(user, "admin");
        assert_eq!(pass, "secret");
    }

    #[test]
    fn session_store_create_and_validate() {
        let store = SessionStore::new();
        let token = store.create();
        assert!(store.validate(&token));
        assert!(!store.validate("bogus"));
    }

    #[tokio::test]
    async fn whitelist_rejects_foreign_peer() {
        let security = security_state("none", vec!["127.0.0.1"]);
        let response = test_router(security)
            .oneshot(request_with_peer(
                "GET",
                "/__tt/health",
                ([192, 168, 1, 5], 9999).into(),
            ))
            .await
            .expect("request completes");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn health_is_exempt_from_auth() {
        let security = security_state("basic", vec![]);
        let response = test_router(security)
            .oneshot(request_with_peer(
                "GET",
                "/__tt/health",
                ([127, 0, 0, 1], 1234).into(),
            ))
            .await
            .expect("request completes");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn unauthenticated_request_gets_401_with_challenge() {
        let security = security_state("basic", vec![]);
        let response = test_router(security)
            .oneshot(request_with_peer(
                "GET",
                "/__tt/file",
                ([127, 0, 0, 1], 1234).into(),
            ))
            .await
            .expect("request completes");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(response
            .headers()
            .get(header::WWW_AUTHENTICATE)
            .expect("challenge present")
            .to_str()
            .unwrap()
            .starts_with("Basic "));
    }

    #[tokio::test]
    async fn basic_login_issues_session_cookie() {
        let security = security_state("basic", vec![]);
        let mut headers = HeaderMap::new();
        let encoded = base64::engine::general_purpose::STANDARD.encode("admin:secret");
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Basic {encoded}")).unwrap(),
        );
        let mut request = request_with_peer("GET", "/__tt/file", ([127, 0, 0, 1], 1234).into());
        *request.headers_mut() = headers;

        let response = test_router(security)
            .oneshot(request)
            .await
            .expect("request completes");
        assert_eq!(response.status(), StatusCode::OK);
        let cookie = response
            .headers()
            .get(header::SET_COOKIE)
            .expect("cookie issued")
            .to_str()
            .unwrap()
            .to_string();
        assert!(cookie.starts_with("tt_session="));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Strict"));
    }

    #[tokio::test]
    async fn wrong_basic_credentials_rejected() {
        let security = security_state("basic", vec![]);
        let mut headers = HeaderMap::new();
        let encoded = base64::engine::general_purpose::STANDARD.encode("admin:wrong");
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Basic {encoded}")).unwrap(),
        );
        let mut request = request_with_peer("GET", "/__tt/file", ([127, 0, 0, 1], 1234).into());
        *request.headers_mut() = headers;

        let response = test_router(security)
            .oneshot(request)
            .await
            .expect("request completes");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(response.headers().get(header::SET_COOKIE).is_none());
    }

    #[tokio::test]
    async fn session_cookie_authenticates() {
        let security = security_state("basic", vec![]);
        let token = security.sessions.create();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("tt_session={token}")).unwrap(),
        );
        let mut request = request_with_peer("GET", "/__tt/file", ([127, 0, 0, 1], 1234).into());
        *request.headers_mut() = headers;

        let response = test_router(security.clone())
            .oneshot(request)
            .await
            .expect("request completes");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn csrf_header_required_for_invoke() {
        let security = security_state("none", vec![]);
        let response = test_router(security.clone())
            .oneshot(request_with_peer(
                "POST",
                "/__tt/invoke/ping",
                ([127, 0, 0, 1], 1234).into(),
            ))
            .await
            .expect("request completes");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let mut headers = HeaderMap::new();
        headers.insert(CSRF_HEADER, HeaderValue::from_static("1"));
        let mut request =
            request_with_peer("POST", "/__tt/invoke/ping", ([127, 0, 0, 1], 1234).into());
        *request.headers_mut() = headers;
        let response = test_router(security.clone())
            .oneshot(request)
            .await
            .expect("request completes");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn file_and_health_do_not_require_csrf_header() {
        let security = security_state("none", vec![]);
        let response = test_router(security)
            .oneshot(request_with_peer(
                "GET",
                "/__tt/file",
                ([127, 0, 0, 1], 1234).into(),
            ))
            .await
            .expect("request completes");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn session_store_bounded_by_max_sessions() {
        let store = SessionStore::new();
        for _ in 0..(MAX_SESSIONS + 16) {
            store.create();
        }
        let count = store.sessions.lock().expect("session store poisoned").len();
        assert!(
            count <= MAX_SESSIONS,
            "session table exceeded its bound: {count}"
        );
    }

    #[tokio::test]
    async fn valid_cookie_does_not_issue_new_session() {
        let security = security_state("basic", vec![]);
        let token = security.sessions.create();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("tt_session={token}")).unwrap(),
        );
        let mut request = request_with_peer("GET", "/__tt/file", ([127, 0, 0, 1], 1234).into());
        *request.headers_mut() = headers;

        let response = test_router(security.clone())
            .oneshot(request)
            .await
            .expect("request completes");
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response.headers().get(header::SET_COOKIE).is_none(),
            "a valid cookie must not mint a new session token"
        );
        let count = security
            .sessions
            .sessions
            .lock()
            .expect("session store poisoned")
            .len();
        assert_eq!(count, 1, "a valid cookie must not mint a new session token");
    }
}
