//! synthetic — the built-in deterministic feed backend
//! (`media:live;synthetic`): a logical clock emitting `items` payloads of
//! `item_bytes` bytes every `interval_ms` (params, all optional; defaults
//! 10 × 32B × 10ms). `pts_us` is the LOGICAL clock (i × interval), so tests
//! are deterministic; `capture_ts_us` is wall clock. `interval_ms = 0`
//! emits as fast as possible — with a small `ring` and a slow consumer this
//! exercises real overruns without hardware. Used by the shared test range
//! and available as a fixture feed everywhere; needs no device stack, so it
//! is built unconditionally.

use crate::bifaci::cartridge_runtime::RuntimeError;
use crate::bifaci::live_feed::{LiveFeedItem, LiveFeedSelector, LiveFeedSink};
use crate::StreamMeta;

/// The content urn the synthetic feed delivers: opaque test frames.
pub const MEDIA_FEED_FRAMES: &str = "media:feed-frames";

/// Open the deterministic clock feed and start emitting into `sink`.
pub fn open(
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
