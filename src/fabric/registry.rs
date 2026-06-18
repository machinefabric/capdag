//! Unified fabric registry: caps + media defs.
//!
//! Two domain payload types:
//! - `Cap` (cap definitions) at `<base>/caps/<sha256-of-canonical-urn>`
//! - `StoredMediaDef` (media defs) at `<base>/media/<sha256-of-canonical-urn>`
//!
//! On disk:
//! - `<cache_dir>/caps/<sha256>.json`
//! - `<cache_dir>/media/<sha256>.json`
//!
//! Resolution policy (same for both domains):
//!   1. In-memory cache hit → return immediately.
//!   2. Synchronous fetch attempt with hard 500 ms deadline.
//!   3. Deadline miss / error → enqueue for background consumer, return
//!      `None` (sync surface) or `Err` (async surface).
//!
//! The cap fetch is **atomic**: if any media URN referenced by a cap fails
//! to fetch, the cap is NOT cached. This guarantees that any cap landing
//! in the cap cache has every one of its referenced media defs already in
//! the media cache (and the extension index).

use crate::cap::definition::ArgSource;
use crate::media::spec::MediaDef;
use crate::Cap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, watch};

const DEFAULT_REGISTRY_BASE_URL: &str = "https://fabric.capdag.com";

/// Wall-clock TTL retained only for the v0 (legacy, flat-path) resolution
/// mode. Versioned objects at v >= 1 are immutable by protocol — once a
/// definition is published at `caps/<sha>/<defver>.json`, its bytes
/// never change — so versioned cache entries never expire.
const CACHE_DURATION_HOURS: u64 = 24;

/// Hard wall-clock budget for the synchronous fetch attempt that
/// `get_cached_cap` and `get_cached_media_def` each make on a cache
/// miss. Anything that doesn't return inside this window times out and
/// falls through to the queue path; the next call hits warm cache.
const SYNC_FETCH_DEADLINE: Duration = Duration::from_millis(500);

// =============================================================================
// CONFIGURATION
// =============================================================================

/// Configuration for the fabric registry.
///
/// Sources, in priority order:
/// 1. Builder methods.
/// 2. Environment variables (`CDG_FABRIC_REGISTRY_URL`, `CDG_SCHEMA_BASE_URL`).
/// 3. Defaults: `https://fabric.capdag.com` for the registry, `<registry>/schema`
///    for schemas.
#[derive(Debug, Clone)]
pub struct RegistryConfig {
    pub registry_base_url: String,
    pub schema_base_url: String,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        let registry_base = env::var("CDG_FABRIC_REGISTRY_URL")
            .unwrap_or_else(|_| DEFAULT_REGISTRY_BASE_URL.to_string());
        let schema_base =
            env::var("CDG_SCHEMA_BASE_URL").unwrap_or_else(|_| format!("{}/schema", registry_base));
        Self {
            registry_base_url: registry_base,
            schema_base_url: schema_base,
        }
    }
}

impl RegistryConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_registry_url(mut self, url: impl Into<String>) -> Self {
        let url = url.into();
        if self.schema_base_url == format!("{}/schema", self.registry_base_url) {
            self.schema_base_url = format!("{}/schema", url);
        }
        self.registry_base_url = url;
        self
    }

    pub fn with_schema_url(mut self, url: impl Into<String>) -> Self {
        self.schema_base_url = url.into();
        self
    }
}

// =============================================================================
// PAYLOAD TYPES
// =============================================================================

/// Stored media def format (matches registry API response)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoredMediaDef {
    pub urn: String,
    /// Per-definition version. 0 ⇒ v0 (frozen flat-path); >= 1 ⇒ pinned
    /// at `media/<sha256-of-urn>/<version>.json` and referenced by a
    /// manifest at that defver.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub version: u32,
    pub media_type: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<crate::MediaValidation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<String>,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

impl StoredMediaDef {
    pub fn to_media_def_def(&self) -> MediaDef {
        MediaDef {
            urn: self.urn.clone(),
            media_type: self.media_type.clone(),
            title: self.title.clone(),
            profile_uri: self.profile_uri.clone(),
            schema: self.schema.clone(),
            description: self.description.clone(),
            documentation: self.documentation.clone(),
            validation: self.validation.clone(),
            metadata: self.metadata.clone(),
            extensions: self.extensions.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CapCacheEntry {
    definition: Cap,
    cached_at: u64,
    ttl_hours: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MediaCacheEntry {
    spec: StoredMediaDef,
    cached_at: u64,
    ttl_hours: u64,
}

trait CacheEntryExt {
    fn cached_at(&self) -> u64;
    fn ttl_hours(&self) -> u64;
    fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now > self.cached_at() + (self.ttl_hours() * 3600)
    }
}
impl CacheEntryExt for CapCacheEntry {
    fn cached_at(&self) -> u64 {
        self.cached_at
    }
    fn ttl_hours(&self) -> u64 {
        self.ttl_hours
    }
}
impl CacheEntryExt for MediaCacheEntry {
    fn cached_at(&self) -> u64 {
        self.cached_at
    }
    fn ttl_hours(&self) -> u64 {
        self.ttl_hours
    }
}

// =============================================================================
// URN NORMALISATION
// =============================================================================

fn normalize_cap_urn(urn: &str) -> String {
    match crate::CapUrn::from_string(urn) {
        Ok(parsed) => parsed.to_string(),
        Err(_) => urn.to_string(),
    }
}

fn normalize_media_urn(urn: &str) -> String {
    match crate::MediaUrn::from_string(urn) {
        Ok(parsed) => parsed.to_string(),
        Err(_) => urn.to_string(),
    }
}

/// Distinguishes domain on the background-fetch queue. Pairs URN with
/// defver so the consumer always hits the right R2 path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum FetchKey {
    Cap { urn: String, defver: u32 },
    Media { urn: String, defver: u32 },
}

/// A versioned registry snapshot. Mirrors `fabric/manifest.schema.json`
/// on the wire.
///
/// v0 (the implicit pre-versioning state) has no manifest object — the
/// registry resolves URNs via the frozen flat R2 paths in that mode.
/// Manifests at version >= 1 explicitly name every URN that belongs to
/// the snapshot, paired with the defver at which it is published.
///
/// A defver of 0 in this manifest's `caps` or `media` map means the
/// entry resolves through the legacy flat path; that is allowed by the
/// wire schema even though no source TOML produces a v0 def.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub previous: u32,
    #[serde(default)]
    pub caps: HashMap<String, u32>,
    #[serde(default)]
    pub media: HashMap<String, u32>,
}

impl Manifest {
    /// Build an empty manifest pinned at `version`. `previous` is set to
    /// `version - 1` so re-publishing the same content stays byte-stable.
    pub fn empty(version: u32) -> Self {
        Self {
            version,
            previous: version.saturating_sub(1),
            caps: HashMap::new(),
            media: HashMap::new(),
        }
    }
}

// =============================================================================
// REGISTRY
// =============================================================================

#[derive(Debug)]
pub struct FabricRegistry {
    client: reqwest::Client,
    /// Root cache directory. Caps and media defs live in `caps/` and
    /// `media/` subdirectories respectively, mirroring the registry's
    /// own URL layout. v0 entries live at `caps/<sha>.json` and
    /// `media/<sha>.json`; v >= 1 entries live at `caps/<sha>/<defver>.json`
    /// and `media/<sha>/<defver>.json`. Manifests live in `manifests/<N>.json`.
    cache_dir: PathBuf,
    cached_caps: Arc<Mutex<HashMap<String, Cap>>>,
    cached_media_defs: Arc<Mutex<HashMap<String, StoredMediaDef>>>,
    /// Lower-case extension → list of canonical media URNs.
    extension_index: Arc<Mutex<HashMap<String, Vec<String>>>>,
    config: RegistryConfig,
    /// Fabric manifest version this registry is pinned to. 0 means
    /// legacy v0 / flat-path resolution (the implicit pre-versioning
    /// mode). >= 1 means manifest-driven resolution. Set at construction
    /// from the caller (engine bakes `capdag::FABRIC_MANIFEST_VERSION`).
    manifest_version: u32,
    /// Live snapshot of the registry pinned at `manifest_version`. For
    /// v0 this is an `empty(0)` placeholder and never consulted for
    /// resolution. For v >= 1 every URN lookup hits this map first to
    /// turn the URN into a `(urn, defver)` pair before fetching.
    /// Wrapped in Mutex because test helpers like `add_caps_to_cache`
    /// mutate it.
    manifest: Arc<Mutex<Manifest>>,
    offline_flag: Arc<AtomicBool>,
    fetch_queue_tx: Option<mpsc::UnboundedSender<FetchKey>>,
    fetch_in_queue: Arc<Mutex<HashSet<FetchKey>>>,
    cache_revision_tx: watch::Sender<u64>,
}

impl FabricRegistry {
    /// Create a new fabric registry pinned at the workspace-baked
    /// `capdag::FABRIC_MANIFEST_VERSION`. Standard entry point — engine
    /// code that doesn't specifically need a different version uses this.
    pub async fn new() -> Result<Self, FabricRegistryError> {
        Self::with_config_and_manifest_version(
            RegistryConfig::default(),
            crate::FABRIC_MANIFEST_VERSION,
        )
        .await
    }

