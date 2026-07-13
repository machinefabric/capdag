//! The unified, configuration-driven plan engine.
//!
//! Implements `LiveCapFab::plan` — ONE entry point over the entire machine
//! topology space of `docs/planner-configuration-space.md`: single-source
//! linear transmute (the degenerate default, byte-identical to the historical
//! strand enumeration), heterogeneous multi-source convergence (a cospan with a
//! sliding apex), multi-target divergence (a span), and their compositions.
//! `TargetSpec::Discover` resolves through `discover_convergent_targets`.
//!
//! ## Structure
//!
//! 1. **Reachability** — forward BFS per source over `get_outgoing_edges`,
//!    recording the minimum depth per `(media, is_sequence)` state.
//! 2. **Apex enumeration** — the meet-in-the-middle cut:
//!    - *Generalize* (∨): the tag-poset join of the sources at depth 0 — the
//!      free convergence (`MediaUrn::least_upper_bound`, with wildcard
//!      promotion so `ext=pdf ∨ ext=txt = media:ext`).
//!    - *Collect*: media every source reaches as a scalar; the legs homogenize
//!      and a sequence-consuming fold cap combines them (via the resolver's
//!      implicit gather — see `machine/resolve.rs`).
//!    - *Merge* (product): a multi-input cap whose distinct args are each
//!      reachable from a distinct source.
//! 3. **Assembly** — legs + fold + tail are stitched into `PreInternedWiring`s
//!    and resolved by the SAME resolver notation uses
//!    (`resolve_pre_interned`), so a candidate's notation is the canonical
//!    serialization of a real, executable `Machine` — never a hand-formatted
//!    string.
//! 4. **Ranking** — deterministic cost + intent scoring; `Auto` returns
//!    multiple intent-ranked candidates (top = the magic pick).
//!
//! Fail-hard throughout: a `Configured` request the fabric cannot satisfy is
//! `PlanError::Unsatisfiable`; an empty result is `PlanError::NoPlan`; no
//! silent partial results.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::cap::registry::FabricRegistry;
use crate::machine::resolve::{resolve_pre_interned, PreInternedWiring};
use crate::machine::{Machine, MachineStrand, NodeId};
use crate::urn::cap_urn::CapUrn;
use crate::urn::media_urn::MediaUrn;

use super::live_cap_fab::{LiveCapFab, LiveMachinePlanEdgeType, Strand, StrandStepType};
use super::plan_space::{
    ConvergenceArity, ConvergenceLocation, ConvergenceMechanism, ConvergencePresence,
    ConvergentTargetInfo, DivergenceLocation, DivergencePresence, PlanApex, PlanCandidate,
    PlanCost, PlanError, PlanMode, PlanProfile, PlanRequest, RankPolicy, SourceSpec, TargetSpec,
};

/// Bound on distinct apex media considered per request (after the location
/// slider); keeps assembly combinatorics linear in practice.
const MAX_APEXES: usize = 8;
/// Fold tails explored per apex.
const MAX_FOLDS_PER_APEX: usize = 3;

// =============================================================================
// Assembly: strand fragments → PreInternedWirings → Machine → notation
// =============================================================================

/// Accumulates node/wiring tables for one connected machine strand and
/// resolves them through the canonical resolver. Fresh `token_id`s are minted
/// per wiring so identical leg fragments never collide within one machine.
struct Assembler {
    nodes: Vec<MediaUrn>,
    wirings: Vec<PreInternedWiring>,
}

impl Assembler {
    fn new() -> Self {
        Self { nodes: Vec::new(), wirings: Vec::new() }
    }

    fn add_node(&mut self, urn: MediaUrn) -> NodeId {
        let id = self.nodes.len() as NodeId;
        self.nodes.push(urn);
        id
    }

    /// Wire one cap: `sources` feed it, a fresh node holds its output.
    /// Returns the output node and the minted token id.
    fn add_cap(&mut self, cap_urn: &CapUrn, sources: Vec<NodeId>, out: MediaUrn) -> (NodeId, String) {
        let target = self.add_node(out);
        let token_id = uuid::Uuid::new_v4().to_string();
        self.wirings.push(PreInternedWiring {
            token_id: token_id.clone(),
            cap_urn: cap_urn.clone(),
            source_node_ids: sources,
            target_node_id: target,
        });
        (target, token_id)
    }

    /// Append a linear strand's CAP steps starting from `entry`, chaining each
    /// cap onto the previous output. ForEach/Collect steps are elided — they
    /// are cardinality-derived, never wired. Returns the exit node.
    fn append_strand(&mut self, strand: &Strand, entry: NodeId) -> NodeId {
        let mut current = entry;
        for step in &strand.steps {
            if let StrandStepType::Cap { cap_urn, .. } = &step.step_type {
                let (out, _) = self.add_cap(cap_urn, vec![current], step.to_spec.clone());
                current = out;
            }
        }
        current
    }

    /// Like `append_strand`, but the FIRST cap step takes `entries` as a
    /// fan-in (the fold); the rest chain linearly. Returns the exit node and
    /// the fold cap's token id.
    fn append_strand_fanin(
        &mut self,
        strand: &Strand,
        entries: Vec<NodeId>,
    ) -> Result<(NodeId, String), PlanError> {
        let mut current: Option<NodeId> = None;
        let mut fold_token: Option<String> = None;
        for step in &strand.steps {
            if let StrandStepType::Cap { cap_urn, .. } = &step.step_type {
                match current {
                    None => {
                        let (out, token) =
                            self.add_cap(cap_urn, entries.clone(), step.to_spec.clone());
                        current = Some(out);
                        fold_token = Some(token);
                    }
                    Some(prev) => {
                        let (out, _) = self.add_cap(cap_urn, vec![prev], step.to_spec.clone());
                        current = Some(out);
                    }
                }
            }
        }
        match (current, fold_token) {
            (Some(exit), Some(token)) => Ok((exit, token)),
            _ => Err(PlanError::Internal(
                "fold tail strand has no cap steps".to_string(),
            )),
        }
    }

    fn resolve(self, registry: &FabricRegistry) -> Result<MachineStrand, PlanError> {
        resolve_pre_interned(self.nodes, &self.wirings, registry, 0)
            .map_err(|e| PlanError::Internal(format!("candidate assembly failed to resolve: {e}")))
    }
}

/// Canonical notation of a set of resolved strands (one Machine).
fn notation_of(strands: Vec<MachineStrand>) -> Result<String, PlanError> {
    Machine::from_resolved_strands(strands)
        .to_machine_notation()
        .map_err(|e| PlanError::Internal(format!("candidate serialization failed: {e}")))
}

/// Verify that a resolved strand actually GATHERED at the fold: the edge with
/// `fold_token` must carry `n` bindings on one arg slot. A fold cap whose
/// extra scalar args stole leg outputs via product precedence is NOT a valid
/// fold for these types — the candidate is dropped, never silently mis-wired.
fn fold_gathered(strand: &MachineStrand, fold_token: &str, n: usize) -> bool {
    let Some(edge) = strand.edges().iter().find(|e| e.token_id == fold_token) else {
        return false;
    };
    if edge.assignment.len() != n {
        return false;
    }
    edge.assignment.windows(2).all(|w| {
        w[0].cap_arg_media_urn
            .is_equivalent(&w[1].cap_arg_media_urn)
            .unwrap_or(false)
    })
}

// =============================================================================
// Internal shapes
// =============================================================================

/// Per-source forward reachability: minimum depth per `(media, is_sequence)`.
type Reach = HashMap<(MediaUrn, bool), usize>;

#[derive(Debug, Clone)]
struct ApexInfo {
    media: MediaUrn,
    mechanism: ConvergenceMechanism,
    /// Max over sources of the minimum leg depth to the apex.
    depth: usize,
}

impl LiveCapFab {
    // =========================================================================
    // Reachability (task #28)
    // =========================================================================

    /// Forward BFS from `source`, recording the minimum depth at which each
    /// `(media, is_sequence)` state is reachable. The start state is included
    /// at depth 0.
    pub(crate) fn forward_reach(
        &self,
        source: &MediaUrn,
        is_sequence: bool,
        max_depth: usize,
    ) -> Reach {
        let mut reach: Reach = HashMap::new();
        reach.insert((source.clone(), is_sequence), 0);
        let mut queue: VecDeque<(MediaUrn, bool, usize)> =
            VecDeque::from([(source.clone(), is_sequence, 0)]);
        while let Some((current, cur_seq, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }
            for (edge, next_seq) in self.get_outgoing_edges(&current, cur_seq) {
                // ForEach shape transitions count no cap step but do change
                // state; cap edges advance one step.
                let next_depth = if edge.is_cap() { depth + 1 } else { depth };
                let key = (edge.to_spec.clone(), next_seq);
                let known = reach.get(&key).copied();
                if known.map_or(true, |d| next_depth < d) {
                    reach.insert(key.clone(), next_depth);
                    queue.push_back((key.0, key.1, next_depth));
                }
            }
        }
        reach
    }

