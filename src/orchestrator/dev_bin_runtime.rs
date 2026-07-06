//! The reference [`EngineRuntime`]: hosts cartridges by spawning them per segment via
//! [`execute_dag`]. Used by the capdag CLI and the cartridge scenarios.
//!
//! Unlike the engine runtime (which reuses a long-lived relay switch and persists
//! terminal output to disk), this runtime builds a fresh cartridge host per segment
//! and keeps all output in memory. A ForEach with N items therefore spawns the body
//! cartridge N times — correct, if not warm. It exposes failures rather than
//! tolerating them: any ForEach body failure fails the whole plan.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::bifaci::cartridge_repo::CartridgeChannel;
use crate::bifaci::protocol_trace::ProtocolTraceSink;
use crate::cap::registry::FabricRegistry;
use crate::orchestrator::execute_plan::{EngineRuntime, SegmentOutput};
use crate::orchestrator::executor::execute_dag;
use crate::orchestrator::stream_io::PipelineProgressTracker;
use crate::orchestrator::types::ResolvedGraph;
use crate::{CapProgressFn, CapStepProgressFn, ExecutionError, NodeData, PipelineLogFn};

/// Reference runtime that spawns cartridges per segment through [`execute_dag`].
pub struct DevBinRuntime {
    pub cartridge_dir: PathBuf,
    pub registry_url: String,
    pub channel: CartridgeChannel,
    pub fabric_manifest_version: u32,
    pub dev_binaries: Vec<PathBuf>,
    pub bundled_providers_dir: Option<PathBuf>,
    pub fabric_registry: Arc<FabricRegistry>,
    /// Optional per-segment protocol trace. When set, every segment run through
    /// [`execute_dag`] appends its switch's L8 snapshot as one JSONL line
    /// (`{ ts, segment, stats }`) — the CLI's `--trace` and the scenario
    /// harness's `CAPDAG_SCENARIO_TRACE` wire this. `None` disables tracing.
    pub trace_sink: Option<Arc<ProtocolTraceSink>>,
}

#[async_trait]
impl EngineRuntime for DevBinRuntime {
    async fn run_segment(
        &self,
        graph: &ResolvedGraph,
        initial_inputs: HashMap<String, Vec<u8>>,
        initial_is_sequence: HashMap<String, bool>,
        cap_arguments: &HashMap<String, Vec<(String, Vec<u8>)>>,
        progress_fn: Option<&CapProgressFn>,
        _step_progress_fn: Option<&CapStepProgressFn>,
        log_fn: Option<&PipelineLogFn>,
        _item_index: Option<usize>,
        _stall_tracker: Option<Arc<PipelineProgressTracker>>,
        // Reference runtime keeps everything in memory — no disk persistence — so the
        // persisted-sink set is not honoured here; the CLI persists terminals itself.
        _persist_sinks: &std::collections::HashSet<String>,
    ) -> Result<SegmentOutput, ExecutionError> {
        let inputs: HashMap<String, NodeData> = initial_inputs
            .into_iter()
            .map(|(k, v)| (k, NodeData::Bytes(v)))
            .collect();

        // execute_dag requires a log sink; supply a no-op when the caller has none.
        let noop_log: PipelineLogFn = Arc::new(|_: &str, _: &str, _: &str, _, _| {});
        let log = log_fn.unwrap_or(&noop_log);

        // The reference runtime runs the ONE shared segment executor (execute_dag →
        // run_dag_on_context). It returns the multi-sink `DagOutput` already decoded —
        // CBOR transport stripped, sinks split into per-item bytes — with no writer
        // results (in-memory) and no flow observer on this path.
        execute_dag(
            graph,
            self.cartridge_dir.clone(),
            self.registry_url.clone(),
            self.channel,
            self.fabric_manifest_version,
            inputs,
            initial_is_sequence,
            self.dev_binaries.clone(),
            self.bundled_providers_dir.clone(),
            self.fabric_registry.clone(),
            progress_fn,
            log,
            cap_arguments,
            // execute_dag samples the segment's protocol snapshot live (250ms) and
            // records a final snapshot on both the Ok and Err paths, so a
            // stalled/failed segment still leaves a trace line. Owned Arc so the
            // sampler task can hold its own reference.
            self.trace_sink.clone(),
        )
        .await
    }

    fn fabric_registry(&self) -> Arc<FabricRegistry> {
        self.fabric_registry.clone()
    }

    async fn foreach_partial_failure_policy(&self) -> String {
        // Reference regime: expose failures — any ForEach body failure fails the plan.
        "fail".to_string()
    }
}
