//! `execute_plan` — the single ForEach-aware, fan-out-aware plan executor.
//!
//! This is the intelligent execution path, shared by the reference/CLI runtime and
//! the engine. It runs a [`MachinePlan`] (a branching DAG) by partitioning it into:
//!
//! - a **trunk**: the maximal ForEach-free subgraph (input slots + caps not inside any
//!   ForEach body). The trunk is run as a single segment through the pluggable
//!   [`EngineRuntime`], which streams cap-to-cap and handles **fan-out** natively.
//! - one **ForEach region** per ForEach node: the per-item body subgraph. Each region
//!   maps its input sequence, running the body (itself possibly a fan-out subgraph)
//!   once per item and collecting every region node's per-item output back into a
//!   sequence.
//!
//! A plan has one terminal per `Output` node — a linear machine has one, a fan-out
//! machine has several — so the result is a [`PipelineResult`] of [`TerminalOutput`]s.
//!
//! Backend differences (how cartridges are hosted, whether terminal output is
//! persisted) live behind [`EngineRuntime`]; the partition, per-item fan-out, and
//! result assembly live here once.
//!
//! Nested ForEach (a sequence produced inside a body, re-mapped) is a hard error.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;

use crate::cap::registry::FabricRegistry;
use crate::orchestrator::plan_converter::plan_to_resolved_graph;
use crate::orchestrator::stream_io::{PipelineProgressTracker, TerminalMeta};
use crate::orchestrator::types::ResolvedGraph;
use crate::orchestrator::ParseOrchestrationError;
use crate::planner::plan_analysis::derive_foreach_media_urns;
use crate::planner::{
    ExecutionNodeType, InputCardinality, MachineNode, MachinePlan, MachinePlanEdge,
};
use crate::{
    BodyOutcome, CapProgressFn, CapStepProgressFn, ExecutionError, PipelineLogFn, StreamMeta,
    PIPELINE_STALL_TIMEOUT_SECS,
};

/// How much of an item's bytes to keep as a preview snippet.
const ITEM_PREVIEW_CAP: usize = 2048;

// =============================================================================
// Result + callback types
// =============================================================================

/// Result of executing a plan. A plan has one [`TerminalOutput`] per `Output` node —
/// a linear machine has one, a fan-out machine has several (one per terminal anchor).
#[derive(Debug, Clone)]
pub struct PipelineResult {
    /// One entry per plan `Output` node, keyed by its node id.
    pub terminals: Vec<TerminalOutput>,
    /// Per-body outcome tracking across every ForEach region (and one entry per
    /// ForEach-free trunk execution).
    pub body_outcomes: Vec<BodyOutcome>,
}

impl PipelineResult {
    /// The terminal for a given `Output` node id.
    pub fn terminal(&self, output_node_id: &str) -> Option<&TerminalOutput> {
        self.terminals.iter().find(|t| t.output_node_id == output_node_id)
    }
}

/// One plan terminal (`Output` node) and the data produced at it.
///
/// When the runtime persists terminal output, `writer_results` carries the file paths
/// and `items` is empty; otherwise `items` holds the in-memory data.
#[derive(Debug, Clone)]
pub struct TerminalOutput {
    /// The plan `Output` node id this terminal corresponds to (e.g. `output_5`).
    pub output_node_id: String,
    /// Unwrapped output blobs. Empty when `writer_results` is populated (on disk).
    pub items: Vec<OutputItem>,
    /// Whether the output is a sequence — a cap with sequence output, or the per-item
    /// results of a ForEach region (structurally a sequence regardless of item count).
    pub is_sequence: bool,
    /// Terminal output media URN.
    pub media_urn: String,
    /// Results from any incremental disk writes. Empty when the runtime does not persist.
    pub writer_results: Vec<WriterResult>,
}

/// A single unwrapped output item.
#[derive(Debug, Clone)]
pub struct OutputItem {
    /// Raw bytes (CBOR transport stripped).
    pub data: Vec<u8>,
    /// Item index (0 for scalar, 0..N for sequence items).
    pub index: usize,
}

/// Result of a runtime persisting a terminal segment's output to disk.
///
/// A segment run through `run_segment` is always linear with a single sink, so one
/// `WriterResult` corresponds to one terminal — `execute_plan` tracks which sink node
/// that is from the segment it ran.
#[derive(Debug, Clone)]
pub struct WriterResult {
    pub is_sequence: bool,
    pub media_urn: String,
    /// Blob: single path. Sequence: one path per item.
    pub saved_paths: Vec<String>,
    pub total_bytes: usize,
    /// Blob-mode stream meta (STREAM_START); `None` in sequence mode.
    pub stream_meta: Option<StreamMeta>,
    /// Sequence-mode per-item meta; empty in blob mode.
    pub item_metas: Vec<Option<StreamMeta>>,
}

/// Per-item callback — delivers each output item as it is produced.
pub type PipelineItemFn = Arc<dyn Fn(&OutputItem, usize) + Send + Sync>;

/// Per-body outcome callback — delivers the running set of body outcomes.
pub type BodyOutcomeFn = Arc<dyn Fn(&[BodyOutcome]) + Send + Sync>;

/// Output of running one resolved (ForEach-free) segment through an [`EngineRuntime`].
///
/// A segment is now an arbitrary ForEach-free DAG (fan-out and convergence included),
/// so its output is the multi-sink [`DagOutput`] the shared executor produces: every
/// node's items, each node's cardinality, and per-persisted-sink writer results.
pub use crate::orchestrator::executor::DagOutput as SegmentOutput;

// =============================================================================
// EngineRuntime — the pluggable cartridge-execution backend
// =============================================================================

/// The cartridge-execution backend `execute_plan` streams segments through.
///
/// Implementations differ only in how cartridges are hosted and whether terminal
/// output is persisted — the trunk/region partition and result assembly are backend
/// independent and live in [`execute_plan`].
#[async_trait]
pub trait EngineRuntime: Send + Sync {
    // ── Backend hooks: the ONLY things that differ between the engine and the CLI ──
    //
    // Every backend reuses ONE long-lived `RelaySwitch` across all segments (including
    // every ForEach body), so a cap's cartridge process is spawned once and each body
    // multiplexes onto it. That single process's `set_capacity(1)` then serializes the
    // model loads — which is what stops a ForEach fan-out from loading N model copies
    // into GPU memory at once. This contract is identical for the engine
    // (`EngineHostRuntime`, daemon-hosted cartridges) and the CLI (`CliRuntime`,
    // in-process dev-bin cartridges); only the hooks below differ.