    /// Create a new fabric registry with custom configuration, pinned at
    /// the workspace-baked manifest version.
    pub async fn with_config(config: RegistryConfig) -> Result<Self, FabricRegistryError> {
        Self::with_config_and_manifest_version(config, crate::FABRIC_MANIFEST_VERSION).await
    }

    /// Full constructor: custom config + explicit pinned manifest version.
    ///
    /// `manifest_version == 0` → legacy v0 / flat-path mode. No manifest
    /// fetch is performed; resolution falls through to the frozen flat
    /// R2 paths.
    ///
    /// `manifest_version >= 1` → manifest-driven. The constructor
    /// **blocks** on a network round-trip to fetch `manifest/<N>.json`
    /// if no local cache copy is present. If neither local cache nor
    /// network can provide it, the constructor returns
    /// `FabricRegistryError::NotFound`. There is no fallback to v0.
    pub async fn with_config_and_manifest_version(
        config: RegistryConfig,
        manifest_version: u32,
    ) -> Result<Self, FabricRegistryError> {
        let cache_dir = Self::default_cache_root()?;
        let caps_dir = cache_dir.join("caps");
        let media_dir = cache_dir.join("media");
        let manifests_dir = cache_dir.join("manifests");
        for d in [&caps_dir, &media_dir, &manifests_dir] {
            fs::create_dir_all(d).map_err(|e| {
                FabricRegistryError::CacheError(format!(
                    "Failed to create cache directory {:?}: {}",
                    d, e
                ))
            })?;
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| {
                FabricRegistryError::HttpError(format!("Failed to create HTTP client: {}", e))
            })?;

        // Bootstrap the manifest before loading on-disk caches so the
        // cache loaders can hydrate the in-memory map with entries
        // matching the manifest's pinned defvers (rather than blindly
        // pulling in stale v0 flat-path bytes that may belong to a
        // different snapshot).
        let manifest = if manifest_version == 0 {
            Manifest::empty(0)
        } else {
            load_or_fetch_manifest(&manifests_dir, &client, &config, manifest_version).await?
        };

        let mut cached_caps_map = Self::load_all_cached_caps(&caps_dir)?;
        let mut cached_specs_map = Self::load_all_cached_media_defs(&media_dir)?;
        // Filter loaded caches by manifest pin: only retain entries
        // whose URN's defver in the manifest matches the cached entry's
        // own version. At v0 the manifest is empty and we retain
        // everything (the load function only walks flat paths anyway
        // because no versioned subdirs are written under v0 mode).
        if manifest_version >= 1 {
            cached_caps_map.retain(|urn, cap| {
                manifest.caps.get(urn).copied().unwrap_or(0) == cap.version
            });
            cached_specs_map.retain(|urn, spec| {
                manifest.media.get(urn).copied().unwrap_or(0) == spec.version
            });
        }
        let extension_index_map = Self::build_extension_index(&cached_specs_map);

        let cached_caps = Arc::new(Mutex::new(cached_caps_map));
        let cached_media_defs = Arc::new(Mutex::new(cached_specs_map));
        let extension_index = Arc::new(Mutex::new(extension_index_map));
        let manifest_arc = Arc::new(Mutex::new(manifest));
        let fetch_in_queue = Arc::new(Mutex::new(HashSet::new()));
        let offline_flag = Arc::new(AtomicBool::new(false));
        let (cache_revision_tx, _) = watch::channel(0u64);

        let fetch_queue_tx = match tokio::runtime::Handle::try_current() {
            Ok(_) => {
                let (tx, rx) = mpsc::unbounded_channel::<FetchKey>();
                tokio::spawn(run_fetch_consumer(
                    rx,
                    client.clone(),
                    cache_dir.clone(),
                    Arc::clone(&cached_caps),
                    Arc::clone(&cached_media_defs),
                    Arc::clone(&extension_index),
                    Arc::clone(&manifest_arc),
                    Arc::clone(&fetch_in_queue),
                    Arc::clone(&offline_flag),
                    config.clone(),
                    cache_revision_tx.clone(),
                ));
                Some(tx)
            }
            Err(_) => None,
        };

        let registry = Self {
            client,
            cache_dir,
            cached_caps,
            cached_media_defs,
            extension_index,
            config,
            manifest_version,
            manifest: manifest_arc,
            offline_flag,
            fetch_queue_tx,
            fetch_in_queue,
            cache_revision_tx,
        };

        // The identity cap is the protocol-mandatory categorical
        // identity morphism — every capset must contain it. Seed it
        // into the in-memory cap cache directly (no network round-trip,
        // no disk write) so it is always available even on a fresh
        // install with no prior cache.
        registry.ensure_identity_cap();

        Ok(registry)
    }

    /// Returns the manifest version this registry is pinned to.
    pub fn manifest_version(&self) -> u32 {
        self.manifest_version
    }

    pub fn config(&self) -> &RegistryConfig {
        &self.config
    }

