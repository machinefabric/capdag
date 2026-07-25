//! Credit-based per-stream flow control (protocol v4).
//!
//! One credit = permission to send one CHUNK frame. A sender starts each stream
//! with the negotiated `initial_credit` window and must wait when the window is
//! exhausted; the receiving endpoint replenishes it with CREDIT frames as it
//! consumes chunks (L9/L10 in the normative bifaci protocol documentation).
//!
//! `CreditGate` is deliberately built on a mutex + notify pair rather than a
//! semaphore so its semantics translate directly to the mirrors: Python uses a
//! `threading.Condition` over an integer, Go a token channel, Swift a
//! continuation queue. The observable contract is identical everywhere:
//! `acquire` waits until credit is available or the gate closes; `close`
//! releases all waiters with an error; grants never block.

use crate::bifaci::frame::{Frame, FrameType, MessageId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

/// Error returned to a credit waiter when its gate closes (request terminal,
/// cancellation, or connection death) — the waiter must stop sending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditClosed {
    /// Human-readable reason the gate closed (e.g. "CANCELLED", "END").
    pub reason: String,
}

impl std::fmt::Display for CreditClosed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "credit gate closed: {}", self.reason)
    }
}

impl std::error::Error for CreditClosed {}

struct GateState {
    /// Chunks the sender may still emit before waiting.
    available: u64,
    /// Set when the gate is closed; all current and future acquires fail.
    closed: Option<String>,
}

/// A replenishable per-stream credit window for one sender.
///
/// - `acquire(1)` before each CHUNK: returns immediately while the window is
///   open, waits when it is exhausted.
/// - `grant(n)` when a CREDIT frame arrives: wakes waiters.
/// - `close(reason)` on request terminal/cancel: releases all waiters with
///   `CreditClosed` (L13 — a credit-blocked sender must never hang).
pub struct CreditGate {
    state: Mutex<GateState>,
    notify: Notify,
}

impl CreditGate {
    pub fn new(initial_credit: u64) -> Self {
        Self {
            state: Mutex::new(GateState {
                available: initial_credit,
                closed: None,
            }),
            notify: Notify::new(),
        }
    }

    /// Acquire `n` credits, waiting if the window is exhausted.
    /// Fails with `CreditClosed` if the gate closes before (or while) waiting.
    pub async fn acquire(&self, n: u64) -> Result<(), CreditClosed> {
        loop {
            // Register interest BEFORE checking state so a grant/close that
            // lands between the check and the await cannot be missed.
            //
            // CRITICAL: creating `notified()` does NOT register it —
            // `notify_waiters()` only wakes futures that have been polled (or
            // explicitly enabled). Without `enable()`, a grant landing between
            // the window check and the first poll is lost forever and the
            // sender stalls permanently (observed as rare, position-varying
            // pipeline deadlocks with zero dropped frames).
            let notified = self.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            {
                let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(reason) = &s.closed {
                    return Err(CreditClosed {
                        reason: reason.clone(),
                    });
                }
                if s.available >= n {
                    s.available -= n;
                    return Ok(());
                }
            }
            notified.await;
        }
    }