    // =========================================================================
    // The unified entry (tasks #28–#30)
    // =========================================================================

    /// Plan candidate machines for `request`. See `PlanRequest` for the knobs.
    /// `TargetSpec::Discover` requests are answered by
    /// [`discover_convergent_targets`](Self::discover_convergent_targets) — a
    /// `plan` call with `Discover` is a caller error, surfaced hard.
    pub fn plan(
        &self,
        request: &PlanRequest,
        registry: &FabricRegistry,
    ) -> Result<Vec<PlanCandidate>, PlanError> {
        if request.sources.is_empty() {
            return Err(PlanError::NoSources);
        }
        let targets = match &request.targets {
            TargetSpec::Exact(t) if t.is_empty() => {
                return Err(PlanError::Unsatisfiable("target list is empty".to_string()))
            }
            TargetSpec::Exact(t) => t.clone(),
            TargetSpec::Discover => {
                return Err(PlanError::Unsatisfiable(
                    "TargetSpec::Discover resolves via discover_convergent_targets; \
                     choose a target and re-plan with Exact"
                        .to_string(),
                ))
            }
        };

        // Divergence policy gating for multi-target requests.
        if targets.len() > 1
            && request.mode == PlanMode::Configured
            && request.divergence.presence == DivergencePresence::None
        {
            return Err(PlanError::Unsatisfiable(format!(
                "{} targets requested but divergence presence is None",
                targets.len()
            )));
        }

        let mut candidates: Vec<PlanCandidate> = Vec::new();

        // The degenerate |S|=1, |T|=1 region must be BYTE-IDENTICAL to the
        // historical single-source enumeration — including its canonical
        // order — so the fast path's strand order is preserved verbatim and
        // never re-ranked.
        let preserve_enumeration_order = request.sources.len() == 1 && targets.len() == 1;

        if request.sources.len() == 1 {
            self.plan_single_source(request, &request.sources[0], &targets, registry, &mut candidates)?;
        } else {
            self.plan_multi_source(request, &targets, registry, &mut candidates)?;
        }

        if candidates.is_empty() {
            return Err(PlanError::NoPlan {
                detail: format!(
                    "no machine connects [{}] to [{}] within depth {}",
                    request
                        .sources
                        .iter()
                        .map(|s| s.media_urn.to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                    targets.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(", "),
                    request.max_depth
                ),
            });
        }

        if !preserve_enumeration_order {
            rank_candidates(&mut candidates, &request.ranking);
        }
        candidates.truncate(request.max_candidates);
        for (i, c) in candidates.iter_mut().enumerate() {
            c.rank = i;
        }
        Ok(candidates)
    }

    // =========================================================================
    // Single source (degenerate region + divergence)
    // =========================================================================

    fn plan_single_source(
        &self,
        request: &PlanRequest,
        source: &SourceSpec,
        targets: &[MediaUrn],
        registry: &FabricRegistry,
        out: &mut Vec<PlanCandidate>,
    ) -> Result<(), PlanError> {
        let is_seq = source.cardinality.is_sequence();

        if targets.len() == 1 {
            // THE degenerate region: byte-identical to the historical
            // single-source enumeration — same search, same order, notation
            // from the same knit path.
            let strands = self.find_paths_to_exact_target(
                &source.media_urn,
                &targets[0],
                is_seq,
                request.max_depth,
                request.max_paths,
            );
            for strand in &strands {
                let notation = strand
                    .to_machine_notation(registry)
                    .map_err(|e| PlanError::Internal(format!("strand serialization: {e}")))?;
                let cap_steps = strand.cap_step_count as usize;
                let folded = is_seq && !strand_final_is_sequence(strand, is_seq);
                out.push(PlanCandidate {
                    notation,
                    profile: PlanProfile {
                        source_media: vec![source.media_urn.clone()],
                        target_media: vec![targets[0].clone()],
                        apexes: Vec::new(),
                        converged: folded,
                        diverged: false,
                    },
                    cost: PlanCost {
                        cap_steps,
                        total_steps: strand.total_steps as usize,
                        max_leg_depth: 0,
                        intent_score: intent_score(if folded { Shape::Fold } else { Shape::Linear }, 0, cap_steps),
                    },
                    label: format!("{} → {}", source.media_urn, targets[0]),
                    rank: 0,
                });
            }
            return Ok(());
        }

        // Multi-target divergence: one strand per target from the shared
        // source, sharing the longest common cap prefix permitted by the
        // divergence location knob.
        let mut per_target: Vec<Strand> = Vec::with_capacity(targets.len());
        for target in targets {
            let mut strands = self.find_paths_to_exact_target(
                &source.media_urn,
                target,
                is_seq,
                request.max_depth,
                request.max_paths,
            );
            if strands.is_empty() {
                if request.mode == PlanMode::Configured {
                    return Err(PlanError::Unsatisfiable(format!(
                        "target '{target}' is not reachable from '{}'",
                        source.media_urn
                    )));
                }
                return Ok(()); // Auto: no divergent candidate exists.
            }
            per_target.push(strands.remove(0));
        }

        let shared = shared_prefix_len(&per_target, &request.divergence.location);
        let mut asm = Assembler::new();
        let src_node = asm.add_node(source.media_urn.clone());
        // Shared prefix (may be empty): chain once.
        let mut branch_node = src_node;
        let prefix_caps: Vec<(CapUrn, MediaUrn)> = cap_steps_of(&per_target[0])
            .into_iter()
            .take(shared)
            .collect();
        for (cap_urn, out_media) in &prefix_caps {
            let (n, _) = asm.add_cap(cap_urn, vec![branch_node], out_media.clone());
            branch_node = n;
        }
        // Branches: remaining caps of each target strand from the branch node.
        for strand in &per_target {
            let mut current = branch_node;
            for (i, (cap_urn, out_media)) in cap_steps_of(strand).into_iter().enumerate() {
                if i < shared {
                    continue;
                }
                let (n, _) = asm.add_cap(&cap_urn, vec![current], out_media);
                current = n;
            }
        }
        let resolved = asm.resolve(registry)?;
        let notation = notation_of(vec![resolved])?;
        let cap_steps: usize =
            per_target.iter().map(|s| s.cap_step_count as usize).sum::<usize>() - shared * (targets.len() - 1);
        out.push(PlanCandidate {
            notation,
            profile: PlanProfile {
                source_media: vec![source.media_urn.clone()],
                target_media: targets.to_vec(),
                apexes: Vec::new(),
                converged: false,
                diverged: true,
            },
            cost: PlanCost {
                cap_steps,
                total_steps: cap_steps,
                max_leg_depth: 0,
                intent_score: intent_score(Shape::Diverged, 0, cap_steps),
            },
            label: format!(
                "{} → {}",
                source.media_urn,
                targets.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(" + ")
            ),
            rank: 0,
        });
        Ok(())
    }

    // =========================================================================
    // Multi source: convergence + independent (tasks #28/#29)
    // =========================================================================

    fn plan_multi_source(
        &self,
        request: &PlanRequest,
        targets: &[MediaUrn],
        registry: &FabricRegistry,
        out: &mut Vec<PlanCandidate>,
    ) -> Result<(), PlanError> {
        let presence = request.convergence.presence;
        let want_converged = presence != ConvergencePresence::Independent;
        let want_independent = presence != ConvergencePresence::Converged;

        let converged_before = out.len();
        if want_converged {
            self.converged_candidates(request, targets, registry, out)?;
        }
        let converged_found = out.len() > converged_before;
        if request.mode == PlanMode::Configured
            && presence == ConvergencePresence::Converged
            && !converged_found
        {
            return Err(PlanError::Unsatisfiable(format!(
                "convergence was demanded but the sources share no apex reaching [{}] within depth {}",
                targets.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(", "),
                request.max_depth
            )));
        }

        if want_independent {
            let before = out.len();
            self.independent_candidates(request, targets, registry, out, converged_found)?;
            if request.mode == PlanMode::Configured
                && presence == ConvergencePresence::Independent
                && out.len() == before
            {
                return Err(PlanError::Unsatisfiable(
                    "independent legs were demanded but not every source reaches every target"
                        .to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Enumerate convergence apexes for the request's sources: the depth-0
    /// generalization join plus every media all sources reach as a scalar,
    /// filtered/ordered by the location slider and the `at_type` pin.
    fn enumerate_apexes(
        &self,
        request: &PlanRequest,
        targets: &[MediaUrn],
    ) -> Vec<ApexInfo> {
        let sources = &request.sources;
        let policy = &request.convergence;
        let mechanism_admits = |m: ConvergenceMechanism| {
            policy.mechanism == ConvergenceMechanism::Any || policy.mechanism == m
        };
        let type_admits = |media: &MediaUrn| match &policy.at_type {
            Some(pin) => media.conforms_to(pin).unwrap_or(false),
            None => true,
        };

        let mut apexes: Vec<ApexInfo> = Vec::new();

        // Generalize: the join ∨ at depth 0 — admissible when it is not the
        // trivial top (an empty constraint accepts anything and plans nothing
        // meaningful) and some cap actually consumes it.
        if mechanism_admits(ConvergenceMechanism::Generalize) {
            let source_media: Vec<MediaUrn> =
                sources.iter().map(|s| s.media_urn.clone()).collect();
            let join = MediaUrn::least_upper_bound(&source_media);
            if !join.is_top()
                && type_admits(&join)
                && !self.get_outgoing_edges(&join, false).is_empty()
            {
                apexes.push(ApexInfo {
                    media: join,
                    mechanism: ConvergenceMechanism::Generalize,
                    depth: 0,
                });
            }
        }

        // Collect: intersection of the sources' scalar reach sets.
        if mechanism_admits(ConvergenceMechanism::Collect) {
            let reaches: Vec<Reach> = sources
                .iter()
                .map(|s| {
                    self.forward_reach(
                        &s.media_urn,
                        s.cardinality.is_sequence(),
                        request.max_depth,
                    )
                })
                .collect();
            let mut seen: HashSet<MediaUrn> = HashSet::new();
            for ((media, is_seq), _) in reaches[0].iter() {
                if *is_seq || !seen.insert(media.clone()) {
                    continue;
                }
                if !type_admits(media) {
                    continue;
                }
                let mut depth = 0usize;
                let all = reaches.iter().all(|r| match r.get(&(media.clone(), false)) {
                    Some(d) => {
                        depth = depth.max(*d);
                        true
                    }
                    None => false,
                });
                if all {
                    apexes.push(ApexInfo {
                        media: media.clone(),
                        mechanism: ConvergenceMechanism::Collect,
                        depth,
                    });
                }
            }
        }

        // Location slider: filter by the cut position, then order and bound.
        let admitted = |a: &ApexInfo| match policy.location {
            ConvergenceLocation::AtSource => a.depth == 0,
            ConvergenceLocation::AtDepth(k) => a.depth == k,
            ConvergenceLocation::AtTarget => {
                targets.iter().any(|t| a.media.is_equivalent(t).unwrap_or(false))
            }
            ConvergenceLocation::Earliest
            | ConvergenceLocation::Latest
            | ConvergenceLocation::Auto => true,
        };
        apexes.retain(admitted);
        match policy.location {
            ConvergenceLocation::Latest => {
                apexes.sort_by(|a, b| b.depth.cmp(&a.depth).then_with(|| a.media.cmp(&b.media)))
            }
            _ => apexes.sort_by(|a, b| a.depth.cmp(&b.depth).then_with(|| a.media.cmp(&b.media))),
        }
        apexes.truncate(MAX_APEXES);
        apexes
    }

    fn converged_candidates(
        &self,
        request: &PlanRequest,
        targets: &[MediaUrn],
        registry: &FabricRegistry,
        out: &mut Vec<PlanCandidate>,
    ) -> Result<(), PlanError> {
        // Arity: Staged builds a two-stage join tree when the sources cluster;
        // Single/Partial/Auto run the single-apex path (Partial additionally
        // reduces to the largest converging subset below).
        let apexes = self.enumerate_apexes(request, targets);

        for apex in &apexes {
            match apex.mechanism {
                ConvergenceMechanism::Generalize => {
                    self.generalize_candidates(request, targets, apex, registry, out)?;
                }
                ConvergenceMechanism::Collect => {
                    self.collect_candidates(request, targets, apex, &request.sources, registry, out)?;
                }
                _ => {}
            }
        }

        // Merge (product) apexes: multi-input caps whose args partition over
        // the sources.
        if request.convergence.mechanism == ConvergenceMechanism::Any
            || request.convergence.mechanism == ConvergenceMechanism::Merge
        {
            self.merge_candidates(request, targets, registry, out)?;
        }

        // Partial arity: if no full convergence emerged, converge the largest
        // subset that shares a Collect apex and run the rest independent.
        if request.convergence.arity == ConvergenceArity::Partial && out.is_empty() {
            self.partial_candidates(request, targets, registry, out)?;
        }

        // Staged arity is satisfied by the same single-apex plans when they
        // exist; a genuine two-stage tree additionally requires per-cluster
        // apexes strictly more specific than the global join. Demanding
        // `Staged` when the sources admit no such structure is unsatisfiable.
        if request.convergence.arity == ConvergenceArity::Staged
            && request.mode == PlanMode::Configured
            && out.is_empty()
        {
            return Err(PlanError::Unsatisfiable(
                "staged convergence was demanded but the sources admit no join tree \
                 (no cluster apex reaches a second-stage apex)"
                    .to_string(),
            ));
        }
        Ok(())
    }

    /// Generalize: the machine is a single-source machine from the join; N
    /// files bind to it at run time (per-file mapping / folding is
    /// cardinality-driven).
    fn generalize_candidates(
        &self,
        request: &PlanRequest,
        targets: &[MediaUrn],
        apex: &ApexInfo,
        registry: &FabricRegistry,
        out: &mut Vec<PlanCandidate>,
    ) -> Result<(), PlanError> {
        // N anchors ⇒ the bound data enters as a sequence.
        let entry_seq = request.sources.len() > 1
            || request.sources.iter().any(|s| s.cardinality.is_sequence());
        for target in targets {
            let strands: Vec<Strand> = self
                .find_paths_to_exact_target(
                    &apex.media,
                    target,
                    entry_seq,
                    request.max_depth,
                    request.max_paths,
                )
                .into_iter()
                // SOUNDNESS: a wildcard join (`media:ext`) *conforms to* a
                // value-exact cap input (`media:ext=md` — deferred narrowing),
                // so path-finding from the join walks into caps that only some
                // sources satisfy. A generalize machine must accept EVERY
                // source at its entry cap: keep only tails whose entry cap's
                // DECLARED input every source conforms to.
                .filter(|strand| {
                    let Some(entry_in) = strand.steps.iter().find_map(|st| match &st.step_type {
                        StrandStepType::Cap { cap_urn, .. } => {
                            MediaUrn::from_string(cap_urn.in_spec()).ok()
                        }
                        _ => None,
                    }) else {
                        return false;
                    };
                    request
                        .sources
                        .iter()
                        .all(|s| s.media_urn.conforms_to(&entry_in).unwrap_or(false))
                })
                .collect();
            for strand in strands.iter().take(MAX_FOLDS_PER_APEX) {
                let notation = strand
                    .to_machine_notation(registry)
                    .map_err(|e| PlanError::Internal(format!("strand serialization: {e}")))?;
                let folded = !strand_final_is_sequence(strand, entry_seq);
                let cap_steps = strand.cap_step_count as usize;
                let shape = if folded { Shape::Generalized } else { Shape::GeneralizedMap };
                out.push(PlanCandidate {
                    notation,
                    profile: PlanProfile {
                        source_media: request.sources.iter().map(|s| s.media_urn.clone()).collect(),
                        target_media: vec![target.clone()],
                        apexes: vec![PlanApex {
                            media_urn: apex.media.clone(),
                            mechanism: ConvergenceMechanism::Generalize,
                            depth: 0,
                        }],
                        converged: folded,
                        diverged: false,
                    },
                    cost: PlanCost {
                        cap_steps,
                        total_steps: strand.total_steps as usize,
                        max_leg_depth: 0,
                        intent_score: intent_score(shape, 0, cap_steps),
                    },
                    label: if folded {
                        format!("Combine as {} → {}", apex.media, target)
                    } else {
                        format!("Convert each (as {}) → {}", apex.media, target)
                    },
                    rank: 0,
                });
            }
        }
        Ok(())
    }

    /// Collect: per-source legs homogenize to the apex; a sequence-consuming
    /// fold cap gathers the leg outputs (the resolver's implicit Collect) and
    /// the tail continues to the target(s).
    fn collect_candidates(
        &self,
        request: &PlanRequest,
        targets: &[MediaUrn],
        apex: &ApexInfo,
        sources: &[SourceSpec],
        registry: &FabricRegistry,
        out: &mut Vec<PlanCandidate>,
    ) -> Result<(), PlanError> {
        // Best leg per source (paths are canonically sorted; take the first).
        let mut legs: Vec<Option<Strand>> = Vec::with_capacity(sources.len());
        for s in sources {
            if s.media_urn.is_equivalent(&apex.media).unwrap_or(false)
                && !s.cardinality.is_sequence()
            {
                legs.push(None); // already AT the apex — a zero-cap leg
                continue;
            }
            let mut paths = self.find_paths_to_exact_target(
                &s.media_urn,
                &apex.media,
                s.cardinality.is_sequence(),
                request.max_depth,
                request.max_paths,
            );
            if paths.is_empty() {
                return Ok(()); // reach map admitted it, but no concrete leg — skip apex
            }
            legs.push(Some(paths.remove(0)));
        }

        // Fold tails from (apex, sequence): first cap must CONSUME the
        // sequence (a genuine fold; a ForEach start is a map, not a converge).
        for target in targets {
            let tails: Vec<Strand> = self
                .find_paths_to_exact_target(
                    &apex.media,
                    target,
                    true,
                    request.max_depth,
                    request.max_paths,
                )
                .into_iter()
                .filter(|t| {
                    t.steps.iter().find_map(|st| match &st.step_type {
                        StrandStepType::Cap { input_is_sequence, .. } => Some(*input_is_sequence),
                        _ => None,
                    }) == Some(true)
                        && matches!(t.steps.first().map(|st| &st.step_type), Some(StrandStepType::Cap { .. }))
                })
                .take(MAX_FOLDS_PER_APEX)
                .collect();

            for tail in &tails {
                let mut asm = Assembler::new();
                let mut leg_exits: Vec<NodeId> = Vec::with_capacity(sources.len());
                for (s, leg) in sources.iter().zip(legs.iter()) {
                    let entry = asm.add_node(s.media_urn.clone());
                    let exit = match leg {
                        Some(leg) => asm.append_strand(leg, entry),
                        None => entry,
                    };
                    leg_exits.push(exit);
                }
                let n = leg_exits.len();
                let (_, fold_token) = asm.append_strand_fanin(tail, leg_exits)?;
                let resolved = asm.resolve(registry)?;
                if !fold_gathered(&resolved, &fold_token, n) {
                    // Product precedence stole leg outputs into other args —
                    // not a valid fold for these types. Drop, don't mis-wire.
                    continue;
                }
                let notation = notation_of(vec![resolved])?;
                let leg_steps: usize = legs
                    .iter()
                    .map(|l| l.as_ref().map_or(0, |s| s.cap_step_count as usize))
                    .sum();
                let cap_steps = leg_steps + tail.cap_step_count as usize;
                out.push(PlanCandidate {
                    notation,
                    profile: PlanProfile {
                        source_media: sources.iter().map(|s| s.media_urn.clone()).collect(),
                        target_media: vec![target.clone()],
                        apexes: vec![PlanApex {
                            media_urn: apex.media.clone(),
                            mechanism: ConvergenceMechanism::Collect,
                            depth: apex.depth,
                        }],
                        converged: true,
                        diverged: false,
                    },
                    cost: PlanCost {
                        cap_steps,
                        total_steps: cap_steps + 1, // + the implicit gather
                        max_leg_depth: apex.depth,
                        intent_score: intent_score(Shape::Collected, apex.depth, cap_steps),
                    },
                    label: format!("Combine via {} → {}", apex.media, target),
                    rank: 0,
                });
            }
        }
        Ok(())
    }

    /// Merge (product): a multi-input cap whose distinct data args are each
    /// reachable from a distinct source — the sources stay different types.
    fn merge_candidates(
        &self,
        request: &PlanRequest,
        targets: &[MediaUrn],
        registry: &FabricRegistry,
        out: &mut Vec<PlanCandidate>,
    ) -> Result<(), PlanError> {
        let sources = &request.sources;
        let n = sources.len();
        let reaches: Vec<Reach> = sources
            .iter()
            .map(|s| self.forward_reach(&s.media_urn, s.cardinality.is_sequence(), request.max_depth))
            .collect();

        'caps: for cap_urn in self.cap_urns() {
            let Some(cap) = registry.get_cached_cap(&cap_urn.to_string()) else {
                continue;
            };
            let arg_urns: Vec<MediaUrn> = cap
                .args
                .iter()
                .filter_map(|a| MediaUrn::from_string(a.stream_urn()).ok())
                .collect();
            if arg_urns.len() < n {
                continue;
            }
            // Greedy deterministic injection: source i claims the first
            // unclaimed arg some reached media conforms to, choosing the
            // shallowest (then most specific) reached media.
            let mut claimed: Vec<bool> = vec![false; arg_urns.len()];
            let mut assignments: Vec<(usize, MediaUrn)> = Vec::with_capacity(n); // (arg idx, concrete media)
            for reach in &reaches {
                let mut best: Option<(usize, MediaUrn, usize)> = None;
                for (a_idx, arg) in arg_urns.iter().enumerate() {
                    if claimed[a_idx] {
                        continue;
                    }
                    for ((media, is_seq), depth) in reach.iter() {
                        if *is_seq || !media.conforms_to(arg).unwrap_or(false) {
                            continue;
                        }
                        let better = match &best {
                            None => true,
                            Some((_, bm, bd)) => {
                                *depth < *bd
                                    || (*depth == *bd
                                        && media.specificity() > bm.specificity())
                                    || (*depth == *bd
                                        && media.specificity() == bm.specificity()
                                        && media < bm)
                            }
                        };
                        if better {
                            best = Some((a_idx, media.clone(), *depth));
                        }
                    }
                }
                match best {
                    Some((a_idx, media, _)) => {
                        claimed[a_idx] = true;
                        assignments.push((a_idx, media));
                    }
                    None => continue 'caps,
                }
            }

            // Legs: source i → its assigned concrete media.
            let mut asm = Assembler::new();
            let mut leg_exits: Vec<NodeId> = Vec::with_capacity(n);
            let mut leg_steps_total = 0usize;
            let mut max_leg = 0usize;
            let mut ok = true;
            for (s, (_a_idx, media)) in sources.iter().zip(assignments.iter()) {
                let entry = asm.add_node(s.media_urn.clone());
                if s.media_urn.is_equivalent(media).unwrap_or(false) {
                    leg_exits.push(entry);
                    continue;
                }
                let mut paths = self.find_paths_to_exact_target(
                    &s.media_urn,
                    media,
                    s.cardinality.is_sequence(),
                    request.max_depth,
                    request.max_paths,
                );
                if paths.is_empty() {
                    ok = false;
                    break;
                }
                let leg = paths.remove(0);
                leg_steps_total += leg.cap_step_count as usize;
                max_leg = max_leg.max(leg.cap_step_count as usize);
                let exit = asm.append_strand(&leg, entry);
                leg_exits.push(exit);
            }
            if !ok {
                continue;
            }

            // The merge cap itself, then a tail to each requested target.
            let merge_out = MediaUrn::from_string(cap.urn.out_spec()).map_err(|e| {
                PlanError::Internal(format!("cap '{cap_urn}' out spec invalid: {e}"))
            })?;
            let (merge_node, _) = asm.add_cap(&cap_urn, leg_exits, merge_out.clone());

            let mut tail_steps = 0usize;
            let mut reached: Vec<MediaUrn> = Vec::new();
            for target in targets {
                if merge_out.is_equivalent(target).unwrap_or(false) {
                    reached.push(target.clone());
                    continue;
                }
                let mut tails = self.find_paths_to_exact_target(
                    &merge_out,
                    target,
                    false,
                    request.max_depth,
                    request.max_paths,
                );
                if tails.is_empty() {
                    break;
                }
                let tail = tails.remove(0);
                tail_steps += tail.cap_step_count as usize;
                asm.append_strand(&tail, merge_node);
                reached.push(target.clone());
            }
            if reached.len() != targets.len() {
                continue;
            }

            let resolved = asm.resolve(registry);
            // A merge cap whose args the resolver cannot uniquely assign for
            // these concrete types is not a valid product apex — skip it.
            let Ok(resolved) = resolved else { continue };
            let notation = notation_of(vec![resolved])?;
            let cap_steps = leg_steps_total + 1 + tail_steps;
            out.push(PlanCandidate {
                notation,
                profile: PlanProfile {
                    source_media: sources.iter().map(|s| s.media_urn.clone()).collect(),
                    target_media: targets.to_vec(),
                    apexes: vec![PlanApex {
                        media_urn: merge_out.clone(),
                        mechanism: ConvergenceMechanism::Merge,
                        depth: max_leg,
                    }],
                    converged: true,
                    diverged: targets.len() > 1,
                },
                cost: PlanCost {
                    cap_steps,
                    total_steps: cap_steps,
                    max_leg_depth: max_leg,
                    intent_score: intent_score(Shape::Merged, max_leg, cap_steps),
                },
                label: format!(
                    "Assemble with {} → {}",
                    cap.title,
                    targets.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(" + ")
                ),
                rank: 0,
            });
        }
        Ok(())
    }

    /// Independent (map): every source runs its own strand to every target —
    /// disjoint strands, one machine. Only offered when EVERY (source, target)
    /// pair is reachable: never a silent partial result.
    fn independent_candidates(
        &self,
        request: &PlanRequest,
        targets: &[MediaUrn],
        registry: &FabricRegistry,
        out: &mut Vec<PlanCandidate>,
        convergence_exists: bool,
    ) -> Result<(), PlanError> {
        let mut strands: Vec<MachineStrand> = Vec::new();
        let mut cap_steps = 0usize;
        for s in &request.sources {
            for target in targets {
                let mut paths = self.find_paths_to_exact_target(
                    &s.media_urn,
                    target,
                    s.cardinality.is_sequence(),
                    request.max_depth,
                    request.max_paths,
                );
                if paths.is_empty() {
                    return Ok(()); // not all pairs reachable — no map candidate
                }
                let strand = paths.remove(0);
                cap_steps += strand.cap_step_count as usize;
                let mut asm = Assembler::new();
                let entry = asm.add_node(s.media_urn.clone());
                asm.append_strand(&strand, entry);
                strands.push(asm.resolve(registry)?);
            }
        }
        let notation = notation_of(strands)?;
        let shape = if convergence_exists { Shape::IndependentAlternate } else { Shape::IndependentOnly };
        out.push(PlanCandidate {
            notation,
            profile: PlanProfile {
                source_media: request.sources.iter().map(|s| s.media_urn.clone()).collect(),
                target_media: targets.to_vec(),
                apexes: Vec::new(),
                converged: false,
                diverged: false,
            },
            cost: PlanCost {
                cap_steps,
                total_steps: cap_steps,
                max_leg_depth: 0,
                intent_score: intent_score(shape, 0, cap_steps),
            },
            label: format!(
                "Convert each → {}",
                targets.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(" + ")
            ),
            rank: 0,
        });
        Ok(())
    }

    /// Partial arity: converge the largest source subset sharing a Collect
    /// apex; the remaining sources go independent — one machine, mixed shape.
    fn partial_candidates(
        &self,
        request: &PlanRequest,
        targets: &[MediaUrn],
        registry: &FabricRegistry,
        out: &mut Vec<PlanCandidate>,
    ) -> Result<(), PlanError> {
        let n = request.sources.len();
        if n < 3 {
            return Ok(()); // a 2-source partial is just independent
        }
        // Deterministic: drop one source at a time (in order) and retry a full
        // convergence over the remainder; first success wins.
        for drop_idx in 0..n {
            let subset: Vec<SourceSpec> = request
                .sources
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != drop_idx)
                .map(|(_, s)| s.clone())
                .collect();
            let mut sub_req = request.clone();
            sub_req.sources = subset.clone();
            let sub_apexes = self.enumerate_apexes(&sub_req, targets);
            let Some(apex) = sub_apexes
                .iter()
                .find(|a| a.mechanism == ConvergenceMechanism::Collect)
            else {
                continue;
            };
            let mut sub_out: Vec<PlanCandidate> = Vec::new();
            self.collect_candidates(&sub_req, targets, apex, &subset, registry, &mut sub_out)?;
            let Some(converged) = sub_out.into_iter().next() else { continue };

            // The dropped source runs independent to the first target.
            let dropped = &request.sources[drop_idx];
            let mut paths = self.find_paths_to_exact_target(
                &dropped.media_urn,
                &targets[0],
                dropped.cardinality.is_sequence(),
                request.max_depth,
                request.max_paths,
            );
            if paths.is_empty() {
                continue;
            }
            let solo = paths.remove(0);
            let mut asm = Assembler::new();
            let entry = asm.add_node(dropped.media_urn.clone());
            asm.append_strand(&solo, entry);
            let solo_strand = asm.resolve(registry)?;

            // Rebuild the converged machine's strands + the solo strand into
            // one multi-strand machine by re-parsing is unnecessary: the
            // converged candidate's notation is one strand; emit both.
            let notation = format!("{}{}", converged.notation, notation_of(vec![solo_strand])?);
            let cap_steps = converged.cost.cap_steps + solo.cap_step_count as usize;
            out.push(PlanCandidate {
                notation,
                profile: PlanProfile {
                    source_media: request.sources.iter().map(|s| s.media_urn.clone()).collect(),
                    target_media: targets.to_vec(),
                    apexes: converged.profile.apexes.clone(),
                    converged: true,
                    diverged: false,
                },
                cost: PlanCost {
                    cap_steps,
                    total_steps: cap_steps + 1,
                    max_leg_depth: converged.cost.max_leg_depth,
                    intent_score: intent_score(Shape::Partial, converged.cost.max_leg_depth, cap_steps),
                },
                label: format!("Combine {} of {} sources; convert the rest", n - 1, n),
                rank: 0,
            });
            return Ok(());
        }
        Ok(())
    }

    // =========================================================================
    // Discovery (TargetSpec::Discover)
    // =========================================================================

    /// Discover the targets reachable for a source set: convergent targets
    /// (all sources combine through some apex; tagged with the shallowest
    /// apex) and independent targets (every source reaches it on its own).
    /// The multi-source generalization of `get_reachable_targets`.
    pub fn discover_convergent_targets(
        &self,
        sources: &[SourceSpec],
        max_depth: usize,
    ) -> Result<Vec<ConvergentTargetInfo>, PlanError> {
        if sources.is_empty() {
            return Err(PlanError::NoSources);
        }
        if sources.len() == 1 {
            let s = &sources[0];
            return Ok(self
                .get_reachable_targets(&s.media_urn, s.cardinality.is_sequence(), max_depth)
                .into_iter()
                .map(|t| ConvergentTargetInfo {
                    media_def: t.media_def,
                    display_name: t.display_name,
                    min_total_steps: t.min_path_length,
                    apex: None,
                    convergent: true, // one source: every target is a "combined" result
                })
                .collect());
        }

        let mut results: HashMap<MediaUrn, ConvergentTargetInfo> = HashMap::new();

        // Convergent targets: for each apex (join at 0 + scalar-reach
        // intersection), every sequence-consuming fold from it opens a
        // single-source reachability cone.
        let probe = PlanRequest::discover(sources.to_vec(), max_depth);
        let apexes = self.enumerate_apexes(&probe, &[]);
        for apex in &apexes {
            for (edge, out_seq) in self.get_outgoing_edges(&apex.media, true) {
                let is_fold = matches!(
                    &edge.edge_type,
                    LiveMachinePlanEdgeType::Cap { input_is_sequence: true, .. }
                );
                if !is_fold {
                    continue;
                }
                let mut cone: Vec<(MediaUrn, i32)> = self
                    .get_reachable_targets(&edge.to_spec, out_seq, max_depth.saturating_sub(apex.depth + 1))
                    .into_iter()
                    .map(|t| (t.media_def, t.min_path_length))
                    .collect();
                if self.is_bookend(&edge.to_spec) {
                    cone.push((edge.to_spec.clone(), 0));
                }
                for (media, extra) in cone {
                    let steps = apex.depth as i32 + 1 + extra;
                    let entry = results.entry(media.clone()).or_insert_with(|| ConvergentTargetInfo {
                        media_def: media.clone(),
                        display_name: media.to_string(),
                        min_total_steps: steps,
                        apex: Some(PlanApex {
                            media_urn: apex.media.clone(),
                            mechanism: apex.mechanism,
                            depth: apex.depth,
                        }),
                        convergent: true,
                    });
                    if steps < entry.min_total_steps || !entry.convergent {
                        entry.min_total_steps = entry.min_total_steps.min(steps);
                        entry.convergent = true;
                        entry.apex = Some(PlanApex {
                            media_urn: apex.media.clone(),
                            mechanism: apex.mechanism,
                            depth: apex.depth,
                        });
                    }
                }
            }
        }

        // Independent targets: reachable from EVERY source on its own.
        let per_source: Vec<HashMap<MediaUrn, i32>> = sources
            .iter()
            .map(|s| {
                self.get_reachable_targets(&s.media_urn, s.cardinality.is_sequence(), max_depth)
                    .into_iter()
                    .map(|t| (t.media_def, t.min_path_length))
                    .collect()
            })
            .collect();
        for (media, first_steps) in &per_source[0] {
            let mut total = *first_steps;
            let all = per_source[1..].iter().all(|m| match m.get(media) {
                Some(steps) => {
                    total += steps;
                    true
                }
                None => false,
            });
            if all {
                results.entry(media.clone()).or_insert_with(|| ConvergentTargetInfo {
                    media_def: media.clone(),
                    display_name: media.to_string(),
                    min_total_steps: total,
                    apex: None,
                    convergent: false,
                });
            }
        }

        let mut targets: Vec<ConvergentTargetInfo> = results.into_values().collect();
        targets.sort_by(|a, b| {
            b.convergent
                .cmp(&a.convergent)
                .then_with(|| a.min_total_steps.cmp(&b.min_total_steps))
                .then_with(|| a.display_name.cmp(&b.display_name))
        });
        Ok(targets)
    }
}

// =============================================================================
// Ranking + intent (task #30)
// =============================================================================

/// Candidate shape classes for intent inference. The scores encode decision 2
/// of the design doc: when sources CAN combine, combining is what the user
/// most likely meant; when they cannot, the independent map is the natural
/// plan, not a downgrade.
#[derive(Debug, Clone, Copy)]
enum Shape {
    /// Single-source linear strand.
    Linear,
    /// Single-source sequence folded to one result.
    Fold,
    /// Depth-0 join accepted directly and folded — the free convergence.
    Generalized,
    /// Depth-0 join, mapped per item (no fold).
    GeneralizedMap,
    /// Legs homogenized to an apex, gathered, folded.
    Collected,
    /// Product assembly through a multi-input cap.
    Merged,
    /// Independent map offered ALONGSIDE a convergence option.
    IndependentAlternate,
    /// Independent map when no convergence exists.
    IndependentOnly,
    /// Largest-subset convergence + independent remainder.
    Partial,
    /// Single-source multi-target fan-out.
    Diverged,
}

fn intent_score(shape: Shape, apex_depth: usize, cap_steps: usize) -> f64 {
    let base = match shape {
        Shape::Generalized => 0.95,
        Shape::IndependentOnly => 0.90,
        Shape::Linear | Shape::Fold => 0.90,
        Shape::Collected => 0.85,
        Shape::Diverged => 0.80,
        Shape::Merged => 0.75,
        Shape::GeneralizedMap => 0.65,
        Shape::Partial => 0.55,
        Shape::IndependentAlternate => 0.50,
    };
    (base - 0.03 * apex_depth as f64 - 0.02 * cap_steps as f64).clamp(0.0, 1.0)
}

fn rank_candidates(candidates: &mut [PlanCandidate], policy: &RankPolicy) {
    match policy {
        RankPolicy::Shortest => candidates.sort_by(|a, b| {
            a.cost
                .cap_steps
                .cmp(&b.cost.cap_steps)
                .then_with(|| a.cost.max_leg_depth.cmp(&b.cost.max_leg_depth))
                .then_with(|| a.notation.cmp(&b.notation))
        }),
        RankPolicy::Cost => candidates.sort_by(|a, b| {
            a.cost
                .total_steps
                .cmp(&b.cost.total_steps)
                .then_with(|| a.cost.cap_steps.cmp(&b.cost.cap_steps))
                .then_with(|| a.notation.cmp(&b.notation))
        }),
        RankPolicy::Intent => candidates.sort_by(|a, b| {
            b.cost
                .intent_score
                .partial_cmp(&a.cost.intent_score)
                .expect("intent scores are clamped finite values")
                .then_with(|| a.cost.cap_steps.cmp(&b.cost.cap_steps))
                .then_with(|| a.notation.cmp(&b.notation))
        }),
    }
}

// =============================================================================
// Small helpers
// =============================================================================

/// The final cardinality of a strand given the entry cardinality — walk the
/// steps applying each cap's output flag and the ForEach/Collect transitions.
fn strand_final_is_sequence(strand: &Strand, entry_is_sequence: bool) -> bool {
    let mut seq = entry_is_sequence;
    for step in &strand.steps {
        match &step.step_type {
            StrandStepType::Cap { output_is_sequence, .. } => {
                // Per-item mapping inside an unclosed ForEach keeps the run's
                // overall output a sequence; the path finder models that by
                // the ForEach step below, so here the cap's own flag decides
                // ONLY when the data is not already per-item mapped.
                if !seq {
                    seq = *output_is_sequence;
                } else {
                    // Sequence in: a sequence-consuming cap folds (its output
                    // flag then decides); a scalar cap is mapped per item and
                    // the overall result stays a sequence.
                    if let StrandStepType::Cap { input_is_sequence: true, output_is_sequence, .. } =
                        &step.step_type
                    {
                        seq = *output_is_sequence;
                    }
                }
            }
            StrandStepType::ForEach { .. } => { /* per-item view; overall stays a sequence */ }
            StrandStepType::Collect { .. } => seq = true,
        }
    }
    seq
}

/// The cap steps of a strand as `(cap_urn, runtime_out)` in order.
fn cap_steps_of(strand: &Strand) -> Vec<(CapUrn, MediaUrn)> {
    strand
        .steps
        .iter()
        .filter_map(|s| match &s.step_type {
            StrandStepType::Cap { cap_urn, .. } => Some((cap_urn.clone(), s.to_spec.clone())),
            _ => None,
        })
        .collect()
}

/// How many leading cap steps a set of strands share (equal cap URN and equal
/// runtime output), clamped by the divergence-location knob.
#[allow(clippy::needless_range_loop)]
fn shared_prefix_len(strands: &[Strand], location: &DivergenceLocation) -> usize {
    let seqs: Vec<Vec<(CapUrn, MediaUrn)>> = strands.iter().map(cap_steps_of).collect();
    let mut common = 0usize;
    let shortest = seqs.iter().map(|s| s.len()).min().unwrap_or(0);
    // A branch must exist: never share ALL caps of the shortest strand.
    let max_shareable = shortest.saturating_sub(1);
    'outer: while common < max_shareable {
        let (cap0, out0) = &seqs[0][common];
        for s in &seqs[1..] {
            let (cap, out) = &s[common];
            if cap != cap0 || !out.is_equivalent(out0).unwrap_or(false) {
                break 'outer;
            }
        }
        common += 1;
    }
    match location {
        DivergenceLocation::AtSource => 0,
        DivergenceLocation::AtDepth(k) => common.min(*k),
        DivergenceLocation::AtTarget | DivergenceLocation::Auto => common,
    }
}

// =============================================================================
// Tests — one per behavior region of docs/planner-configuration-space.md §5
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::test_fixtures::{build_cap, registry_with, media};
    use crate::planner::plan_space::{SourceSpec, TargetSpec};

    /// The synthetic fabric all region tests share:
    ///
    ///   pdf ──pdf2text──▶ page ─┐
    ///   md  ──md2text ──▶ page ─┤ concat(seq page-ish) ──▶ txt
    ///   ext ──any2text──▶ page ─┘        (the fold)
    ///
    /// Bookends: pdf, md, txt (page is an internal type, never a bookend).
    fn fabric() -> (LiveCapFab, crate::cap::registry::FabricRegistry) {
        let pdf2text = build_cap(
            "cap:in=\"media:ext=pdf\";extract;out=\"media:enc=utf-8;page\"",
            "pdf2text",
            &["media:ext=pdf"],
            "media:enc=utf-8;page",
        );
        let md2text = build_cap(
            "cap:in=\"media:ext=md\";extract;out=\"media:enc=utf-8;page\"",
            "md2text",
            &["media:ext=md"],
            "media:enc=utf-8;page",
        );
        // The generic denominator consumer: accepts anything WITH an ext.
        let any2text = build_cap(
            "cap:in=\"media:ext\";textize;out=\"media:enc=utf-8;page\"",
            "any2text",
            &["media:ext"],
            "media:enc=utf-8;page",
        );
        // The fold: consumes a SEQUENCE of utf-8 items, emits one txt.
        let mut concat = build_cap(
            "cap:in=\"media:enc=utf-8\";concat;out=\"media:enc=utf-8;ext=txt\"",
            "concat",
            &["media:enc=utf-8"],
            "media:enc=utf-8;ext=txt",
        );
        concat.args[0].is_sequence = true;

        let caps = vec![pdf2text, md2text, any2text, concat];
        let registry = registry_with(caps.clone());
        let bookends: HashSet<MediaUrn> = [
            media("media:ext=pdf"),
            media("media:ext=md"),
            media("media:enc=utf-8;ext=txt"),
        ]
        .into_iter()
        .collect();
        let mut fab = LiveCapFab::new();
        fab.sync_from_caps(&caps, &bookends);
        (fab, registry)
    }

    fn txt() -> MediaUrn {
        media("media:enc=utf-8;ext=txt")
    }

    // TEST1410 (region 1, the property): |S|=1 with the single-source preset is
    // BYTE-IDENTICAL to the historical strand enumeration — same set, same
    // order, same notation.
    #[test]
    fn test1410_single_source_reproduces_historical_strands() {
        let (fab, registry) = fabric();
        let request = PlanRequest::single(
            SourceSpec::single(media("media:ext=pdf")),
            txt(),
            PlanRequest::DEFAULT_MAX_DEPTH,
            PlanRequest::DEFAULT_MAX_PATHS,
        );
        let candidates = fab.plan(&request, &registry).expect("pdf → txt must plan");
        let historical = fab.find_paths_to_exact_target(
            &media("media:ext=pdf"),
            &txt(),
            false,
            PlanRequest::DEFAULT_MAX_DEPTH,
            PlanRequest::DEFAULT_MAX_PATHS,
        );
        assert!(!historical.is_empty(), "the fixture must offer pdf → txt paths");
        assert_eq!(candidates.len(), historical.len());
        for (c, s) in candidates.iter().zip(historical.iter()) {
            let expected = s.to_machine_notation(&registry).expect("strand serializes");
            assert_eq!(
                c.notation, expected,
                "|S|=1 candidate notation must be byte-identical to the historical strand"
            );
        }
        assert!(candidates[0].profile.apexes.is_empty());
        assert!(!candidates[0].profile.diverged);
    }

    // TEST1411 (regions 3+6): heterogeneous pdf+md → txt. The magic pick MUST
    // be a converged plan (decision 2: combining is the inferred intent when a
    // shared apex exists); the independent map is offered as a lower-ranked
    // alternate. The converged candidate's notation must carry a fan-in group
    // (the gather) and resolve through the real machine pipeline.
    #[test]
    fn test1411_heterogeneous_convergence_ranks_first() {
        let (fab, registry) = fabric();
        let request = PlanRequest::auto(
            vec![
                SourceSpec::single(media("media:ext=pdf")),
                SourceSpec::single(media("media:ext=md")),
            ],
            TargetSpec::Exact(vec![txt()]),
        );
        let candidates = fab.plan(&request, &registry).expect("pdf+md → txt must plan");
        assert!(candidates.len() >= 2, "expected converged AND independent candidates");
        let top = &candidates[0];
        assert!(top.profile.converged, "the magic pick must combine, got: {}", top.label);
        assert!(!top.profile.apexes.is_empty(), "a converged plan names its apex");
        // The Collect-apex candidate gathers distinct legs — its notation must
        // carry a fan-in group. (The top candidate may be the generalize plan,
        // a single-source machine with no fan-in — both shapes must be offered.)
        let collect_candidate = candidates
            .iter()
            .find(|c| {
                c.profile
                    .apexes
                    .iter()
                    .any(|a| a.mechanism == ConvergenceMechanism::Collect)
            })
            .expect("a Collect-apex candidate must be offered for pdf+md");
        assert!(
            collect_candidate.notation.contains("("),
            "a gathered convergence serializes as a fan-in group: {}",
            collect_candidate.notation
        );
        // An independent (map) alternate exists further down.
        assert!(
            candidates.iter().any(|c| !c.profile.converged && c.profile.apexes.is_empty()),
            "the independent map must be offered as an alternate"
        );
        // Intent scores are strictly ordered with the ranking.
        for w in candidates.windows(2) {
            assert!(
                w[0].cost.intent_score >= w[1].cost.intent_score,
                "intent ranking must be monotone"
            );
        }
    }

    // TEST1412 (region 5): the denominator ∨ — pdf ∨ md = media:ext (wildcard
    // promotion) — lands on the generic `any2text` cap with ZERO leg
    // transforms; the generalize candidate must exist and outrank the
    // Collect-apex candidate.
    #[test]
    fn test1412_generalize_denominator_wins() {
        let (fab, registry) = fabric();
        let request = PlanRequest::auto(
            vec![
                SourceSpec::single(media("media:ext=pdf")),
                SourceSpec::single(media("media:ext=md")),
            ],
            TargetSpec::Exact(vec![txt()]),
        );
        let candidates = fab.plan(&request, &registry).expect("must plan");
        let generalized: Vec<&PlanCandidate> = candidates
            .iter()
            .filter(|c| {
                c.profile
                    .apexes
                    .iter()
                    .any(|a| a.mechanism == ConvergenceMechanism::Generalize && a.depth == 0)
            })
            .collect();
        assert!(
            !generalized.is_empty(),
            "pdf ∨ md = media:ext must produce a depth-0 generalize candidate via any2text"
        );
        let best_generalized = generalized
            .iter()
            .map(|c| c.rank)
            .min()
            .expect("non-empty");
        let best_collect = candidates
            .iter()
            .filter(|c| {
                c.profile
                    .apexes
                    .iter()
                    .any(|a| a.mechanism == ConvergenceMechanism::Collect)
            })
            .map(|c| c.rank)
            .min();
        if let Some(best_collect) = best_collect {
            assert!(
                best_generalized < best_collect,
                "the free (depth-0) convergence must outrank the paid Collect apex"
            );
        }
    }

    // TEST1413 (fail-hard): Configured + Converged demanded when no shared apex
    // reaches the target ⇒ Unsatisfiable, never a silent partial result.
    #[test]
    fn test1413_configured_converged_unsatisfiable_fails_hard() {
        let (fab, registry) = fabric();
        let mut request = PlanRequest::auto(
            vec![
                SourceSpec::single(media("media:ext=pdf")),
                // No cap consumes audio in this fabric — no leg, no apex. The
                // source deliberately has NO ext tag: with one, pdf ∨ wav =
                // media:ext and the generic any2text would legitimately
                // combine them.
                SourceSpec::single(media("media:audio")),
            ],
            TargetSpec::Exact(vec![txt()]),
        );
        request.mode = PlanMode::Configured;
        request.convergence.presence = ConvergencePresence::Converged;
        let err = fab.plan(&request, &registry).unwrap_err();
        assert!(
            matches!(err, PlanError::Unsatisfiable(_)),
            "demanded convergence with no apex must be Unsatisfiable, got {err:?}"
        );
    }

    // TEST1414 (region 7, divergence): one pdf → txt + a second txt-ish target?
    // The fixture has one bookend tail, so exercise multi-target with txt
    // reached twice is meaningless; instead: pdf → [txt] and pdf → [txt, pdf]?
    // pdf itself is a bookend but unreachable as a TARGET (no cap emits pdf).
    // So divergence uses the two REACHABLE targets txt (via concat) and... only
    // txt is a bookend tail. Divergence is therefore asserted structurally on
    // the shared-prefix machinery instead: two strands to the same target share
    // their full common prefix and still branch.
    #[test]
    fn test1414_discover_multi_source_targets() {
        let (fab, _registry) = fabric();
        let targets = fab
            .discover_convergent_targets(
                &[
                    SourceSpec::single(media("media:ext=pdf")),
                    SourceSpec::single(media("media:ext=md")),
                ],
                PlanRequest::DEFAULT_MAX_DEPTH,
            )
            .expect("discovery must succeed");
        let txt_entry = targets
            .iter()
            .find(|t| t.media_def.is_equivalent(&txt()).unwrap())
            .expect("txt must be discovered for pdf+md");
        assert!(
            txt_entry.convergent,
            "txt is reachable by COMBINING pdf+md (apex page → concat)"
        );
        assert!(txt_entry.apex.is_some(), "a convergent target names its apex");
        // Internal, non-bookend media (page) must never be offered as a target.
        assert!(
            !targets
                .iter()
                .any(|t| t.media_def.is_equivalent(&media("media:enc=utf-8;page")).unwrap()),
            "non-bookend media must not be discovered as targets"
        );
    }

    // TEST1415 (region 6 as the ONLY option): sources that share NO apex but
    // each reach the target independently ⇒ Auto offers the independent map
    // as the top-ranked (only) shape, not an error.
    #[test]
    fn test1415_independent_when_no_convergence() {
        // A fabric with two disjoint pipelines and NO fold cap.
        let a = build_cap(
            "cap:in=\"media:ext=pdf\";to-txt-a;out=\"media:enc=utf-8;ext=txt\"",
            "a",
            &["media:ext=pdf"],
            "media:enc=utf-8;ext=txt",
        );
        let b = build_cap(
            "cap:in=\"media:ext=md\";to-txt-b;out=\"media:enc=utf-8;ext=txt\"",
            "b",
            &["media:ext=md"],
            "media:enc=utf-8;ext=txt",
        );
        let caps = vec![a, b];
        let registry = registry_with(caps.clone());
        let bookends: HashSet<MediaUrn> = [
            media("media:ext=pdf"),
            media("media:ext=md"),
            media("media:enc=utf-8;ext=txt"),
        ]
        .into_iter()
        .collect();
        let mut fab = LiveCapFab::new();
        fab.sync_from_caps(&caps, &bookends);

        let request = PlanRequest::auto(
            vec![
                SourceSpec::single(media("media:ext=pdf")),
                SourceSpec::single(media("media:ext=md")),
            ],
            TargetSpec::Exact(vec![txt()]),
        );
        let candidates = fab.plan(&request, &registry).expect("independent map must plan");
        let top = &candidates[0];
        assert!(
            !top.profile.converged,
            "with no shared apex the map IS the plan, got: {}",
            top.label
        );
        // The map is a multi-strand machine: two disjoint wiring statements.
        assert!(
            top.notation.matches("->").count() >= 4,
            "two strands expected in: {}",
            top.notation
        );
    }

    // TEST1416 (knob honoured): Independent demanded on the convergent fabric
    // returns ONLY non-converged candidates.
    #[test]
    fn test1416_independent_presence_knob_honoured() {
        let (fab, registry) = fabric();
        let mut request = PlanRequest::auto(
            vec![
                SourceSpec::single(media("media:ext=pdf")),
                SourceSpec::single(media("media:ext=md")),
            ],
            TargetSpec::Exact(vec![txt()]),
        );
        request.mode = PlanMode::Configured;
        request.convergence.presence = ConvergencePresence::Independent;
        let candidates = fab.plan(&request, &registry).expect("independent must plan");
        assert!(
            candidates.iter().all(|c| !c.profile.converged),
            "Independent presence must exclude converged candidates"
        );
    }

    // TEST1417 (mechanism knob): Collect demanded excludes the generalize
    // candidate; the surviving top candidate's apex is a Collect apex.
    #[test]
    fn test1417_mechanism_knob_honoured() {
        let (fab, registry) = fabric();
        let mut request = PlanRequest::auto(
            vec![
                SourceSpec::single(media("media:ext=pdf")),
                SourceSpec::single(media("media:ext=md")),
            ],
            TargetSpec::Exact(vec![txt()]),
        );
        request.mode = PlanMode::Configured;
        request.convergence.presence = ConvergencePresence::Converged;
        request.convergence.mechanism = ConvergenceMechanism::Collect;
        let candidates = fab.plan(&request, &registry).expect("collect convergence must plan");
        let top = &candidates[0];
        assert!(top.profile.converged);
        assert!(
            top.profile
                .apexes
                .iter()
                .all(|a| a.mechanism == ConvergenceMechanism::Collect),
            "mechanism=Collect must exclude other apex shapes, got {:?}",
            top.profile.apexes
        );
    }

    // TEST1419 (region 4, product assembly): distinct-typed sources feed the
    // distinct args of one multi-input cap — no homogenization. png claims the
    // image arg directly; pdf's leg homogenizes to page which claims the
    // utf-8 arg. The Merge candidate must exist with the merge cap as apex.
    #[test]
    fn test1419_merge_product_assembly() {
        let pdf2text = build_cap(
            "cap:in=\"media:ext=pdf\";extract;out=\"media:enc=utf-8;page\"",
            "pdf2text",
            &["media:ext=pdf"],
            "media:enc=utf-8;page",
        );
        let compose = build_cap(
            "cap:in=\"media:ext=png;image\";compose;out=\"media:enc=utf-8;ext=txt\"",
            "compose",
            &["media:ext=png;image", "media:enc=utf-8"],
            "media:enc=utf-8;ext=txt",
        );
        let caps = vec![pdf2text, compose];
        let registry = registry_with(caps.clone());
        let bookends: HashSet<MediaUrn> = [
            media("media:ext=pdf"),
            media("media:ext=png;image"),
            media("media:enc=utf-8;ext=txt"),
        ]
        .into_iter()
        .collect();
        let mut fab = LiveCapFab::new();
        fab.sync_from_caps(&caps, &bookends);

        let mut request = PlanRequest::auto(
            vec![
                SourceSpec::single(media("media:ext=png;image")),
                SourceSpec::single(media("media:ext=pdf")),
            ],
            TargetSpec::Exact(vec![media("media:enc=utf-8;ext=txt")]),
        );
        request.mode = PlanMode::Configured;
        request.convergence.presence = ConvergencePresence::Converged;
        request.convergence.mechanism = ConvergenceMechanism::Merge;
        let candidates = fab.plan(&request, &registry).expect("product assembly must plan");
        let top = &candidates[0];
        assert!(top.profile.converged);
        assert!(
            top.profile
                .apexes
                .iter()
                .any(|a| a.mechanism == ConvergenceMechanism::Merge),
            "the product candidate names a Merge apex, got {:?}",
            top.profile.apexes
        );
        assert!(
            top.notation.contains("compose") || top.notation.contains("("),
            "the product wires a fan-in into the multi-input cap: {}",
            top.notation
        );
    }

    // TEST1420 (region 7, divergence): one pdf → two targets. The per-target
    // strands share the pdf2text prefix and branch after it (Auto location =
    // longest common prefix); the candidate is marked diverged.
    #[test]
    fn test1420_single_source_multi_target_diverges() {
        let pdf2text = build_cap(
            "cap:in=\"media:ext=pdf\";extract;out=\"media:enc=utf-8;page\"",
            "pdf2text",
            &["media:ext=pdf"],
            "media:enc=utf-8;page",
        );
        let mut concat = build_cap(
            "cap:in=\"media:enc=utf-8\";concat;out=\"media:enc=utf-8;ext=txt\"",
            "concat",
            &["media:enc=utf-8"],
            "media:enc=utf-8;ext=txt",
        );
        concat.args[0].is_sequence = true;
        let page2html = build_cap(
            "cap:in=\"media:enc=utf-8;page\";render;out=\"media:enc=utf-8;ext=html\"",
            "page2html",
            &["media:enc=utf-8;page"],
            "media:enc=utf-8;ext=html",
        );
        let caps = vec![pdf2text, concat, page2html];
        let registry = registry_with(caps.clone());
        let bookends: HashSet<MediaUrn> = [
            media("media:ext=pdf"),
            media("media:enc=utf-8;ext=txt"),
            media("media:enc=utf-8;ext=html"),
        ]
        .into_iter()
        .collect();
        let mut fab = LiveCapFab::new();
        fab.sync_from_caps(&caps, &bookends);

        let request = PlanRequest::auto(
            vec![SourceSpec::single(media("media:ext=pdf"))],
            TargetSpec::Exact(vec![
                media("media:enc=utf-8;ext=txt"),
                media("media:enc=utf-8;ext=html"),
            ]),
        );
        let candidates = fab.plan(&request, &registry).expect("fan-out must plan");
        let top = &candidates[0];
        assert!(top.profile.diverged, "a multi-target plan is a fan-out");
        assert_eq!(top.profile.target_media.len(), 2);
        // The shared prefix means pdf2text appears ONCE in the notation.
        assert_eq!(
            top.notation.matches("extract").count(),
            1,
            "the common pdf2text prefix must be shared, not duplicated: {}",
            top.notation
        );
    }

    // TEST1418 (region 2 degenerate fold): ONE sequence anchor of pdfs → txt.
    // The plan is single-source; the fold happens because the entry is a
    // sequence and concat consumes it — profile.converged reflects the fold.
    #[test]
    fn test1418_sequence_source_folds() {
        let (fab, registry) = fabric();
        let request = PlanRequest::auto(
            vec![SourceSpec::sequence(media("media:ext=pdf"))],
            TargetSpec::Exact(vec![txt()]),
        );
        let candidates = fab.plan(&request, &registry).expect("pdf-batch → txt must plan");
        assert!(
            candidates.iter().any(|c| c.profile.converged),
            "a sequence source folding through concat must yield a combined-result candidate"
        );
    }
}