    pub fn set_offline(&self, offline: bool) {
        self.offline_flag.store(offline, Ordering::Relaxed);
    }

    pub fn subscribe_cache_revisions(&self) -> watch::Receiver<u64> {
        self.cache_revision_tx.subscribe()
    }

    fn default_cache_root() -> Result<PathBuf, FabricRegistryError> {
        let mut cache_dir = dirs::cache_dir().ok_or_else(|| {
            FabricRegistryError::CacheError("Could not determine cache directory".to_string())
        })?;
        cache_dir.push("capdag");
        Ok(cache_dir)
    }

    fn ensure_identity_cap(&self) {
        use crate::standard::caps::identity_cap;
        // STANDARD_CAPS travel with the manifest: their per-def version
        // is always the registry's pinned manifest version. The
        // publisher applies the same rule on the wire so the bytes on
        // R2 carry `version = manifestVersion` for every snapshot.
        let mut identity = identity_cap();
        identity.version = self.manifest_version;
        let urn = identity.urn_string();
        let normalized_urn = normalize_cap_urn(&urn);
        if let Ok(mut cached_caps) = self.cached_caps.lock() {
            if !cached_caps.contains_key(&normalized_urn) {
                cached_caps.insert(normalized_urn.clone(), identity);
            }
        }
        // Record the identity cap's defver in the manifest so any
        // resolution that consults the manifest finds it. At v0 this is
        // a no-op (manifest is `empty(0)`, never consulted).
        if self.manifest_version >= 1 {
            if let Ok(mut m) = self.manifest.lock() {
                m.caps.insert(normalized_urn, self.manifest_version);
            }
        }
    }

    // -------------------------------------------------------------------------
    // CAP API
    // -------------------------------------------------------------------------

    /// Get a cap from in-memory cache or fetch from registry. Atomic with
    /// respect to referenced media defs: a cap whose media-def footprint
    /// can't be fully fetched is not cached and the call returns `Err`.
    pub async fn get_cap(&self, urn: &str) -> Result<Cap, FabricRegistryError> {
        let normalized_urn = normalize_cap_urn(urn);
        if let Some(cap) = self
            .cached_caps
            .lock()
            .ok()
            .and_then(|m| m.get(&normalized_urn).cloned())
        {
            return Ok(cap);
        }
        let defver = self.cap_defver(&normalized_urn)?;
        fetch_one_cap_atomic(
            &self.client,
            &self.cache_dir,
            &self.cached_caps,
            &self.cached_media_defs,
            &self.extension_index,
            &self.manifest,
            &self.offline_flag,
            &self.config,
            self.manifest_version,
            &self.cache_revision_tx,
            &normalized_urn,
            defver,
        )
        .await
    }

    /// Resolve a normalized cap URN to its defver under the pinned
    /// manifest. At v0 this is unconditionally 0 (flat path). At v >= 1
    /// the URN must be in the manifest's `caps` map; if absent the
    /// caller has asked for a URN that is not part of the snapshot and
    /// we surface that as `NotFound` rather than silently fetching from
    /// flat paths (which would mix snapshot versions).
    fn cap_defver(&self, normalized_urn: &str) -> Result<u32, FabricRegistryError> {
        if self.manifest_version == 0 {
            return Ok(0);
        }
        let m = self.manifest.lock().map_err(|e| {
            FabricRegistryError::CacheError(format!("Failed to lock manifest: {}", e))
        })?;
        m.caps.get(normalized_urn).copied().ok_or_else(|| {
            FabricRegistryError::NotFound(format!(
                "cap '{}' is not part of manifest v{}",
                normalized_urn, self.manifest_version
            ))
        })
    }

    /// Resolve a normalized media URN to its defver under the pinned
    /// manifest. Same rules as `cap_defver`.
    fn media_defver(&self, normalized_urn: &str) -> Result<u32, FabricRegistryError> {
        if self.manifest_version == 0 {
            return Ok(0);
        }
        // The empty / wildcard URN `media:` is a sentinel — caps use it
        // to denote "any media", and it has no published spec. Anywhere
        // we resolve a URN to a defver we must skip it; the upstream
        // fetch path already special-cases it for fetching, so we just
        // mirror that here by returning 0 (which would map to a flat
        // path that doesn't exist, but the caller never reaches the
        // fetch with this URN).
        if normalized_urn == "media:" {
            return Ok(0);
        }
        let m = self.manifest.lock().map_err(|e| {
            FabricRegistryError::CacheError(format!("Failed to lock manifest: {}", e))
        })?;
        m.media.get(normalized_urn).copied().ok_or_else(|| {
            FabricRegistryError::NotFound(format!(
                "media def '{}' is not part of manifest v{}",
                normalized_urn, self.manifest_version
            ))
        })
    }

    /// Get multiple caps at once - fails if any cap is not available.
    pub async fn get_caps(&self, urns: &[&str]) -> Result<Vec<Cap>, FabricRegistryError> {
        let mut caps = Vec::new();
        for urn in urns {
            caps.push(self.get_cap(urn).await?);
        }
        Ok(caps)
    }

    /// Get all currently cached caps from in-memory cache.
    pub async fn get_cached_caps(&self) -> Result<Vec<Cap>, FabricRegistryError> {
        let cached_caps = self.cached_caps.lock().map_err(|e| {
            FabricRegistryError::CacheError(format!("Failed to lock cap cache: {}", e))
        })?;
        Ok(cached_caps.values().cloned().collect())
    }

    /// Synchronous cap lookup that warms its own cache. See module docs.
    pub fn get_cached_cap(&self, urn: &str) -> Option<Cap> {
        let normalized_urn = normalize_cap_urn(urn);
        if let Some(cap) = self
            .cached_caps
            .lock()
            .ok()
            .and_then(|m| m.get(&normalized_urn).cloned())
        {
            return Some(cap);
        }
        // If the URN is not in the manifest under v >= 1, there's
        // nothing to fetch — return None without enqueuing.
        let defver = self.cap_defver(&normalized_urn).ok()?;
        let runtime = tokio::runtime::Handle::try_current().ok()?;
        if !matches!(
            runtime.runtime_flavor(),
            tokio::runtime::RuntimeFlavor::MultiThread
        ) {
            self.enqueue_for_background_fetch(FetchKey::Cap {
                urn: normalized_urn,
                defver,
            });
            return None;
        }
        let sync_attempt = tokio::task::block_in_place(|| {
            runtime.block_on(async {
                tokio::time::timeout(
                    SYNC_FETCH_DEADLINE,
                    fetch_one_cap_atomic(
                        &self.client,
                        &self.cache_dir,
                        &self.cached_caps,
                        &self.cached_media_defs,
                        &self.extension_index,
                        &self.manifest,
                        &self.offline_flag,
                        &self.config,
                        self.manifest_version,
                        &self.cache_revision_tx,
                        &normalized_urn,
                        defver,
                    ),
                )
                .await
            })
        });
        match sync_attempt {
            Ok(Ok(cap)) => return Some(cap),
            Ok(Err(e)) => {
                tracing::debug!(
                    target: "capdag::fabric::registry",
                    urn = %normalized_urn, error = %e,
                    "Synchronous cap fetch errored within deadline; enqueueing for background fetch."
                );
            }
            Err(_elapsed) => {
                tracing::debug!(
                    target: "capdag::fabric::registry",
                    urn = %normalized_urn,
                    "Synchronous cap fetch did not complete within deadline; enqueueing for background fetch."
                );
            }
        }
        self.enqueue_for_background_fetch(FetchKey::Cap {
            urn: normalized_urn,
            defver,
        });
        None
    }

