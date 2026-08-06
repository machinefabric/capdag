//! Protocol observability primitives shared by every bifaci runtime.
//!
//! Two counter families, deliberately distinct because they mean opposite
//! things:
//!
//! - `DropCounters` is the L8 substrate for frames lost to something going
//!   WRONG: every dropped frame increments exactly one `DropReason` ×
//!   `FrameType` counter — frames are never dropped silently, and a non-zero
//!   drop total is always worth investigating.
//! - `StragglerCounters` counts the benign teardown crossing: flow frames
//!   that arrive after their request's terminal, which the protocol expects
//!   (in-flight frames legally race END/ERR — e.g. a callee that ENDs
//!   before draining its input, or a final credit grant crossing the
//!   terminal). Stragglers are moot by protocol — nothing went wrong, no
//!   data was lost — and every stats surface indicates them as benign,
//!   never as drops or failures.
//!
//! All counters are lock-free atomics so they can be bumped from writer
//! threads, async tasks, and blocking contexts alike, and snapshot into
//! serializable maps for the protocol stats surfaces.

use crate::bifaci::frame::{DropReason, FlowKey, FrameType};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};

fn frame_type_idx(frame_type: FrameType) -> usize {
    FrameType::ALL
        .iter()
        .position(|t| *t == frame_type)
        .expect("FrameType::ALL covers every variant")
}

/// Per-reason × per-frame-type dropped-frame counters (L8). Cheap to bump,
/// snapshot on demand. Drops mean something went wrong — the benign
/// post-terminal case is NOT recorded here (see [`StragglerCounters`]).
#[derive(Debug, Default)]
pub struct DropCounters {
    counters: [[AtomicU64; FrameType::ALL.len()]; DropReason::ALL.len()],
}

impl DropCounters {
    pub fn new() -> Self {
        Self::default()
    }

    fn idx(reason: DropReason) -> usize {
        DropReason::ALL
            .iter()
            .position(|r| *r == reason)
            .expect("DropReason::ALL covers every variant")
    }

    /// Record one dropped frame of the given type. Returns the new total for
    /// that reason (across frame types).
    pub fn record(&self, reason: DropReason, frame_type: FrameType) -> u64 {
        self.counters[Self::idx(reason)][frame_type_idx(frame_type)]
            .fetch_add(1, Ordering::Relaxed);
        self.get(reason)
    }

    /// Current count for one reason, summed across frame types.
    pub fn get(&self, reason: DropReason) -> u64 {
        self.counters[Self::idx(reason)]
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .sum()
    }

    /// Current count for one (reason, frame type) cell.
    pub fn get_frame(&self, reason: DropReason, frame_type: FrameType) -> u64 {
        self.counters[Self::idx(reason)][frame_type_idx(frame_type)].load(Ordering::Relaxed)
    }

    /// Total drops across all reasons.
    pub fn total(&self) -> u64 {
        DropReason::ALL.iter().map(|r| self.get(*r)).sum()
    }

    /// Serializable snapshot keyed by the stable snake_case reason names —
    /// the field-name contract mirrors replicate. `by_reason` carries the
    /// per-reason totals; `by_reason_frame_type` breaks each reason down by
    /// the dropped frame's type, so a trace names WHAT was dropped without
    /// archaeology. Zero-count entries are omitted from both.
    pub fn snapshot(&self) -> DropSnapshot {
        let mut by_reason = BTreeMap::new();
        let mut by_reason_frame_type = BTreeMap::new();
        for reason in DropReason::ALL {
            let count = self.get(reason);
            if count > 0 {
                by_reason.insert(reason.as_str().to_string(), count);
                let mut by_frame = BTreeMap::new();
                for frame_type in FrameType::ALL {
                    let cell = self.get_frame(reason, frame_type);
                    if cell > 0 {
                        by_frame.insert(frame_type.as_str().to_string(), cell);
                    }
                }
                by_reason_frame_type.insert(reason.as_str().to_string(), by_frame);
            }
        }
        DropSnapshot {
            total: self.total(),
            by_reason,
            by_reason_frame_type,
        }
    }
}

