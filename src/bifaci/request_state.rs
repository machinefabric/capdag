//! Unified per-request state for routing runtimes (protocol v3, L7/L8).
//!
//! One `RequestState` per in-flight request replaces the parallel routing maps
//! (routing entry, origin, peer markers, parent→child links, response channel,
//! rid→xid index) that previously had to be mutated consistently by hand.
//! Registration and termination are single operations: a request is registered
//! once and terminated once (End | Err | Cancelled | MasterDied); after
//! `terminate` returns, zero state for the key remains (L7).
//!
//! The table is also the observability substrate: per-stream flow counters,
//! phase tracking, and a bounded ring of recently-terminated summaries feed the
//! protocol stats snapshots (L8) without retaining routing state.

use crate::bifaci::frame::{Frame, FrameType, MessageId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::Instant;
use tokio::sync::mpsc;

/// (XID, RID) — the unique key of a routed request.
pub type RequestKey = (MessageId, MessageId);

/// Where a request came from and where it is going, as master indices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingEntry {
    /// Master the request arrived from (None = external caller / engine).
    pub source_master_idx: Option<usize>,
    /// Master the request was dispatched to.
    pub destination_master_idx: usize,
}

/// How a request's lifecycle ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalKind {
    End,
    Err,
    Cancelled,
    MasterDied,
}

impl TerminalKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TerminalKind::End => "end",
            TerminalKind::Err => "err",
            TerminalKind::Cancelled => "cancelled",
            TerminalKind::MasterDied => "master_died",
        }
    }
}

/// Live phase of a request. `Terminated` never appears in the active table —
/// termination removes the entry (L7) and leaves a `TerminatedSummary` in the
/// recent ring instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestPhase {
    /// Registered; no flow frames observed yet.
    Created,
    /// At least one flow frame has moved through the runtime.
    Streaming,
}

/// Direction of a recorded frame relative to this runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameDirection {
    Inbound,
    Outbound,
}

/// Per-stream flow accounting. Keyed by stream_id (None = frames not tied to a
/// specific stream: REQ, END, ERR, LOG).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamFlowStats {
    pub frames_in: u64,
    pub frames_out: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub chunks_in: u64,
    pub chunks_out: u64,
    /// Credits granted through this runtime minus chunks that consumed them.
    /// Diagnostic — the endpoints hold the authoritative windows.
    pub credit_outstanding: i64,
    /// Stream announced with unbounded=true (no length promise).
    pub unbounded: bool,
    /// STREAM_END observed.
    pub ended: bool,
}

/// Everything a routing runtime knows about one in-flight request.
#[derive(Debug)]
pub struct RequestState {
    pub routing: RoutingEntry,
    /// Master index the response must return to (None = external caller).
    pub origin: Option<usize>,
    /// Response delivery channel for externally-registered requests.
    pub external_channel: Option<mpsc::UnboundedSender<Frame>>,
    /// Whether this is a cartridge-initiated peer invocation.
    pub is_peer: bool,
    /// Cap URN of the originating REQ, when known at registration — the
    /// request's nameable identity on the L8 surface. Without it a stats
    /// snapshot shows only anonymous rids, making background chatter
    /// indistinguishable from run traffic.
    pub cap_urn: Option<String>,
    /// Child peer calls spawned under this request (cancel cascade).
    pub children: Vec<RequestKey>,
    pub phase: RequestPhase,
    /// Per-stream flow stats (None key = non-stream frames).
    pub streams: HashMap<Option<String>, StreamFlowStats>,
    pub created_at: Instant,
    pub last_activity: Instant,
}

impl RequestState {
    pub fn new(
        routing: RoutingEntry,
        origin: Option<usize>,
        external_channel: Option<mpsc::UnboundedSender<Frame>>,
        is_peer: bool,
    ) -> Self {
        let now = Instant::now();
        Self {
            routing,
            origin,
            external_channel,
            is_peer,
            cap_urn: None,
            children: Vec::new(),
            phase: RequestPhase::Created,
            streams: HashMap::new(),
            created_at: now,
            last_activity: now,
        }
    }

    /// Attach the originating REQ's cap URN — the request's nameable
    /// identity in observability surfaces.
    pub fn with_cap_urn(mut self, cap_urn: Option<String>) -> Self {
        self.cap_urn = cap_urn;
        self
    }

