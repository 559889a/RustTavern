use std::borrow::Cow;

use base64::Engine;

use crate::presentation::errors::CommandError;

/// Decode a base64-encoded chunk payload (the `data` field of a chunked
/// invoke). The old Tauri raw-body path is gone; HTTP chunked commands always
/// use base64-JSON through the generic dispatch.
pub fn decode_base64_chunk(data: &str) -> Result<Cow<'_, [u8]>, CommandError> {
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map(Cow::Owned)
        .map_err(|_| CommandError::BadRequest("Base64 chunk body is invalid".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_chunk_decodes() {
        let bytes = decode_base64_chunk("AQIDBA==").expect("base64 bytes should parse");
        assert_eq!(bytes.as_ref(), &[1, 2, 3, 4]);
    }

    #[test]
    fn malformed_base64_chunks_are_rejected() {
        assert!(matches!(
            decode_base64_chunk("***"),
            Err(CommandError::BadRequest(_))
        ));
    }
}