    /// In-memory-only cap lookup for latency-critical planner sync.
    ///
    /// This never performs the bounded synchronous network fetch used by
    /// `get_cached_cap`. If the cap is missing, the caller can enqueue it
    /// for asynchronous cache hydration and rely on cache revision events to
    /// retry graph admission.
    pub fn get_cached_cap_in_memory(&self, urn: &str) -> Option<Cap> {
        let normalized_urn = normalize_cap_urn(urn);
        self.cached_caps
            .lock()
            .ok()
            .and_then(|m| m.get(&normalized_urn).cloned())
    }

    /// Request asynchronous hydration of a cap definition without waiting.
    pub fn request_cap_cache_hydration(&self, urn: &str) {
        let normalized_urn = normalize_cap_urn(urn);
        if let Ok(defver) = self.cap_defver(&normalized_urn) {
            self.enqueue_for_background_fetch(FetchKey::Cap {
                urn: normalized_urn,
                defver,
            });
        }
    }

    /// Validate a local cap against its canonical definition.
    pub async fn validate_cap(&self, cap: &Cap) -> Result<(), FabricRegistryError> {
        let canonical_cap = self.get_cap(&cap.urn_string()).await?;
        if cap.command != canonical_cap.command {
            return Err(FabricRegistryError::ValidationError(format!(
                "Command mismatch. Local: {}, Canonical: {}",
                cap.command, canonical_cap.command
            )));
        }
        let local_stdin = cap.get_stdin_media_urn();
        let canonical_stdin = canonical_cap.get_stdin_media_urn();
        if local_stdin != canonical_stdin {
            return Err(FabricRegistryError::ValidationError(format!(
                "stdin mismatch. Local: {:?}, Canonical: {:?}",
                local_stdin, canonical_stdin
            )));
        }
        Ok(())
    }

    /// Check whether a cap URN exists in the registry (cached or online).
    pub async fn cap_exists(&self, urn: &str) -> bool {
        self.get_cap(urn).await.is_ok()
    }

    /// Add caps to the in-memory cache. Test helper.
    ///
    /// Each cap is recorded in the manifest. If the cap's own
    /// `version` is 0, it is stamped to the registry's pinned manifest
    /// version (since v0 in this context means "the test forgot to set
    /// it" and the natural assignment is the snapshot we belong to).
    /// An explicitly-non-zero version is honored as-is — test fixtures
    /// can simulate cross-snapshot scenarios when they need to.
    pub fn add_caps_to_cache(&self, caps: Vec<Cap>) {
        let mut changed = false;
        let pin = self.manifest_version;
        let mut manifest_guard = self.manifest.lock().ok();
        if let Ok(mut cached_caps) = self.cached_caps.lock() {
            for mut cap in caps {
                let urn = cap.urn_string();
                let normalized_urn = normalize_cap_urn(&urn);
                if cap.version == 0 && pin >= 1 {
                    cap.version = pin;
                }
                let cap_version = cap.version;
                if let Some(m) = manifest_guard.as_mut() {
                    m.caps.insert(normalized_urn.clone(), cap_version);
                }
                cached_caps.insert(normalized_urn, cap);
                changed = true;
            }
        }
        drop(manifest_guard);
        if changed {
            publish_cache_revision(&self.cache_revision_tx);
        }
    }

    // -------------------------------------------------------------------------
    // MEDIA-DEF API
    // -------------------------------------------------------------------------

    /// Get a media def from cache or fetch from registry.
    pub async fn get_media_def(&self, urn: &str) -> Result<StoredMediaDef, FabricRegistryError> {
        let normalized = normalize_media_urn(urn);
        if let Some(spec) = self
            .cached_media_defs
            .lock()
            .ok()
            .and_then(|m| m.get(&normalized).cloned())
        {
            return Ok(spec);
        }
        let defver = self.media_defver(&normalized)?;
        fetch_one_media_def(
            &self.client,
            &self.cache_dir,
            &self.cached_media_defs,
            &self.extension_index,
            &self.offline_flag,
            &self.config,
            &self.cache_revision_tx,
            &normalized,
            defver,
        )
        .await
    }

    /// Get multiple media defs at once.
    pub async fn get_media_defs(
        &self,
        urns: &[&str],
    ) -> Result<Vec<StoredMediaDef>, FabricRegistryError> {
        let mut specs = Vec::new();
        for urn in urns {
            specs.push(self.get_media_def(urn).await?);
        }
        Ok(specs)
    }

    /// Get all currently cached media defs.
    pub async fn get_cached_media_defs(&self) -> Result<Vec<StoredMediaDef>, FabricRegistryError> {
        let cached_specs = self.cached_media_defs.lock().map_err(|e| {
            FabricRegistryError::CacheError(format!("Failed to lock media-def cache: {}", e))
        })?;
        Ok(cached_specs.values().cloned().collect())
    }

    /// Synchronous media-def lookup that warms its own cache.
    pub fn get_cached_media_def(&self, urn: &str) -> Option<StoredMediaDef> {
        let normalized = normalize_media_urn(urn);
        if let Some(spec) = self
            .cached_media_defs
            .lock()
            .ok()
            .and_then(|m| m.get(&normalized).cloned())
        {
            return Some(spec);
        }
        let defver = self.media_defver(&normalized).ok()?;
        let runtime = tokio::runtime::Handle::try_current().ok()?;
        if !matches!(
            runtime.runtime_flavor(),
            tokio::runtime::RuntimeFlavor::MultiThread
        ) {
            self.enqueue_for_background_fetch(FetchKey::Media {
                urn: normalized,
                defver,
            });
            return None;
        }
        let sync_attempt = tokio::task::block_in_place(|| {
            runtime.block_on(async {
                tokio::time::timeout(
                    SYNC_FETCH_DEADLINE,
                    fetch_one_media_def(
                        &self.client,
                        &self.cache_dir,
                        &self.cached_media_defs,
                        &self.extension_index,
                        &self.offline_flag,
                        &self.config,
                        &self.cache_revision_tx,
                        &normalized,
                        defver,
                    ),
                )
                .await
            })
        });
        match sync_attempt {
            Ok(Ok(spec)) => return Some(spec),
            Ok(Err(e)) => {
                tracing::debug!(
                    target: "capdag::fabric::registry",
                    urn = %normalized, error = %e,
                    "Synchronous media-def fetch errored within deadline; enqueueing for background fetch."
                );
            }
            Err(_elapsed) => {
                tracing::debug!(
                    target: "capdag::fabric::registry",
                    urn = %normalized,
                    "Synchronous media-def fetch did not complete within deadline; enqueueing for background fetch."
                );
            }
        }
        self.enqueue_for_background_fetch(FetchKey::Media {
            urn: normalized,
            defver,
        });
        None
    }