    fn record(&mut self, direction: FrameDirection, frame: &Frame) {
        self.last_activity = Instant::now();
        if frame.is_flow_frame() {
            self.phase = RequestPhase::Streaming;
        }
        let stats = self.streams.entry(frame.stream_id.clone()).or_default();
        let bytes = frame.payload.as_ref().map(|p| p.len() as u64).unwrap_or(0);
        match direction {
            FrameDirection::Inbound => {
                stats.frames_in += 1;
                stats.bytes_in += bytes;
                if frame.frame_type == FrameType::Chunk {
                    stats.chunks_in += 1;
                    stats.credit_outstanding -= 1;
                }
            }
            FrameDirection::Outbound => {
                stats.frames_out += 1;
                stats.bytes_out += bytes;
                if frame.frame_type == FrameType::Chunk {
                    stats.chunks_out += 1;
                }
            }
        }
        match frame.frame_type {
            FrameType::StreamStart if frame.is_unbounded() => stats.unbounded = true,
            FrameType::StreamEnd => stats.ended = true,
            FrameType::Credit => {
                stats.credit_outstanding += frame.credit_count().unwrap_or(0) as i64;
            }
            _ => {}
        }
    }
}

/// Summary of a finished request, retained in a bounded ring for stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminatedSummary {
    pub xid: String,
    pub rid: String,
    pub kind: TerminalKind,
    pub is_peer: bool,
    #[serde(default)]
    pub cap_urn: Option<String>,
    pub lifetime_ms: u64,
    pub frames_in: u64,
    pub frames_out: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

/// How many terminated-request summaries the ring retains.
const RECENT_TERMINATED_CAP: usize = 64;

/// The unified request table (L7): one entry per in-flight request, one
/// registration, one termination, plus the rid→xid secondary index and the
/// recently-terminated ring.
#[derive(Default)]
pub struct RequestTable {
    entries: HashMap<RequestKey, RequestState>,
    rid_index: HashMap<MessageId, MessageId>,
    recent_terminated: VecDeque<TerminatedSummary>,
    total_registered: u64,
    terminated_by_kind: BTreeMap<&'static str, u64>,
    /// Called with every termination's summary, synchronously under the
    /// table guard — observers must be cheap and non-blocking (an engine
    /// aggregating per-run history, a test recorder). The bounded ring
    /// serves polling; this hook serves accumulation that must not miss
    /// terminations between polls (the ring evicts at 64).
    terminate_observer: Option<Box<dyn Fn(&TerminatedSummary) + Send + Sync>>,
}

impl std::fmt::Debug for RequestTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequestTable")
            .field("entries", &self.entries.len())
            .field("recent_terminated", &self.recent_terminated.len())
            .field("total_registered", &self.total_registered)
            .finish()
    }
}

