//! Cartridge binary signature verification (minisign / ed25519).
//!
//! Registry-published cartridges ship a per-platform **pure binary** artifact
//! whose integrity is proven by an ed25519 detached signature in the
//! [minisign](https://jedisct1.github.io/minisign/) format. The signature text
//! is embedded in the cartridge-registry manifest (`cartridgeBuild.binary
//! .signature`) and a `.minisig` sidecar is published next to the `.bin` so
//! stock `minisign -V` can verify out-of-band.
//!
//! A binary's signature is checked against the release key that a chain-valid
//! release-key certificate authorizes (see [`super::release_cert`]). The trusted
//! ROOT public keys are baked into the binary at build time
//! (`MFR_CARTRIDGE_ROOT_PUBKEYS` → [`crate::CARTRIDGE_ROOT_PUBKEYS`]), paired
//! with the baked cartridge registry URL so a build that knows which registry to
//! download from also knows which roots that registry's artifacts must chain to;
//! `capdag/build.rs` enforces the pairing at compile time.
//!
//! Two verification primitives live here:
//! - [`verify_binary_signature`] — minisign-prehashed, via the zero-dependency
//!   `minisign-verify` crate. The runtime download-integrity path.
//! - [`raw_verify`] — raw ed25519 over exact bytes, for the certificate and
//!   manifest signatures.

use std::fmt::Write as _;

/// Verification failure for a signature. Every variant names its cause — a
/// caller must be able to distinguish "the registry manifest is corrupt"
/// (malformed key/signature) from "the artifact does not match its signature"
/// (tampering or wrong key).
#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    /// The configured public key is not a decodable minisign public key.
    #[error("malformed cartridge signing public key: {0}")]
    MalformedPublicKey(String),
    /// The signature text is not a decodable signature.
    #[error("malformed cartridge binary signature: {0}")]
    MalformedSignature(String),
    /// Key and signature decode, but the signature does not sign these bytes
    /// with this key — the artifact is tampered or signed by a different key.
    #[error("cartridge binary signature verification failed: {0}")]
    VerificationFailed(String),
}

/// Verify a minisign ed25519 signature over `bytes`.
///
/// `pubkey_b64` is the base64 public key (the second line of a `.pub` file,
/// `RW...`); `signature_text` is the full minisign signature document (the
/// content of a `.minisig` file / the manifest's embedded `binary.signature`).
///
/// Success means: the key decodes, the signature decodes, and the signature
/// (including its signed trusted comment) verifies over exactly these bytes.
pub fn verify_binary_signature(
    pubkey_b64: &str,
    signature_text: &str,
    bytes: &[u8],
) -> Result<(), SignatureError> {
    let public_key = minisign_verify::PublicKey::from_base64(pubkey_b64.trim())
        .map_err(|e| SignatureError::MalformedPublicKey(e.to_string()))?;
    let signature = minisign_verify::Signature::decode(signature_text.trim())
        .map_err(|e| SignatureError::MalformedSignature(e.to_string()))?;
    public_key
        .verify(bytes, &signature, false)
        .map_err(|e| SignatureError::VerificationFailed(e.to_string()))
}