/// Serializable view of the drop counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DropSnapshot {
    pub total: u64,
    /// reason name (snake_case) → count; zero-count reasons omitted.
    pub by_reason: BTreeMap<String, u64>,
    /// reason name → (frame type name → count); zero cells omitted. Absent
    /// reasons mirror `by_reason`.
    #[serde(default)]
    pub by_reason_frame_type: BTreeMap<String, BTreeMap<String, u64>>,
}

/// Per-frame-type counters for benign post-terminal stragglers.
///
/// A straggler is a flow frame that arrives after its request's terminal
/// (END/ERR) — the ordinary, protocol-legal teardown crossing (L13): a
/// callee may END before draining its input, a final CREDIT grant may cross
/// the terminal in flight. Nothing went wrong and no data was lost; the
/// frame is simply moot. Counted per frame type so surfaces can say exactly
/// what crossed ("late credit" vs "late chunk") — and always indicated as
/// benign, never as a drop or failure.
#[derive(Debug, Default)]
pub struct StragglerCounters {
    counters: [AtomicU64; FrameType::ALL.len()],
}

impl StragglerCounters {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one benign post-terminal straggler. Returns the new total.
    pub fn record(&self, frame_type: FrameType) -> u64 {
        self.counters[frame_type_idx(frame_type)].fetch_add(1, Ordering::Relaxed);
        self.total()
    }

    /// Current count for one frame type.
    pub fn get(&self, frame_type: FrameType) -> u64 {
        self.counters[frame_type_idx(frame_type)].load(Ordering::Relaxed)
    }

    /// Total stragglers across all frame types.
    pub fn total(&self) -> u64 {
        self.counters
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .sum()
    }

    /// Serializable snapshot keyed by the stable snake_case frame-type
    /// names; zero-count types omitted.
    pub fn snapshot(&self) -> StragglerSnapshot {
        let mut by_frame_type = BTreeMap::new();
        for frame_type in FrameType::ALL {
            let count = self.get(frame_type);
            if count > 0 {
                by_frame_type.insert(frame_type.as_str().to_string(), count);
            }
        }
        StragglerSnapshot {
            total: self.total(),
            by_frame_type,
        }
    }
}

/// Serializable view of the straggler counters — benign by definition.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StragglerSnapshot {
    pub total: u64,
    /// frame type name (snake_case) → count; zero-count types omitted.
    pub by_frame_type: BTreeMap<String, u64>,
}

/// Terminated-flow set for the writer-side terminal gate (L4).
///
/// After a flow's END/ERR is written, any later flow frame for the same
/// FlowKey is post-terminal: it is dropped and counted instead of written.
/// The set is capacity-bounded FIFO — with seq state already removed at the
/// terminal, an evicted entry can only readmit a straggler that the receiving
/// side's reorder/routing layers then reject; the cap bounds memory on
/// long-lived cartridges, it does not change protocol correctness.
#[derive(Debug)]
pub struct TerminatedFlows {
    order: VecDeque<FlowKey>,
    set: HashSet<FlowKey>,
    cap: usize,
}