    /// Returns `true` if the URN is a bookend-eligible file format — its
    /// stored spec has at least one registered file extension.
    pub fn is_bookend(&self, urn: &str) -> bool {
        match self.get_cached_media_def(urn) {
            Some(spec) => !spec.extensions.is_empty(),
            None => false,
        }
    }

    /// Snapshot of every bookend-eligible URN currently in the cache.
    pub fn bookend_urns(&self) -> std::collections::HashSet<crate::MediaUrn> {
        let cached = match self.cached_media_defs.lock() {
            Ok(g) => g,
            Err(_) => return Default::default(),
        };
        cached
            .values()
            .filter(|spec| !spec.extensions.is_empty())
            .filter_map(|spec| crate::MediaUrn::from_string(&spec.urn).ok())
            .collect()
    }

    /// Returns all media URNs registered for the given file extension.
    pub fn media_urns_for_extension(
        &self,
        extension: &str,
    ) -> Result<Vec<String>, FabricRegistryError> {
        let ext_lower = extension.to_lowercase();
        let index = self.extension_index.lock().map_err(|e| {
            FabricRegistryError::CacheError(format!("Failed to lock extension index: {}", e))
        })?;
        index.get(&ext_lower).cloned().ok_or_else(|| {
            FabricRegistryError::ExtensionNotFound(format!(
                "No media def registered for extension '{}'",
                extension
            ))
        })
    }

    /// Get all extension → URNs mappings.
    pub fn get_extension_mappings(
        &self,
    ) -> Result<Vec<(String, Vec<String>)>, FabricRegistryError> {
        let index = self.extension_index.lock().map_err(|e| {
            FabricRegistryError::CacheError(format!("Failed to lock extension index: {}", e))
        })?;
        Ok(index.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }

    /// Insert a media def into the in-memory cache. Test helper.
    ///
    /// Records the media def in the manifest. If the spec's own
    /// `version` is 0, it is stamped to the registry's pinned manifest
    /// version (same "test forgot to set it" handling as
    /// `add_caps_to_cache`).
    pub fn insert_cached_media_def_for_test(&self, mut spec: StoredMediaDef) {
        let normalized = normalize_media_urn(&spec.urn);
        let pin = self.manifest_version;
        if spec.version == 0 && pin >= 1 {
            spec.version = pin;
        }
        let spec_version = spec.version;
        if let Ok(mut cache) = self.cached_media_defs.lock() {
            cache.insert(normalized.clone(), spec.clone());
        }
        if let Ok(mut idx) = self.extension_index.lock() {
            for ext in &spec.extensions {
                let ext_lower = ext.to_lowercase();
                let urns = idx.entry(ext_lower).or_default();
                if !urns.contains(&spec.urn) {
                    urns.push(spec.urn.clone());
                }
            }
        }
        if let Ok(mut m) = self.manifest.lock() {
            m.media.insert(normalized, spec_version);
        }
        publish_cache_revision(&self.cache_revision_tx);
    }

    /// Check if a media URN exists in registry (cached or online).
    pub async fn media_def_exists(&self, urn: &str) -> bool {
        self.get_media_def(urn).await.is_ok()
    }

    // -------------------------------------------------------------------------
    // SHARED ADMIN API
    // -------------------------------------------------------------------------

    /// Clear both caches (in-memory and on disk). The manifest snapshot
    /// is preserved — clearing the byte caches is the natural way to
    /// force re-fetch under the same snapshot, not to switch snapshots.
    pub fn clear_cache(&self) -> Result<(), FabricRegistryError> {
        if let Ok(mut g) = self.cached_caps.lock() {
            g.clear();
        }
        if let Ok(mut g) = self.cached_media_defs.lock() {
            g.clear();
        }
        if let Ok(mut g) = self.extension_index.lock() {
            g.clear();
        }
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir).map_err(|e| {
                FabricRegistryError::CacheError(format!("Failed to clear cache directory: {}", e))
            })?;
            for sub in ["caps", "media", "manifests"] {
                fs::create_dir_all(self.cache_dir.join(sub)).map_err(|e| {
                    FabricRegistryError::CacheError(format!(
                        "Failed to recreate cache directory: {}",
                        e
                    ))
                })?;
            }
        }
        Ok(())
    }

    // -------------------------------------------------------------------------
    // QUEUE
    // -------------------------------------------------------------------------

    /// Look up an arbitrary URN's pinned defver under this registry's
    /// manifest. Public so external callers (e.g. fetchcartridge) can
    /// resolve URN → (urn, defver) before issuing a network request.
    pub fn cap_defver_for(&self, urn: &str) -> Result<u32, FabricRegistryError> {
        let normalized = normalize_cap_urn(urn);
        self.cap_defver(&normalized)
    }

    /// As `cap_defver_for` but for media URNs.
    pub fn media_defver_for(&self, urn: &str) -> Result<u32, FabricRegistryError> {
        let normalized = normalize_media_urn(urn);
        self.media_defver(&normalized)
    }

    fn enqueue_for_background_fetch(&self, key: FetchKey) {
        let Some(tx) = self.fetch_queue_tx.as_ref() else {
            return;
        };
        let mut in_queue = match self.fetch_in_queue.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if !in_queue.insert(key.clone()) {
            return;
        }
        if let Err(e) = tx.send(key.clone()) {
            in_queue.remove(&key);
            tracing::warn!(
                target: "capdag::fabric::registry",
                key = ?key, error = %e,
                "Background fetch queue send failed (consumer task is gone); dropping URN."
            );
        }
    }

    // -------------------------------------------------------------------------
    // DISK LOAD
    // -------------------------------------------------------------------------

