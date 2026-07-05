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
//! Walking the strand in data-flow (dependency) order, it emits one `Cap` step per
//! edge, instantiating the runtime media type through each cap's MAIN input
//! (`apply_to_runtime_input_media`), and inserts a `ForEach` step before any cap the
//! resolver already marked `is_loop`.
//!
//! A cap edge's resolver `assignment` binds each wiring source to one of the cap's
//! arguments by media URN. Exactly one of those is the cap's **stdin** (main) input —
//! it threads the runtime media of the chain and is the step's `from_spec`. Every
//! OTHER binding is a **convergence** input: another cap's output routed into a
//! non-main argument, recorded on the step as a [`SecondaryArg`]. This is what lets a
//! strand express a DAG (a cap with more than one incoming producer), not just a
//! linear chain — the executable model the engine and reference path share.
//!
//! `is_loop` is the single source of truth for cardinality: `resolve.rs` derives it
//! from `Cap::needs_foreach` (a sequence source feeding a scalar-input cap); this
//! converter reads `edge.is_loop`, never recomputing it.
//!
//! ## Invariants (enforced, no fallbacks)
//!
//! - **Exactly one stdin (main) input per cap.** The cap definition declares one
//!   `Stdin` argument; the resolver's assignment binds a source to it. A cap with no
//!   stdin arg, or an edge with no binding to it, is a hard error.
//! - **Convergence wires only cap outputs.** A non-main argument fed by wiring must be
//!   another cap's output. A raw input feeding a non-main arg is an argument VALUE
//!   (default / setting / config / user input), delivered through the value channel,
//!   never wired — a wiring source that is not a producer is a hard error.
//! - **Connected data-flow graph per strand.** Every edge's sources must become
//!   available (input anchors, or already-emitted producers); an unreachable edge is a
//!   hard error.

use std::collections::HashMap;

use crate::cap::registry::FabricRegistry;
use crate::machine::graph::{MachineStrand, NodeId};
use crate::machine::MachineAbstractionError;
use crate::planner::{ArgSourceRef, CapInput, Strand, StrandStep, StrandStepType};
use crate::urn::media_urn::MediaUrn;