    /// The long-lived relay switch to run this segment's caps on. `graph` lets a
    /// backend lazily ensure the segment's cartridges are hosted before returning the
    /// switch (the CLI registers dev-bin cartridge hosts on demand; the engine ignores
    /// `graph` and returns its already-populated switch). Called once per segment; the
    /// returned switch is shared, never torn down here.
    async fn segment_switch(
        &self,
        graph: &ResolvedGraph,
    ) -> Result<Arc<crate::bifaci::relay_switch::RelaySwitch>, ExecutionError>;

    /// Per-item activity timeout (seconds) for this segment. The engine reads it from
    /// its config service (a config-service failure is propagated — fail hard, never
    /// silently defaulted); the CLI reads it from the terminal cap's metadata. A value
    /// the source does not specify uses the documented default, but an *error* fetching
    /// it aborts the segment.
    async fn activity_timeout_secs(
        &self,
        graph: &ResolvedGraph,
    ) -> Result<u64, ExecutionError>;

    /// Disk writer factory for persisted terminal sinks. `Some` for the engine (each
    /// persisted sink gets a writer bound to the run artifact dir); `None` for the CLI
    /// (everything stays in memory). Default `None`.
    fn writer_factory(&self) -> Option<Box<crate::orchestrator::stream_io::SegmentWriterFactory>> {
        None
    }

    /// Flow observer correlating request ids to strand steps for run flow snapshots.
    /// `Some` for the engine; `None` for the CLI. Default `None`.
    fn flow_observer(&self) -> Option<&dyn crate::orchestrator::stream_io::FlowObserver> {
        None
    }

    /// Per-segment protocol trace sink. `Some` when the CLI's `--trace` /
    /// the scenario harness's `CAPDAG_SCENARIO_TRACE` is active — the segment is then
    /// sampled live (250ms) and snapshotted at teardown. `None` otherwise (the engine
    /// runs its own switch-level dev trace instead). Default `None`.
    fn trace_sink(&self) -> Option<Arc<crate::bifaci::protocol_trace::ProtocolTraceSink>> {
        None
    }

    /// The fabric registry, for plan→resolved-graph conversion.
    fn fabric_registry(&self) -> Arc<FabricRegistry>;

    /// ForEach partial-failure policy: `"fail"` (any body failure fails the plan) or
    /// `"allow"` (fail only when every body failed).
    async fn foreach_partial_failure_policy(&self) -> String;

    // ── Provided orchestration: identical for every backend ──

    /// Run one resolved segment — an arbitrary ForEach-free DAG (fan-out and
    /// convergence included). The shared executor decomposes it into pipelined chains,
    /// materialising producers at every non-linear junction, so both backends handle
    /// identical input.
    ///
    /// This is the abstraction's job and is the SAME for every backend: get the shared
    /// switch, build a per-segment [`ExecutionContext`] over it, set inputs, wire the
    /// optional protocol trace, and drive `run_dag_on_context`. Backends customise only
    /// via the hooks above — they do not override this.
    ///
    /// - `item_index`: `Some(i)` when this is the body of ForEach iteration `i`.
    /// - `persist_sinks`: the sink node ids whose output is plan terminal. A persisting
    ///   runtime writes each to disk (one writer per sink) and returns its
    ///   `writer_results`; every other node is kept in memory.
    async fn run_segment(
        &self,
        graph: &ResolvedGraph,
        initial_inputs: HashMap<String, Vec<u8>>,
        initial_is_sequence: HashMap<String, bool>,
        cap_arguments: &HashMap<String, Vec<(String, Vec<u8>)>>,
        progress_fn: Option<&CapProgressFn>,
        step_progress_fn: Option<&CapStepProgressFn>,
        log_fn: Option<&PipelineLogFn>,
        item_index: Option<usize>,
        stall_tracker: Option<Arc<PipelineProgressTracker>>,
        persist_sinks: &HashSet<String>,
    ) -> Result<SegmentOutput, ExecutionError> {
        use crate::orchestrator::executor::{run_dag_on_context, ExecutionContext};

        let activity_timeout_secs = self.activity_timeout_secs(graph).await?;

        // Shared, long-lived switch (cartridges hosted + reused). The per-segment
        // context is lightweight: its own node_data, no cleanup handles — so dropping
        // it at the end of the segment does NOT tear down the shared cartridge host.
        let switch = self.segment_switch(graph).await?;
        let mut ctx = ExecutionContext::from_switch(switch).await?;

        // Strict 1:1 between initial_inputs and initial_is_sequence — every input node
        // carries an explicit scalar/sequence flag (the invariant run_dag_on_context
        // relies on; a missing flag would silently send a sequence as a scalar blob).
        let input_keys: HashSet<&str> = initial_inputs.keys().map(|s| s.as_str()).collect();
        let flag_keys: HashSet<&str> = initial_is_sequence.keys().map(|s| s.as_str()).collect();
        let missing: Vec<&str> = input_keys.difference(&flag_keys).copied().collect();
        if !missing.is_empty() {
            return Err(ExecutionError::HostError(format!(
                "run_segment: initial_is_sequence is missing flags for input node(s) {missing:?}"
            )));
        }
        let extra: Vec<&str> = flag_keys.difference(&input_keys).copied().collect();
        if !extra.is_empty() {
            return Err(ExecutionError::HostError(format!(
                "run_segment: initial_is_sequence has stale flags for node(s) {extra:?}"
            )));
        }
        for (node, data) in initial_inputs {
            let is_seq = *initial_is_sequence
                .get(&node)
                .expect("key set verified above");
            ctx.set_node_is_sequence(node.clone(), is_seq);
            ctx.set_node_data(node, data);
        }

        // Optional per-segment protocol trace (CLI). When a sink is supplied, sample the
        // switch's L8 snapshot live every 250ms so a HANGING segment still leaves a stall
        // line, and snapshot once more at teardown on both the Ok and Err paths. A live
        // sample write failure is logged and swallowed; the final snapshot is fail-hard.
        let trace_sink = self.trace_sink();
        let trace_label = trace_sink.as_ref().map(|_| {
            graph
                .edges
                .last()
                .map(|e| e.cap_urn.clone())
                .unwrap_or_else(|| "empty-graph".to_string())
        });
        let trace_sampler = match (&trace_sink, &trace_label) {
            (Some(sink), Some(label)) => {
                let switch = ctx.switch().clone();
                let sink = sink.clone();
                let label = label.clone();
                Some(tokio::spawn(async move {
                    let mut ticker =
                        tokio::time::interval(std::time::Duration::from_millis(250));
                    loop {
                        ticker.tick().await;
                        let stats = switch.protocol_stats().await;
                        if let Err(e) = sink.record_deduped(&stats, &label).await {
                            tracing::debug!(
                                error = %e, segment = %label,
                                "protocol trace live sample failed (continuing)"
                            );
                        }
                    }
                }))
            }
            _ => None,
        };

        let writer = self.writer_factory();
        let observer = self.flow_observer();
        let out = run_dag_on_context(
            &mut ctx,
            graph,
            cap_arguments,
            progress_fn,
            step_progress_fn,
            log_fn,
            item_index,
            stall_tracker,
            writer.as_deref(),
            persist_sinks,
            activity_timeout_secs,
            observer,
        )
        .await;

        // Stop the live sampler before the final snapshot so they cannot race on the file.
        if let Some(handle) = trace_sampler {
            handle.abort();
            let _ = handle.await;
        }
        if let (Some(sink), Some(label)) = (&trace_sink, &trace_label) {
            let stats = ctx.switch().protocol_stats().await;
            if let Err(e) = sink.record_deduped(&stats, label).await {
                match &out {
                    Ok(_) => {
                        return Err(ExecutionError::HostError(format!(
                            "protocol trace write failed for segment '{label}': {e}"
                        )))
                    }
                    Err(_) => tracing::error!(
                        error = %e, segment = %label,
                        "protocol trace write failed on the segment error path"
                    ),
                }
            }
        }
        out
    }
}