    /// Walk the cap cache directory recursively, picking up both v0 flat
    /// files (`caps/<sha>.json`) and v >= 1 versioned files
    /// (`caps/<sha>/<defver>.json`). TTL applies only to v0 entries —
    /// v >= 1 entries are immutable by protocol so no expiry pass.
    fn load_all_cached_caps(caps_dir: &Path) -> Result<HashMap<String, Cap>, FabricRegistryError> {
        let mut caps = HashMap::new();
        if !caps_dir.exists() {
            return Ok(caps);
        }
        let mut stack: Vec<PathBuf> = vec![caps_dir.to_path_buf()];
        let mut is_v0_layer = true;
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).map_err(|e| {
                FabricRegistryError::CacheError(format!(
                    "Failed to read cap cache directory {:?}: {}",
                    dir, e
                ))
            })? {
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::warn!("Failed to read cap cache entry: {}", e);
                        continue;
                    }
                };
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }
                let content = match fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!("Failed to read cap cache file {:?}: {}", path, e);
                        continue;
                    }
                };
                let cache_entry: CapCacheEntry = match serde_json::from_str(&content) {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::warn!("Failed to parse cap cache file {:?}: {}", path, e);
                        let _ = fs::remove_file(&path);
                        continue;
                    }
                };
                // TTL applies only to v0 (flat) entries. Versioned
                // entries are immutable by protocol.
                if cache_entry.definition.version == 0 && cache_entry.is_expired() {
                    let _ = fs::remove_file(&path);
                    continue;
                }
                let urn = cache_entry.definition.urn_string();
                caps.insert(normalize_cap_urn(&urn), cache_entry.definition);
            }
            let _ = is_v0_layer;
            is_v0_layer = false;
        }
        Ok(caps)
    }

    /// Same recursive walk as `load_all_cached_caps`, for media defs.
    fn load_all_cached_media_defs(
        media_dir: &Path,
    ) -> Result<HashMap<String, StoredMediaDef>, FabricRegistryError> {
        let mut specs = HashMap::new();
        if !media_dir.exists() {
            return Ok(specs);
        }
        let mut stack: Vec<PathBuf> = vec![media_dir.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).map_err(|e| {
                FabricRegistryError::CacheError(format!(
                    "Failed to read media cache directory {:?}: {}",
                    dir, e
                ))
            })? {
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::warn!("Failed to read media cache entry: {}", e);
                        continue;
                    }
                };
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }
                let content = match fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!("Failed to read media cache file {:?}: {}", path, e);
                        continue;
                    }
                };
                let cache_entry: MediaCacheEntry = match serde_json::from_str(&content) {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::warn!("Failed to parse media cache file {:?}: {}", path, e);
                        let _ = fs::remove_file(&path);
                        continue;
                    }
                };
                if cache_entry.spec.version == 0 && cache_entry.is_expired() {
                    let _ = fs::remove_file(&path);
                    continue;
                }
                specs.insert(normalize_media_urn(&cache_entry.spec.urn), cache_entry.spec);
            }
        }
        Ok(specs)
    }

    fn build_extension_index(
        specs: &HashMap<String, StoredMediaDef>,
    ) -> HashMap<String, Vec<String>> {
        let mut index: HashMap<String, Vec<String>> = HashMap::new();
        for spec in specs.values() {
            for ext in &spec.extensions {
                let ext_lower = ext.to_lowercase();
                index.entry(ext_lower).or_default().push(spec.urn.clone());
            }
        }
        index
    }

    // -------------------------------------------------------------------------
    // TEST HELPERS
    // -------------------------------------------------------------------------

    /// Synchronous test constructor with a fresh empty cache. Pins the
    /// registry at v1 with an empty manifest, so test helpers like
    /// `add_caps_to_cache` flow caps into the manifest at their declared
    /// version. Spawns a fetch consumer when called inside a tokio
    /// runtime; otherwise leaves the queue inert.
    pub fn new_for_test() -> Self {
        Self::new_for_test_with_config(RegistryConfig::default())
    }

    /// Test constructor with custom config; pins at v1.
    pub fn new_for_test_with_config(config: RegistryConfig) -> Self {
        Self::new_for_test_with_config_and_version(config, 1)
    }

    /// Full test constructor: custom config + explicit pinned manifest
    /// version. Builds an empty manifest at that version; no network.
    pub fn new_for_test_with_config_and_version(
        config: RegistryConfig,
        manifest_version: u32,
    ) -> Self {
        let cache_dir = PathBuf::from("/tmp/capdag-test-cache");
        let _ = fs::create_dir_all(cache_dir.join("caps"));
        let _ = fs::create_dir_all(cache_dir.join("media"));
        let _ = fs::create_dir_all(cache_dir.join("manifests"));
        let cached_caps = Arc::new(Mutex::new(HashMap::new()));
        let cached_media_defs = Arc::new(Mutex::new(HashMap::new()));
        let extension_index = Arc::new(Mutex::new(HashMap::new()));
        let manifest_arc = Arc::new(Mutex::new(Manifest::empty(manifest_version)));
        let fetch_in_queue = Arc::new(Mutex::new(HashSet::new()));
        let offline_flag = Arc::new(AtomicBool::new(false));
        let client = reqwest::Client::new();
        let (cache_revision_tx, _) = watch::channel(0u64);

        let fetch_queue_tx = match tokio::runtime::Handle::try_current() {
            Ok(_) => {
                let (tx, rx) = mpsc::unbounded_channel::<FetchKey>();
                tokio::spawn(run_fetch_consumer(
                    rx,
                    client.clone(),
                    cache_dir.clone(),
                    Arc::clone(&cached_caps),
                    Arc::clone(&cached_media_defs),
                    Arc::clone(&extension_index),
                    Arc::clone(&manifest_arc),
                    Arc::clone(&fetch_in_queue),
                    Arc::clone(&offline_flag),
                    config.clone(),
                    cache_revision_tx.clone(),
                ));
                Some(tx)
            }
            Err(_) => None,
        };

        let registry = Self {
            client,
            cache_dir,
            cached_caps,
            cached_media_defs,
            extension_index,
            config,
            manifest_version,
            manifest: manifest_arc,
            offline_flag,
            fetch_queue_tx,
            fetch_in_queue,
            cache_revision_tx,
        };
        registry.ensure_identity_cap();
        registry
    }
}

// =============================================================================
// ATOMIC FETCH HELPERS (free functions)
// =============================================================================