/// Compile-time validation for `MFR_CARTRIDGE_ROOT_PUBKEYS` — the exact mirror
/// of [`crate::registry_url_from_build_env`].
///
/// Valid states:
/// - `None`    => dev build; no root keys are baked and registry cartridge
///                downloads / manifest verification are disabled at runtime.
/// - `Some(s)` where `s` is non-empty => published build; `s` is the
///                comma-separated list of base64 minisign ROOT public keys
///                (Root A, Root B, Root C) that release-key certificates must
///                verify against (2-of-3).
///
/// Invalid state:
/// - `Some("")` => the variable was exported empty. Neither a dev build nor a
///   usable root set — fail the compile so a build can never silently ship with
///   signature verification disabled while claiming a registry identity.
pub const fn root_pubkeys_from_build_env(raw: Option<&'static str>) -> Option<&'static str> {
    match raw {
        None => None,
        Some(keys) => {
            if keys.is_empty() {
                panic!(
                    "MFR_CARTRIDGE_ROOT_PUBKEYS must be unset for dev builds or set to a comma-separated list of base64 minisign root public keys for published builds; empty string is invalid"
                );
            }
            Some(keys)
        }
    }
}

/// Compile-time validation for `MFR_SIGNING_ENVIRONMENT` — the environment
/// label (`prod` / `staging`) release-key certificates are bound to. Baked
/// alongside the root set; a certificate issued for the other environment is
/// rejected even though the roots are shared, so a staging-signed manifest can
/// never stand in for a prod one.
pub const fn signing_environment_from_build_env(raw: Option<&'static str>) -> Option<&'static str> {
    match raw {
        None => None,
        Some(env) => {
            if env.is_empty() {
                panic!(
                    "MFR_SIGNING_ENVIRONMENT must be unset for dev builds or set to 'prod' or 'staging' for published builds; empty string is invalid"
                );
            }
            Some(env)
        }
    }
}

/// Split a baked comma-separated root-pubkey list into individual keys. Empty
/// segments are rejected at build time (build.rs validates every segment), so
/// this is a plain split for consumers.
pub fn split_root_pubkeys(baked: &str) -> Vec<&str> {
    baked
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

// ─── Raw ed25519 layer (certificate + manifest signature verification) ──────
//
// Release-key certificates and manifest signatures are raw ed25519 over the
// exact signed bytes (no minisign prehash). The keys are minisign keypairs; the
// helper below parses the minisign public-key format and hands the underlying
// ed25519 material to `ed25519-dalek`.

/// Byte layout of a minisign public key: alg(2) + keynum(8) + pk(32).
const MINISIGN_ALG_ED: &[u8; 2] = b"Ed";
const MINISIGN_PK_BYTES: usize = 42;

/// A minisign public key parsed down to its raw parts.
pub struct ParsedPublicKey {
    /// The key id (minisign keynum) as lowercase hex — a stable, human-
    /// greppable identifier for "which key signed this".
    pub key_id: String,
    /// The raw 32-byte ed25519 public key.
    pub ed25519_public_key: [u8; 32],
}

/// Parse a base64 minisign public key (the `RW…` value) into its raw ed25519
/// public key + key id.
pub fn parse_minisign_public_key(pubkey_b64: &str) -> Result<ParsedPublicKey, SignatureError> {
    let bytes = base64_decode(pubkey_b64.trim())
        .ok_or_else(|| SignatureError::MalformedPublicKey("not valid base64".to_string()))?;
    if bytes.len() != MINISIGN_PK_BYTES {
        return Err(SignatureError::MalformedPublicKey(format!(
            "expected {MINISIGN_PK_BYTES} bytes, got {}",
            bytes.len()
        )));
    }
    if &bytes[0..2] != MINISIGN_ALG_ED {
        return Err(SignatureError::MalformedPublicKey(
            "missing ed25519 'Ed' algorithm tag".to_string(),
        ));
    }
    let key_id = hex_lower(&bytes[2..10]);
    let mut ed25519_public_key = [0u8; 32];
    ed25519_public_key.copy_from_slice(&bytes[10..42]);
    Ok(ParsedPublicKey {
        key_id,
        ed25519_public_key,
    })
}

/// Verify a raw base64 ed25519 signature over exact bytes against a base64
/// minisign public key.
pub fn raw_verify(pubkey_b64: &str, signature_b64: &str, bytes: &[u8]) -> Result<(), SignatureError> {
    use ed25519_dalek::Verifier;
    let parsed = parse_minisign_public_key(pubkey_b64)?;
    let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&parsed.ed25519_public_key)
        .map_err(|e| SignatureError::MalformedPublicKey(e.to_string()))?;
    let sig_bytes = base64_decode(signature_b64.trim())
        .ok_or_else(|| SignatureError::MalformedSignature("not valid base64".to_string()))?;
    let sig_bytes: [u8; 64] = sig_bytes.try_into().map_err(|v: Vec<u8>| {
        SignatureError::MalformedSignature(format!("expected 64 bytes, got {}", v.len()))
    })?;
    let signature = ed25519_dalek::Signature::from_bytes(&sig_bytes);
    verifying_key
        .verify(bytes, &signature)
        .map_err(|e| SignatureError::VerificationFailed(e.to_string()))
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut acc, byte| {
        write!(acc, "{:02x}", byte).expect("writing hex into a String cannot fail");
        acc
    })
}

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 decode (padding required, no whitespace). Returns `None` on
/// any malformed input. Hand-rolled: capdag deliberately carries no
/// general-purpose base64 dependency, and the call sites here are small and
/// pinned by the fixture tests.
fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if bytes.is_empty() || bytes.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let mut buffer: u32 = 0;
    let mut bits = 0u32;
    let mut padding = 0usize;
    for (idx, &ch) in bytes.iter().enumerate() {
        if ch == b'=' {
            // Padding is only valid in the last two positions.
            if idx + 2 < bytes.len() {
                return None;
            }
            padding += 1;
            buffer <<= 6;
            bits += 6;
        } else {
            if padding > 0 {
                return None; // data after padding
            }
            let value = BASE64_ALPHABET.iter().position(|b| *b == ch)? as u32;
            buffer = (buffer << 6) | value;
            bits += 6;
        }
        if bits == 24 {
            out.push((buffer >> 16) as u8);
            out.push((buffer >> 8) as u8);
            out.push(buffer as u8);
            buffer = 0;
            bits = 0;
        }
    }
    out.truncate(out.len() - padding);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Committed real-crypto fixtures: capdag verifies pre-signed artifacts and
    // tampers them in-memory to prove rejection.
    const RELEASE_PUBKEY: &str = include_str!("../../tests/fixtures/nocommit/signing/release.pubkey");
    const WRONG_PUBKEY: &str = include_str!("../../tests/fixtures/nocommit/signing/wrong.pubkey");
    const ARTIFACT: &[u8] = include_bytes!("../../tests/fixtures/nocommit/signing/artifact.bin");
    const ARTIFACT_SIG: &str = include_str!("../../tests/fixtures/nocommit/signing/artifact.bin.minisig");

    // TEST8029: a real release-key signature verifies over the exact artifact
    // bytes with the release public key.
    #[test]
    fn test8029_valid_binary_signature_verifies() {
        verify_binary_signature(RELEASE_PUBKEY, ARTIFACT_SIG, ARTIFACT)
            .expect("the committed signature must verify over the committed artifact");
    }

    // TEST8030: flipping a single artifact byte breaks verification — the
    // signature vouches for exact bytes.
    #[test]
    fn test8030_tampered_artifact_rejected() {
        let mut tampered = ARTIFACT.to_vec();
        tampered[0] ^= 0x01;
        let err = verify_binary_signature(RELEASE_PUBKEY, ARTIFACT_SIG, &tampered)
            .expect_err("a flipped byte must fail verification");
        assert!(matches!(err, SignatureError::VerificationFailed(_)), "got: {err:?}");
    }

    // TEST8031: the same signature under a DIFFERENT public key fails even over
    // the identical, untampered artifact — verification binds the key.
    #[test]
    fn test8031_wrong_key_rejected() {
        let err = verify_binary_signature(WRONG_PUBKEY, ARTIFACT_SIG, ARTIFACT)
            .expect_err("a signature must not verify under a different key");
        assert!(matches!(err, SignatureError::VerificationFailed(_)), "got: {err:?}");
    }

    // TEST8044: `parse_minisign_public_key` extracts a 32-byte ed25519 key and
    // an 8-byte (16-hex) keynum from a real minisign public key.
    #[test]
    fn test8044_parse_release_pubkey() {
        let parsed = parse_minisign_public_key(RELEASE_PUBKEY).expect("release pubkey must parse");
        assert_eq!(parsed.key_id.len(), 16, "keynum is 8 bytes = 16 hex chars");
        assert!(parsed.key_id.chars().all(|c| c.is_ascii_hexdigit()));
        // Malformed input is rejected cleanly.
        assert!(matches!(
            parse_minisign_public_key("not base64!!"),
            Err(SignatureError::MalformedPublicKey(_))
        ));
    }

    // TEST8038: `root_pubkeys_from_build_env` passes a non-empty baked root set
    // through unchanged, and `split_root_pubkeys` yields each key (mirror of
    // TEST1872 for the registry URL).
    #[test]
    fn test8038_root_pubkeys_from_build_env_passes_through_nonempty() {
        let keys = "RWRootAKeyBase64,RWRootBKeyBase64,RWRootCKeyBase64";
        assert_eq!(root_pubkeys_from_build_env(Some(keys)), Some(keys));
        assert_eq!(
            split_root_pubkeys(keys),
            vec!["RWRootAKeyBase64", "RWRootBKeyBase64", "RWRootCKeyBase64"]
        );
        assert_eq!(signing_environment_from_build_env(Some("staging")), Some("staging"));
    }

    // TEST8045: absent env ⇒ dev build ⇒ None for both the root set and the
    // environment label (mirror of TEST1873).
    #[test]
    fn test8045_root_pubkeys_from_build_env_none_for_dev() {
        assert_eq!(root_pubkeys_from_build_env(None), None);
        assert_eq!(signing_environment_from_build_env(None), None);
    }

    // TEST8046: an exported-but-empty root set is a hard failure — a build must
    // never silently ship with verification disabled (mirror of TEST1874).
    #[test]
    #[should_panic(expected = "MFR_CARTRIDGE_ROOT_PUBKEYS")]
    fn test8046_root_pubkeys_from_build_env_panics_on_empty() {
        let _ = root_pubkeys_from_build_env(Some(""));
    }
}
