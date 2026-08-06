//! Live-feed transport resolution (13.2 §Reference Media, live family).
//!
//! A live feed is an input that arrives BY REFERENCE: the wire value is a
//! small selector record, and the runtime — never the op — resolves it by
//! opening a capture device through a registered [`LiveFeedProvider`] and
//! delivering an UNBOUNDED SEQUENCE stream of items labeled with the arg's
//! stdin content URN. The op is transport-blind: it consumes frames exactly
//! as it would consume a file's bytes, and cannot tell the difference.
//!
//! Backpressure is end-to-end and every stage has a defined full-state
//! behavior (12.5 §Overrun): wire credit stalls the op's output → the op
//! stops consuming input → the BOUNDED delivery channel fills → the feeder
//! stops draining the capture ring → the ring fills → the capture edge
//! applies the feed's declared overrun policy. Loss can occur only at the
//! capture edge, only under the declared policy, and always counted — with
//! an in-band `gap` marker on the next delivered item so downstream sees
//! the discontinuity.
//!
//! The runtime ships one built-in provider, [`SyntheticFeedProvider`]
//! (`media:live;synthetic`): a deterministic clock source used by the
//! shared test range and available everywhere as a fixture feed. Hardware
//! providers (microphone, webcam) are registered by capture-capable
//! cartridges; on sandboxed platforms the host captures instead and this
//! seam is not involved.

use crate::bifaci::cartridge_runtime::{RuntimeError, StreamMeta};
use crate::urn::media_urn::MediaUrn;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

/// Reference-family pattern: any media URN carrying the `live` marker tag
/// is a live-feed reference (the live analog of `media:file-path`).
pub const MEDIA_LIVE_FEED: &str = "media:live";

/// The built-in deterministic test feed's reference URN.
pub const MEDIA_LIVE_SYNTHETIC: &str = "media:live;synthetic";

/// How the capture edge behaves when the ring is full because the consumer
/// lags reality (12.5 §Overrun).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverrunPolicy {
    /// Evict the oldest ring entry, count the overrun, stamp the next
    /// delivered item with a `gap` marker. Real-time consumers prefer
    /// fresh data over complete data. The default.
    #[default]
    DropOldest,
    /// End the feed with a classified `FEED_OVERRUN` error. For pipelines
    /// that need every frame and say so explicitly.
    Fail,
}

/// Stop conditions for a feed (absent = "until stopped"). Unknown fields
/// are rejected like the selector's own: a misspelled stop condition
/// silently ignored would run an unbounded feed the caller meant to bound.
#[derive(Debug, Clone, Copy, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveFeedStop {
    /// End the feed after this much capture time.
    pub duration_ms: Option<u64>,
    /// End the feed after this many CAPTURED items (dropped items count —
    /// they were captured).
    pub max_items: Option<u64>,
}

/// The selector record carried as a live-feed reference arg's value (JSON).
/// An empty value is the all-defaults selector.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveFeedSelector {
    /// Provider-defined device selector. A provider with exactly one
    /// device may default this.
    pub device: Option<String>,
    /// Provider-defined capture parameters (sample rate, resolution, …).
    #[serde(default)]
    pub params: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub stop: LiveFeedStop,
    #[serde(default)]
    pub on_overrun: OverrunPolicy,
}

impl LiveFeedSelector {
    /// Parse a selector from the reference value bytes. Empty (or
    /// whitespace-only) bytes are the all-defaults selector; anything else
    /// must be a valid selector record — an unparseable selector is a hard
    /// error, never a silent default.
    pub fn parse(bytes: &[u8]) -> Result<Self, RuntimeError> {
        let text = String::from_utf8_lossy(bytes);
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(Self::default());
        }
        serde_json::from_str(trimmed).map_err(|e| {
            RuntimeError::Handler(format!(
                "live-feed selector is not a valid selector record: {} (value: {})",
                e, trimmed
            ))
        })
    }
}