/// Realize a single resolved `MachineStrand` into a `Strand`, instantiating runtime
/// media from `source_urn` (the concrete media flowing into the strand's input
/// anchors).
///
/// `strand_index` is used only for diagnostics.
pub fn realize_strand(
    machine_strand: &MachineStrand,
    registry: &FabricRegistry,
    source_urn: &MediaUrn,
    strand_index: usize,
) -> Result<Strand, MachineAbstractionError> {
    // Per-node runtime media. A convergence strand fans out from its input and
    // converges at a multi-input cap, so each node carries its own runtime media —
    // there is no single linear thread. Input anchors carry the concrete input media
    // (`source_urn`); each emitted cap sets its target node's media.
    let mut node_media: HashMap<NodeId, MediaUrn> = HashMap::new();
    for &anchor in machine_strand.input_anchor_ids() {
        node_media.insert(anchor, source_urn.clone());
    }
    // The step (by stable `token_id`) that produced each node, for wiring convergence
    // args. Input anchors have no producing step.
    let mut node_producer: HashMap<NodeId, String> = HashMap::new();

    let edges = machine_strand.edges();
    let mut emitted = vec![false; edges.len()];
    let mut steps: Vec<StrandStep> = Vec::with_capacity(edges.len() * 2);

    // Emit edges in dependency order: an edge is emittable once every one of its
    // wiring sources has a known runtime media (its producer has been emitted, or it
    // is an input anchor). Fan-in is permitted — the emittability test is over ALL
    // sources, not a single one.
    for _ in 0..edges.len() {
        let next = edges
            .iter()
            .enumerate()
            .find(|(i, e)| {
                !emitted[*i]
                    && e.assignment
                        .iter()
                        .all(|b| node_media.contains_key(&b.source))
            })
            .map(|(i, _)| i);
        let Some(i) = next else {
            return Err(MachineAbstractionError::DisconnectedStrand { strand_index });
        };
        emitted[i] = true;
        let edge = &edges[i];

        let cap = registry
            .get_cached_cap(&edge.cap_urn.to_string())
            .ok_or_else(|| MachineAbstractionError::UnknownCap {
                cap_urn: edge.cap_urn.to_string(),
            })?;
        let (input_is_sequence, output_is_sequence) = cap.sequence_shape();

        // The cap's MAIN input is the argument whose `Stdin` source URN is the cap's
        // `in=` (the one special input tag — a cap has exactly one `in`). Its slot
        // media URN selects the primary binding in the resolver's assignment. Every
        // other stdin-declaring arg is a convergence input. Compared by tagged-URN
        // equivalence, never as strings; never by arg position.
        let in_spec_urn = MediaUrn::from_string(edge.cap_urn.in_spec()).map_err(|e| {
            MachineAbstractionError::RuntimeMediaInference {
                strand_index,
                cap_urn: edge.cap_urn.to_string(),
                runtime_input: edge.cap_urn.in_spec().to_string(),
                reason: format!("cap `in=` is not a valid media URN: {e}"),
            }
        })?;
        let stdin_arg_str = cap
            .args
            .iter()
            .find(|a| a.is_main_input(&in_spec_urn))
            .map(|a| a.media_urn.clone())
            .ok_or_else(|| MachineAbstractionError::CapDoesNotDeclareInput {
                strand_index,
                cap_urn: edge.cap_urn.to_string(),
            })?;
        let stdin_arg_urn = MediaUrn::from_string(&stdin_arg_str).map_err(|e| {
            MachineAbstractionError::RuntimeMediaInference {
                strand_index,
                cap_urn: edge.cap_urn.to_string(),
                runtime_input: stdin_arg_str.clone(),
                reason: format!("stdin arg URN is not a valid media URN: {e}"),
            }
        })?;

        let primary = edge
            .assignment
            .iter()
            .find(|b| {
                b.cap_arg_media_urn
                    .is_equivalent(&stdin_arg_urn)
                    .unwrap_or(false)
            })
            .ok_or_else(|| MachineAbstractionError::NoStdinBinding {
                strand_index,
                cap_urn: edge.cap_urn.to_string(),
                stdin_arg: stdin_arg_str.clone(),
            })?;

        let primary_media = node_media
            .get(&primary.source)
            .expect("primary source media present: the edge was chosen emittable")
            .clone();

        // ForEach synthesis — read the resolver's cardinality decision (`is_loop`); the
        // media URN is unchanged (a shape transition, not a type transition).
        if edge.is_loop {
            steps.push(StrandStep::new(
                StrandStepType::ForEach {
                    media_def: primary_media.clone(),
                },
                primary_media.clone(),
                primary_media.clone(),
            ));
        }

        let runtime_out = edge
            .cap_urn
            .apply_to_runtime_input_media(&primary_media)
            .map_err(|err| MachineAbstractionError::RuntimeMediaInference {
                strand_index,
                cap_urn: edge.cap_urn.to_string(),
                runtime_input: primary_media.to_string(),
                reason: err.to_string(),
            })?;

        // Build the full explicit input list. Each binding names its producer: a
        // produced node → the producing step; an input anchor → the strand input.
        // Only the PRIMARY (stdin) input may be fed by an input anchor; a non-main
        // arg fed by a non-producer is an argument VALUE, not a wiring, and is exposed
        // hard (see module invariants).
        let mut inputs: Vec<CapInput> = Vec::with_capacity(edge.assignment.len());
        for b in &edge.assignment {
            let is_primary = b
                .cap_arg_media_urn
                .is_equivalent(&stdin_arg_urn)
                .unwrap_or(false);
            let source = match node_producer.get(&b.source) {
                Some(tok) => ArgSourceRef::Step {
                    token_id: tok.clone(),
                },
                None if is_primary => ArgSourceRef::StrandInput,
                None => {
                    return Err(MachineAbstractionError::NonProducerSecondaryArg {
                        strand_index,
                        cap_urn: edge.cap_urn.to_string(),
                        arg_urn: b.cap_arg_media_urn.to_string(),
                    })
                }
            };
            inputs.push(CapInput {
                arg_urn: b.cap_arg_media_urn.clone(),
                source,
            });
        }

        let mut step = StrandStep::new(
            StrandStepType::Cap {
                cap_urn: edge.cap_urn.clone(),
                title: cap.title.clone(),
                specificity: edge.cap_urn.specificity(),
                input_is_sequence,
                output_is_sequence,
                inputs,
            },
            primary_media,
            runtime_out.clone(),
        );
        // Preserve the resolved edge's stable identity so live updates map back and so
        // convergence args can reference this step as their producer.
        step.token_id = edge.token_id.clone();
        node_media.insert(edge.target, runtime_out);
        node_producer.insert(edge.target, edge.token_id.clone());
        steps.push(step);
    }

    // The strand's realized target media is its output anchor's runtime media. A
    // well-formed strand has exactly one output anchor, produced by a cap above; a
    // missing anchor or media is a structural bug, exposed hard.
    let &output_anchor = machine_strand.output_anchor_ids().first().ok_or(
        MachineAbstractionError::DisconnectedStrand { strand_index },
    )?;
    let target_media_urn = node_media
        .get(&output_anchor)
        .ok_or(MachineAbstractionError::DisconnectedStrand { strand_index })?
        .clone();

    let cap_step_count = steps.iter().filter(|s| s.is_cap()).count() as i32;
    let total_steps = steps.len() as i32;
    Ok(Strand {
        steps,
        source_media_urn: source_urn.clone(),
        target_media_urn,
        total_steps,
        cap_step_count,
        description: format!("realized machine strand {strand_index}"),
    })
}
