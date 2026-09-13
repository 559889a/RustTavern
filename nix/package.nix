{
  lib,
  stdenv,
  rustPlatform,
  fetchPnpmDeps,
  nodejs_22,
  openssl,
  pkg-config,
  pnpm_10,
  pnpmConfigHook,
  src,
  gitBranch ? "main",
  gitRevision ? "",
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "rusttavern";
  version = "2.2.0";

  inherit src;

  cargoRoot = ".";
  buildAndTestSubdir = "crates/rusttavern";
  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  pnpmDeps = fetchPnpmDeps {
    inherit (finalAttrs)
      pname
      version
      src
      ;
    pnpm = pnpm_10;
    fetcherVersion = 3;
    hash = "sha256-aDxsMBQcMWYJl4FPTo+cReYnkqbiMuSvKRzslwmkGVM=";
  };

  nativeBuildInputs = [
    nodejs_22
    pkg-config
    pnpm_10
    pnpmConfigHook
  ];

  buildInputs = [
    openssl
  ];

  # The server ships the frontend sources and resources next to the binary,
  # matching the release zip layout expected by config.rs (resources root =
  # executable directory, web root = <exe dir>/src).
  postInstall = ''
    mkdir -p "$out/bin/src/scripts/templates" "$out/bin/default"
    cp -r src/. "$out/bin/src/"
    cp -r default/. "$out/bin/default/"
  '';

  # The repository harness owns the intentional feature matrix; the generic
  # buildRustPackage check phase is not a supported test configuration for
  # this workspace.
  doCheck = false;

  env = {
    RUSTTAVERN_BUILD_BRANCH = gitBranch;
    RUSTTAVERN_BUILD_REVISION = gitRevision;
  };

  meta = {
    description = "SillyTavern-compatible HTTP server with Rust backend";
    homepage = "https://github.com/559889a/RustTavern";
    license = lib.licenses.agpl3Only;
    mainProgram = "rusttavern";
    platforms = [
      "x86_64-linux"
      "aarch64-linux"
    ];
  };
})
