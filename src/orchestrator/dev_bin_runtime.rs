//! The reference [`EngineRuntime`]: hosts cartridges by spawning them per segment via
//! [`execute_dag`]. Used by the capdag CLI and the cartridge scenarios.
//!
//! Unlike the engine runtime (which reuses a long-lived relay switch and persists
//! terminal output to disk), this runtime builds a fresh cartridge host per segment
//! and keeps all output in memory. A ForEach with N items therefore spawns the body
//! cartridge N times — correct, if not warm. It exposes failures rather than
//! tolerating them: any ForEach body failure fails the whole plan.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::bifaci::cartridge_repo::CartridgeChannel;
use crate::cap::registry::FabricRegistry;
use crate::orchestrator::cbor_util::split_cbor_sequence;
use crate::orchestrator::execute_plan::{EngineRuntime, SegmentOutput};
use crate::orchestrator::executor::execute_dag;
use crate::orchestrator::stream_io::{PipelineProgressTracker, TerminalMeta};
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
        _is_terminal: bool,
    ) -> Result<SegmentOutput, ExecutionError> {
        // Per-node output cardinality: a cap-produced node takes its producing cap's
        // output cardinality; a root keeps its declared input flag. This is what lets
        // us split a stored CBOR sequence back into per-item bytes.
        let mut node_is_sequence: HashMap<String, bool> = initial_is_sequence.clone();
        for edge in &graph.edges {
            let out_seq = edge.cap.output.as_ref().map_or(false, |o| o.is_sequence);
            node_is_sequence.insert(edge.to.clone(), out_seq);
        }

        let inputs: HashMap<String, NodeData> = initial_inputs
            .into_iter()
            .map(|(k, v)| (k, NodeData::Bytes(v)))
            .collect();

        // execute_dag requires a log sink; supply a no-op when the caller has none.
        let noop_log: PipelineLogFn =
            Arc::new(|_: &str, _: &str, _: &str, _, _| {});
        let log = log_fn.unwrap_or(&noop_log);

        let raw = execute_dag(
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
        )
        .await?;

        let mut node_data: HashMap<String, Vec<Vec<u8>>> = HashMap::with_capacity(raw.len());
        for (node, data) in raw {
            let bytes = match data {
                NodeData::Bytes(b) => b,
                NodeData::Text(t) => t.into_bytes(),
                NodeData::FilePath(p) => tokio::fs::read(&p).await.map_err(|e| {
                    ExecutionError::HostError(format!(
                        "run_segment: reading output file '{}': {e}",
                        p.display()
                    ))
                })?,
            };
            let items = if node_is_sequence.get(&node).copied().unwrap_or(false) {
                split_cbor_sequence(&bytes).map_err(|e| {
                    ExecutionError::HostError(format!(
                        "run_segment: splitting sequence at '{node}': {e}"
                    ))
                })?
            } else {
                vec![bytes]
            };
            node_data.insert(node, items);
        }

        // The segment's terminal is its sink node (a `to` that is never a `from`); its
        // cardinality is the segment's `is_sequence`.
        let froms: HashSet<&str> = graph.edges.iter().map(|e| e.from.as_str()).collect();
        let is_sequence = graph
            .edges
            .iter()
            .map(|e| e.to.as_str())
            .find(|to| !froms.contains(to))
            .and_then(|sink| node_is_sequence.get(sink))
            .copied();

        Ok(SegmentOutput {
            node_data,
            is_sequence,
            // Reference runtime keeps output in memory — no disk persistence.
            writer_results: Vec::new(),
            // execute_dag does not surface per-item CHUNK metadata; ForEach body titles
            // are a UI-provenance concern the reference path does not populate.
            terminal_meta: TerminalMeta::default(),
        })
    }

    fn fabric_registry(&self) -> Arc<FabricRegistry> {
        self.fabric_registry.clone()
    }

    async fn foreach_partial_failure_policy(&self) -> String {
        // Reference regime: expose failures — any ForEach body failure fails the plan.
        "fail".to_string()
    }
}