/// One captured item, as a provider hands it to the sink. `seq` and gap
/// accounting are assigned by the sink/feeder — providers supply only the
/// payload and its timestamps.
#[derive(Debug)]
pub struct LiveFeedItem {
    /// Raw item bytes (one audio buffer, one video frame, …).
    pub payload: Vec<u8>,
    /// Presentation timestamp, microseconds from capture start.
    pub pts_us: u64,
    /// Wall-clock capture time, Unix microseconds.
    pub capture_ts_us: u64,
}

/// A captured item annotated by the sink at push time.
struct RingItem {
    item: LiveFeedItem,
    /// Capture-order index, monotonic from 0, counting dropped items too —
    /// a gap in delivered `seq` is real (12.4 §Live Feeds).
    seq: u64,
}

struct FeedState {
    ring: VecDeque<RingItem>,
    /// Set when the producer finished (stop condition, device closed) —
    /// the feeder drains the ring then ends the stream.
    producer_done: bool,
    /// Set by `on_overrun = fail` with the failure message; the feeder
    /// surfaces it as the stream's terminal error.
    failed: Option<String>,
}

struct FeedShared {
    state: Mutex<FeedState>,
    cond: Condvar,
    ring_cap: usize,
    policy: OverrunPolicy,
    /// Feed closed (stop, abort, or delivery side gone): providers observe
    /// this via `push()` returning false and must stop capturing.
    closed: AtomicBool,
    /// Items captured so far (delivered + dropped) — the `seq` source and
    /// the `max_items` stop-condition counter.
    captured: AtomicU64,
    /// Items dropped at the capture edge since the last delivered item —
    /// swapped to zero when the feeder stamps a `gap` marker.
    dropped_since_delivery: AtomicU64,
    /// This feed's total overruns.
    overruns: AtomicU64,
    /// Runtime-wide overrun counter (rides heartbeat meta as
    /// `overruns_total`).
    runtime_overruns: Arc<AtomicU64>,
}

impl FeedShared {
    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.cond.notify_all();
    }
}

/// The provider's write side of a feed. Owned by the provider's capture
/// thread; `push` applies the overrun policy at the capture edge.
pub struct LiveFeedSink {
    shared: Arc<FeedShared>,
    max_items: Option<u64>,
}

impl LiveFeedSink {
    /// Push one captured item. Returns `false` when the feed is closed —
    /// the provider must stop capturing and release the device. A full
    /// ring applies the feed's overrun policy (12.5 §Overrun); under
    /// `fail` the feed ends and this returns `false`.
    pub fn push(&self, item: LiveFeedItem) -> bool {
        if self.shared.closed.load(Ordering::SeqCst) {
            return false;
        }
        let seq = self.shared.captured.fetch_add(1, Ordering::SeqCst);
        let mut state = self.shared.state.lock().unwrap();
        if state.ring.len() >= self.shared.ring_cap {
            match self.shared.policy {
                OverrunPolicy::DropOldest => {
                    state.ring.pop_front();
                    self.shared.overruns.fetch_add(1, Ordering::Relaxed);
                    self.shared
                        .dropped_since_delivery
                        .fetch_add(1, Ordering::Relaxed);
                    self.shared.runtime_overruns.fetch_add(1, Ordering::Relaxed);
                }
                OverrunPolicy::Fail => {
                    state.failed = Some(format!(
                        "FEED_OVERRUN: capture ring full at item seq={} — the consumer's \
                         window lagged reality and the feed declared on_overrun=fail",
                        seq
                    ));
                    drop(state);
                    self.shared.close();
                    return false;
                }
            }
        }
        state.ring.push_back(RingItem { item, seq });
        self.shared.cond.notify_all();
        drop(state);

        // max_items counts CAPTURED items; reaching it finishes the
        // producer side (the ring still drains).
        if let Some(max) = self.max_items {
            if seq + 1 >= max {
                self.finish();
                return false;
            }
        }
        !self.shared.closed.load(Ordering::SeqCst)
    }