// =============================================================================
// Region model
// =============================================================================

/// One ForEach region: the per-item body subgraph mapped over `input_node`'s sequence.
struct Region {
    fe_id: String,
    /// The producer node whose sequence output this region iterates.
    input_node: String,
    /// The body's per-item entry cap (fed the single item).
    body_entry: String,
    /// Synthetic input-slot id for the body sub-plan.
    body_input_id: String,
    /// The item's media URN (the element type of the input sequence).
    item_media: String,
    /// Every cap node inside the body (in-body fan-out included).
    body_nodes: Vec<String>,
    /// Cap URNs of the body, for outcome reporting.
    body_cap_urns: Vec<String>,
}

/// Compute the ForEach regions of a plan. Each ForEach node defines a region whose body
/// is every cap reachable from `body_entry` via `Direct` edges (in-body fan-out
/// included), stopping at `Output` nodes. Regions are disjoint (one producer per node).
async fn compute_regions(
    plan: &MachinePlan,
    registry: &FabricRegistry,
) -> Result<Vec<Region>, ExecutionError> {
    // Forward adjacency over Direct edges only (Iteration/Collection wire ForEach nodes).
    let mut direct_adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &plan.edges {
        if matches!(edge.edge_type, crate::planner::EdgeType::Direct) {
            direct_adj.entry(edge.from_node.as_str()).or_default().push(edge.to_node.as_str());
        }
    }

    let mut regions = Vec::new();
    for (fe_id, node) in &plan.nodes {
        let ExecutionNodeType::ForEach { input_node, body_entry, .. } = &node.node_type else {
            continue;
        };

        // BFS the body: caps reachable from body_entry via Direct edges, excluding
        // Output nodes and any other ForEach node. A cap whose input is a
        // SEQUENCE closes the region: it consumes the region's collected
        // per-item output as one sequence (the fold), so it belongs to the
        // post-region trunk, never to the body.
        let mut body_nodes: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(body_entry.clone());
        seen.insert(body_entry.clone());
        while let Some(nid) = queue.pop_front() {
            match plan.nodes.get(&nid).map(|n| &n.node_type) {
                Some(ExecutionNodeType::Cap { cap_urn, .. }) => {
                    if nid != *body_entry {
                        let cap = registry.get_cached_cap(cap_urn).ok_or_else(|| {
                            ExecutionError::HostError(format!(
                                "cap '{cap_urn}' at '{nid}' is not in the registry cache"
                            ))
                        })?;
                        let (input_is_sequence, _) = cap.sequence_shape();
                        if input_is_sequence {
                            // Fold consumer — closes the region; runs post-region.
                            continue;
                        }
                    }
                    body_nodes.push(nid.clone());
                }
                Some(ExecutionNodeType::ForEach { .. }) => {
                    return Err(ExecutionError::HostError(format!(
                        "nested ForEach reached from body of '{fe_id}' at '{nid}' — unsupported"
                    )));
                }
                _ => continue, // Output / InputSlot / Collect: not body caps.
            }
            if let Some(children) = direct_adj.get(nid.as_str()) {
                for &child in children {
                    if seen.insert(child.to_string()) {
                        queue.push_back(child.to_string());
                    }
                }
            }
        }
        body_nodes.sort();

        let body_cap_urns: Vec<String> = body_nodes
            .iter()
            .filter_map(|nid| match &plan.nodes.get(nid)?.node_type {
                ExecutionNodeType::Cap { cap_urn, .. } => Some(cap_urn.clone()),
                _ => None,
            })
            .collect();

        let (_list_media, item_media) =
            derive_foreach_media_urns(plan, input_node).map_err(|e| {
                ExecutionError::HostError(format!("derive foreach item media for '{fe_id}': {e}"))
            })?;
        regions.push(Region {
            fe_id: fe_id.clone(),
            input_node: input_node.clone(),
            body_entry: body_entry.clone(),
            body_input_id: format!("{fe_id}_body_input"),
            item_media,
            body_nodes,
            body_cap_urns,
        });
    }
    // Deterministic order.
    regions.sort_by(|a, b| a.fe_id.cmp(&b.fe_id));
    Ok(regions)
}

/// The trunk caps that depend — transitively, over data-flow edges — on a
/// ForEach region's output: they can only run AFTER the regions (a fold cap
/// consuming the collected per-item sequence, and everything downstream of
/// it). Includes gather `Collect` nodes on the post side.
fn compute_post_region_caps(
    plan: &MachinePlan,
    region_nodes: &HashSet<String>,
) -> HashSet<String> {
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &plan.edges {
        if matches!(
            edge.edge_type,
            crate::planner::EdgeType::Direct
                | crate::planner::EdgeType::Arg { .. }
                | crate::planner::EdgeType::Collection
        ) {
            adj.entry(edge.from_node.as_str()).or_default().push(edge.to_node.as_str());
        }
    }
    let mut post: HashSet<String> = HashSet::new();
    let mut queue: std::collections::VecDeque<&str> =
        region_nodes.iter().map(|s| s.as_str()).collect();
    let mut seen: HashSet<&str> = queue.iter().copied().collect();
    while let Some(nid) = queue.pop_front() {
        if let Some(children) = adj.get(nid) {
            for &child in children {
                if !seen.insert(child) {
                    continue;
                }
                queue.push_back(child);
                if region_nodes.contains(child) {
                    continue;
                }
                match plan.nodes.get(child).map(|n| &n.node_type) {
                    Some(ExecutionNodeType::Cap { .. })
                    | Some(ExecutionNodeType::Collect { .. }) => {
                        post.insert(child.to_string());
                    }
                    _ => {}
                }
            }
        }
    }
    post
}