/// Build the R2 URL for a per-cap object at the given defver. defver==0
/// addresses the frozen v0 flat path; defver>=1 addresses the versioned
/// subpath. The cache file path mirrors the URL structure.
fn cap_url_and_cache_path(
    cache_dir: &Path,
    config: &RegistryConfig,
    normalized_urn: &str,
    defver: u32,
) -> (String, PathBuf) {
    let mut hasher = Sha256::new();
    hasher.update(normalized_urn.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    if defver == 0 {
        (
            format!("{}/caps/{}", config.registry_base_url, hash),
            cache_dir.join("caps").join(format!("{}.json", hash)),
        )
    } else {
        (
            format!(
                "{}/caps/{}/{}.json",
                config.registry_base_url, hash, defver
            ),
            cache_dir
                .join("caps")
                .join(&hash)
                .join(format!("{}.json", defver)),
        )
    }
}

/// Build the R2 URL for a per-media object at the given defver.
fn media_url_and_cache_path(
    cache_dir: &Path,
    config: &RegistryConfig,
    normalized_urn: &str,
    defver: u32,
) -> (String, PathBuf) {
    let mut hasher = Sha256::new();
    hasher.update(normalized_urn.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    if defver == 0 {
        (
            format!("{}/media/{}", config.registry_base_url, hash),
            cache_dir.join("media").join(format!("{}.json", hash)),
        )
    } else {
        (
            format!(
                "{}/media/{}/{}.json",
                config.registry_base_url, hash, defver
            ),
            cache_dir
                .join("media")
                .join(&hash)
                .join(format!("{}.json", defver)),
        )
    }
}

/// Atomic cap fetcher. Fetches the cap body, then ensures every media URN
/// it references is in the media cache. Caches the cap only on full
/// success; otherwise returns `Err` and writes nothing.
///
/// At pin >= 1 the referenced media URN footprint is resolved against
/// the manifest so each referenced URN is fetched at its pinned defver.
/// If a referenced URN is absent from the manifest the fetch fails —
/// snapshots are required to be self-consistent.
#[allow(clippy::too_many_arguments)]
async fn fetch_one_cap_atomic(
    client: &reqwest::Client,
    cache_dir: &Path,
    cached_caps: &Arc<Mutex<HashMap<String, Cap>>>,
    cached_media_defs: &Arc<Mutex<HashMap<String, StoredMediaDef>>>,
    extension_index: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    manifest: &Arc<Mutex<Manifest>>,
    offline_flag: &Arc<AtomicBool>,
    config: &RegistryConfig,
    manifest_version: u32,
    cache_revision_tx: &watch::Sender<u64>,
    normalized_urn: &str,
    defver: u32,
) -> Result<Cap, FabricRegistryError> {
    if offline_flag.load(Ordering::Relaxed) {
        return Err(FabricRegistryError::NetworkBlocked(format!(
            "Network access blocked by policy — cannot fetch cap '{}'",
            normalized_urn
        )));
    }

    let (url, cache_file) = cap_url_and_cache_path(cache_dir, config, normalized_urn, defver);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| FabricRegistryError::HttpError(format!("Failed to fetch cap: {}", e)))?;
    if !response.status().is_success() {
        return Err(FabricRegistryError::NotFound(format!(
            "Cap '{}' (defver {}) not found in registry (HTTP {}) at {}",
            normalized_urn,
            defver,
            response.status(),
            url
        )));
    }
    let cap: Cap = response.json().await.map_err(|e| {
        FabricRegistryError::ParseError(format!("Failed to parse cap '{}': {}", normalized_urn, e))
    })?;

    // Walk every media URN referenced by the cap. Empty/wildcard URN
    // (`media:`) is the identity / wildcard sentinel — it has no
    // fetchable spec and must be skipped.
    let mut referenced: Vec<String> = Vec::new();
    let push = |v: &mut Vec<String>, s: &str| {
        let n = normalize_media_urn(s);
        if n != "media:" && !v.contains(&n) {
            v.push(n);
        }
    };
    push(&mut referenced, cap.urn.in_spec());
    push(&mut referenced, cap.urn.out_spec());
    for arg in &cap.args {
        push(&mut referenced, &arg.media_urn);
        for source in &arg.sources {
            if let ArgSource::Stdin { stdin } = source {
                push(&mut referenced, stdin);
            }
        }
    }
    if let Some(out) = &cap.output {
        push(&mut referenced, &out.media_urn);
    }

    for media_urn in &referenced {
        let already_cached = cached_media_defs
            .lock()
            .ok()
            .map(|m| m.contains_key(media_urn))
            .unwrap_or(false);
        if already_cached {
            continue;
        }
        // Resolve the referenced media URN's defver under the manifest.
        // At v0 every URN maps to defver 0 (flat path).
        let media_defver = if manifest_version == 0 {
            0
        } else {
            match manifest.lock() {
                Ok(m) => match m.media.get(media_urn).copied() {
                    Some(v) => v,
                    None => {
                        return Err(FabricRegistryError::NotFound(format!(
                            "cap '{}' references media URN '{}' which is not in manifest v{}",
                            normalized_urn, media_urn, manifest_version
                        )));
                    }
                },
                Err(e) => {
                    return Err(FabricRegistryError::CacheError(format!(
                        "failed to lock manifest while resolving referenced media: {}",
                        e
                    )));
                }
            }
        };
        if let Err(e) = fetch_one_media_def(
            client,
            cache_dir,
            cached_media_defs,
            extension_index,
            offline_flag,
            config,
            cache_revision_tx,
            media_urn,
            media_defver,
        )
        .await
        {
            tracing::warn!(
                target: "capdag::fabric::registry",
                cap_urn = %normalized_urn,
                missing_media_urn = %media_urn,
                error = %e,
                "Aborting cap cache write: a referenced media def could not be fetched. \
                 The cap is NOT cached so the next attempt re-tries cleanly."
            );
            return Err(FabricRegistryError::NotFound(format!(
                "cap '{}' references media URN '{}' which could not be fetched: {}",
                normalized_urn, media_urn, e
            )));
        }
    }

    // All referenced media defs in cache. Write the cap.
    let cache_entry = CapCacheEntry {
        definition: cap.clone(),
        cached_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        ttl_hours: CACHE_DURATION_HOURS,
    };
    if let Some(parent) = cache_file.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            FabricRegistryError::CacheError(format!(
                "Failed to create cap cache parent directory {:?}: {}",
                parent, e
            ))
        })?;
    }
    let content = serde_json::to_string_pretty(&cache_entry).map_err(|e| {
        FabricRegistryError::CacheError(format!("Failed to serialize cap cache entry: {}", e))
    })?;
    fs::write(&cache_file, content).map_err(|e| {
        FabricRegistryError::CacheError(format!("Failed to write cap cache file: {}", e))
    })?;

    if let Ok(mut cached) = cached_caps.lock() {
        cached.insert(normalized_urn.to_string(), cap.clone());
    }
    publish_cache_revision(cache_revision_tx);

    Ok(cap)
}

/// Atomic media-def fetcher.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn fetch_one_media_def(
    client: &reqwest::Client,
    cache_dir: &Path,
    cached_media_defs: &Arc<Mutex<HashMap<String, StoredMediaDef>>>,
    extension_index: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    offline_flag: &Arc<AtomicBool>,
    config: &RegistryConfig,
    cache_revision_tx: &watch::Sender<u64>,
    normalized_urn: &str,
    defver: u32,
) -> Result<StoredMediaDef, FabricRegistryError> {
    if offline_flag.load(Ordering::Relaxed) {
        return Err(FabricRegistryError::NetworkBlocked(format!(
            "Network access blocked by policy — cannot fetch media def '{}'",
            normalized_urn
        )));
    }

    let (url, cache_file) = media_url_and_cache_path(cache_dir, config, normalized_urn, defver);

    let response =
        client.get(&url).send().await.map_err(|e| {
            FabricRegistryError::HttpError(format!("Failed to fetch media def: {}", e))
        })?;
    if !response.status().is_success() {
        return Err(FabricRegistryError::NotFound(format!(
            "Media def '{}' (defver {}) not found in registry (HTTP {}) at {}",
            normalized_urn,
            defver,
            response.status(),
            url
        )));
    }
    let spec: StoredMediaDef = response.json().await.map_err(|e| {
        FabricRegistryError::ParseError(format!(
            "Failed to parse media def '{}': {}",
            normalized_urn, e
        ))
    })?;

    let cache_entry = MediaCacheEntry {
        spec: spec.clone(),
        cached_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        ttl_hours: CACHE_DURATION_HOURS,
    };
    if let Some(parent) = cache_file.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            FabricRegistryError::CacheError(format!(
                "Failed to create media cache parent directory {:?}: {}",
                parent, e
            ))
        })?;
    }
    let content = serde_json::to_string_pretty(&cache_entry).map_err(|e| {
        FabricRegistryError::CacheError(format!("Failed to serialize media cache entry: {}", e))
    })?;
    fs::write(&cache_file, content).map_err(|e| {
        FabricRegistryError::CacheError(format!("Failed to write media cache file: {}", e))
    })?;

    if let Ok(mut cached) = cached_media_defs.lock() {
        cached.insert(normalized_urn.to_string(), spec.clone());
    }
    if let Ok(mut idx) = extension_index.lock() {
        for ext in &spec.extensions {
            let ext_lower = ext.to_lowercase();
            let urns = idx.entry(ext_lower).or_default();
            if !urns.contains(&spec.urn) {
                urns.push(spec.urn.clone());
            }
        }
    }
    publish_cache_revision(cache_revision_tx);
    Ok(spec)
}

