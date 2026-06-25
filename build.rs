//! Build-time bake of the fabric registry's pinned manifest version.
//!
//! Every Rust binary in the workspace that consumes the fabric registry
//! (engine, anything that resolves URNs, anything that writes
//! `cartridge.json`) needs to know which manifest version of the
//! registry it's tied to. The version is a single workspace-wide value
//! sourced from `fabric/manifest-version.txt`. The `dx` build pipeline
//! reads that file and exports it as `MFR_FABRIC_MANIFEST_VERSION` for
//! every `cargo` invocation it shells.
//!
//! This build script reads that env var and writes a generated
//! `fabric_manifest_version.rs` into `OUT_DIR`, which the crate
//! `include!`s. Two safety properties this guarantees:
//!
//!   1. A raw `cargo build` without the env var **fails the build**
//!      with a descriptive message. There is no implicit default — if
//!      a developer is building outside `dx`, that's an unsupported
//!      path and must be opted-into explicitly by exporting the var.
//!   2. The value is a `pub const u32` known at compile time, so every
//!      consumer can rely on it in `const` contexts (signatures of
//!      `FabricRegistry::new`, default fields on `CartridgeJson`, etc.).

use std::env;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-env-changed=MFR_FABRIC_MANIFEST_VERSION");

    let raw = env::var("MFR_FABRIC_MANIFEST_VERSION").unwrap_or_else(|_| {
        panic!(
            "MFR_FABRIC_MANIFEST_VERSION is not set. Every cargo invocation against the \
             MachineFabric workspace must export this variable, sourced from \
             fabric/manifest-version.txt. Run builds and tests through `dx` \
             (which exports it for you) instead of invoking cargo directly."
        );
    });

    let trimmed = raw.trim();
    let version: u32 = trimmed.parse().unwrap_or_else(|e| {
        panic!(
            "MFR_FABRIC_MANIFEST_VERSION must be a non-negative integer (got {:?}): {}",
            trimmed, e
        );
    });
    // 0 is reserved for legacy v0 cartridges already in the wild and is
    // never a valid bake target — the workspace builds only at v >= 1.
    if version < 1 {
        panic!(
            "MFR_FABRIC_MANIFEST_VERSION must be >= 1 (got {}). v0 is the implicit \
             pre-versioning state for legacy cartridges and is not a build target.",
            version
        );
    }

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by cargo");
    let dest = Path::new(&out_dir).join("fabric_manifest_version.rs");
    let body = format!(
        "/// Fabric registry manifest version this build is pinned to. Sourced from\n\
         /// `fabric/manifest-version.txt` at build time via `MFR_FABRIC_MANIFEST_VERSION`.\n\
         pub const FABRIC_MANIFEST_VERSION: u32 = {};\n",
        version
    );
    std::fs::write(&dest, body).unwrap_or_else(|e| {
        panic!(
            "failed to write {}: {}",
            dest.display(),
            e
        );
    });

    generate_bundled_provider_hashes(&out_dir);
}

/// Bake the expected content hashes of the build's BUNDLED providers
/// (datacartridge / fetchcartridge / modelcartridge — shipped inside the
/// engine/daemon/capdag-CLI binary) into a compile-time constant the discovery
/// path verifies against. Same mechanism as `MFR_FABRIC_MANIFEST_VERSION`: the
/// build pipeline computes the hashes (after building the provider binaries,
/// before compiling this crate's consumers) and exports them in
/// `MFR_BUNDLED_PROVIDER_HASHES` as a JSON object `{ "<name>": { "<version>":
/// "<sha256>" } }`.
///
/// ABSENT var ⇒ empty set (a build with no bundled providers — e.g. plain
/// `cargo test` of capdag — is valid). MALFORMED var ⇒ hard build failure (a
/// pipeline that sets it must set it correctly; a silent empty set would
/// disable integrity checking without anyone noticing).
fn generate_bundled_provider_hashes(out_dir: &str) {
    println!("cargo:rerun-if-env-changed=MFR_BUNDLED_PROVIDER_HASHES");

    let entries: Vec<(String, String, String)> = match env::var("MFR_BUNDLED_PROVIDER_HASHES") {
        Err(_) => Vec::new(),
        Ok(raw) if raw.trim().is_empty() => Vec::new(),
        Ok(raw) => parse_bundled_provider_hashes(&raw),
    };

    let mut body = String::from(
        "/// Expected content hashes of this build's bundled providers, baked from\n\
         /// `MFR_BUNDLED_PROVIDER_HASHES` at build time. `(name, version, sha256)`.\n\
         /// Empty when no providers were bundled. Discovery verifies any cartridge\n\
         /// marked `installed_from: bundle` against this set.\n\
         pub const BUNDLED_PROVIDER_HASHES: &[(&str, &str, &str)] = &[\n",
    );
    for (name, version, sha256) in &entries {
        // Values are validated below to be hex/identifier-safe, but emit via
        // escaped string literals regardless so codegen can never break.
        body.push_str(&format!(
            "    ({:?}, {:?}, {:?}),\n",
            name, version, sha256
        ));
    }
    body.push_str("];\n");

    let dest = Path::new(out_dir).join("bundled_provider_hashes.rs");
    std::fs::write(&dest, body)
        .unwrap_or_else(|e| panic!("failed to write {}: {}", dest.display(), e));
}

/// Parse `{ "<name>": { "<version>": "<sha256>" } }` into a flat, sorted
/// `(name, version, sha256)` list. Minimal hand-rolled validation (no serde
/// dep in build.rs): every leaf must be a 64-char lowercase hex string. Any
/// structural or value error panics the build.
fn parse_bundled_provider_hashes(raw: &str) -> Vec<(String, String, String)> {
    let value: serde_json::Value = serde_json::from_str(raw).unwrap_or_else(|e| {
        panic!("MFR_BUNDLED_PROVIDER_HASHES is not valid JSON: {e}");
    });
    let obj = value.as_object().unwrap_or_else(|| {
        panic!("MFR_BUNDLED_PROVIDER_HASHES must be a JSON object {{name: {{version: sha256}}}}");
    });
    let mut out: Vec<(String, String, String)> = Vec::new();
    for (name, versions) in obj {
        let versions = versions.as_object().unwrap_or_else(|| {
            panic!("MFR_BUNDLED_PROVIDER_HASHES['{name}'] must be an object {{version: sha256}}");
        });
        for (version, sha) in versions {
            let sha = sha.as_str().unwrap_or_else(|| {
                panic!("MFR_BUNDLED_PROVIDER_HASHES['{name}']['{version}'] must be a string sha256");
            });
            let is_hex64 = sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
            if !is_hex64 {
                panic!(
                    "MFR_BUNDLED_PROVIDER_HASHES['{name}']['{version}'] must be a 64-char lowercase hex sha256, got {sha:?}"
                );
            }
            out.push((name.clone(), version.clone(), sha.to_string()));
        }
    }
    out.sort();
    out
}