    /// Non-waiting acquire. Returns false when the window is exhausted.
    /// Fails with `CreditClosed` if the gate is closed.
    pub fn try_acquire(&self, n: u64) -> Result<bool, CreditClosed> {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(reason) = &s.closed {
            return Err(CreditClosed {
                reason: reason.clone(),
            });
        }
        if s.available >= n {
            s.available -= n;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Blocking acquire for non-async contexts (FFI threads, spawn_blocking).
    /// Spins on try_acquire with a short park; the park interval is invisible
    /// to the protocol (only wall-clock throughput of a blocked sender).
    pub fn blocking_acquire(&self, n: u64) -> Result<(), CreditClosed> {
        loop {
            if self.try_acquire(n)? {
                return Ok(());
            }
            std::thread::park_timeout(std::time::Duration::from_millis(5));
        }
    }

    /// Replenish the window by `n` chunks and wake all waiters.
    pub fn grant(&self, n: u64) {
        {
            let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if s.closed.is_some() {
                return; // grants after close are no-ops
            }
            s.available = s.available.saturating_add(n);
        }
        self.notify.notify_waiters();
    }

    /// Close the gate: all current and future acquires fail with `CreditClosed`.
    pub fn close(&self, reason: &str) {
        {
            let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if s.closed.is_none() {
                s.closed = Some(reason.to_string());
            }
        }
        self.notify.notify_waiters();
    }

    /// Currently available credit (diagnostic/stats).
    pub fn available(&self) -> u64 {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .available
    }

    /// Whether the gate has been closed.
    pub fn is_closed(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .closed
            .is_some()
    }
}

/// Routes inbound CREDIT frames to the gates of the streams they credit.
///
/// Keyed by (rid, stream_id). A CREDIT frame with no stream_id credits the
/// request's sole/default stream: it matches the request's single registered
/// gate when exactly one exists.
#[derive(Clone, Default)]
pub struct CreditRouter {
    gates: Arc<Mutex<HashMap<(MessageId, Option<String>), Arc<CreditGate>>>>,
}

impl CreditRouter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a gate for a stream a local sender is about to write.
    pub fn register(&self, rid: MessageId, stream_id: Option<String>, gate: Arc<CreditGate>) {
        self.gates
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert((rid, stream_id), gate);
    }

    /// Remove and close every gate belonging to a request (terminal/cancel).
    /// Waiters blocked on those gates are released with `CreditClosed` (L13).
    pub fn close_request(&self, rid: &MessageId, reason: &str) {
        let mut gates = self.gates.lock().unwrap_or_else(|e| e.into_inner());
        let keys: Vec<_> = gates
            .keys()
            .filter(|(r, _)| r == rid)
            .cloned()
            .collect();
        for key in keys {
            if let Some(gate) = gates.remove(&key) {
                gate.close(reason);
            }
        }
    }

    /// Deliver a CREDIT frame's grant to the matching gate.
    /// Returns false when no gate matches (request finished or the sender is
    /// not credit-registered) — a correct no-op, since grants only unblock.
    pub fn grant(&self, frame: &Frame) -> bool {
        if frame.frame_type != FrameType::Credit {
            return false;
        }
        let Some(credits) = frame.credit_count() else {
            return false;
        };
        let gates = self.gates.lock().unwrap_or_else(|e| e.into_inner());
        let exact = gates.get(&(frame.id.clone(), frame.stream_id.clone()));
        if let Some(gate) = exact {
            gate.grant(credits);
            return true;
        }
        // No stream_id on the grant: match the request's sole gate if exactly one.
        if frame.stream_id.is_none() {
            let mut request_gates = gates.iter().filter(|((r, _), _)| *r == frame.id);
            if let (Some((_, gate)), None) = (request_gates.next(), request_gates.next()) {
                gate.grant(credits);
                return true;
            }
        }
        false
    }

    /// Number of registered gates (diagnostic/stats).
    pub fn len(&self) -> usize {
        self.gates.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // TEST7015: CreditGate acquire succeeds immediately within the initial window and waits when exhausted until a grant arrives.
    #[tokio::test]
    async fn test7015_credit_gate_acquire_and_grant() {
        let gate = Arc::new(CreditGate::new(2));
        gate.acquire(1).await.unwrap();
        gate.acquire(1).await.unwrap();
        assert_eq!(gate.available(), 0);

        let g2 = Arc::clone(&gate);
        let waiter = tokio::spawn(async move { g2.acquire(1).await });
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!waiter.is_finished(), "acquire must wait at zero credit");

        gate.grant(1);
        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("waiter must wake on grant")
            .unwrap()
            .unwrap();
    }

    // TEST7016: CreditGate close releases blocked waiters with CreditClosed and fails all future acquires.
    #[tokio::test]
    async fn test7016_credit_gate_close_releases_waiters() {
        let gate = Arc::new(CreditGate::new(0));
        let g2 = Arc::clone(&gate);
        let waiter = tokio::spawn(async move { g2.acquire(1).await });
        tokio::time::sleep(Duration::from_millis(50)).await;

        gate.close("CANCELLED");
        let err = tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("waiter must wake on close")
            .unwrap()
            .unwrap_err();
        assert_eq!(err.reason, "CANCELLED");
        assert!(gate.acquire(1).await.is_err(), "closed gate rejects acquire");
        gate.grant(5); // no-op after close
        assert!(gate.acquire(1).await.is_err());
    }

    // TEST7017: CreditRouter routes grants by (rid, stream_id), falls back to a request's sole gate for stream-less grants, and reports unmatched grants.
    #[tokio::test]
    async fn test7017_credit_router_routing() {
        let router = CreditRouter::new();
        let rid = MessageId::new_uuid();
        let gate = Arc::new(CreditGate::new(0));
        router.register(rid.clone(), Some("s1".to_string()), Arc::clone(&gate));

        // Exact (rid, stream) match
        let f = Frame::credit(rid.clone(), Some("s1".to_string()), 3, crate::bifaci::frame::CreditDirection::Response);
        assert!(router.grant(&f));
        assert_eq!(gate.available(), 3);

        // Stream-less grant matches the sole gate
        let f = Frame::credit(rid.clone(), None, 2, crate::bifaci::frame::CreditDirection::Response);
        assert!(router.grant(&f));
        assert_eq!(gate.available(), 5);

        // Second gate makes a stream-less grant ambiguous → unmatched
        let gate2 = Arc::new(CreditGate::new(0));
        router.register(rid.clone(), Some("s2".to_string()), gate2);
        let f = Frame::credit(rid.clone(), None, 1, crate::bifaci::frame::CreditDirection::Response);
        assert!(!router.grant(&f));

        // Unknown request → unmatched no-op
        let f = Frame::credit(MessageId::new_uuid(), None, 1, crate::bifaci::frame::CreditDirection::Response);
        assert!(!router.grant(&f));
    }

    // TEST7018: CreditRouter close_request closes and removes every gate of the request, releasing their waiters.
    #[tokio::test]
    async fn test7018_credit_router_close_request() {
        let router = CreditRouter::new();
        let rid = MessageId::new_uuid();
        let g1 = Arc::new(CreditGate::new(0));
        let g2 = Arc::new(CreditGate::new(0));
        router.register(rid.clone(), Some("a".to_string()), Arc::clone(&g1));
        router.register(rid.clone(), Some("b".to_string()), Arc::clone(&g2));

        let g1c = Arc::clone(&g1);
        let waiter = tokio::spawn(async move { g1c.acquire(1).await });
        tokio::time::sleep(Duration::from_millis(50)).await;

        router.close_request(&rid, "END");
        assert!(router.is_empty());
        assert!(g2.is_closed());
        let err = tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert_eq!(err.reason, "END");
    }
}