    /// The producer finished on its own (stop condition, device closed).
    /// The feeder drains the remaining ring, then the stream ends.
    pub fn finish(&self) {
        let mut state = self.shared.state.lock().unwrap();
        state.producer_done = true;
        self.shared.cond.notify_all();
    }

    /// Whether the feed has been closed (stop/abort/consumer gone).
    pub fn is_closed(&self) -> bool {
        self.shared.closed.load(Ordering::SeqCst)
    }
}

/// A handle to one open feed, held by the runtime per request so a stop
/// (non-force Cancel on a feed-bearing request) can close the tap and let
/// the run drain (15.2 §Runs Stop).
pub struct LiveFeedHandle {
    shared: Arc<FeedShared>,
}

impl LiveFeedHandle {
    /// Close the tap: the provider's next `push` returns false, the feeder
    /// drains what was already captured, and the stream ends — the drain
    /// path of a stopped run.
    pub fn close(&self) {
        self.shared.close();
    }

    /// This feed's overrun total so far.
    pub fn overruns(&self) -> u64 {
        self.shared.overruns.load(Ordering::Relaxed)
    }
}

/// A live-capture backend. Implementations own a capture thread: `open`
/// starts capture pushing into `sink` and returns the stream-level format
/// actuals (sample rate, resolution, …) for STREAM_START meta. `push`
/// returning false — or `sink.is_closed()` — means stop capturing and
/// release the device.
pub trait LiveFeedProvider: Send + Sync {
    /// Provider name, for errors and logs.
    fn name(&self) -> &str;
    /// Open the device described by `selector` and start capturing into
    /// `sink`. A device that cannot be opened is a hard error — never a
    /// silent empty feed.
    fn open(
        &self,
        selector: &LiveFeedSelector,
        sink: LiveFeedSink,
    ) -> Result<Option<StreamMeta>, RuntimeError>;
}

/// Registered providers: reference-URN pattern → provider. First
/// registered pattern that ACCEPTS the incoming reference URN wins;
/// registration order is deliberate (a cartridge may register a more
/// specific device provider before the generic family).
#[derive(Default)]
pub struct LiveFeedProviders {
    entries: Mutex<Vec<(MediaUrn, Arc<dyn LiveFeedProvider>)>>,
    /// Runtime-wide overrun total (heartbeat `overruns_total`).
    overruns_total: Arc<AtomicU64>,
}

impl LiveFeedProviders {
    pub fn new() -> Self {
        let providers = Self::default();
        providers.register(MEDIA_LIVE_SYNTHETIC, Arc::new(SyntheticFeedProvider));
        providers
    }

    /// Register a provider for a reference-URN pattern. The pattern must
    /// itself be a live-feed reference (carry the `live` marker) — a
    /// provider registered off-family would be unreachable, which is a
    /// wiring bug surfaced loudly here.
    pub fn register(&self, pattern: &str, provider: Arc<dyn LiveFeedProvider>) {
        let pattern_urn = MediaUrn::from_string(pattern).unwrap_or_else(|e| {
            panic!(
                "BUG: live-feed provider pattern '{}' is not a valid media URN: {}",
                pattern, e
            )
        });
        let family = MediaUrn::from_string(MEDIA_LIVE_FEED)
            .expect("BUG: MEDIA_LIVE_FEED constant is invalid");
        if !family.accepts(&pattern_urn).unwrap_or(false) {
            panic!(
                "BUG: live-feed provider pattern '{}' is outside the live reference \
                 family '{}' — it would never be resolved",
                pattern, MEDIA_LIVE_FEED
            );
        }
        self.entries
            .lock()
            .unwrap()
            .push((pattern_urn, provider));
    }

    fn find(&self, reference: &MediaUrn) -> Option<Arc<dyn LiveFeedProvider>> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .find(|(pattern, _)| pattern.accepts(reference).unwrap_or(false))
            .map(|(_, p)| Arc::clone(p))
    }

    /// Runtime-wide overrun total (rides heartbeat meta).
    pub fn overruns_total(&self) -> u64 {
        self.overruns_total.load(Ordering::Relaxed)
    }
}