/// Manifest bootstrap. Tries the local cache first; falls back to a
/// blocking network GET; if neither produces a manifest, returns an
/// error — there is no v0 fallback (caller chose v >= 1 explicitly).
async fn load_or_fetch_manifest(
    manifests_dir: &Path,
    client: &reqwest::Client,
    config: &RegistryConfig,
    version: u32,
) -> Result<Manifest, FabricRegistryError> {
    let cache_file = manifests_dir.join(format!("{}.json", version));
    if cache_file.exists() {
        let content = fs::read_to_string(&cache_file).map_err(|e| {
            FabricRegistryError::CacheError(format!(
                "Failed to read cached manifest at {:?}: {}",
                cache_file, e
            ))
        })?;
        match serde_json::from_str::<Manifest>(&content) {
            Ok(m) => {
                if m.version != version {
                    return Err(FabricRegistryError::ParseError(format!(
                        "Cached manifest at {:?} reports version {} but file is {}.json",
                        cache_file, m.version, version
                    )));
                }
                return Ok(m);
            }
            Err(e) => {
                tracing::warn!(
                    "Cached manifest at {:?} did not parse: {}; re-fetching from network",
                    cache_file,
                    e
                );
                let _ = fs::remove_file(&cache_file);
            }
        }
    }

    let url = format!("{}/manifest/{}.json", config.registry_base_url, version);
    let response = client.get(&url).send().await.map_err(|e| {
        FabricRegistryError::HttpError(format!(
            "Failed to fetch manifest v{} at {}: {}",
            version, url, e
        ))
    })?;
    if !response.status().is_success() {
        return Err(FabricRegistryError::NotFound(format!(
            "Manifest v{} not found in registry (HTTP {}) at {}",
            version,
            response.status(),
            url
        )));
    }
    let body = response.text().await.map_err(|e| {
        FabricRegistryError::HttpError(format!(
            "Failed to read manifest v{} body: {}",
            version, e
        ))
    })?;
    let manifest: Manifest = serde_json::from_str(&body).map_err(|e| {
        FabricRegistryError::ParseError(format!("Failed to parse manifest v{}: {}", version, e))
    })?;
    if manifest.version != version {
        return Err(FabricRegistryError::ParseError(format!(
            "Manifest fetched as v{} reports version {}",
            version, manifest.version
        )));
    }
    fs::write(&cache_file, &body).map_err(|e| {
        FabricRegistryError::CacheError(format!(
            "Failed to write manifest cache to {:?}: {}",
            cache_file, e
        ))
    })?;
    Ok(manifest)
}

fn publish_cache_revision(tx: &watch::Sender<u64>) {
    let next = {
        let current = *tx.borrow();
        current.wrapping_add(1)
    };
    let _ = tx.send(next);
}

/// Single shared background fetch consumer for both cap and media URNs.
/// Drains the queue serially; failures are logged and dropped. The
/// queue keys carry both URN and defver, so the consumer never needs to
/// re-resolve through the manifest.
#[allow(clippy::too_many_arguments)]
async fn run_fetch_consumer(
    mut rx: mpsc::UnboundedReceiver<FetchKey>,
    client: reqwest::Client,
    cache_dir: PathBuf,
    cached_caps: Arc<Mutex<HashMap<String, Cap>>>,
    cached_media_defs: Arc<Mutex<HashMap<String, StoredMediaDef>>>,
    extension_index: Arc<Mutex<HashMap<String, Vec<String>>>>,
    manifest: Arc<Mutex<Manifest>>,
    fetch_in_queue: Arc<Mutex<HashSet<FetchKey>>>,
    offline_flag: Arc<AtomicBool>,
    config: RegistryConfig,
    cache_revision_tx: watch::Sender<u64>,
) {
    let manifest_version = manifest.lock().map(|m| m.version).unwrap_or(0);
    while let Some(key) = rx.recv().await {
        match &key {
            FetchKey::Cap {
                urn: normalized_urn,
                defver,
            } => {
                let already_cached = cached_caps
                    .lock()
                    .ok()
                    .map(|m| m.contains_key(normalized_urn))
                    .unwrap_or(false);
                if !already_cached {
                    match fetch_one_cap_atomic(
                        &client,
                        &cache_dir,
                        &cached_caps,
                        &cached_media_defs,
                        &extension_index,
                        &manifest,
                        &offline_flag,
                        &config,
                        manifest_version,
                        &cache_revision_tx,
                        normalized_urn,
                        *defver,
                    )
                    .await
                    {
                        Ok(_) => {
                            tracing::debug!(
                                target: "capdag::fabric::registry::fetch_consumer",
                                urn = %normalized_urn, defver = %defver,
                                "Background-fetched cap; cache is now warm."
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                target: "capdag::fabric::registry::fetch_consumer",
                                urn = %normalized_urn, defver = %defver, error = %e,
                                "Background cap fetch failed; URN dropped from queue (no retry)."
                            );
                        }
                    }
                }
            }
            FetchKey::Media {
                urn: normalized_urn,
                defver,
            } => {
                let already_cached = cached_media_defs
                    .lock()
                    .ok()
                    .map(|m| m.contains_key(normalized_urn))
                    .unwrap_or(false);
                if !already_cached {
                    match fetch_one_media_def(
                        &client,
                        &cache_dir,
                        &cached_media_defs,
                        &extension_index,
                        &offline_flag,
                        &config,
                        &cache_revision_tx,
                        normalized_urn,
                        *defver,
                    )
                    .await
                    {
                        Ok(_) => {
                            tracing::debug!(
                                target: "capdag::fabric::registry::fetch_consumer",
                                urn = %normalized_urn, defver = %defver,
                                "Background-fetched media def; cache is now warm."
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                target: "capdag::fabric::registry::fetch_consumer",
                                urn = %normalized_urn, defver = %defver, error = %e,
                                "Background media-def fetch failed; URN dropped from queue (no retry)."
                            );
                        }
                    }
                }
            }
        }
        if let Ok(mut in_queue) = fetch_in_queue.lock() {
            in_queue.remove(&key);
        }
    }
}

// =============================================================================
// ERROR
// =============================================================================

#[derive(Debug, thiserror::Error)]
pub enum FabricRegistryError {
    #[error("HTTP error: {0}")]
    HttpError(String),

    #[error("Not found in registry: {0}")]
    NotFound(String),

    #[error("Failed to parse registry response: {0}")]
    ParseError(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Network access blocked: {0}")]
    NetworkBlocked(String),

    #[error("No media def registered for extension: {0}")]
    ExtensionNotFound(String),
}
