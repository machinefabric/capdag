//! Pure `MachinePlan` analysis used by plan execution.
//!
//! These functions derive output nodes, output/producing media URNs, ForEach item
//! media, and the Collect paired with a ForEach — all from the plan structure alone,
//! with no engine dependency. They live in capdag's planner so the reference executor
//! and the engine share one implementation (DRY); they were previously duplicated in
//! the engine's `capdag_service`.

use std::collections::{BTreeMap, HashMap};

use crate::urn::cap_urn::CapUrn;
use crate::urn::media_urn::MediaUrn;

use super::plan::{EdgeType, ExecutionNodeType, MachinePlan, MachinePlanEdge};
use super::{PlannerError, PlannerResult};

/// Resolve a plan's terminal output node and its data from executed node data.
///
/// Tries each declared output node directly, then through an `Output` meta-node to its
/// source. Fails hard if no output data is present — the executor must not silently
/// return empty.
pub fn resolve_plan_output(
    plan: &MachinePlan,
    node_data: &HashMap<String, Vec<Vec<u8>>>,
) -> PlannerResult<(String, Vec<Vec<u8>>)> {
    for output_id in &plan.output_nodes {
        if let Some(items) = node_data.get(output_id) {
            return Ok((output_id.clone(), items.clone()));
        }
        if let Some(node) = plan.get_node(output_id) {
            if let ExecutionNodeType::Output { source_node, .. } = &node.node_type {
                if let Some(items) = node_data.get(source_node) {
                    return Ok((output_id.clone(), items.clone()));
                }
            }
        }
    }

    Err(PlannerError::Internal(format!(
        "No output data found. output_nodes={:?}, available={:?}",
        plan.output_nodes,
        node_data.keys().collect::<Vec<_>>()
    )))
}

/// Derive the runtime output media URN for a plan's output node by walking back to the
/// cap that produces it and applying each cap to its runtime input.
pub fn derive_output_media_urn(plan: &MachinePlan, output_node_id: &str) -> PlannerResult<String> {
    Ok(resolve_node_output_media(plan, output_node_id, &mut BTreeMap::new())?.to_string())
}

fn select_cap_runtime_input_edge<'a>(
    node_id: &str,
    incoming_edges: &'a [&'a MachinePlanEdge],
) -> PlannerResult<&'a MachinePlanEdge> {
    let non_iteration_edges: Vec<_> = incoming_edges
        .iter()
        .copied()
        .filter(|edge| !matches!(edge.edge_type, EdgeType::Iteration))
        .collect();

    match non_iteration_edges.as_slice() {
        [edge] => Ok(*edge),
        [] => match incoming_edges {
            [edge] => Ok(*edge),
            [] => Err(PlannerError::Internal(format!(
                "Cap node '{}' has no incoming edge",
                node_id
            ))),
            _ => Err(PlannerError::Internal(format!(
                "Cap node '{}' has only structural incoming edges ({:?}); effective runtime media is ambiguous",
                node_id,
                incoming_edges.iter().map(|edge| &edge.edge_type).collect::<Vec<_>>()
            ))),
        },
        _ => Err(PlannerError::Internal(format!(
            "Cap node '{}' has {} non-structural incoming edges; effective runtime media is ambiguous",
            node_id,
            non_iteration_edges.len()
        ))),
    }
}

fn resolve_node_output_media(
    plan: &MachinePlan,
    node_id: &str,
    memo: &mut BTreeMap<String, MediaUrn>,
) -> PlannerResult<MediaUrn> {
    if let Some(cached) = memo.get(node_id) {
        return Ok(cached.clone());
    }

    let node = plan
        .get_node(node_id)
        .ok_or_else(|| PlannerError::Internal(format!("Plan node '{}' not found", node_id)))?;

    let resolved = match &node.node_type {
        ExecutionNodeType::Output { source_node, .. } => {
            resolve_node_output_media(plan, source_node, memo)?
        }
        ExecutionNodeType::InputSlot {
            expected_media_urn, ..
        } => MediaUrn::from_string(expected_media_urn).map_err(|e| {
            PlannerError::Internal(format!(
                "Failed to parse input slot media URN '{}': {}",
                expected_media_urn, e
            ))
        })?,
        ExecutionNodeType::Cap { cap_urn, .. } => {
            let parsed = CapUrn::from_string(cap_urn).map_err(|e| {
                PlannerError::Internal(format!("Failed to parse cap URN '{}': {}", cap_urn, e))
            })?;
            let incoming_edges: Vec<_> = plan
                .edges
                .iter()
                .filter(|edge| edge.to_node == node_id)
                .collect();
            let incoming_edge = select_cap_runtime_input_edge(node_id, &incoming_edges)?;
            let runtime_input = resolve_node_output_media(plan, &incoming_edge.from_node, memo)?;
            parsed
                .apply_to_runtime_input_media(&runtime_input)
                .map_err(|e| {
                    PlannerError::Internal(format!(
                        "Failed to apply cap '{}' to runtime input '{}': {}",
                        cap_urn, runtime_input, e
                    ))
                })?
        }
        ExecutionNodeType::ForEach { input_node, .. } => {
            resolve_node_output_media(plan, input_node, memo)?
        }
        ExecutionNodeType::Collect {
            input_nodes,
            output_media_urn,
        } => {
            if let Some(explicit_urn) = output_media_urn {
                MediaUrn::from_string(explicit_urn).map_err(|e| {
                    PlannerError::Internal(format!(
                        "Failed to parse collect output media URN '{}': {}",
                        explicit_urn, e
                    ))
                })?
            } else {
                let first_input = input_nodes.first().ok_or_else(|| {
                    PlannerError::Internal(format!("Collect node '{}' has no input nodes", node_id))
                })?;
                resolve_node_output_media(plan, first_input, memo)?
            }
        }
        ExecutionNodeType::Split { input_node, .. } => {
            resolve_node_output_media(plan, input_node, memo)?
        }
        ExecutionNodeType::Merge { input_nodes, .. } => {
            let first_input = input_nodes.first().ok_or_else(|| {
                PlannerError::Internal(format!("Merge node '{}' has no input nodes", node_id))
            })?;
            resolve_node_output_media(plan, first_input, memo)?
        }
    };

    memo.insert(node_id.to_string(), resolved.clone());
    Ok(resolved)
}

