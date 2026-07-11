//! [`CliRuntime`] — the capdag CLI's implementation of [`EngineRuntime`].
//!
//! It hosts cartridges in-process (via [`CartridgeHostRuntime`]) and, exactly like the
//! engine's daemon-hosted runtime, reuses ONE long-lived [`RelaySwitch`] across every
//! segment — including every ForEach body. A cap's cartridge process is therefore
//! spawned once and each body multiplexes onto it, so the cartridge's own
//! `set_capacity(1)` serializes model loads instead of N bodies each loading a fresh
//! copy of the model into GPU memory.
//!
//! Where the cartridges come from — dev binaries, installed cartridges, or bundled
//! providers — is orthogonal to how they are hosted and called; this runtime resolves
//! them lazily on first need and hosts each exactly once (deduped by binary path).
//!
//! Reference-regime specifics vs the engine: terminal output stays in memory (the CLI
//! persists it itself → no writer factory), there is no flow observer, and any ForEach
//! body failure fails the whole plan (failures are exposed, not tolerated). Everything
//! else — the segment orchestration — is the shared `EngineRuntime::run_segment` default.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::bifaci::cartridge_repo::CartridgeChannel;
use crate::bifaci::protocol_trace::ProtocolTraceSink;
use crate::bifaci::relay_switch::RelaySwitch;
use crate::cap::registry::FabricRegistry;
use crate::orchestrator::execute_plan::EngineRuntime;
use crate::orchestrator::executor::{
    discover_bundled_provider_cartridges, segment_activity_timeout, CartridgeManager,
    ExecutionContext,
};
use crate::orchestrator::types::ResolvedGraph;
use crate::ExecutionError;

/// The capdag CLI's cartridge-hosting runtime. Build with [`CliRuntime::new`]; the
/// shared cartridge host is constructed lazily on first segment and torn down when the
/// runtime is dropped.
pub struct CliRuntime {
    cartridge_dir: PathBuf,
    registry_url: Option<String>,
    channel: CartridgeChannel,
    fabric_manifest_version: u32,
    dev_binaries: Vec<PathBuf>,
    bundled_providers_dir: Option<PathBuf>,
    fabric_registry: Arc<FabricRegistry>,
    /// Optional per-segment protocol trace. When set, the shared `run_segment` samples
    /// the switch's L8 snapshot live (250ms) and once at teardown, appending one JSONL
    /// line per sample — the CLI's `--trace` and the scenario harness's
    /// `CAPDAG_SCENARIO_TRACE` wire this. `None` disables tracing.
    trace_sink: Option<Arc<ProtocolTraceSink>>,
    /// The long-lived in-process cartridge host, built on demand. Behind a mutex for
    /// interior mutability under the `&self` trait hooks; the lock is held only while
    /// registering newly-needed cartridges, never across segment execution.
    host: Mutex<CliHost>,
}

/// Lazily-initialised shared host state. `ctx` owns the switch, the host tasks, and
/// their cleanup handles for the runtime's whole lifetime; per-segment execution builds
/// cheap `ExecutionContext::from_switch` clones that share this switch and are dropped
/// (without tearing the host down) at the end of each segment.
struct CliHost {
    ctx: Option<ExecutionContext>,
    manager: Option<CartridgeManager>,
    /// Cap URNs already hosted — the fast path for repeated ForEach bodies of the same
    /// cap, which must NOT re-resolve or re-host anything.
    registered_caps: HashSet<String>,
    /// Cartridge binaries already hosted — a binary serving several caps is hosted once.
    registered_paths: HashSet<PathBuf>,
    /// Bundled providers are discovered and registered exactly once.
    bundled_registered: bool,
}

impl CliRuntime {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cartridge_dir: PathBuf,
        registry_url: Option<String>,
        channel: CartridgeChannel,
        fabric_manifest_version: u32,
        dev_binaries: Vec<PathBuf>,
        bundled_providers_dir: Option<PathBuf>,
        fabric_registry: Arc<FabricRegistry>,
        trace_sink: Option<Arc<ProtocolTraceSink>>,
    ) -> Self {
        Self {
            cartridge_dir,
            registry_url,
            channel,
            fabric_manifest_version,
            dev_binaries,
            bundled_providers_dir,
            fabric_registry,
            trace_sink,
            host: Mutex::new(CliHost {
                ctx: None,
                manager: None,
                registered_caps: HashSet::new(),
                registered_paths: HashSet::new(),
                bundled_registered: false,
            }),
        }
    }
}