/// The post-region sub-plan: the post caps/Collects with every data-flow edge
/// INTO them (external producers stay as root references — their materialized
/// data seeds the segment's node table).
fn build_post_subplan(plan: &MachinePlan, post_ids: &HashSet<String>) -> MachinePlan {
    let mut post = MachinePlan::new(&format!("{} [post-region]", plan.name));
    for id in post_ids {
        if let Some(node) = plan.nodes.get(id) {
            post.add_node(node.clone());
        }
    }
    for edge in &plan.edges {
        if matches!(
            edge.edge_type,
            crate::planner::EdgeType::Direct
                | crate::planner::EdgeType::Arg { .. }
                | crate::planner::EdgeType::Collection
        ) && post_ids.contains(&edge.to_node)
        {
            post.add_edge(edge.clone());
        }
    }
    post
}

/// The trunk sub-plan: input slots + every cap NOT inside a ForEach body, with the
/// `Direct` edges among them. ForEach nodes, region caps, and `Output` nodes are
/// dropped — the resulting graph is ForEach-free and fan-out-capable.
fn build_trunk_subplan(plan: &MachinePlan, region_nodes: &HashSet<String>) -> MachinePlan {
    let mut trunk = MachinePlan::new(&format!("{} [trunk]", plan.name));
    let mut trunk_ids: HashSet<String> = HashSet::new();
    for (id, node) in &plan.nodes {
        match &node.node_type {
            ExecutionNodeType::InputSlot { .. } => {
                trunk.add_node(node.clone());
                trunk_ids.insert(id.clone());
            }
            ExecutionNodeType::Cap { .. } if !region_nodes.contains(id) => {
                trunk.add_node(node.clone());
                trunk_ids.insert(id.clone());
            }
            // Standalone / gather Collects (output_media_urn set) are trunk
            // structure: plan_converter resolves them through into per-producer
            // edges and the segment executor gathers at the consuming arg.
            ExecutionNodeType::Collect {
                output_media_urn: Some(_),
                ..
            } => {
                trunk.add_node(node.clone());
                trunk_ids.insert(id.clone());
            }
            _ => {}
        }
    }
    for edge in &plan.edges {
        // Data-flow edges among trunk nodes: the main input (`Direct`), any
        // convergence input (`Arg`), and gather wiring into a Collect
        // (`Collection`). All must survive into the subgraph the executor runs.
        if matches!(
            edge.edge_type,
            crate::planner::EdgeType::Direct
                | crate::planner::EdgeType::Arg { .. }
                | crate::planner::EdgeType::Collection
        ) && trunk_ids.contains(&edge.from_node)
            && trunk_ids.contains(&edge.to_node)
        {
            trunk.add_edge(edge.clone());
        }
    }
    trunk
}

/// The body sub-plan for a region: a synthetic input slot feeding `body_entry`, plus
/// every body cap and the `Direct` edges among them. Run once per item; the executor
/// reads each body cap's output from the returned `node_data` (so in-body fan-out is
/// handled by the segment runtime, which does fan-out).
fn build_body_subplan(plan: &MachinePlan, region: &Region) -> MachinePlan {
    let mut body = MachinePlan::new(&format!("{} [foreach body {}]", plan.name, region.fe_id));
    body.add_node(MachineNode::input_slot(
        &region.body_input_id,
        "item_input",
        &region.item_media,
        InputCardinality::Single,
    ));
    let body_set: HashSet<&str> = region.body_nodes.iter().map(|s| s.as_str()).collect();
    for nid in &region.body_nodes {
        if let Some(node) = plan.nodes.get(nid) {
            body.add_node(node.clone());
        }
    }
    body.add_edge(MachinePlanEdge::direct(&region.body_input_id, &region.body_entry));
    for edge in &plan.edges {
        // Data-flow edges among body nodes: the main input (`Direct`) and any in-body
        // convergence input (`Arg`).
        if matches!(
            edge.edge_type,
            crate::planner::EdgeType::Direct | crate::planner::EdgeType::Arg { .. }
        ) && body_set.contains(edge.from_node.as_str())
            && body_set.contains(edge.to_node.as_str())
        {
            body.add_edge(edge.clone());
        }
    }
    body
}

// =============================================================================
// Sub-plan execution
// =============================================================================

/// Run one ForEach-free `MachinePlan` (the trunk, or a ForEach body) as a single DAG
/// through the engine runtime. The sub-plan may fan out and converge freely; the
/// shared executor (`run_dag_on_context`, inside `run_segment`) decomposes it into
/// pipelined chains, materialising producers at every non-linear junction — so the old
/// per-chain decomposition is gone and convergence (`Arg`) edges survive to execution.
/// `roots` supplies each input slot's materialised bytes + sequence flag (keyed by the
/// slot node id); `persist_sinks` names the cap nodes whose output is a plan terminal.
/// Returns the runtime's multi-sink [`SegmentOutput`]. Progress `p` is mapped into
/// `[base, base + weight]`.
#[allow(clippy::too_many_arguments)]
async fn run_subplan(
    runtime: &Arc<dyn EngineRuntime>,
    registry: &Arc<FabricRegistry>,
    subplan: &MachinePlan,
    roots: HashMap<String, (Vec<u8>, bool)>,
    persist_sinks: &HashSet<String>,
    cap_arguments: &HashMap<String, Vec<(String, Vec<u8>)>>,
    progress_fn: Option<&CapProgressFn>,
    step_progress_fn: Option<&CapStepProgressFn>,
    log_fn: Option<&PipelineLogFn>,
    item_index: Option<usize>,
    stall_tracker: Option<Arc<PipelineProgressTracker>>,
    progress_base: f32,
    progress_weight: f32,
) -> Result<SegmentOutput, ExecutionError> {
    let graph = to_graph(subplan, registry).await?;

    let mut inputs: HashMap<String, Vec<u8>> = HashMap::new();
    let mut is_seq: HashMap<String, bool> = HashMap::new();
    for (id, (bytes, seq)) in roots {
        inputs.insert(id.clone(), bytes);
        is_seq.insert(id, seq);
    }

    // Scale the segment's own [0,1] progress into this sub-plan's slice of the run.
    let seg_pfn: Option<CapProgressFn> = progress_fn.map(|parent| {
        let parent = parent.clone();
        Arc::new(move |p: f32, cap_urn: &str, msg: &str| {
            parent(progress_base + progress_weight * p, cap_urn, msg);
        }) as CapProgressFn
    });

    runtime
        .run_segment(
            &graph,
            inputs,
            is_seq,
            cap_arguments,
            seg_pfn.as_ref(),
            step_progress_fn,
            log_fn,
            item_index,
            stall_tracker,
            persist_sinks,
        )
        .await
}