/// Walk back from an output node to the cap that produces it and return that cap's URN
/// — the cap whose realized-path edge live activity lights up. Fails hard if none.
pub fn derive_output_producing_cap_urn(
    plan: &MachinePlan,
    output_node_id: &str,
) -> PlannerResult<String> {
    fn walk(plan: &MachinePlan, node_id: &str, depth: usize) -> PlannerResult<String> {
        if depth > plan.nodes.len() + 1 {
            return Err(PlannerError::Internal(format!(
                "Cycle while resolving producing cap at node '{}'",
                node_id
            )));
        }
        let node = plan
            .get_node(node_id)
            .ok_or_else(|| PlannerError::Internal(format!("Plan node '{}' not found", node_id)))?;
        match &node.node_type {
            ExecutionNodeType::Cap { cap_urn, .. } => Ok(cap_urn.clone()),
            ExecutionNodeType::Output { source_node, .. } => walk(plan, source_node, depth + 1),
            ExecutionNodeType::ForEach { input_node, .. } => walk(plan, input_node, depth + 1),
            ExecutionNodeType::Split { input_node, .. } => walk(plan, input_node, depth + 1),
            ExecutionNodeType::Collect { input_nodes, .. }
            | ExecutionNodeType::Merge { input_nodes, .. } => {
                let first = input_nodes.first().ok_or_else(|| {
                    PlannerError::Internal(format!(
                        "Node '{}' has no input nodes while resolving producing cap",
                        node_id
                    ))
                })?;
                walk(plan, first, depth + 1)
            }
            ExecutionNodeType::InputSlot { .. } => Err(PlannerError::Internal(format!(
                "Output node '{}' resolves to an input slot with no producing cap",
                node_id
            ))),
        }
    }
    walk(plan, output_node_id, 0)
}

/// Derive the media URN for ForEach items from the input node. ForEach is a shape
/// transition (is_sequence flips), not a type transition, so the item URN equals the
/// input URN.
pub fn derive_foreach_media_urns(
    plan: &MachinePlan,
    input_node_id: &str,
) -> PlannerResult<(MediaUrn, String)> {
    let input_node = plan.get_node(input_node_id).ok_or_else(|| {
        PlannerError::Internal(format!("ForEach input node '{}' not found", input_node_id))
    })?;

    let output_media_str = match &input_node.node_type {
        ExecutionNodeType::Cap { .. } => derive_output_media_urn(plan, input_node_id)?,
        ExecutionNodeType::InputSlot {
            expected_media_urn, ..
        } => expected_media_urn.clone(),
        ExecutionNodeType::Collect {
            output_media_urn, ..
        } => output_media_urn.as_ref().cloned().ok_or_else(|| {
            PlannerError::Internal(format!(
                "ForEach input node '{}' is Collect without output_media_urn",
                input_node_id
            ))
        })?,
        other => {
            return Err(PlannerError::Internal(format!(
                "ForEach input node '{}' is {:?}, expected Cap/InputSlot/Collect",
                input_node_id,
                std::mem::discriminant(other)
            )));
        }
    };

    let media_urn = MediaUrn::from_string(&output_media_str).map_err(|e| {
        PlannerError::Internal(format!(
            "Failed to parse media URN '{}': {}",
            output_media_str, e
        ))
    })?;
    let item_urn_str = media_urn.to_string();
    Ok((media_urn, item_urn_str))
}

/// Find the Collect node paired with a ForEach node, if any.
pub fn find_collect_for_foreach(plan: &MachinePlan, foreach_node_id: &str) -> Option<String> {
    let foreach_node = plan.get_node(foreach_node_id)?;
    let body_exit = match &foreach_node.node_type {
        ExecutionNodeType::ForEach { body_exit, .. } => body_exit,
        _ => return None,
    };

    for edge in &plan.edges {
        if edge.from_node == *body_exit {
            if let EdgeType::Collection = &edge.edge_type {
                if let Some(target) = plan.get_node(&edge.to_node) {
                    if matches!(target.node_type, ExecutionNodeType::Collect { .. }) {
                        return Some(edge.to_node.clone());
                    }
                }
            }
        }
    }
    None
}

/// Derive the media URN for collected output from a Collect node. Collect changes
/// shape (is_sequence), not type, so absent an explicit URN the output equals the item
/// type.
pub fn derive_collected_media_urn(
    plan: &MachinePlan,
    collect_node_id: &str,
    item_media_urn: &str,
) -> PlannerResult<String> {
    let collect_node = plan.get_node(collect_node_id).ok_or_else(|| {
        PlannerError::Internal(format!("Collect node '{}' not found", collect_node_id))
    })?;
    if let ExecutionNodeType::Collect {
        output_media_urn, ..
    } = &collect_node.node_type
    {
        if let Some(explicit_urn) = output_media_urn {
            return Ok(explicit_urn.clone());
        }
    }
    Ok(item_media_urn.to_string())
}