impl CliHost {
    /// Ensure every cartridge this segment's caps need is hosted on the shared switch,
    /// building the switch on first use. Idempotent per binary: a cartridge already
    /// hosted (by an earlier segment or another cap of the same binary) is not hosted
    /// again. Fails hard if a cap cannot be resolved to a cartridge — a missing plugin
    /// is exposed, never silently skipped.
    async fn ensure_hosted(
        &mut self,
        graph: &ResolvedGraph,
        rt: &CliRuntime,
    ) -> Result<(), ExecutionError> {
        let cap_urns: Vec<&str> = graph.edges.iter().map(|e| e.cap_urn.as_str()).collect();

        // Fast path: every cap this segment needs is already hosted. This is the hot
        // path for a ForEach — bodies 2..N of the same cap must not re-resolve or
        // re-host; they just reuse the already-spawned cartridge process.
        if self.ctx.is_some() && cap_urns.iter().all(|c| self.registered_caps.contains(*c)) {
            return Ok(());
        }

        if self.manager.is_none() {
            let mut manager = CartridgeManager::new(
                rt.cartridge_dir.clone(),
                rt.registry_url.clone(),
                rt.channel,
                rt.fabric_manifest_version,
                rt.dev_binaries.clone(),
                crate::bifaci::release_cert::RegistryTrust::from_build_constants(),
            );
            manager.init().await?;
            self.manager = Some(manager);
        }

        // Resolve this segment's caps to cartridges (immutable borrow scoped to here).
        let mut resolved = {
            let manager = self.manager.as_ref().expect("initialised immediately above");
            manager.resolve_cartridges(&cap_urns).await?
        };

        // Bundled providers are hosted once, on the first segment, beside the resolved
        // dev/registry cartridges.
        if !self.bundled_registered {
            if let Some(dir) = &rt.bundled_providers_dir {
                resolved.extend(
                    discover_bundled_provider_cartridges(
                        dir,
                        rt.channel,
                        rt.registry_url.as_deref(),
                        rt.fabric_manifest_version,
                    )
                    .await?,
                );
            }
            self.bundled_registered = true;
        }

        // Host only cartridges not already hosted (dedup by binary path).
        resolved.retain(|(path, _, _)| !self.registered_paths.contains(path));

        // Build the bootstrap context on first use. `ExecutionContext::new` creates the
        // switch and starts its background frame pump, so RelayNotify capability updates
        // keep flowing between segments — the reuse invariant depends on it.
        if self.ctx.is_none() {
            self.ctx = Some(ExecutionContext::new(rt.fabric_registry.clone()).await?);
        }

        if !resolved.is_empty() {
            for (path, _, _) in &resolved {
                self.registered_paths.insert(path.clone());
            }
            self.ctx
                .as_mut()
                .expect("bootstrap context ensured above")
                .add_cartridge_host(resolved)
                .await?;
        }

        // Every cap this segment needs is now hosted — record them so the next body of
        // this cap takes the fast path above.
        for c in &cap_urns {
            self.registered_caps.insert((*c).to_string());
        }
        Ok(())
    }
}

#[async_trait]
impl EngineRuntime for CliRuntime {
    async fn segment_switch(
        &self,
        graph: &ResolvedGraph,
    ) -> Result<Arc<RelaySwitch>, ExecutionError> {
        let mut host = self.host.lock().await;
        host.ensure_hosted(graph, self).await?;
        Ok(host
            .ctx
            .as_ref()
            .expect("ensure_hosted guarantees a bootstrap context")
            .switch()
            .clone())
    }

    async fn activity_timeout_secs(
        &self,
        graph: &ResolvedGraph,
    ) -> Result<u64, ExecutionError> {
        Ok(segment_activity_timeout(graph))
    }

    fn trace_sink(&self) -> Option<Arc<ProtocolTraceSink>> {
        self.trace_sink.clone()
    }

    fn fabric_registry(&self) -> Arc<FabricRegistry> {
        self.fabric_registry.clone()
    }

    async fn foreach_partial_failure_policy(&self) -> String {
        // Reference regime: expose failures — any ForEach body failure fails the plan.
        "fail".to_string()
    }
}

impl Drop for CliRuntime {
    /// Tear down the long-lived in-process cartridge host. `ExecutionContext` has no
    /// `Drop`, so aborting its host tasks — which kills the cartridge child processes —
    /// must be explicit here, or every `CliRuntime` (one per CLI run / per scenario
    /// test) would leak its cartridge processes. `get_mut` needs no lock (we hold
    /// `&mut self`), and `shutdown()` is synchronous (it only aborts task handles), so
    /// it is safe to call from `drop`.
    fn drop(&mut self) {
        if let Some(ctx) = self.host.get_mut().ctx.take() {
            let _ = ctx.shutdown();
        }
    }
}