impl RequestTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a request. A request is registered exactly once (L7):
    /// re-registering a live key, or a RID already indexed to a different
    /// XID, is a protocol violation and is rejected.
    pub fn register(&mut self, key: RequestKey, state: RequestState) -> Result<(), String> {
        if self.entries.contains_key(&key) {
            return Err(format!(
                "request ({}, {}) already registered — a request is registered exactly once (L7)",
                key.0, key.1
            ));
        }
        if let Some(existing_xid) = self.rid_index.get(&key.1) {
            if *existing_xid != key.0 {
                return Err(format!(
                    "rid {} already indexed to xid {} — cannot re-index to xid {} (L7)",
                    key.1, existing_xid, key.0
                ));
            }
        }
        self.rid_index.insert(key.1.clone(), key.0.clone());
        self.entries.insert(key, state);
        self.total_registered += 1;
        Ok(())
    }

    pub fn get(&self, key: &RequestKey) -> Option<&RequestState> {
        self.entries.get(key)
    }

    pub fn get_mut(&mut self, key: &RequestKey) -> Option<&mut RequestState> {
        self.entries.get_mut(key)
    }

    pub fn contains(&self, key: &RequestKey) -> bool {
        self.entries.contains_key(key)
    }

    /// Look up the XID a bare RID belongs to (continuation frames arriving
    /// without routing IDs).
    pub fn xid_for_rid(&self, rid: &MessageId) -> Option<MessageId> {
        self.rid_index.get(rid).cloned()
    }

    /// Terminate a request: remove the entry and its rid index atomically,
    /// record a summary, and return the removed state (children for cancel
    /// cascades, the external channel for final delivery). After this returns,
    /// zero state for the key remains (L7). Returns None if the key is not
    /// live (already terminated — termination happens exactly once).
    pub fn terminate(&mut self, key: &RequestKey, kind: TerminalKind) -> Option<RequestState> {
        let state = self.entries.remove(key)?;
        // Only remove the rid index if it points at THIS xid — a re-used RID
        // under another XID (never valid per register, but defensive against
        // the impossible) must not lose its index.
        if self.rid_index.get(&key.1) == Some(&key.0) {
            self.rid_index.remove(&key.1);
        }

        let totals = state.streams.values().fold((0u64, 0u64, 0u64, 0u64), |acc, s| {
            (
                acc.0 + s.frames_in,
                acc.1 + s.frames_out,
                acc.2 + s.bytes_in,
                acc.3 + s.bytes_out,
            )
        });
        if self.recent_terminated.len() == RECENT_TERMINATED_CAP {
            self.recent_terminated.pop_front();
        }
        self.recent_terminated.push_back(TerminatedSummary {
            xid: key.0.to_string(),
            rid: key.1.to_string(),
            kind,
            is_peer: state.is_peer,
            cap_urn: state.cap_urn.clone(),
            lifetime_ms: state.created_at.elapsed().as_millis() as u64,
            frames_in: totals.0,
            frames_out: totals.1,
            bytes_in: totals.2,
            bytes_out: totals.3,
        });
        *self.terminated_by_kind.entry(kind.as_str()).or_insert(0) += 1;
        if let Some(observer) = &self.terminate_observer {
            observer(
                self.recent_terminated
                    .back()
                    .expect("summary was just pushed"),
            );
        }
        Some(state)
    }

    /// Install the termination observer (see field docs). One observer;
    /// installing replaces any previous one.
    pub fn set_terminate_observer(
        &mut self,
        observer: Box<dyn Fn(&TerminatedSummary) + Send + Sync>,
    ) {
        self.terminate_observer = Some(observer);
    }

    /// Record a frame moving through the runtime for this request.
    /// Unknown keys are ignored — the caller decides whether that is a
    /// counted drop (it is, at the routing layer) — recording is accounting,
    /// not routing.
    pub fn record_frame(&mut self, key: &RequestKey, direction: FrameDirection, frame: &Frame) {
        if let Some(state) = self.entries.get_mut(key) {
            state.record(direction, frame);
        }
    }

    /// Register a child peer call under its parent (cancel cascade).
    pub fn link_child(&mut self, parent: &RequestKey, child: RequestKey) {
        if let Some(state) = self.entries.get_mut(parent) {
            state.children.push(child);
        }
    }

    /// Keys of all live requests (for sweeps). Cloned so the caller can
    /// mutate the table while iterating.
    pub fn keys(&self) -> Vec<RequestKey> {
        self.entries.keys().cloned().collect()
    }

    /// Keys of live requests matching a predicate on their state.
    pub fn keys_where(&self, pred: impl Fn(&RequestState) -> bool) -> Vec<RequestKey> {
        self.entries
            .iter()
            .filter(|(_, s)| pred(s))
            .map(|(k, _)| k.clone())
            .collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Serializable snapshot of the table: live requests + recent terminations
    /// + lifetime totals. Field names are the mirror contract.
    pub fn snapshot(&self) -> RequestTableSnapshot {
        let mut active: Vec<RequestSnapshot> = self
            .entries
            .iter()
            .map(|(key, s)| RequestSnapshot {
                xid: key.0.to_string(),
                rid: key.1.to_string(),
                phase: s.phase,
                is_peer: s.is_peer,
                cap_urn: s.cap_urn.clone(),
                origin_master: s.origin,
                destination_master: s.routing.destination_master_idx,
                age_ms: s.created_at.elapsed().as_millis() as u64,
                idle_ms: s.last_activity.elapsed().as_millis() as u64,
                children: s.children.len() as u64,
                streams: s
                    .streams
                    .iter()
                    .map(|(id, stats)| StreamSnapshot {
                        stream_id: id.clone(),
                        stats: stats.clone(),
                    })
                    .collect(),
            })
            .collect();
        active.sort_by(|a, b| a.rid.cmp(&b.rid));
        RequestTableSnapshot {
            active,
            recent_terminated: self.recent_terminated.iter().cloned().collect(),
            total_registered: self.total_registered,
            terminated_by_kind: self
                .terminated_by_kind
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect(),
        }
    }
}