/// Ring capacity when the selector's params don't override it (`ring`).
const DEFAULT_RING_CAP: usize = 64;
/// Bounded delivery-channel capacity — the op-side half of the
/// backpressure chain. Small on purpose: the ring is the elastic stage.
const DELIVERY_CHANNEL_CAP: usize = 8;

/// Everything `open_feed` returns to the demux: the delivery receiver the
/// `InputStream` consumes, the stream-level meta for STREAM_START, and the
/// handle the runtime registers for stop.
pub struct OpenedFeed {
    pub rx: tokio::sync::mpsc::Receiver<
        Result<(ciborium::Value, Option<StreamMeta>), crate::bifaci::cartridge_runtime::StreamError>,
    >,
    pub stream_meta: Option<StreamMeta>,
    pub handle: LiveFeedHandle,
}

/// Resolve a live-feed reference: find the provider, open the device, and
/// bridge capture → bounded delivery through the ring + feeder thread.
pub fn open_feed(
    providers: &LiveFeedProviders,
    reference_urn: &str,
    selector: LiveFeedSelector,
) -> Result<OpenedFeed, RuntimeError> {
    use crate::bifaci::cartridge_runtime::StreamError;

    let reference = MediaUrn::from_string(reference_urn).map_err(|e| {
        RuntimeError::Handler(format!(
            "live-feed reference URN '{}' is not a valid media URN: {}",
            reference_urn, e
        ))
    })?;
    let provider = providers.find(&reference).ok_or_else(|| {
        RuntimeError::Handler(format!(
            "no live-feed provider registered for reference '{}' — the runtime \
             cannot open this feed",
            reference_urn
        ))
    })?;

    let ring_cap = selector
        .params
        .get("ring")
        .and_then(|v| v.as_u64())
        .map(|v| v.max(1) as usize)
        .unwrap_or(DEFAULT_RING_CAP);

    let shared = Arc::new(FeedShared {
        state: Mutex::new(FeedState {
            ring: VecDeque::with_capacity(ring_cap),
            producer_done: false,
            failed: None,
        }),
        cond: Condvar::new(),
        ring_cap,
        policy: selector.on_overrun,
        closed: AtomicBool::new(false),
        captured: AtomicU64::new(0),
        dropped_since_delivery: AtomicU64::new(0),
        overruns: AtomicU64::new(0),
        runtime_overruns: Arc::clone(&providers.overruns_total),
    });

    let sink = LiveFeedSink {
        shared: Arc::clone(&shared),
        max_items: selector.stop.max_items,
    };
    let stream_meta = provider.open(&selector, sink)?;

    let (tx, rx) = tokio::sync::mpsc::channel(DELIVERY_CHANNEL_CAP);

    // The feeder: ring → bounded delivery. Blocking sends give the real
    // backpressure — when the op lags, the feeder blocks, the ring fills,
    // and the capture edge applies the overrun policy. A `duration_ms`
    // stop condition is enforced here (uniformly across providers).
    let feeder_shared = Arc::clone(&shared);
    let deadline = selector
        .stop
        .duration_ms
        .map(|ms| std::time::Instant::now() + std::time::Duration::from_millis(ms));
    std::thread::spawn(move || {
        let mut last_delivered_pts: Option<u64> = None;
        loop {
            if let Some(deadline) = deadline {
                if std::time::Instant::now() >= deadline {
                    feeder_shared.close();
                }
            }
            let RingItem { item, seq } = {
                let mut state = feeder_shared.state.lock().unwrap();
                loop {
                    // An overrun failure preempts remaining ring items: the
                    // feed declared on_overrun=fail, so the loss IS the
                    // outcome — surface it as the stream's terminal error.
                    if let Some(msg) = state.failed.take() {
                        drop(state);
                        let _ = tx.blocking_send(Err(StreamError::Protocol(msg)));
                        return;
                    }
                    if let Some(entry) = state.ring.pop_front() {
                        break entry;
                    }
                    if state.producer_done || feeder_shared.closed.load(Ordering::SeqCst) {
                        return; // drained + done → stream ends (tx drops)
                    }
                    let (next, _timeout) = feeder_shared
                        .cond
                        .wait_timeout(state, std::time::Duration::from_millis(50))
                        .unwrap();
                    state = next;
                    if let Some(deadline) = deadline {
                        if std::time::Instant::now() >= deadline {
                            feeder_shared.close();
                        }
                    }
                }
            };

            let mut meta: StreamMeta = StreamMeta::new();
            meta.insert("seq".to_string(), ciborium::Value::Integer(seq.into()));
            meta.insert(
                "pts_us".to_string(),
                ciborium::Value::Integer(item.pts_us.into()),
            );
            meta.insert(
                "capture_ts_us".to_string(),
                ciborium::Value::Integer(item.capture_ts_us.into()),
            );
            let dropped = feeder_shared
                .dropped_since_delivery
                .swap(0, Ordering::Relaxed);
            if dropped > 0 {
                let duration_us = last_delivered_pts
                    .map(|prev| item.pts_us.saturating_sub(prev))
                    .unwrap_or(0);
                meta.insert(
                    "gap".to_string(),
                    ciborium::Value::Map(vec![
                        (
                            ciborium::Value::Text("dropped".to_string()),
                            ciborium::Value::Integer(dropped.into()),
                        ),
                        (
                            ciborium::Value::Text("duration_us".to_string()),
                            ciborium::Value::Integer(duration_us.into()),
                        ),
                    ]),
                );
            }
            last_delivered_pts = Some(item.pts_us);

            if tx
                .blocking_send(Ok((ciborium::Value::Bytes(item.payload), Some(meta))))
                .is_err()
            {
                // Consumer gone (handler dropped the stream): close the tap
                // so the provider stops capturing.
                feeder_shared.close();
                return;
            }
        }
    });

    Ok(OpenedFeed {
        rx,
        stream_meta,
        handle: LiveFeedHandle { shared },
    })
}