// =============================================================================
// execute_plan
// =============================================================================

/// Execute a `MachinePlan` (a branching DAG), returning one [`TerminalOutput`] per
/// plan `Output` node.
#[allow(clippy::too_many_arguments)]
pub async fn execute_plan(
    plan: &MachinePlan,
    runtime: Arc<dyn EngineRuntime>,
    initial_inputs: HashMap<String, Vec<u8>>,
    initial_is_sequence: HashMap<String, bool>,
    cap_arguments: &HashMap<String, Vec<(String, Vec<u8>)>>,
    progress_fn: Option<&CapProgressFn>,
    step_progress_fn: Option<&CapStepProgressFn>,
    log_fn: Option<&PipelineLogFn>,
    item_fn: Option<&PipelineItemFn>,
    body_outcome_fn: Option<&BodyOutcomeFn>,
) -> Result<PipelineResult, ExecutionError> {
    let registry = runtime.fabric_registry();

    // Terminals: (Output node id, source producer node id), deterministically ordered.
    let mut outputs: Vec<(String, String)> = plan
        .nodes
        .iter()
        .filter_map(|(id, n)| match &n.node_type {
            ExecutionNodeType::Output { source_node, .. } => {
                Some((id.clone(), source_node.clone()))
            }
            _ => None,
        })
        .collect();
    outputs.sort();
    if outputs.is_empty() {
        return Err(ExecutionError::HostError("plan has no Output node".to_string()));
    }

    let regions = compute_regions(plan, &registry).await?;
    let region_nodes: HashSet<String> =
        regions.iter().flat_map(|r| r.body_nodes.iter().cloned()).collect();
    // Caps that consume region output (the fold and everything after it) run
    // AFTER the regions, in their own segment.
    let post_ids = compute_post_region_caps(plan, &region_nodes);

    // Accumulated producer output items (node id → items) across trunk + regions.
    let mut node_data: HashMap<String, Vec<Vec<u8>>> = HashMap::new();
    let mut node_seq: HashMap<String, bool> = HashMap::new();
    let mut node_writers: HashMap<String, Vec<WriterResult>> = HashMap::new();
    let mut body_outcomes: Vec<BodyOutcome> = Vec::new();

    // Cap nodes whose output is a plan terminal — persisted when the runtime persists.
    let persist_sinks: HashSet<String> = outputs.iter().map(|(_, src)| src.clone()).collect();

    // ── Trunk (ForEach-free, pre-region) — decomposed into linear chains,
    // materialized at fan-out. Region caps AND post-region caps are excluded. ──
    let mut trunk_excluded = region_nodes.clone();
    trunk_excluded.extend(post_ids.iter().cloned());
    let trunk = build_trunk_subplan(plan, &trunk_excluded);
    let mut trunk_roots: HashMap<String, (Vec<u8>, bool)> = HashMap::new();
    for (k, v) in initial_inputs {
        let seq = *initial_is_sequence.get(&k).ok_or_else(|| {
            ExecutionError::HostError(format!("initial input '{k}' has no sequence flag"))
        })?;
        trunk_roots.insert(k, (v, seq));
    }

    // Trunk gets the first slice of the progress band; regions share the rest.
    let trunk_weight = if regions.is_empty() { 1.0 } else { 0.15 };
    let trunk_start = Instant::now();
    let trunk_seg = run_subplan(
        &runtime,
        &registry,
        &trunk,
        trunk_roots,
        &persist_sinks,
        cap_arguments,
        progress_fn,
        step_progress_fn,
        log_fn,
        None,
        None,
        0.0,
        trunk_weight,
    )
    .await?;
    let trunk_ms = trunk_start.elapsed().as_millis() as u64;
    let trunk_writers = trunk_seg.writer_results;
    // Cap URNs run in the trunk, for the trunk BodyOutcome (linear/no-ForEach case).
    let trunk_cap_urns: Vec<String> = trunk
        .nodes
        .values()
        .filter_map(|n| match &n.node_type {
            ExecutionNodeType::Cap { cap_urn, .. } => Some(cap_urn.clone()),
            _ => None,
        })
        .collect();
    for (nid, items) in trunk_seg.node_data {
        node_data.insert(nid, items);
    }
    for (nid, seq) in &trunk_seg.node_is_sequence {
        node_seq.insert(nid.clone(), *seq);
    }
    // Record a trunk BodyOutcome ONLY for the linear/no-ForEach case, where the trunk
    // is the whole pipeline (one outcome, like a linear run). When ForEach regions
    // exist, `body_outcomes` must be the per-item bodies only — the trunk caps surface
    // as their own run-graph nodes, so a trunk outcome would render as a phantom extra
    // media item (an empty "Item 1") in the ForEach group.
    if regions.is_empty() {
        let trunk_saved: Vec<String> =
            trunk_writers.values().flatten().flat_map(|w| w.saved_paths.clone()).collect();
        let trunk_bytes: usize = trunk_writers.values().flatten().map(|w| w.total_bytes).sum();
        body_outcomes.push(BodyOutcome {
            body_index: 0,
            success: true,
            cap_urns: trunk_cap_urns,
            failed_cap: None,
            error: None,
            failed_arg_urn: None,
            title: None,
            saved_paths: trunk_saved,
            total_bytes: trunk_bytes,
            duration_ms: trunk_ms,
            item_preview_text: None,
            item_byte_count: 0,
        });
    }
    for (sink, ws) in trunk_writers {
        node_writers.entry(sink).or_default().extend(ws);
    }

    // ── ForEach regions ──
    let post_weight = if post_ids.is_empty() { 0.0 } else { 0.2 };
    let region_band = 1.0 - trunk_weight - post_weight;
    let region_slice = if regions.is_empty() { 0.0 } else { region_band / regions.len() as f32 };
    for (ri, region) in regions.iter().enumerate() {
        let input_items = node_data.get(&region.input_node).cloned().ok_or_else(|| {
            ExecutionError::HostError(format!(
                "ForEach region '{}' input '{}' produced no data (have: {:?})",
                region.fe_id,
                region.input_node,
                node_data.keys().collect::<Vec<_>>()
            ))
        })?;
        let body_plan = build_body_subplan(plan, region);
        let base = trunk_weight + region_slice * ri as f32;

        let per_item = run_region_bodies(
            &runtime,
            &registry,
            &body_plan,
            &persist_sinks,
            region,
            &input_items,
            cap_arguments,
            progress_fn,
            step_progress_fn,
            log_fn,
            item_fn,
            body_outcome_fn,
            base,
            region_slice,
            &mut body_outcomes,
        )
        .await?;

        // Accumulate each body node's per-item output into a sequence.
        for nid in &region.body_nodes {
            let mut seq: Vec<Vec<u8>> = Vec::new();
            for (item_map, _) in &per_item {
                if let Some(items) = item_map.get(nid) {
                    seq.extend(items.iter().cloned());
                }
            }
            node_data.insert(nid.clone(), seq);
            // A region node's accumulated output is structurally a sequence.
            node_seq.insert(nid.clone(), true);
        }
        // Region writer results, keyed by sink node.
        for (_, writers_by_sink) in &per_item {
            for (sink, ws) in writers_by_sink {
                node_writers.entry(sink.clone()).or_default().extend(ws.clone());
            }
        }
    }

    // ── Post-region segment: the fold consumer(s) and everything after them.
    // Their external producers (region body nodes, pre-trunk caps, input slots)
    // are materialized from the accumulated node_data as segment roots — a
    // region node's accumulated per-item output enters as ONE sequence. ──
    if !post_ids.is_empty() {
        let post_plan = build_post_subplan(plan, &post_ids);
        let mut post_roots: HashMap<String, (Vec<u8>, bool)> = HashMap::new();
        for edge in &plan.edges {
            if !post_ids.contains(&edge.to_node) || post_ids.contains(&edge.from_node) {
                continue;
            }
            if post_roots.contains_key(&edge.from_node) {
                continue;
            }
            let items = node_data.get(&edge.from_node).ok_or_else(|| {
                ExecutionError::HostError(format!(
                    "post-region segment needs '{}' but it produced no data",
                    edge.from_node
                ))
            })?;
            let is_seq = *node_seq.get(&edge.from_node).ok_or_else(|| {
                ExecutionError::HostError(format!(
                    "post-region segment root '{}' has no sequence flag — a producer \
                     completed without recording its cardinality",
                    edge.from_node
                ))
            })?;
            let bytes = if is_seq {
                crate::orchestrator::cbor_util::wrap_raw_items_as_cbor_sequence(items).map_err(
                    |e| {
                        ExecutionError::HostError(format!(
                            "materialise post-region root '{}': {e}",
                            edge.from_node
                        ))
                    },
                )?
            } else {
                items.first().cloned().ok_or_else(|| {
                    ExecutionError::HostError(format!(
                        "post-region root '{}' holds no items",
                        edge.from_node
                    ))
                })?
            };
            post_roots.insert(edge.from_node.clone(), (bytes, is_seq));
        }

        let post_seg = run_subplan(
            &runtime,
            &registry,
            &post_plan,
            post_roots,
            &persist_sinks,
            cap_arguments,
            progress_fn,
            step_progress_fn,
            log_fn,
            None,
            None,
            1.0 - post_weight,
            post_weight,
        )
        .await?;
        for (nid, items) in post_seg.node_data {
            node_data.insert(nid, items);
        }
        for (nid, seq) in &post_seg.node_is_sequence {
            node_seq.insert(nid.clone(), *seq);
        }
        for (sink, ws) in post_seg.writer_results {
            node_writers.entry(sink).or_default().extend(ws);
        }
    }

    // ── Assemble terminals ──
    let mut terminals = Vec::with_capacity(outputs.len());
    for (out_id, src) in &outputs {
        let in_region = region_nodes.contains(src);
        let media_urn = terminal_media(plan, &registry, src).await?;
        let writers = node_writers.remove(src).unwrap_or_default();
        if !writers.is_empty() {
            let is_sequence = in_region || writers.iter().any(|w| w.is_sequence);
            terminals.push(TerminalOutput {
                output_node_id: out_id.clone(),
                items: Vec::new(),
                is_sequence,
                media_urn,
                writer_results: writers,
            });
            continue;
        }
        let data = node_data.get(src).cloned().ok_or_else(|| {
            ExecutionError::HostError(format!(
                "terminal '{out_id}' source '{src}' produced no data"
            ))
        })?;
        let is_sequence = if in_region {
            true
        } else {
            producer_is_sequence(plan, &registry, src).await
        };
        let items: Vec<OutputItem> = data
            .into_iter()
            .enumerate()
            .map(|(i, d)| OutputItem { data: d, index: i })
            .collect();
        if let Some(ifn) = item_fn {
            let n = items.len();
            for it in &items {
                ifn(it, n);
            }
        }
        terminals.push(TerminalOutput {
            output_node_id: out_id.clone(),
            items,
            is_sequence,
            media_urn,
            writer_results: Vec::new(),
        });
    }

    if let Some(pfn) = progress_fn {
        pfn(1.0, "", "Completed");
    }
    Ok(PipelineResult { terminals, body_outcomes })
}

