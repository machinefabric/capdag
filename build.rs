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
}
