//! Live-feed transport resolution (13.2 §Reference Media, live family).
//!
//! A live feed is an input that arrives BY REFERENCE: the wire value is a
//! small selector record, and the resolving runtime — never the op —
//! opens the device through the built-in capture dispatch
//! (`crate::capture::open`, the device analog of file-path reading) and
//! delivers an UNBOUNDED SEQUENCE stream of items labeled with the arg's
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
//! This module owns the feed TRANSPORT: the selector contract, the ring +
//! feeder backpressure bridge ([`bridge_feed`]), the sink the capture
//! backends push into, and the stop/drain handle. The capture backends
//! themselves — which device families exist and how each opens — live in
//! `crate::capture` as plain compile-time dispatch: capture is transport
//! resolution, not a capability and not a plugin surface. Which runtime
//! resolves is a deployment detail (13.2 §Reference Media): the cartridge
//! runtime when the consumer is an op; the HOST when the cartridge cannot
//! reach the device, or when the host itself consumes the items
//! (engine-side per-item dispatch over a live source).

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
    /// Backend-defined device selector. A backend with exactly one
    /// device may default this.
    pub device: Option<String>,
    /// Backend-defined capture parameters (sample rate, resolution, …).
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

/// One captured item, as a capture backend hands it to the sink. `seq` and gap
/// accounting are assigned by the sink/feeder — backends supply only the
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
    /// Feed closed (stop, abort, or delivery side gone): backends observe
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

/// The capture backend's write side of a feed. Owned by the backend's capture
/// thread; `push` applies the overrun policy at the capture edge.
pub struct LiveFeedSink {
    shared: Arc<FeedShared>,
    max_items: Option<u64>,
}

impl LiveFeedSink {
    /// Push one captured item. Returns `false` when the feed is closed —
    /// the backend must stop capturing and release the device. A full
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

    /// The producer FAILED (device error mid-capture). The feed ends with a
    /// delivered stream error — a dying device must never masquerade as a
    /// clean end-of-feed, or a 3-second recording quietly becomes 3
    /// milliseconds of data.
    pub fn fail(&self, message: String) {
        let mut state = self.shared.state.lock().unwrap();
        state.failed = Some(message);
        self.shared.cond.notify_all();
        drop(state);
        self.shared.close();
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
#[derive(Clone)]
pub struct LiveFeedHandle {
    shared: Arc<FeedShared>,
}

impl LiveFeedHandle {
    /// Close the tap: the backend's next `push` returns false, the feeder
    /// drains what was already captured, and the stream ends — the drain
    /// path of a stopped run.
    pub fn close(&self) {
        self.shared.close();
    }

    /// This feed's overrun total so far.
    pub fn overruns(&self) -> u64 {
        self.shared.overruns.load(Ordering::Relaxed)
    }

    /// Whether the tap is closed (stop, abort, or feed end).
    pub fn is_closed(&self) -> bool {
        self.shared.closed.load(Ordering::SeqCst)
    }
}

/// Ring capacity when the selector's params don't override it (`ring`).
const DEFAULT_RING_CAP: usize = 64;
/// Bounded delivery-channel capacity — the op-side half of the
/// backpressure chain. Small on purpose: the ring is the elastic stage.
const DELIVERY_CHANNEL_CAP: usize = 8;

/// Everything `bridge_feed` returns to the resolver: the delivery receiver the
/// `InputStream` consumes, the stream-level meta for STREAM_START, and the
/// handle the runtime registers for stop.
pub struct OpenedFeed {
    pub rx: tokio::sync::mpsc::Receiver<
        Result<(ciborium::Value, Option<StreamMeta>), crate::bifaci::cartridge_runtime::StreamError>,
    >,
    pub stream_meta: Option<StreamMeta>,
    pub handle: LiveFeedHandle,
}

/// Bridge one opened capture into bounded delivery: build the ring + sink,
/// hand the sink to `open_device` (a `crate::capture` backend), and spawn
/// the feeder thread enforcing the backpressure chain and the
/// `duration_ms` stop condition. `overruns_total` is the calling runtime's
/// aggregate overrun counter.
pub fn bridge_feed(
    selector: LiveFeedSelector,
    overruns_total: Arc<AtomicU64>,
    open_device: impl FnOnce(&LiveFeedSelector, LiveFeedSink) -> Result<Option<StreamMeta>, RuntimeError>,
) -> Result<OpenedFeed, RuntimeError> {
    use crate::bifaci::cartridge_runtime::StreamError;

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
        runtime_overruns: overruns_total,
    });

    let sink = LiveFeedSink {
        shared: Arc::clone(&shared),
        max_items: selector.stop.max_items,
    };
    let stream_meta = open_device(&selector, sink)?;

    let (tx, rx) = tokio::sync::mpsc::channel(DELIVERY_CHANNEL_CAP);

    // The feeder: ring → bounded delivery. Blocking sends give the real
    // backpressure — when the op lags, the feeder blocks, the ring fills,
    // and the capture edge applies the overrun policy. A `duration_ms`
    // stop condition is enforced here (uniformly across backends).
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
                // so the backend stops capturing.
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