/// The built-in deterministic feed (`media:live;synthetic`): a logical
/// clock emitting `items` payloads of `item_bytes` bytes every
/// `interval_ms` (params, all optional; defaults 10 × 32B × 10ms).
/// `pts_us` is the LOGICAL clock (i × interval), so tests are
/// deterministic; `capture_ts_us` is wall clock. `interval_ms = 0` emits
/// as fast as possible — with a small `ring` and a slow consumer this
/// exercises real overruns without hardware.
pub struct SyntheticFeedProvider;

impl LiveFeedProvider for SyntheticFeedProvider {
    fn name(&self) -> &str {
        "synthetic"
    }

    fn open(
        &self,
        selector: &LiveFeedSelector,
        sink: LiveFeedSink,
    ) -> Result<Option<StreamMeta>, RuntimeError> {
        let items = selector
            .params
            .get("items")
            .and_then(|v| v.as_u64())
            .unwrap_or(10);
        let interval_ms = selector
            .params
            .get("interval_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(10);
        let item_bytes = selector
            .params
            .get("item_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(32)
            .max(1) as usize;

        std::thread::spawn(move || {
            let start = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_micros() as u64)
                .unwrap_or(0);
            for i in 0..items {
                if sink.is_closed() {
                    break;
                }
                // Deterministic payload: the item index repeated.
                let payload = vec![(i % 256) as u8; item_bytes];
                let pushed = sink.push(LiveFeedItem {
                    payload,
                    pts_us: i * interval_ms * 1000,
                    capture_ts_us: start + i * interval_ms * 1000,
                });
                if !pushed {
                    break;
                }
                if interval_ms > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(interval_ms));
                }
            }
            sink.finish();
        });

        let mut meta = StreamMeta::new();
        meta.insert(
            "feed".to_string(),
            ciborium::Value::Text("synthetic".to_string()),
        );
        meta.insert(
            "interval_ms".to_string(),
            ciborium::Value::Integer(interval_ms.into()),
        );
        Ok(Some(meta))
    }
}
