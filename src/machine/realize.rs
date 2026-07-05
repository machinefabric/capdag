//! Realize a resolved `MachineStrand` into an executable `Strand` of steps.
//!
//! This is the single, shared conversion from a resolved notation strand into the
//! `Strand` the canonical plan builder (`MachinePlanBuilder::build_plan_from_path`)
//! consumes. It is the inverse of [`crate::planner::Strand::knit`] and the logic the
//! engine's editor-run realization and the reference/CLI path both use — one
//! implementation, no duplication.
//!
//! ## What it does
//!
//! Walking the strand in data-flow order from its input anchor, it emits one `Cap`
//! step per edge, instantiating the runtime media type through each cap
//! (`apply_to_runtime_input_media`), and inserts a `ForEach` step before any cap the
//! resolver already marked `is_loop`.
//!
//! `is_loop` is the single source of truth for cardinality: `resolve.rs` derives it
//! from `Cap::needs_foreach` (a sequence source feeding a scalar-input cap). Entering
//! edge *i*, the incoming cardinality equals the previous cap's `output_is_sequence`
//! — which is exactly what `is_loop` already encodes — so this converter never
//! recomputes cardinality; it reads `edge.is_loop`.
//!
//! ## Invariants (enforced, no fallbacks)
//!
//! - **One stdin per cap.** An edge with more than one incoming data source is a hard
//!   error — there is no second stdin.
//! - **Single connected chain per strand.** Strands are connected components; an edge
//!   not reachable from the input anchor is a hard error.

use std::collections::HashSet;

use crate::cap::registry::FabricRegistry;
use crate::machine::graph::{MachineStrand, NodeId};
use crate::machine::MachineAbstractionError;
use crate::planner::{Strand, StrandStep, StrandStepType};
use crate::urn::media_urn::MediaUrn;

/// Realize a single resolved `MachineStrand` into a `Strand`, instantiating runtime
/// media from `source_urn` (the concrete media flowing into the strand's input).
///
/// `strand_index` is used only for diagnostics.
pub fn realize_strand(
    machine_strand: &MachineStrand,
    registry: &FabricRegistry,
    source_urn: &MediaUrn,
    strand_index: usize,
) -> Result<Strand, MachineAbstractionError> {
    // Order edges into data-flow order: an edge is emittable once its single data
    // source is available (a strand input anchor, or an already-emitted edge's
    // target). One producer per node makes this a strict linear order.
    struct ChainEdge<'a> {
        cap_urn: &'a crate::urn::cap_urn::CapUrn,
        token_id: &'a str,
        source: NodeId,
        target: NodeId,
        is_loop: bool,
    }
    let mut pending: Vec<ChainEdge> = Vec::with_capacity(machine_strand.edges().len());
    for edge in machine_strand.edges() {
        if edge.assignment.len() != 1 {
            return Err(MachineAbstractionError::MultipleDataSources {
                strand_index,
                cap_urn: edge.cap_urn.to_string(),
                source_count: edge.assignment.len(),
            });
        }
        pending.push(ChainEdge {
            cap_urn: &edge.cap_urn,
            token_id: &edge.token_id,
            source: edge.assignment[0].source,
            target: edge.target,
            is_loop: edge.is_loop,
        });
    }

    let mut available: HashSet<NodeId> =
        machine_strand.input_anchor_ids().iter().copied().collect();
    let mut emitted: HashSet<usize> = HashSet::new();
    let mut order: Vec<usize> = Vec::with_capacity(pending.len());
    while order.len() < pending.len() {
        let next = pending
            .iter()
            .enumerate()
            .find(|(i, e)| !emitted.contains(i) && available.contains(&e.source))
            .map(|(i, _)| i);
        match next {
            Some(i) => {
                available.insert(pending[i].target);
                emitted.insert(i);
                order.push(i);
            }
            None => return Err(MachineAbstractionError::DisconnectedStrand { strand_index }),
        }
    }

    let mut steps: Vec<StrandStep> = Vec::with_capacity(order.len() * 2);
    let mut current_runtime = source_urn.clone();
    for &i in &order {
        let e = &pending[i];
        let cap = registry
            .get_cached_cap(&e.cap_urn.to_string())
            .ok_or_else(|| MachineAbstractionError::UnknownCap {
                cap_urn: e.cap_urn.to_string(),
            })?;
        let (input_is_sequence, output_is_sequence) = cap.sequence_shape();

        // ForEach synthesis — read the cardinality decision the resolver already made
        // (`is_loop`); do not recompute it. Media URN is unchanged (shape transition).
        if e.is_loop {
            steps.push(StrandStep::new(
                StrandStepType::ForEach {
                    media_def: current_runtime.clone(),
                },
                current_runtime.clone(),
                current_runtime.clone(),
            ));
        }

        let runtime_out = e.cap_urn.apply_to_runtime_input_media(&current_runtime).map_err(
            |err| MachineAbstractionError::RuntimeMediaInference {
                strand_index,
                cap_urn: e.cap_urn.to_string(),
                runtime_input: current_runtime.to_string(),
                reason: err.to_string(),
            },
        )?;

        let mut step = StrandStep::new(
            StrandStepType::Cap {
                cap_urn: e.cap_urn.clone(),
                title: cap.title.clone(),
                specificity: e.cap_urn.specificity(),
                input_is_sequence,
                output_is_sequence,
            },
            current_runtime.clone(),
            runtime_out.clone(),
        );
        // Preserve the resolved edge's stable identity so live updates map back.
        step.token_id = e.token_id.to_string();
        steps.push(step);

        current_runtime = runtime_out;
    }

    let cap_step_count = steps.iter().filter(|s| s.is_cap()).count() as i32;
    let total_steps = steps.len() as i32;
    Ok(Strand {
        steps,
        source_media_urn: source_urn.clone(),
        target_media_urn: current_runtime,
        total_steps,
        cap_step_count,
        description: format!("realized machine strand {strand_index}"),
    })
}
