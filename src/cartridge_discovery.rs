//! Shared cartridge discovery.
//!
//! The on-disk scan + identity validation + HELLO probe that classifies each
//! installed cartridge version directory as attachable (`Directory`) or
//! `Incompatible`. This is the single source of truth used by BOTH:
//!
//! - the engine, for the bundled `providers/` tree next to its binary, and
//! - `machfab-daemon`, for the user-installed cartridge tree.
//!
//! Keeping one implementation guarantees the two hosts accept exactly the same
//! cartridges and reject the rest with byte-identical verdicts. The host's
//! identity (channel / registry URL / fabric manifest version) is passed in via
//! [`DiscoveryIdentity`] rather than read from a compile-time constant, so the
//! same code serves a host built for any channel/registry.
//!
//! Managed layout (relative to the root passed to [`discover_cartridges`]):
//! `{root}/{slug}/{channel}/{name}/{version}/cartridge.json`.

use crate::bifaci::cartridge_json::{validate_registry_url_scheme, CartridgeJson, RegistryUrlSchemeResult};
use crate::bifaci::cartridge_repo::CartridgeChannel;
use crate::bifaci::cartridge_slug::slug_for;
use crate::bifaci::manifest::CapManifest;
use crate::bifaci::relay_switch::{CartridgeAttachmentError, CartridgeAttachmentErrorKind};
use crate::CapGroup;
use std::path::{Path, PathBuf};
use tracing::{error, warn};

/// The identity a host accepts cartridges for. A cartridge whose `cartridge.json`
/// diverges from this on channel, registry URL, registry scheme, or fabric
/// manifest version is surfaced as `Incompatible` — never hosted.
#[derive(Debug, Clone)]
pub struct DiscoveryIdentity {
    pub channel: CartridgeChannel,
    /// `Some(url)` for release/nightly hosts, `None` for dev hosts (cartridges
    /// then live under the reserved dev slug and any registry scheme is allowed).
    pub registry_url: Option<String>,
    pub fabric_manifest_version: u32,
}

impl DiscoveryIdentity {
    /// On-disk top-level slug for this identity (`dev` when `registry_url` is None).
    pub fn slug(&self) -> String {
        slug_for(self.registry_url.as_deref())
    }

    fn dev_mode(&self) -> bool {
        self.registry_url.is_none()
    }
}

/// A discovered cartridge version directory, classified.
///
/// - `Directory` — passed every identity check and its HELLO probe succeeded.
///   Its caps will be registered for dispatch.
/// - `Incompatible` — found on disk but failed a check. NOT spawned, caps never
///   enter the dispatch graph; surfaced with a structured `attachment_error` so
///   the UI can render the reason. This is the uniform surface for every
///   discovery-time rejection — no silent log-and-skip.
#[derive(Debug, Clone)]
pub enum DiscoveredCartridge {
    Directory {
        entry_point: PathBuf,
        version_dir: PathBuf,
        id: String,
        channel: CartridgeChannel,
        registry_url: Option<String>,
        version: String,
        cap_groups: Vec<CapGroup>,
    },
    Incompatible {
        version_dir: PathBuf,
        id: String,
        channel: CartridgeChannel,
        registry_url: Option<String>,
        version: String,
        error: CartridgeAttachmentError,
    },
}