/// One stream's stats in a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamSnapshot {
    pub stream_id: Option<String>,
    #[serde(flatten)]
    pub stats: StreamFlowStats,
}

/// One live request in a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestSnapshot {
    pub xid: String,
    pub rid: String,
    pub phase: RequestPhase,
    pub is_peer: bool,
    #[serde(default)]
    pub cap_urn: Option<String>,
    pub origin_master: Option<usize>,
    pub destination_master: usize,
    pub age_ms: u64,
    pub idle_ms: u64,
    pub children: u64,
    pub streams: Vec<StreamSnapshot>,
}

/// Full table snapshot: the L8 observability surface for request state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestTableSnapshot {
    pub active: Vec<RequestSnapshot>,
    pub recent_terminated: Vec<TerminatedSummary>,
    pub total_registered: u64,
    pub terminated_by_kind: BTreeMap<String, u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(x: u64, r: u64) -> RequestKey {
        (MessageId::Uint(x), MessageId::Uint(r))
    }

    fn state(dest: usize, origin: Option<usize>, is_peer: bool) -> RequestState {
        RequestState::new(
            RoutingEntry {
                source_master_idx: origin,
                destination_master_idx: dest,
            },
            origin,
            None,
            is_peer,
        )
    }

    // TEST7087: Protocol stats snapshots serialize with stable field names — the snapshot shape is the mirror contract.
    #[test]
    fn test7092_cap_urn_attribution_survives_lifecycle() {
        // TEST7092: A request registered with its originating REQ's cap URN
        // carries that identity through the ACTIVE snapshot and into the
        // terminated ring — observability surfaces can always NAME a request
        // (background chatter vs run traffic), never just show a bare rid.
        // A request registered without one (pre-attribution mirror, unknown
        // origin) snapshots with cap_urn null — absent, never invented.
        let mut table = RequestTable::new();
        let named = key(1, 9);
        table
            .register(
                named.clone(),
                state(0, Some(1), false).with_cap_urn(Some("cap:effect=none".to_string())),
            )
            .unwrap();
        let anonymous = key(2, 10);
        table.register(anonymous.clone(), state(0, Some(1), true)).unwrap();

        let snapshot = table.snapshot();
        let by_rid = |rid: &str| snapshot.active.iter().find(|r| r.rid == rid).unwrap();
        assert_eq!(
            by_rid("9").cap_urn.as_deref(),
            Some("cap:effect=none"),
            "active snapshot names the request's cap"
        );
        assert_eq!(by_rid("10").cap_urn, None, "unknown identity stays absent");

        table.terminate(&named, TerminalKind::End).unwrap();
        let snapshot = table.snapshot();
        assert_eq!(
            snapshot.recent_terminated[0].cap_urn.as_deref(),
            Some("cap:effect=none"),
            "the terminated ring keeps the cap identity"
        );
    }

    #[test]
    fn test7087_snapshot_field_names_are_stable() {
        let mut table = RequestTable::new();
        let k = key(1, 9);
        table.register(k.clone(), state(0, Some(1), true)).unwrap();
        let rid = MessageId::Uint(9);
        let ss = Frame::stream_start(
            rid,
            "s".to_string(),
            "media:enc=utf-8".to_string(),
            Some(false),
        );
        table.record_frame(&k, FrameDirection::Inbound, &ss);

        let json = serde_json::to_value(table.snapshot()).unwrap();
        for field in ["active", "recent_terminated", "total_registered", "terminated_by_kind"] {
            assert!(json.get(field).is_some(), "missing top-level field {}", field);
        }
        let req = &json["active"][0];
        for field in [
            "xid",
            "rid",
            "phase",
            "is_peer",
            "origin_master",
            "destination_master",
            "age_ms",
            "idle_ms",
            "children",
            "streams",
        ] {
            assert!(req.get(field).is_some(), "missing request field {}", field);
        }
        assert_eq!(req["phase"], "streaming", "phase serializes snake_case");
        let stream = &req["streams"][0];
        for field in [
            "stream_id",
            "frames_in",
            "frames_out",
            "bytes_in",
            "bytes_out",
            "chunks_in",
            "chunks_out",
            "credit_outstanding",
            "unbounded",
            "ended",
        ] {
            assert!(stream.get(field).is_some(), "missing stream field {}", field);
        }

        table.terminate(&k, TerminalKind::MasterDied).unwrap();
        let json = serde_json::to_value(table.snapshot()).unwrap();
        let summary = &json["recent_terminated"][0];
        for field in [
            "xid",
            "rid",
            "kind",
            "is_peer",
            "lifetime_ms",
            "frames_in",
            "frames_out",
            "bytes_in",
            "bytes_out",
        ] {
            assert!(summary.get(field).is_some(), "missing summary field {}", field);
        }
        assert_eq!(summary["kind"], "master_died", "kind serializes snake_case");
    }

    // TEST7088: last_activity is monotonic non-decreasing across a long-lived streaming request — idle time resets on every recorded frame and never runs backwards.
    #[test]
    fn test7088_last_activity_monotonic() {
        let mut table = RequestTable::new();
        let k = key(1, 5);
        table.register(k.clone(), state(0, None, false)).unwrap();
        let rid = MessageId::Uint(5);

        let mut last_activity_points = Vec::new();
        for i in 0..3u64 {
            std::thread::sleep(std::time::Duration::from_millis(15));
            let payload = vec![0u8; 4];
            let checksum = Frame::compute_checksum(&payload);
            let chunk = Frame::chunk(rid.clone(), "s".to_string(), i, payload, i, checksum);
            table.record_frame(&k, FrameDirection::Inbound, &chunk);
            let entry = table.get(&k).unwrap();
            assert!(
                entry.last_activity >= entry.created_at,
                "activity never precedes creation"
            );
            last_activity_points.push(entry.last_activity);
        }
        for pair in last_activity_points.windows(2) {
            assert!(
                pair[1] >= pair[0],
                "last_activity must be monotonic non-decreasing"
            );
        }
        // idle_ms in the snapshot reflects the LAST activity, not the first:
        // it must be (much) smaller than the request's age.
        std::thread::sleep(std::time::Duration::from_millis(15));
        let snap = table.snapshot();
        let req = &snap.active[0];
        assert!(
            req.idle_ms <= req.age_ms,
            "idle {}ms cannot exceed age {}ms",
            req.idle_ms,
            req.age_ms
        );
        assert!(
            req.age_ms >= 45,
            "age accumulates across the request lifetime"
        );
    }

    // TEST7030: A request registers exactly once and terminates exactly once — duplicate registration and double termination are rejected, and after terminate zero state remains for the key.
    #[test]
    fn test7030_register_once_terminate_once() {
        let mut table = RequestTable::new();
        let k = key(1, 100);

        table.register(k.clone(), state(0, None, false)).unwrap();
        assert!(table.contains(&k));
        assert_eq!(table.xid_for_rid(&MessageId::Uint(100)), Some(MessageId::Uint(1)));

        // Duplicate registration of a live key is a protocol violation.
        let err = table
            .register(k.clone(), state(0, None, false))
            .unwrap_err();
        assert!(err.contains("already registered"));

        // Same RID under a different XID is rejected while live.
        let err = table
            .register(key(2, 100), state(0, None, false))
            .unwrap_err();
        assert!(err.contains("already indexed"));

        let removed = table.terminate(&k, TerminalKind::End).expect("live entry");
        assert!(!removed.is_peer);
        assert!(!table.contains(&k), "no entry remains after terminate");
        assert_eq!(
            table.xid_for_rid(&MessageId::Uint(100)),
            None,
            "rid index removed with the entry (L7)"
        );
        assert!(
            table.terminate(&k, TerminalKind::End).is_none(),
            "termination happens exactly once"
        );
    }

    // TEST7031: The rid index and the entry table never disagree across register/terminate cycles, and a terminated rid is immediately reusable.
    #[test]
    fn test7031_rid_index_consistency() {
        let mut table = RequestTable::new();
        for round in 0..3u64 {
            for n in 0..10u64 {
                let k = key(round * 100 + n, n);
                table.register(k, state(0, None, false)).unwrap();
            }
            for n in 0..10u64 {
                let k = key(round * 100 + n, n);
                let xid = table.xid_for_rid(&MessageId::Uint(n)).expect("indexed");
                assert_eq!(xid, k.0, "index resolves to the live entry's xid");
                assert!(table.contains(&(xid, MessageId::Uint(n))));
                table.terminate(&k, TerminalKind::End).unwrap();
                assert_eq!(table.xid_for_rid(&MessageId::Uint(n)), None);
            }
        }
        assert!(table.is_empty());
        assert_eq!(table.snapshot().total_registered, 30);
    }

    // TEST7032: record_frame accumulates per-stream frame/byte/chunk counters by direction, flips phase Created→Streaming on the first flow frame, and tracks unbounded/ended/credit stream markers.
    #[test]
    fn test7032_record_frame_stats_and_phase() {
        let mut table = RequestTable::new();
        let k = key(1, 7);
        table.register(k.clone(), state(0, None, false)).unwrap();
        assert_eq!(table.get(&k).unwrap().phase, RequestPhase::Created);

        let rid = MessageId::Uint(7);
        let ss = Frame::stream_start_unbounded(
            rid.clone(),
            "s1".to_string(),
            "media:enc=utf-8".to_string(),
            None,
        );
        table.record_frame(&k, FrameDirection::Inbound, &ss);
        assert_eq!(table.get(&k).unwrap().phase, RequestPhase::Streaming);

        let payload = vec![0u8; 100];
        let checksum = Frame::compute_checksum(&payload);
        let chunk = Frame::chunk(rid.clone(), "s1".to_string(), 0, payload, 0, checksum);
        table.record_frame(&k, FrameDirection::Inbound, &chunk);
        table.record_frame(&k, FrameDirection::Outbound, &chunk);

        let credit = Frame::credit(rid.clone(), Some("s1".to_string()), 4, crate::bifaci::frame::CreditDirection::Response);
        table.record_frame(&k, FrameDirection::Outbound, &credit);

        let se = Frame::stream_end_unbounded(rid, "s1".to_string());
        table.record_frame(&k, FrameDirection::Inbound, &se);

        let entry = table.get(&k).unwrap();
        let s1 = entry.streams.get(&Some("s1".to_string())).unwrap();
        assert_eq!(s1.frames_in, 3, "stream_start + chunk + stream_end");
        assert_eq!(s1.frames_out, 2, "chunk + credit");
        assert_eq!(s1.chunks_in, 1);
        assert_eq!(s1.chunks_out, 1);
        assert_eq!(s1.bytes_in, 100);
        assert_eq!(s1.bytes_out, 100);
        assert!(s1.unbounded);
        assert!(s1.ended);
        // +4 granted, -1 consumed inbound chunk
        assert_eq!(s1.credit_outstanding, 3);
    }

    // TEST7033: Terminated requests leave a bounded ring of summaries carrying kind, lifetime, and flow totals, and the ring evicts oldest-first at capacity.
    #[test]
    fn test7033_terminated_summaries_ring() {
        let mut table = RequestTable::new();
        for n in 0..(RECENT_TERMINATED_CAP as u64 + 3) {
            let k = key(n, n);
            table.register(k.clone(), state(0, Some(2), true)).unwrap();
            let payload = vec![0u8; 10];
            let checksum = Frame::compute_checksum(&payload);
            let chunk = Frame::chunk(
                MessageId::Uint(n),
                "s".to_string(),
                0,
                payload,
                0,
                checksum,
            );
            table.record_frame(&k, FrameDirection::Inbound, &chunk);
            table.terminate(&k, TerminalKind::Cancelled).unwrap();
        }
        let snap = table.snapshot();
        assert_eq!(snap.recent_terminated.len(), RECENT_TERMINATED_CAP);
        // Oldest evicted: first retained summary is rid "3"
        assert_eq!(snap.recent_terminated[0].rid, MessageId::Uint(3).to_string());
        let last = snap.recent_terminated.last().unwrap();
        assert_eq!(last.kind, TerminalKind::Cancelled);
        assert!(last.is_peer);
        assert_eq!(last.frames_in, 1);
        assert_eq!(last.bytes_in, 10);
        assert_eq!(
            snap.terminated_by_kind.get("cancelled"),
            Some(&(RECENT_TERMINATED_CAP as u64 + 3))
        );
    }
}
