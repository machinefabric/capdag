//! Protocol observability primitives shared by every bifaci runtime.
//!
//! `DropCounters` is the L8 substrate: every frame a runtime drops increments
//! exactly one `DropReason` counter — frames are never dropped silently. The
//! counters are lock-free atomics so they can be bumped from writer threads,
//! async tasks, and blocking contexts alike, and snapshot into serializable
//! maps for the protocol stats surfaces.

use crate::bifaci::frame::{DropReason, FlowKey};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};

/// Per-reason dropped-frame counters (L8). Cheap to bump, snapshot on demand.
#[derive(Debug, Default)]
pub struct DropCounters {
    counters: [AtomicU64; DropReason::ALL.len()],
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

    /// Record one dropped frame. Returns the new total for that reason.
    pub fn record(&self, reason: DropReason) -> u64 {
        self.counters[Self::idx(reason)].fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Current count for one reason.
    pub fn get(&self, reason: DropReason) -> u64 {
        self.counters[Self::idx(reason)].load(Ordering::Relaxed)
    }

    /// Total drops across all reasons.
    pub fn total(&self) -> u64 {
        self.counters.iter().map(|c| c.load(Ordering::Relaxed)).sum()
    }

    /// Serializable snapshot keyed by the stable snake_case reason names —
    /// the field-name contract mirrors replicate.
    pub fn snapshot(&self) -> DropSnapshot {
        let mut by_reason = BTreeMap::new();
        for reason in DropReason::ALL {
            let count = self.get(reason);
            if count > 0 {
                by_reason.insert(reason.as_str().to_string(), count);
            }
        }
        DropSnapshot {
            total: self.total(),
            by_reason,
        }
    }
}

/// Serializable view of the drop counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DropSnapshot {
    pub total: u64,
    /// reason name (snake_case) → count; zero-count reasons omitted.
    pub by_reason: BTreeMap<String, u64>,
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

    // TEST7019: Drop counters record per-reason exactly once per drop, and the snapshot omits zero-count reasons while totalling all of them.
    #[test]
    fn test7019_drop_counters_record_and_snapshot() {
        let counters = DropCounters::new();
        assert_eq!(counters.total(), 0);
        assert_eq!(counters.snapshot(), DropSnapshot::default());

        assert_eq!(counters.record(DropReason::PostTerminal), 1);
        assert_eq!(counters.record(DropReason::PostTerminal), 2);
        assert_eq!(counters.record(DropReason::ChannelClosed), 1);

        assert_eq!(counters.get(DropReason::PostTerminal), 2);
        assert_eq!(counters.get(DropReason::ChannelClosed), 1);
        assert_eq!(counters.get(DropReason::NoRoute), 0);
        assert_eq!(counters.total(), 3);

        let snap = counters.snapshot();
        assert_eq!(snap.total, 3);
        assert_eq!(snap.by_reason.get("post_terminal"), Some(&2));
        assert_eq!(snap.by_reason.get("channel_closed"), Some(&1));
        assert!(
            !snap.by_reason.contains_key("no_route"),
            "zero-count reasons are omitted from the snapshot"
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