// =============================================================================
// Region body execution
// =============================================================================

/// Run a ForEach region's body once per input item, concurrently, collecting each
/// item's full body `node_data` (so every body node — in-body fan-out included — is
/// captured) and honoring the runtime's partial-failure policy.
#[allow(clippy::too_many_arguments)]
async fn run_region_bodies(
    runtime: &Arc<dyn EngineRuntime>,
    registry: &Arc<FabricRegistry>,
    body_subplan: &MachinePlan,
    persist_sinks: &HashSet<String>,
    region: &Region,
    input_items: &[Vec<u8>],
    cap_arguments: &HashMap<String, Vec<(String, Vec<u8>)>>,
    progress_fn: Option<&CapProgressFn>,
    step_progress_fn: Option<&CapStepProgressFn>,
    log_fn: Option<&PipelineLogFn>,
    item_fn: Option<&PipelineItemFn>,
    body_outcome_fn: Option<&BodyOutcomeFn>,
    progress_base: f32,
    progress_weight: f32,
    body_outcomes: &mut Vec<BodyOutcome>,
) -> Result<Vec<(HashMap<String, Vec<Vec<u8>>>, HashMap<String, Vec<WriterResult>>)>, ExecutionError>
{
    let item_count = input_items.len();
    let fe_token_id = region.fe_id.clone();

    let body_progress_slots: Arc<Vec<AtomicU32>> =
        Arc::new((0..item_count).map(|_| AtomicU32::new(0f32.to_bits())).collect());
    let stall_tracker = Arc::new(PipelineProgressTracker::new());
    let mut stall_warning_logged = false;

    type BodyOk = (
        usize,
        HashMap<String, Vec<Vec<u8>>>,
        HashMap<String, Vec<WriterResult>>,
        u64,
    );
    type BodyErr = (usize, ExecutionError, u64);

    let (done_tx, mut done_rx) =
        tokio::sync::mpsc::unbounded_channel::<Result<BodyOk, BodyErr>>();

    for (i, raw_item_bytes) in input_items.iter().enumerate() {
        let runtime = runtime.clone();
        let registry = registry.clone();
        let body_subplan = body_subplan.clone();
        let persist_sinks = persist_sinks.clone();
        let body_input_id = region.body_input_id.clone();
        let cap_arguments = cap_arguments.clone();
        let raw_item_bytes = raw_item_bytes.clone();
        let body_log_fn = log_fn.cloned();
        let body_stall_tracker = stall_tracker.clone();
        let done_tx = done_tx.clone();

        let item_pfn: Option<CapProgressFn> = progress_fn.map(|parent| {
            let parent = parent.clone();
            let slots = body_progress_slots.clone();
            let tracker = stall_tracker.clone();
            let n = item_count as f32;
            let step_sink = step_progress_fn.cloned();
            let fe_token_id = fe_token_id.clone();
            Arc::new(move |p: f32, cap_urn: &str, msg: &str| {
                slots[i].store(p.to_bits(), Ordering::Relaxed);
                tracker.touch();
                let sum: f32 =
                    slots.iter().map(|s| f32::from_bits(s.load(Ordering::Relaxed))).sum();
                let step = if n > 0.0 { sum / n } else { 0.0 };
                if let Some(sink) = &step_sink {
                    sink(step.clamp(0.0, 1.0), cap_urn, &fe_token_id);
                }
                parent(progress_base + progress_weight * step, cap_urn, msg);
            }) as CapProgressFn
        });

        tokio::spawn(async move {
            let started = Instant::now();
            let mut roots: HashMap<String, (Vec<u8>, bool)> = HashMap::new();
            roots.insert(body_input_id, (raw_item_bytes, false));
            // The item's local progress is [0,1]; item_pfn aggregates it across items.
            let res = run_subplan(
                &runtime,
                &registry,
                &body_subplan,
                roots,
                &persist_sinks,
                &cap_arguments,
                item_pfn.as_ref(),
                None,
                body_log_fn.as_ref(),
                Some(i),
                Some(body_stall_tracker),
                0.0,
                1.0,
            )
            .await;
            let ms = started.elapsed().as_millis() as u64;
            let _ = match res {
                Ok(seg) => done_tx.send(Ok((i, seg.node_data, seg.writer_results, ms))),
                Err(e) => done_tx.send(Err((i, e, ms))),
            };
        });
    }
    drop(done_tx);

    let mut item_results: Vec<
        Option<(HashMap<String, Vec<Vec<u8>>>, HashMap<String, Vec<WriterResult>>)>,
    > = (0..item_count).map(|_| None).collect();
    let mut completed = 0usize;
    let mut succeeded = 0usize;
    let mut failed = 0usize;

    while completed < item_count {
        tokio::select! {
            biased;
            recv = done_rx.recv() => {
                let Some(result) = recv else { break };
                match result {
                    Ok((i, node_data, writers, ms)) => {
                        stall_tracker.touch();
                        stall_warning_logged = false;
                        let item_bytes: usize = writers
                            .values()
                            .flatten()
                            .map(|w| w.total_bytes)
                            .sum::<usize>()
                            .max(node_data.values().flatten().map(|d| d.len()).sum());
                        let saved_paths: Vec<String> =
                            writers.values().flatten().flat_map(|w| w.saved_paths.clone()).collect();
                        body_outcomes.push(BodyOutcome {
                            body_index: i,
                            success: true,
                            cap_urns: region.body_cap_urns.clone(),
                            failed_cap: None,
                            error: None,
                            failed_arg_urn: None,
                            title: None,
                            saved_paths,
                            total_bytes: item_bytes,
                            duration_ms: ms,
                            item_preview_text: input_items.get(i).and_then(|b| item_preview_snippet(b)),
                            item_byte_count: input_items.get(i).map(|b| b.len() as u64).unwrap_or(0),
                        });
                        if let Some(bofn) = body_outcome_fn { bofn(body_outcomes); }
                        if let Some(ifn) = item_fn {
                            // Surface the region's body-entry per-item output as it lands.
                            if let Some(items) = node_data.get(&region.body_entry) {
                                for d in items {
                                    ifn(&OutputItem { data: d.clone(), index: i }, item_count);
                                }
                            }
                        }
                        item_results[i] = Some((node_data, writers));
                        completed += 1;
                        succeeded += 1;
                    }
                    Err((i, e, ms)) => {
                        stall_tracker.touch();
                        stall_warning_logged = false;
                        let error_str = format!("{e}");
                        let failed_cap = region
                            .body_cap_urns
                            .iter()
                            .find(|u| error_str.contains(u.as_str()))
                            .cloned();
                        body_outcomes.push(BodyOutcome {
                            body_index: i,
                            success: false,
                            cap_urns: region.body_cap_urns.clone(),
                            failed_cap,
                            error: Some(error_str),
                            failed_arg_urn: e.failure_arg_urn().map(str::to_string),
                            title: None,
                            saved_paths: vec![],
                            total_bytes: 0,
                            duration_ms: ms,
                            item_preview_text: input_items.get(i).and_then(|b| item_preview_snippet(b)),
                            item_byte_count: input_items.get(i).map(|b| b.len() as u64).unwrap_or(0),
                        });
                        if let Some(bofn) = body_outcome_fn { bofn(body_outcomes); }
                        if let Some(lfn) = log_fn {
                            lfn("", "error", &format!("ForEach body {i} failed: {e}"), None, Some(i));
                        }
                        tracing::error!("[execute_plan] region '{}' body {i} failed: {e}", region.fe_id);
                        completed += 1;
                        failed += 1;
                    }
                }
                if let Some(pfn) = progress_fn {
                    let sum: f32 =
                        body_progress_slots.iter().map(|s| f32::from_bits(s.load(Ordering::Relaxed))).sum();
                    let step = if item_count > 0 { sum / item_count as f32 } else { 1.0 };
                    pfn(progress_base + progress_weight * step, "", &format!("Completed {completed}/{item_count}"));
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                if stall_tracker.is_stalled() && !stall_warning_logged {
                    if let Some(lfn) = log_fn {
                        lfn("", "warn", &format!("ForEach region '{}' has had no progress for {PIPELINE_STALL_TIMEOUT_SECS}s — continuing to wait. Use Cancel to abort.", region.fe_id), None, None);
                    }
                    stall_warning_logged = true;
                }
            }
        }
    }

    if failed > 0 {
        let policy = runtime.foreach_partial_failure_policy().await;
        let should_fail = match policy.as_str() {
            "fail" => true,
            _ => succeeded == 0,
        };
        if should_fail {
            return Err(ExecutionError::HostError(format!(
                "ForEach region '{}' failed: {failed}/{item_count} bodies failed (policy={policy})",
                region.fe_id
            )));
        }
    }

    // Ordered by item index; failed items (tolerated by policy) contribute nothing.
    Ok(item_results.into_iter().flatten().collect())
}

// =============================================================================
// Helpers
// =============================================================================

async fn to_graph(
    plan: &MachinePlan,
    registry: &FabricRegistry,
) -> Result<ResolvedGraph, ExecutionError> {
    plan_to_resolved_graph(plan, registry)
        .await
        .map_err(|e: ParseOrchestrationError| {
            ExecutionError::HostError(format!("plan → resolved graph: {e}"))
        })
}

/// The media URN a producer node emits: a cap's output spec, or an input slot's media.
async fn terminal_media(
    plan: &MachinePlan,
    registry: &FabricRegistry,
    source_node: &str,
) -> Result<String, ExecutionError> {
    let node = plan.nodes.get(source_node).ok_or_else(|| {
        ExecutionError::HostError(format!("terminal source '{source_node}' not in plan"))
    })?;
    match &node.node_type {
        ExecutionNodeType::Cap { cap_urn, .. } => {
            let cap = registry.get_cap(cap_urn).await.map_err(|e| {
                ExecutionError::HostError(format!("resolve cap '{cap_urn}' for terminal media: {e}"))
            })?;
            Ok(cap.urn.out_spec().to_string())
        }
        ExecutionNodeType::InputSlot { expected_media_urn, .. } => Ok(expected_media_urn.clone()),
        other => Err(ExecutionError::HostError(format!(
            "terminal source '{source_node}' is not a data producer: {other:?}"
        ))),
    }
}

/// Whether a (trunk) producer emits a sequence: a cap by its output cardinality, an
/// input slot by its declared cardinality.
async fn producer_is_sequence(plan: &MachinePlan, registry: &FabricRegistry, node: &str) -> bool {
    match plan.nodes.get(node).map(|n| &n.node_type) {
        Some(ExecutionNodeType::Cap { cap_urn, .. }) => registry
            .get_cached_cap(cap_urn)
            .and_then(|c| c.output.as_ref().map(|o| o.is_sequence))
            .unwrap_or(false),
        Some(ExecutionNodeType::InputSlot { cardinality, .. }) => {
            matches!(cardinality, InputCardinality::Sequence)
        }
        _ => false,
    }
}

/// A bounded UTF-8 preview of an item's bytes, or `None` if binary/empty.
fn item_preview_snippet(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    let mut end = bytes.len().min(ITEM_PREVIEW_CAP);
    for _ in 0..3 {
        match std::str::from_utf8(&bytes[..end]) {
            Ok(text) => return Some(text.to_string()),
            Err(e) if e.valid_up_to() > 0 && end == bytes.len().min(ITEM_PREVIEW_CAP) => {
                end = e.valid_up_to();
            }
            Err(_) => return None,
        }
    }
    match std::str::from_utf8(&bytes[..end]) {
        Ok(text) if !text.is_empty() => Some(text.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::MachineNode;

    /// Structural fixture: input → mapper (inside a ForEach region) → fold →
    /// sink, plus an independent pre-trunk cap off the same input.
    fn plan_with_region_and_fold() -> (MachinePlan, HashSet<String>) {
        let mut plan = MachinePlan::new("region+fold");
        plan.add_node(MachineNode::input_slot(
            "input",
            "input",
            "media:ext=pdf",
            crate::planner::InputCardinality::Single,
        ));
        plan.add_node(MachineNode::cap("render", "cap:in=\"media:ext=pdf\";render;out=\"media:ext=png;image\""));
        plan.add_node(MachineNode::cap("mapper", "cap:in=\"media:ext=png;image\";upscale;out=\"media:ext=png;image;up\""));
        plan.add_node(MachineNode::for_each("fe", "render", "mapper", "mapper"));
        plan.add_node(MachineNode::cap("fold", "cap:in=\"media:ext=png;image\";animate;out=\"media:ext=gif;image\""));
        plan.add_node(MachineNode::cap("meta", "cap:in=\"media:ext=pdf\";meta;out=\"media:enc=utf-8;record\""));
        plan.add_node(MachineNode::output("out_gif", "result", "fold"));
        plan.add_node(MachineNode::output("out_meta", "result", "meta"));

        plan.add_edge(MachinePlanEdge::direct("input", "render"));
        plan.add_edge(MachinePlanEdge::direct("render", "fe"));
        plan.add_edge(MachinePlanEdge::iteration("fe", "mapper"));
        plan.add_edge(MachinePlanEdge::direct("mapper", "fold"));
        plan.add_edge(MachinePlanEdge::direct("input", "meta"));
        plan.add_edge(MachinePlanEdge::direct("fold", "out_gif"));
        plan.add_edge(MachinePlanEdge::direct("meta", "out_meta"));

        let region_nodes: HashSet<String> = ["mapper".to_string()].into_iter().collect();
        (plan, region_nodes)
    }

    // TEST1434: the post-region partition is exactly the transitive consumers
    // of region output — the fold, NOT the independent pre-trunk cap. The
    // pre-trunk subplan excludes both region and post caps; the post subplan
    // carries the fold with its external-producer edge intact.
    #[test]
    fn test1434_post_region_partition() {
        let (plan, region_nodes) = plan_with_region_and_fold();

        let post = compute_post_region_caps(&plan, &region_nodes);
        assert!(post.contains("fold"), "the fold consumes region output → post");
        assert!(!post.contains("meta"), "an independent cap stays pre-trunk");
        assert!(!post.contains("mapper"), "region caps are never post");
        assert_eq!(post.len(), 1);

        let mut excluded = region_nodes.clone();
        excluded.extend(post.iter().cloned());
        let trunk = build_trunk_subplan(&plan, &excluded);
        assert!(trunk.nodes.contains_key("render"));
        assert!(trunk.nodes.contains_key("meta"));
        assert!(trunk.nodes.contains_key("input"));
        assert!(!trunk.nodes.contains_key("mapper"));
        assert!(!trunk.nodes.contains_key("fold"));

        let post_plan = build_post_subplan(&plan, &post);
        assert!(post_plan.nodes.contains_key("fold"));
        assert_eq!(post_plan.nodes.len(), 1);
        // The external-producer edge (mapper → fold) survives so the segment
        // can seed 'mapper' as a materialised root.
        assert!(
            post_plan
                .edges
                .iter()
                .any(|e| e.from_node == "mapper" && e.to_node == "fold"),
            "the post subplan keeps the edge from its external root"
        );
    }
}