impl TerminatedFlows {
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "TerminatedFlows cap must be positive");
        Self {
            order: VecDeque::with_capacity(cap),
            set: HashSet::with_capacity(cap),
            cap,
        }
    }

    /// Mark a flow terminated. Evicts the oldest entry at capacity.
    pub fn insert(&mut self, key: FlowKey) {
        if self.set.contains(&key) {
            return;
        }
        if self.order.len() == self.cap {
            if let Some(oldest) = self.order.pop_front() {
                self.set.remove(&oldest);
            }
        }
        self.order.push_back(key.clone());
        self.set.insert(key);
    }

    /// Whether this flow has already seen its terminal frame.
    pub fn contains(&self, key: &FlowKey) -> bool {
        self.set.contains(key)
    }

    pub fn len(&self) -> usize {
        self.set.len()
    }

    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bifaci::frame::MessageId;

    // TEST7019: Drop counters record per-reason × per-frame-type exactly once
    // per drop; the snapshot totals all of them, breaks each reason down by
    // frame type, and omits zero-count entries.
    #[test]
    fn test7019_drop_counters_record_and_snapshot() {
        let counters = DropCounters::new();
        assert_eq!(counters.total(), 0);
        assert_eq!(counters.snapshot(), DropSnapshot::default());

        assert_eq!(counters.record(DropReason::NoRoute, FrameType::Chunk), 1);
        assert_eq!(counters.record(DropReason::NoRoute, FrameType::Credit), 2);
        assert_eq!(counters.record(DropReason::ChannelClosed, FrameType::Log), 1);

        assert_eq!(counters.get(DropReason::NoRoute), 2);
        assert_eq!(counters.get(DropReason::ChannelClosed), 1);
        assert_eq!(counters.get(DropReason::Cancelled), 0);
        assert_eq!(counters.get_frame(DropReason::NoRoute, FrameType::Chunk), 1);
        assert_eq!(counters.get_frame(DropReason::NoRoute, FrameType::Credit), 1);
        assert_eq!(counters.get_frame(DropReason::NoRoute, FrameType::End), 0);
        assert_eq!(counters.total(), 3);

        let snap = counters.snapshot();
        assert_eq!(snap.total, 3);
        assert_eq!(snap.by_reason.get("no_route"), Some(&2));
        assert_eq!(snap.by_reason.get("channel_closed"), Some(&1));
        assert!(
            !snap.by_reason.contains_key("cancelled"),
            "zero-count reasons are omitted from the snapshot"
        );
        let no_route = snap
            .by_reason_frame_type
            .get("no_route")
            .expect("per-frame-type breakdown present for non-zero reasons");
        assert_eq!(no_route.get("chunk"), Some(&1));
        assert_eq!(no_route.get("credit"), Some(&1));
        assert!(
            !no_route.contains_key("end"),
            "zero-count frame types are omitted from the breakdown"
        );
    }

    // TEST8127: Straggler counters — the benign post-terminal category is
    // separate from drops, counted per frame type, and its snapshot names
    // what crossed the terminal (late credit vs late chunk) while omitting
    // zero-count types.
    #[test]
    fn test8127_straggler_counters_record_and_snapshot() {
        let stragglers = StragglerCounters::new();
        assert_eq!(stragglers.total(), 0);
        assert_eq!(stragglers.snapshot(), StragglerSnapshot::default());

        assert_eq!(stragglers.record(FrameType::Credit), 1);
        assert_eq!(stragglers.record(FrameType::Credit), 2);
        assert_eq!(stragglers.record(FrameType::Chunk), 3);

        assert_eq!(stragglers.get(FrameType::Credit), 2);
        assert_eq!(stragglers.get(FrameType::Chunk), 1);
        assert_eq!(stragglers.get(FrameType::End), 0);

        let snap = stragglers.snapshot();
        assert_eq!(snap.total, 3);
        assert_eq!(snap.by_frame_type.get("credit"), Some(&2));
        assert_eq!(snap.by_frame_type.get("chunk"), Some(&1));
        assert!(
            !snap.by_frame_type.contains_key("end"),
            "zero-count frame types are omitted from the snapshot"
        );
    }

    // TEST7029: TerminatedFlows membership is exact up to capacity and evicts strictly oldest-first beyond it.
    #[test]
    fn test7029_terminated_flows_capacity_and_eviction() {
        let mut flows = TerminatedFlows::new(2);
        let k = |n: u64| FlowKey {
            rid: MessageId::Uint(n),
            xid: None,
        };

        flows.insert(k(1));
        flows.insert(k(1)); // duplicate insert is a no-op
        flows.insert(k(2));
        assert_eq!(flows.len(), 2);
        assert!(flows.contains(&k(1)) && flows.contains(&k(2)));

        flows.insert(k(3)); // evicts k(1), the oldest
        assert_eq!(flows.len(), 2);
        assert!(!flows.contains(&k(1)));
        assert!(flows.contains(&k(2)) && flows.contains(&k(3)));

        // XID-bearing key is a distinct flow from the bare-RID key
        let with_xid = FlowKey {
            rid: MessageId::Uint(2),
            xid: Some(MessageId::Uint(9)),
        };
        assert!(!flows.contains(&with_xid));
    }
}