/// Current wall-clock time as Unix seconds, for stamping
/// `CartridgeAttachmentError.detected_at_unix_seconds`. A pre-epoch clock
/// returns 0 (display-ordering only).
fn unix_seconds_now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Probe a cartridge binary for its capability surface.
///
/// Spawns the binary, performs the bifaci HELLO handshake, parses the manifest,
/// returns its full `cap_groups` (caps + adapter_urns), then kills the process.
/// A binary that fails to spawn, fails HELLO, or returns an unparseable manifest
/// is an error — the caller surfaces it as `HandshakeFailed`.
pub async fn probe_cartridge_cap_groups(path: &Path) -> anyhow::Result<Vec<CapGroup>> {
    use crate::{handshake, FrameReader, FrameWriter};
    use tokio::io::{BufReader, BufWriter};
    use tokio::process::Command;

    let mut child = Command::new(path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn cartridge {:?}: {}", path, e))?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("cartridge {:?} stdin pipe missing", path))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("cartridge {:?} stdout pipe missing", path))?;

    let mut reader = FrameReader::new(BufReader::new(stdout));
    let mut writer = FrameWriter::new(BufWriter::new(stdin));

    let result = handshake(&mut reader, &mut writer)
        .await
        .map_err(|e| anyhow::anyhow!("cartridge {:?} HELLO failed: {}", path, e))?;

    // SIGKILL immediately — we have the manifest and don't wait for a clean exit.
    if let Err(e) = child.start_kill() {
        warn!(path = %path.display(), error = %e, "probe_cartridge_cap_groups: start_kill failed (process may have already exited)");
    }

    let manifest: CapManifest = serde_json::from_slice(&result.manifest).map_err(|e| {
        let preview = String::from_utf8_lossy(&result.manifest[..result.manifest.len().min(500)]);
        anyhow::anyhow!("cartridge {:?} invalid manifest ({}): {}", path, e, preview)
    })?;

    Ok(manifest.cap_groups)
}

/// Discover every cartridge under `{cartridges_root}/{slug}/{channel}/`, where
/// slug+channel come from `identity`. Each cartridge name directory's newest
/// version is validated against `identity` and probed; the result is the full
/// classified roster (attachable + incompatible). An empty/absent scan root is
/// not an error — it yields an empty roster. A real IO failure reading an
/// existing scan root IS an error (it would otherwise masquerade as "no
/// cartridges installed").
pub async fn discover_cartridges(
    cartridges_root: &Path,
    identity: &DiscoveryIdentity,
) -> anyhow::Result<Vec<DiscoveredCartridge>> {
    let slug = identity.slug();
    let scan_root = cartridges_root.join(&slug).join(identity.channel.as_str());

    let mut discovered: Vec<DiscoveredCartridge> = Vec::new();
    if !scan_root.is_dir() {
        return Ok(discovered);
    }

    let name_entries = std::fs::read_dir(&scan_root)
        .map_err(|e| anyhow::anyhow!("read_dir({}): {}", scan_root.display(), e))?;

    for entry in name_entries {
        let entry = entry.map_err(|e| anyhow::anyhow!("read_dir entry in {}: {}", scan_root.display(), e))?;
        let name_dir = entry.path();

        if !name_dir.is_dir() {
            let file_name = name_dir.file_name().unwrap_or_default().to_string_lossy();
            if file_name != ".DS_Store" {
                error!(path = %name_dir.display(), "Unmanaged file in {{slug}}/{{channel}}/ — only cartridge name directories belong here");
            }
            continue;
        }

        let sub_entries = match std::fs::read_dir(&name_dir) {
            Ok(e) => e,
            Err(e) => {
                error!(dir = %name_dir.display(), error = %e, "Cannot read cartridge name directory");
                continue;
            }
        };

        let mut version_dirs: Vec<PathBuf> = Vec::new();
        for sub_entry in sub_entries.flatten() {
            let sub_path = sub_entry.path();
            if sub_path.is_dir() {
                version_dirs.push(sub_path);
            } else {
                let file_name = sub_path.file_name().unwrap_or_default().to_string_lossy();
                if file_name != ".DS_Store" {
                    error!(path = %sub_path.display(), "Unmanaged file inside cartridge name directory — only version directories belong here");
                }
            }
        }

        if version_dirs.is_empty() {
            error!(dir = %name_dir.display(), "Cartridge name directory contains no version subdirectories");
            continue;
        }

        // Prefer the newest version (lexical-descending on the version folder name).
        version_dirs.sort_by(|a, b| {
            let va = a.file_name().unwrap_or_default().to_string_lossy().to_string();
            let vb = b.file_name().unwrap_or_default().to_string_lossy().to_string();
            vb.cmp(&va)
        });
        let version_dir = &version_dirs[0];

        let path_derived_name = name_dir
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".to_string());
        let path_derived_version = version_dir
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".to_string());

        let detected_at = unix_seconds_now();

        let cj = match CartridgeJson::read_from_dir(version_dir, &slug) {
            Ok(cj) => cj,
            Err(e) => {
                error!(dir = %version_dir.display(), error = %e, "cartridge.json invalid — surfacing as incompatible");
                discovered.push(DiscoveredCartridge::Incompatible {
                    version_dir: version_dir.clone(),
                    id: path_derived_name.clone(),
                    channel: identity.channel,
                    registry_url: identity.registry_url.clone(),
                    version: path_derived_version.clone(),
                    error: CartridgeAttachmentError {
                        kind: CartridgeAttachmentErrorKind::ManifestInvalid,
                        message: format!("cartridge.json failed to load: {}", e),
                        detected_at_unix_seconds: detected_at,
                    },
                });
                continue;
            }
        };

        if cj.channel != identity.channel {
            discovered.push(DiscoveredCartridge::Incompatible {
                version_dir: version_dir.clone(),
                id: cj.name.clone(),
                channel: cj.channel,
                registry_url: cj.registry_url.clone(),
                version: cj.version.clone(),
                error: CartridgeAttachmentError {
                    kind: CartridgeAttachmentErrorKind::BadInstallation,
                    message: format!(
                        "Channel mismatch: cartridge declares '{}' but host is pinned to '{}'. Release and nightly artefacts must not mix.",
                        cj.channel, identity.channel
                    ),
                    detected_at_unix_seconds: detected_at,
                },
            });
            continue;
        }

        if cj.registry_url.as_deref() != identity.registry_url.as_deref() {
            discovered.push(DiscoveredCartridge::Incompatible {
                version_dir: version_dir.clone(),
                id: cj.name.clone(),
                channel: cj.channel,
                registry_url: cj.registry_url.clone(),
                version: cj.version.clone(),
                error: CartridgeAttachmentError {
                    kind: CartridgeAttachmentErrorKind::BadInstallation,
                    message: format!(
                        "registry_url mismatch: cartridge declares {:?} but host is pinned to {:?}. Cartridges from a different registry are a separate identity.",
                        cj.registry_url, identity.registry_url
                    ),
                    detected_at_unix_seconds: detected_at,
                },
            });
            continue;
        }

        if let Some(url) = cj.registry_url.as_deref() {
            match validate_registry_url_scheme(url, identity.dev_mode()) {
                RegistryUrlSchemeResult::Ok => {}
                RegistryUrlSchemeResult::NonHttps { scheme } => {
                    discovered.push(DiscoveredCartridge::Incompatible {
                        version_dir: version_dir.clone(),
                        id: cj.name.clone(),
                        channel: cj.channel,
                        registry_url: cj.registry_url.clone(),
                        version: cj.version.clone(),
                        error: CartridgeAttachmentError {
                            kind: CartridgeAttachmentErrorKind::Incompatible,
                            message: format!(
                                "registry_url uses '{}' scheme, must be https in non-dev builds. Rebuild the cartridge with an https registry URL.",
                                scheme
                            ),
                            detected_at_unix_seconds: detected_at,
                        },
                    });
                    continue;
                }
                RegistryUrlSchemeResult::NotAUrl(bad) => {
                    discovered.push(DiscoveredCartridge::Incompatible {
                        version_dir: version_dir.clone(),
                        id: cj.name.clone(),
                        channel: cj.channel,
                        registry_url: cj.registry_url.clone(),
                        version: cj.version.clone(),
                        error: CartridgeAttachmentError {
                            kind: CartridgeAttachmentErrorKind::Incompatible,
                            message: format!("registry_url '{}' is not a well-formed URL.", bad),
                            detected_at_unix_seconds: detected_at,
                        },
                    });
                    continue;
                }
            }
        }

        if cj.fabric_manifest_version != identity.fabric_manifest_version {
            discovered.push(DiscoveredCartridge::Incompatible {
                version_dir: version_dir.clone(),
                id: cj.name.clone(),
                channel: cj.channel,
                registry_url: cj.registry_url.clone(),
                version: cj.version.clone(),
                error: CartridgeAttachmentError {
                    kind: CartridgeAttachmentErrorKind::FabricManifestVersionMismatch,
                    message: format!(
                        "Cartridge built against fabric manifest version {}, but host is pinned to {}. Rebuild the cartridge with MFR_FABRIC_MANIFEST_VERSION={}.",
                        cj.fabric_manifest_version, identity.fabric_manifest_version, identity.fabric_manifest_version
                    ),
                    detected_at_unix_seconds: detected_at,
                },
            });
            continue;
        }

        let entry_point = cj.resolve_entry_point(version_dir);
        match probe_cartridge_cap_groups(&entry_point).await {
            Ok(cap_groups) => {
                discovered.push(DiscoveredCartridge::Directory {
                    entry_point,
                    version_dir: version_dir.clone(),
                    id: cj.name,
                    channel: cj.channel,
                    registry_url: cj.registry_url,
                    version: cj.version,
                    cap_groups,
                });
            }
            Err(e) => {
                error!(cartridge = %version_dir.display(), error = %e, "Failed to probe cartridge entry point — surfacing as incompatible");
                discovered.push(DiscoveredCartridge::Incompatible {
                    version_dir: version_dir.clone(),
                    id: cj.name,
                    channel: cj.channel,
                    registry_url: cj.registry_url,
                    version: cj.version,
                    error: CartridgeAttachmentError {
                        kind: CartridgeAttachmentErrorKind::HandshakeFailed,
                        message: format!("HELLO handshake / cap discovery probe failed: {}", e),
                        detected_at_unix_seconds: detected_at,
                    },
                });
            }
        }
    }

    Ok(discovered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    fn nightly_dev_identity() -> DiscoveryIdentity {
        DiscoveryIdentity {
            channel: CartridgeChannel::Nightly,
            registry_url: None,
            fabric_manifest_version: 1,
        }
    }

    /// Lay down `{root}/{slug}/{channel_folder}/{name}/{version}/`. When
    /// `cartridge_json` is `Some`, also write it plus an executable `entry`
    /// binary so `read_from_dir` accepts the directory and discovery reaches its
    /// own identity checks.
    fn install_fixture(
        root: &Path,
        slug: &str,
        channel_folder: &str,
        name: &str,
        version: &str,
        cartridge_json: Option<&str>,
        entry: &str,
    ) {
        let dir = root.join(slug).join(channel_folder).join(name).join(version);
        fs::create_dir_all(&dir).unwrap();
        if let Some(json) = cartridge_json {
            fs::write(dir.join("cartridge.json"), json).unwrap();
            let entry_path = dir.join(entry);
            fs::write(&entry_path, b"#!/bin/sh\nexit 0\n").unwrap();
            fs::set_permissions(&entry_path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    fn dev_cartridge_json(channel: &str, fabric_manifest_version: u32) -> String {
        format!(
            r#"{{"name":"cart","version":"1.0.0","channel":"{channel}","registry_url":null,"entry":"cart","installed_at":"2024-01-01T00:00:00Z","fabric_manifest_version":{fabric_manifest_version}}}"#
        )
    }

    fn expect_incompatible(out: &[DiscoveredCartridge], kind: CartridgeAttachmentErrorKind) {
        assert_eq!(out.len(), 1, "expected exactly one discovered entry");
        match &out[0] {
            DiscoveredCartridge::Incompatible { error, .. } => {
                assert_eq!(error.kind, kind, "wrong attachment-error kind: {}", error.message);
            }
            other => panic!("expected Incompatible({kind:?}), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test999_absent_scan_root_yields_empty_roster() {
        let root = tempdir().unwrap();
        let out = discover_cartridges(root.path(), &nightly_dev_identity())
            .await
            .unwrap();
        assert!(out.is_empty(), "no install tree must be an empty roster, not an error");
    }

    #[tokio::test]
    async fn test999_missing_cartridge_json_is_manifest_invalid() {
        let root = tempdir().unwrap();
        install_fixture(root.path(), "dev", "nightly", "cart", "1.0.0", None, "cart");
        let out = discover_cartridges(root.path(), &nightly_dev_identity())
            .await
            .unwrap();
        expect_incompatible(&out, CartridgeAttachmentErrorKind::ManifestInvalid);
    }

    #[tokio::test]
    async fn test999_channel_mismatch_is_bad_installation() {
        let root = tempdir().unwrap();
        // Declares release but lives under nightly/ — host is nightly.
        let json = dev_cartridge_json("release", 1);
        install_fixture(root.path(), "dev", "nightly", "cart", "1.0.0", Some(&json), "cart");
        let out = discover_cartridges(root.path(), &nightly_dev_identity())
            .await
            .unwrap();
        expect_incompatible(&out, CartridgeAttachmentErrorKind::BadInstallation);
    }

    #[tokio::test]
    async fn test999_fabric_manifest_mismatch_is_flagged() {
        let root = tempdir().unwrap();
        let json = dev_cartridge_json("nightly", 999);
        install_fixture(root.path(), "dev", "nightly", "cart", "1.0.0", Some(&json), "cart");
        let out = discover_cartridges(root.path(), &nightly_dev_identity())
            .await
            .unwrap();
        expect_incompatible(&out, CartridgeAttachmentErrorKind::FabricManifestVersionMismatch);
    }

    #[tokio::test]
    async fn test999_registry_url_under_dev_slug_is_rejected() {
        let root = tempdir().unwrap();
        // A non-null registry_url placed under the reserved dev slug violates the
        // three-place rule — read_from_dir rejects it before any host check.
        let json = r#"{"name":"cart","version":"1.0.0","channel":"nightly","registry_url":"https://cartridges.example.com/manifest","entry":"cart","installed_at":"2024-01-01T00:00:00Z","fabric_manifest_version":1}"#;
        install_fixture(root.path(), "dev", "nightly", "cart", "1.0.0", Some(json), "cart");
        let out = discover_cartridges(root.path(), &nightly_dev_identity())
            .await
            .unwrap();
        expect_incompatible(&out, CartridgeAttachmentErrorKind::ManifestInvalid);
    }
}
