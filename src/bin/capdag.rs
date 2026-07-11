//! capdag: Machine notation DAG executor for Cap pipelines
//!
//! A unified CLI for executing and validating machine notation pipelines.

use capdag::machine::parse_machine_with_node_names;
use capdag::orchestrator::{
    build_plans_from_notation, execute_plan, parse_machine_to_cap_dag, CliRuntime, EngineRuntime,
};
use capdag::{
    CapProgressFn, CartridgeChannel, ExecutionNodeType, FabricRegistry, PipelineLogFn, StreamMeta,
};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::sync::Arc;

/// Distribution channel of this `capdag` build. Compile-time constant —
/// `MFR_CARTRIDGE_CHANNEL` is set by `dx cartridge build --release` /
/// `--nightly`, which the build wrapper exports for every cargo
/// invocation in the workspace. A release build of the binary can only
/// orchestrate release cartridges, and a nightly build only nightly —
/// channels never cross.
const BUILD_CHANNEL: CartridgeChannel =
    CartridgeChannel::from_build_env(env!("MFR_CARTRIDGE_CHANNEL"));

/// Cartridge registry identity — baked at build time exactly like the
/// engine's (`MFR_CARTRIDGE_REGISTRY_URL` via option_env!): `None` = dev
/// build (dev-bins + bundled providers only; registry downloads are
/// disabled), `Some(url)` = a product build bound to that registry. Never a
/// hardcoded literal — the URL is part of the build identity.
const BAKED_REGISTRY_URL: Option<&str> =
    capdag::registry_url_from_build_env(option_env!("MFR_CARTRIDGE_REGISTRY_URL"));

/// The per-user cartridge install root: `~/.capdag/cartridges`, in the same
/// `{registry_slug}/{channel}/{name}/{version}/` tree every host uses.
fn user_cartridge_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".capdag").join("cartridges")
}

/// Bundled providers shipped beside this CLI binary (the executor's own
/// `providers/` tree, staged by `dx capdag-bundle` with baked content
/// hashes). Present only in a packaged build; absent for a bare `cargo run`.
fn bundled_providers_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("providers")))
        .filter(|dir| dir.is_dir())
}

/// The stderr progress/log hooks shared by every execution mode.
fn progress_hooks() -> (CapProgressFn, PipelineLogFn) {
    let progress: CapProgressFn = Arc::new(|p: f32, cap_urn: &str, msg: &str| {
        eprintln!("  [{:5.1}%] {} {}", p * 100.0, cap_urn, msg);
    });
    let log_fn: PipelineLogFn = Arc::new(
        |cap_urn: &str,
         level: &str,
         message: &str,
         meta: Option<StreamMeta>,
         body_index: Option<usize>| {
            let meta_suffix = match meta.as_ref().and_then(|meta| meta.get("progress")) {
                Some(ciborium::Value::Float(progress)) => {
                    format!(" [meta progress={:.1}%]", progress * 100.0)
                }
                Some(ciborium::Value::Integer(progress)) => {
                    let progress: i128 = (*progress).into();
                    format!(" [meta progress={}]", progress)
                }
                _ => meta
                    .as_ref()
                    .map(|meta| format!(" [meta {:?}]", meta))
                    .unwrap_or_default(),
            };
            match body_index {
                Some(index) => {
                    eprintln!(
                        "  [log:{} body={}]{} {} {}",
                        level, index, meta_suffix, cap_urn, message
                    )
                }
                None => eprintln!("  [log:{}]{} {} {}", level, meta_suffix, cap_urn, message),
            }
        },
    );
    (progress, log_fn)
}

/// Expand dev binary path - supports single file or directory of executables
fn expand_dev_binary_path(path: &str) -> Vec<PathBuf> {
    let path_buf = PathBuf::from(path);

    if path_buf.is_file() {
        vec![path_buf]
    } else if path_buf.is_dir() {
        // Find all executable files in directory
        match fs::read_dir(&path_buf) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| is_executable_file(p))
                .collect(),
            Err(e) => {
                eprintln!("Error reading dev-bins directory '{}': {}", path, e);
                vec![]
            }
        }
    } else {
        eprintln!("Dev binary path does not exist: {}", path);
        vec![]
    }
}

#[cfg(unix)]
fn is_executable_file(path: &PathBuf) -> bool {
    use std::os::unix::fs::PermissionsExt;

    if !path.is_file() {
        return false;
    }
    match path.metadata() {
        Ok(meta) => meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

#[cfg(windows)]
fn is_executable_file(path: &PathBuf) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
}

#[cfg(not(any(unix, windows)))]
fn is_executable_file(_path: &PathBuf) -> bool {
    false
}

/// Find input nodes in the machine notation (root sources with no incoming edges).
///
/// Parses the machine notation into a `Machine` (alongside the
/// per-strand `name → NodeId` map) and returns the user-written
/// node names of every input anchor across all strands. The
/// resolver computes the input anchors as part of the resolved
/// `MachineStrand`; we just translate the NodeIds back to the
/// names the user wrote.
fn find_input_nodes(notation: &str, registry: &FabricRegistry) -> Vec<String> {
    let (machine, strand_node_names) = match parse_machine_with_node_names(notation, registry) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!(
                "Failed to parse machine notation for input node detection: {}",
                e
            );
            return vec![];
        }
    };

    let mut seen = std::collections::HashSet::new();
    let mut inputs: Vec<String> = Vec::new();
    for (strand, name_to_id) in machine.strands().iter().zip(strand_node_names.iter()) {
        // Invert name → NodeId so we can label each input
        // anchor with its user-written name.
        let mut id_to_name: HashMap<u32, String> = HashMap::with_capacity(name_to_id.len());
        for (name, id) in name_to_id {
            id_to_name.insert(*id, name.clone());
        }
        for anchor_id in strand.input_anchor_ids() {
            if let Some(name) = id_to_name.get(anchor_id) {
                if seen.insert(name.clone()) {
                    inputs.push(name.clone());
                }
            }
        }
    }
    inputs
}

/// File extensions to skip when expanding directories
const SKIP_EXTENSIONS: &[&str] = &[
    "json", "log", "txt", "md", "yml", "yaml", "toml", "sh", "py", "rb", "js", "ts", "rs", "go",
    "c", "h", "cpp", "zip", "tar", "gz", "bz2", "xz",
];

/// Files to always skip
const SKIP_FILES: &[&str] = &[".DS_Store", "Thumbs.db", ".gitignore", ".gitkeep"];

/// Check if a file should be included based on extension/name
fn should_include_file(path: &PathBuf) -> bool {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    // Skip hidden files and known skip files
    if filename.starts_with('.') || SKIP_FILES.contains(&filename) {
        return false;
    }

    // Skip directories
    if path.is_dir() {
        return false;
    }

    // Skip known non-content extensions
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if SKIP_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
            return false;
        }
    }

    true
}

/// Expand input path to list of files
/// Supports: single file, directory, glob pattern
fn expand_input_path(path: &str) -> Vec<PathBuf> {
    let path_buf = PathBuf::from(path);

    // Check if it's a glob pattern (contains * or ?)
    if path.contains('*') || path.contains('?') {
        match glob::glob(path) {
            Ok(entries) => {
                let files: Vec<PathBuf> = entries
                    .filter_map(|e| e.ok())
                    .filter(|p| p.is_file())
                    .collect();
                if files.is_empty() {
                    eprintln!("No files matched glob pattern '{}'", path);
                }
                files
            }
            Err(e) => {
                eprintln!("Error parsing glob pattern '{}': {}", path, e);
                vec![]
            }
        }
    } else if path_buf.is_dir() {
        // Directory: list content files (non-recursive), filtering out non-content
        match fs::read_dir(&path_buf) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| should_include_file(p))
                .collect(),
            Err(e) => {
                eprintln!("Error reading directory '{}': {}", path, e);
                vec![]
            }
        }
    } else if path_buf.is_file() {
        vec![path_buf]
    } else {
        eprintln!("Path does not exist: {}", path);
        vec![]
    }
}

fn print_usage(program: &str) {
    eprintln!(
        "Usage:\n\
           {p} <cap-alias-or-urn> [cap args] [inputs...] [options]   Run one cap\n\
           {p} run <machine-file> [inputs...] [options]              Run a .machine file\n\
           {p} resolve <cap-alias-or-urn>                            Show the providing cartridge(s)\n\
           {p} install <cap-alias-or-urn-or-cartridge-id>            Download + verify without running\n\n\
         Single-cap mode drives the cap's OWN declared interface — exactly like\n\
         invoking the cartridge directly, except the cap runs inside a full bifaci\n\
         host with the bundled providers (data/fetch/model cartridges) registered,\n\
         so peer calls (e.g. model downloads) work:\n\
           - piped stdin, or input file paths, feed the cap's stdin arg\n\
           - the cap's declared --flags and positional args are accepted natively\n\
           - --arg <flag-or-media-urn>=<value> addresses any arg explicitly\n\
             (value form @<path> reads the file's bytes)\n\n\
         Output (pipe discipline): a single scalar result streams RAW to stdout;\n\
         sequences and fan-outs write files (named {{input}}.{{node}}[.{{i}}].{{ext}})\n\
         and list their paths on stdout. Logs/progress go to stderr.\n\n\
         Options:\n\
           -o, --output <dir>       Write result files into <dir> (default: cwd)\n\
           --force                  Overwrite existing output files\n\
           --arg <name>=<value>     Explicit cap argument (repeatable; single-cap mode)\n\
           --values <file.json>     Argument values per node (run mode)\n\
           --gen-values             Output a values JSON template and exit (run mode)\n\
           --mermaid                Output Mermaid diagram code and exit (run mode)\n\
           --dev-bins <binary> ...  Use local cartridge binaries\n\
           --trace <file.trace>     Write a per-segment bifaci protocol trace (JSONL)\n\
           --help                   Show this help\n\n\
         Utility subcommand: hash-cartridge-dir.\n\n\
         Examples:\n\
           {p} pdf2summary report.pdf\n\
           cat report.pdf | {p} pdf2summary > summary.txt\n\
           {p} disbind-pdf --index-range 1-5 report.pdf -o pages/\n\
           {p} run pipeline.machine /tmp/pdfs/",
        p = program
    );
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage(&args[0]);
        process::exit(1);
    }

    // `hash-cartridge-dir <dir>` — print the deterministic content hash of a
    // cartridge version directory and exit. This is the SINGLE source of truth
    // for cartridge-directory hashing: the bundle build scripts
    // (build-engine-bundle.sh/.ps1) call this to compute the bundled-provider
    // hashes they bake into the engine via MFR_BUNDLED_PROVIDER_HASHES, so the
    // build-time hash is byte-identical to what the engine's discovery computes
    // at runtime (capdag::hash_cartridge_directory). Never reimplement the walk
    // in bash/pwsh — it would silently drift.
    if args[1] == "hash-cartridge-dir" {
        let Some(dir) = args.get(2) else {
            eprintln!("Usage: {} hash-cartridge-dir <version-dir>", args[0]);
            process::exit(2);
        };
        match capdag::hash_cartridge_directory(std::path::Path::new(dir)) {
            Ok(hash) => {
                println!("{hash}");
                process::exit(0);
            }
            Err(e) => {
                eprintln!("hash-cartridge-dir: failed to hash '{dir}': {e}");
                process::exit(1);
            }
        }
    }

    // ── Dispatch ───────────────────────────────────────────────────────────
    // Reserved subcommands, then: a `.machine` first token is a usage error
    // pointing at `run` (no silent dispatch), an option-like token is a usage
    // error, anything else is SINGLE-CAP MODE (alias or cap URN).
    match args[1].as_str() {
        "run" => cmd_run(&args).await,
        "resolve" => cmd_resolve(&args).await,
        "install" => cmd_install(&args).await,
        "--help" | "-h" | "help" => {
            print_usage(&args[0]);
            process::exit(0);
        }
        token if token.ends_with(".machine") => {
            eprintln!(
                "'{token}' is a machine file — run it with: {} run {token} [inputs...]",
                args[0]
            );
            process::exit(2);
        }
        token if token.starts_with('-') => {
            eprintln!("Unknown option '{token}'.");
            print_usage(&args[0]);
            process::exit(2);
        }
        _ => cmd_cap(&args).await,
    }
}

/// `capdag run <machine-file> [inputs…]` — execute a .machine pipeline.
async fn cmd_run(args: &[String]) -> ! {
    // Parse arguments
    let mut dev_binaries = Vec::new();
    let mut mermaid_mode = false;
    let mut gen_values_mode = false;
    let mut values_file: Option<String> = None;
    let mut trace_file: Option<String> = None;
    let mut output_dir: Option<PathBuf> = None;
    let mut force_overwrite = false;
    let mut arg_idx = 2;

    // Parse flags
    while arg_idx < args.len() {
        match args[arg_idx].as_str() {
            "--help" | "-h" => {
                print_usage(&args[0]);
                process::exit(0);
            }
            "--mermaid" => {
                mermaid_mode = true;
                arg_idx += 1;
            }
            "--gen-values" => {
                gen_values_mode = true;
                arg_idx += 1;
            }
            "--values" => {
                arg_idx += 1;
                if arg_idx >= args.len() {
                    eprintln!("--values requires a JSON file path");
                    process::exit(1);
                }
                values_file = Some(args[arg_idx].clone());
                arg_idx += 1;
            }
            "--trace" => {
                arg_idx += 1;
                if arg_idx >= args.len() {
                    eprintln!("--trace requires a file path");
                    process::exit(1);
                }
                trace_file = Some(args[arg_idx].clone());
                arg_idx += 1;
            }
            "-o" | "--output" => {
                arg_idx += 1;
                if arg_idx >= args.len() {
                    eprintln!("--output requires a directory path");
                    process::exit(1);
                }
                output_dir = Some(PathBuf::from(&args[arg_idx]));
                arg_idx += 1;
            }
            "--force" => {
                force_overwrite = true;
                arg_idx += 1;
            }
            "--dev-bins" => {
                arg_idx += 1;
                while arg_idx < args.len()
                    && !args[arg_idx].starts_with("--")
                    && !args[arg_idx].ends_with(".machine")
                {
                    let expanded = expand_dev_binary_path(&args[arg_idx]);
                    if expanded.is_empty() {
                        eprintln!("No executables found in: {}", args[arg_idx]);
                        process::exit(1);
                    }
                    dev_binaries.extend(expanded);
                    arg_idx += 1;
                }
            }
            _ => break,
        }
    }

    if arg_idx >= args.len() {
        eprintln!("Missing machine file argument");
        print_usage(&args[0]);
        process::exit(1);
    }

    let machine_file = &args[arg_idx];
    arg_idx += 1;

    // Read machine file
    let notation = match fs::read_to_string(machine_file) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading machine file '{}': {}", machine_file, e);
            process::exit(1);
        }
    };

    // Create the unified FabricRegistry. Holds cap definitions and media defs
    // together; consumed by `build_plans_from_notation` (for resolution) and the
    // runtime (for cap lookup and adapter dispatch during execution).
    let registry = match FabricRegistry::new().await {
        Ok(reg) => Arc::new(reg),
        Err(e) => {
            eprintln!("Error creating FabricRegistry: {}", e);
            process::exit(1);
        }
    };

    // --mermaid: render the resolved DAG as a diagram and exit. This is a flat
    // visualization (parse_machine_to_cap_dag), not an execution path — execution
    // runs through the ForEach/Collect-aware planner below.
    if mermaid_mode {
        match parse_machine_to_cap_dag(&notation, registry.as_ref()).await {
            Ok(graph) => {
                println!("{}", graph.to_mermaid());
                process::exit(0);
            }
            Err(e) => {
                eprintln!("Validation failed: {}", e);
                process::exit(1);
            }
        }
    }

    // Build execution plans through the single ForEach/Collect-aware front-end — the
    // same planner path the engine runs. One plan per connected strand.
    let plans = match build_plans_from_notation(&notation, registry.clone()).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Validation failed: {}", e);
            process::exit(1);
        }
    };

    // --gen-values: emit a values JSON template and exit. For each cap step, list the
    // non-stdin args (the ones no data-flow edge can supply — model-spec, budgets, …)
    // keyed by plan node id → arg media URN → default. These keys are exactly what
    // `--values` feeds back into the executor as extra-arg streams.
    if gen_values_mode {
        let mut template: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
        for plan in &plans {
            for (node_id, node) in &plan.nodes {
                let Some(cap_urn) = node.cap_urn() else {
                    continue;
                };
                let cap = match registry.get_cap(cap_urn).await {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Error resolving cap '{}': {}", cap_urn, e);
                        process::exit(1);
                    }
                };
                let mut node_args = serde_json::Map::new();
                for arg in cap.get_args() {
                    let has_stdin = arg
                        .sources
                        .iter()
                        .any(|s| matches!(s, capdag::cap::definition::ArgSource::Stdin { .. }));
                    if has_stdin {
                        continue;
                    }
                    let value = arg.default_value.clone().unwrap_or(serde_json::Value::Null);
                    node_args.insert(arg.media_urn.clone(), value);
                }
                if !node_args.is_empty() {
                    template.insert(node_id.clone(), serde_json::Value::Object(node_args));
                }
            }
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::Value::Object(template))
                .expect("JSON serialization cannot fail for this structure")
        );
        process::exit(0);
    }

    // Find input nodes automatically
    let input_nodes = find_input_nodes(&notation, registry.as_ref());
    if input_nodes.is_empty() {
        eprintln!("No input nodes found in machine notation");
        process::exit(1);
    }

    // Collect all input paths and expand them
    let mut all_files: Vec<PathBuf> = Vec::new();
    for arg in &args[arg_idx..] {
        let expanded = expand_input_path(arg);
        all_files.extend(expanded);
    }

    if all_files.is_empty() {
        eprintln!("No input files found");
        process::exit(1);
    }

    // Sort files for consistent ordering
    all_files.sort();

    eprintln!("=== capdag: Machine Notation Execution ===\n");
    eprintln!("Machine file: {}", machine_file);
    eprintln!("Input node(s): {}", input_nodes.join(", "));
    eprintln!("Strands (plans): {}", plans.len());
    eprintln!("Input files: {}", all_files.len());
    for f in &all_files {
        eprintln!("  - {}", f.display());
    }

    let cartridge_dir = user_cartridge_dir();

    let registry_url: Option<String> = BAKED_REGISTRY_URL.map(str::to_string);

    let bundled_providers_dir = bundled_providers_dir();

    // Load argument values file
    let node_values: HashMap<String, HashMap<String, serde_json::Value>> =
        if let Some(ref vf) = values_file {
            match fs::read_to_string(vf) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Error parsing values file '{}': {}", vf, e);
                        process::exit(1);
                    }
                },
                Err(e) => {
                    eprintln!("Error reading values file '{}': {}", vf, e);
                    process::exit(1);
                }
            }
        } else {
            HashMap::new()
        };

    // The executor speaks `cap_arguments` (raw arg-stream bytes) — its single argument
    // format. Serialize the JSON values file: a string arg is its own UTF-8 bytes,
    // anything else is its JSON encoding.
    let cap_arguments: HashMap<String, Vec<(String, Vec<u8>)>> = node_values
        .iter()
        .map(|(node, args)| {
            let pairs = args
                .iter()
                .map(|(media, value)| {
                    let bytes = match value {
                        serde_json::Value::String(s) => s.as_bytes().to_vec(),
                        other => serde_json::to_vec(other).unwrap_or_else(|e| {
                            eprintln!("Error serializing value for arg '{}': {}", media, e);
                            process::exit(1);
                        }),
                    };
                    (media.clone(), bytes)
                })
                .collect();
            (node.clone(), pairs)
        })
        .collect();

    eprintln!("\n=== Executing ===\n");
    if !dev_binaries.is_empty() {
        eprintln!("Dev mode: {} local binaries", dev_binaries.len());
        for bin in &dev_binaries {
            eprintln!("  - {}", bin.display());
        }
    }
    if !node_values.is_empty() {
        eprintln!("Values: {} node(s) configured", node_values.len());
    }

    // --trace: open the per-segment protocol-trace sink up front. A trace the
    // user asked for that cannot be opened is a hard error — fail before running
    // rather than discover it segment by segment.
    let trace_sink: Option<Arc<capdag::ProtocolTraceSink>> = match &trace_file {
        Some(path) => match capdag::ProtocolTraceSink::open(path).await {
            Ok(sink) => {
                eprintln!("Protocol trace: {}", path);
                Some(sink)
            }
            Err(e) => {
                eprintln!("Error opening protocol trace file '{}': {}", path, e);
                process::exit(1);
            }
        },
        None => None,
    };

    // The CLI runtime: hosts cartridges in-process on ONE reused relay switch (a cap's
    // cartridge is spawned once and every ForEach body multiplexes onto it, like the
    // engine), keeps output in memory, and fails hard on any ForEach body failure.
    // execute_plan drives the ForEach/Collect decomposition on top of it.
    let runtime: Arc<dyn EngineRuntime> = Arc::new(CliRuntime::new(
        cartridge_dir.clone(),
        registry_url.clone(),
        BUILD_CHANNEL,
        capdag::FABRIC_MANIFEST_VERSION,
        dev_binaries.clone(),
        bundled_providers_dir.clone(),
        registry.clone(),
        trace_sink,
    ));

    let (progress, log_fn) = progress_hooks();

    // Process each file
    let mut success_count = 0;
    let mut error_count = 0;

    for file in &all_files {
        eprintln!("--- Processing: {} ---", file.display());
        eprintln!("Run: {}", notation);

        // The CLI feeds a single file as a scalar blob into each plan's single input
        // slot. A ForEach inside the strand is driven by an intermediate cap's
        // sequence output, never by this input.
        let file_bytes = match fs::read(file) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Error reading input file '{}': {}", file.display(), e);
                error_count += 1;
                continue;
            }
        };

        // Each connected strand is its own plan; run them all against this file.
        let mut file_failed = false;
        for (idx, plan) in plans.iter().enumerate() {
            // The plan's single input slot receives the file. A strand with more than
            // one input anchor cannot be driven by one CLI file — fail hard rather than
            // guess which input gets the data.
            let input_slots: Vec<&String> = plan
                .nodes
                .iter()
                .filter(|(_, n)| matches!(n.node_type, ExecutionNodeType::InputSlot { .. }))
                .map(|(id, _)| id)
                .collect();
            let input_slot_id = match input_slots.as_slice() {
                [single] => (*single).clone(),
                other => {
                    eprintln!(
                        "strand {idx} has {} input anchors — a single CLI file drives a \
                         single-input machine only (inputs: {:?})",
                        other.len(),
                        other
                    );
                    file_failed = true;
                    continue;
                }
            };
            let mut initial_inputs: HashMap<String, Vec<u8>> = HashMap::new();
            initial_inputs.insert(input_slot_id.clone(), file_bytes.clone());
            // The executor requires every input node to carry an explicit sequence
            // flag — a default would hide a wiring mismatch. This one is scalar.
            let mut initial_is_sequence: HashMap<String, bool> = HashMap::new();
            initial_is_sequence.insert(input_slot_id, false);

            match execute_plan(
                plan,
                runtime.clone(),
                initial_inputs,
                initial_is_sequence,
                &cap_arguments,
                Some(&progress),
                None,
                Some(&log_fn),
                None,
                None,
            )
            .await
            {
                Ok(result) => {
                    // Real output emission (pipe discipline; see cli_output).
                    // The stdout fast-path only applies when this execution
                    // can produce exactly one scalar result overall — with
                    // several strands or several input files, force file
                    // mode so results never interleave on stdout.
                    let effective_dir = if plans.len() > 1 || all_files.len() > 1 {
                        Some(output_dir.clone().unwrap_or_else(|| PathBuf::from(".")))
                    } else {
                        output_dir.clone()
                    };
                    let stem = file
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "input".to_string());
                    let options = capdag::orchestrator::EmitOptions {
                        output_dir: effective_dir,
                        force: force_overwrite,
                        input_stem: if plans.len() > 1 {
                            format!("{stem}.strand{idx}")
                        } else {
                            stem
                        },
                    };
                    let mut stdout = std::io::stdout();
                    if let Err(e) =
                        capdag::orchestrator::emit_terminals(&result, &options, &mut stdout)
                    {
                        eprintln!("{e}");
                        file_failed = true;
                    }
                }
                Err(e) => {
                    eprintln!("{}", e);
                    file_failed = true;
                }
            }
        }

        if file_failed {
            error_count += 1;
        } else {
            success_count += 1;
        }
    }

    eprintln!("=== Summary ===");
    eprintln!("Processed: {}", all_files.len());
    eprintln!("Success: {}", success_count);
    eprintln!("Errors: {}", error_count);

    if error_count > 0 {
        process::exit(1);
    }
    process::exit(0);
}

/// Build the FabricRegistry or exit with the error.
async fn fabric_registry_or_exit() -> Arc<FabricRegistry> {
    match FabricRegistry::new().await {
        Ok(reg) => Arc::new(reg),
        Err(e) => {
            eprintln!("Error creating FabricRegistry: {}", e);
            process::exit(1);
        }
    }
}

/// Resolve a cap token (alias or URN) to a `Cap` definition, or exit.
async fn resolve_cap_or_exit(registry: &FabricRegistry, token: &str) -> capdag::Cap {
    let cap_ref = match capdag::orchestrator::classify_cap_token(token) {
        Ok(capdag::orchestrator::CapToken::Urn(urn)) => urn,
        Ok(capdag::orchestrator::CapToken::Alias(alias)) => alias,
        Err(e) => {
            eprintln!("{e}");
            process::exit(2);
        }
    };
    // `get_cap` accepts both forms: an alias resolves at its typed boundary
    // (a media alias fails hard), a URN resolves against the pinned
    // manifest.
    match registry.get_cap(&cap_ref).await {
        Ok(cap) => cap,
        Err(e) => {
            eprintln!("Error resolving cap '{token}': {e}");
            process::exit(1);
        }
    }
}

/// A `CartridgeManager` bound to the baked registry + trust, initialized
/// (manifest synced + chain-verified), or exit.
async fn registry_manager_or_exit(dev_binaries: Vec<PathBuf>) -> capdag::orchestrator::CartridgeManager {
    let mut manager = capdag::orchestrator::CartridgeManager::new(
        user_cartridge_dir(),
        BAKED_REGISTRY_URL.map(str::to_string),
        BUILD_CHANNEL,
        capdag::FABRIC_MANIFEST_VERSION,
        dev_binaries,
        capdag::RegistryTrust::from_build_constants(),
    );
    if let Err(e) = manager.init().await {
        eprintln!("{e}");
        process::exit(1);
    }
    manager
}

/// `capdag <cap-alias-or-urn> [cap args] [inputs…]` — single-cap mode.
///
/// The invocation surface is the cap's OWN declared interface (piped stdin,
/// native flags, positional args — exactly as when the cartridge is invoked
/// directly), but execution runs inside a full bifaci host: the providing
/// cartridge is resolved from the signed registry (downloaded + verified if
/// missing) and hosted on the shared switch BESIDE the bundled providers, so
/// peer calls (e.g. an ML cap peer-invoking modelcartridge's download-model)
/// route exactly as they do in the engine and the scenario harness.
async fn cmd_cap(args: &[String]) -> ! {
    let cap_token = &args[1];

    // Split the remaining tokens: options reserved by the CLI itself are
    // consumed here; EVERYTHING else — the cap's own flags and positional
    // values, and input paths — goes to the cap-invocation mapper. A cap
    // flag that collides with a reserved name is addressed via
    // `--arg <media-urn>=<value>`.
    let mut cap_tokens: Vec<String> = Vec::new();
    let mut explicit_pairs: Vec<(String, String)> = Vec::new();
    let mut output_dir: Option<PathBuf> = None;
    let mut force_overwrite = false;
    let mut dev_binaries: Vec<PathBuf> = Vec::new();
    let mut trace_file: Option<String> = None;
    let mut idx = 2usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--help" | "-h" => {
                print_usage(&args[0]);
                process::exit(0);
            }
            "-o" | "--output" => {
                idx += 1;
                let Some(dir) = args.get(idx) else {
                    eprintln!("--output requires a directory path");
                    process::exit(2);
                };
                output_dir = Some(PathBuf::from(dir));
            }
            "--force" => force_overwrite = true,
            "--arg" => {
                idx += 1;
                let Some(pair) = args.get(idx) else {
                    eprintln!("--arg requires <name-or-media-urn>=<value>");
                    process::exit(2);
                };
                let Some((name, value)) = pair.split_once('=') else {
                    eprintln!("--arg '{pair}' is not of the form <name>=<value>");
                    process::exit(2);
                };
                explicit_pairs.push((name.to_string(), value.to_string()));
            }
            "--trace" => {
                idx += 1;
                let Some(path) = args.get(idx) else {
                    eprintln!("--trace requires a file path");
                    process::exit(2);
                };
                trace_file = Some(path.clone());
            }
            "--dev-bins" => {
                idx += 1;
                while idx < args.len() && !args[idx].starts_with('-') {
                    let expanded = expand_dev_binary_path(&args[idx]);
                    if expanded.is_empty() {
                        eprintln!("No executables found in: {}", args[idx]);
                        process::exit(1);
                    }
                    dev_binaries.extend(expanded);
                    idx += 1;
                }
                continue;
            }
            other => cap_tokens.push(other.to_string()),
        }
        idx += 1;
    }

    let registry = fabric_registry_or_exit().await;
    let cap = resolve_cap_or_exit(&registry, cap_token).await;

    // The cap's declared interface, applied to the tokens.
    let notation = match capdag::orchestrator::synthesize_single_cap_notation(&cap) {
        Ok(notation) => notation,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };
    let invocation = match capdag::orchestrator::map_invocation(&cap, &cap_tokens, &explicit_pairs)
    {
        Ok(invocation) => invocation,
        Err(e) => {
            eprintln!("{e}");
            process::exit(2);
        }
    };

    // Inputs: file paths from the invocation, else piped stdin, else usage.
    enum InputSource {
        Files(Vec<PathBuf>),
        Stdin(Vec<u8>),
    }
    let inputs = if invocation.input_paths.is_empty() {
        if atty::is(atty::Stream::Stdin) {
            eprintln!(
                "cap {} needs input: pipe it in (cat doc.pdf | {} {cap_token}) or pass \
                 file path(s).",
                cap.urn, args[0]
            );
            process::exit(2);
        }
        let mut bytes = Vec::new();
        use std::io::Read;
        if let Err(e) = std::io::stdin().read_to_end(&mut bytes) {
            eprintln!("failed to read stdin: {e}");
            process::exit(1);
        }
        if bytes.is_empty() {
            eprintln!("stdin was empty — nothing to run the cap on");
            process::exit(2);
        }
        InputSource::Stdin(bytes)
    } else {
        let mut files: Vec<PathBuf> = Vec::new();
        for path in &invocation.input_paths {
            let expanded = expand_input_path(path);
            if expanded.is_empty() {
                eprintln!("No input files found at '{path}'");
                process::exit(1);
            }
            files.extend(expanded);
        }
        files.sort();
        InputSource::Files(files)
    };

    // Build the plan through the same planner front-end as every other mode.
    let plans = match build_plans_from_notation(&notation, registry.clone()).await {
        Ok(plans) => plans,
        Err(e) => {
            eprintln!("Failed to plan cap execution: {e}");
            process::exit(1);
        }
    };
    let [plan] = plans.as_slice() else {
        eprintln!(
            "internal error: single-cap notation produced {} plans (expected 1)",
            plans.len()
        );
        process::exit(1);
    };
    let input_slot_id = {
        let slots: Vec<&String> = plan
            .nodes
            .iter()
            .filter(|(_, n)| matches!(n.node_type, ExecutionNodeType::InputSlot { .. }))
            .map(|(id, _)| id)
            .collect();
        match slots.as_slice() {
            [single] => (*single).clone(),
            other => {
                eprintln!(
                    "internal error: single-cap plan has {} input slots (expected 1)",
                    other.len()
                );
                process::exit(1);
            }
        }
    };

    // Cap arguments land on the edge's destination node.
    let mut cap_arguments: HashMap<String, Vec<(String, Vec<u8>)>> = HashMap::new();
    if !invocation.cap_arguments.is_empty() {
        cap_arguments.insert(
            capdag::orchestrator::SINGLE_CAP_OUTPUT_NODE.to_string(),
            invocation.cap_arguments.clone(),
        );
    }

    let trace_sink: Option<Arc<capdag::ProtocolTraceSink>> = match &trace_file {
        Some(path) => match capdag::ProtocolTraceSink::open(path).await {
            Ok(sink) => Some(sink),
            Err(e) => {
                eprintln!("Error opening protocol trace file '{}': {}", path, e);
                process::exit(1);
            }
        },
        None => None,
    };

    let runtime: Arc<dyn EngineRuntime> = Arc::new(CliRuntime::new(
        user_cartridge_dir(),
        BAKED_REGISTRY_URL.map(str::to_string),
        BUILD_CHANNEL,
        capdag::FABRIC_MANIFEST_VERSION,
        dev_binaries,
        bundled_providers_dir(),
        registry.clone(),
        trace_sink,
    ));
    let (progress, log_fn) = progress_hooks();

    // One run per input (stdin = a single run).
    let runs: Vec<(String, Vec<u8>)> = match inputs {
        InputSource::Stdin(bytes) => vec![("stdin".to_string(), bytes)],
        InputSource::Files(files) => {
            let mut runs = Vec::with_capacity(files.len());
            for file in files {
                let stem = file
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "input".to_string());
                match fs::read(&file) {
                    Ok(bytes) => runs.push((stem, bytes)),
                    Err(e) => {
                        eprintln!("Error reading input file '{}': {}", file.display(), e);
                        process::exit(1);
                    }
                }
            }
            runs
        }
    };
    let multi_run = runs.len() > 1;

    let mut error_count = 0usize;
    for (stem, bytes) in runs {
        let mut initial_inputs: HashMap<String, Vec<u8>> = HashMap::new();
        initial_inputs.insert(input_slot_id.clone(), bytes);
        let mut initial_is_sequence: HashMap<String, bool> = HashMap::new();
        initial_is_sequence.insert(input_slot_id.clone(), false);

        match execute_plan(
            plan,
            runtime.clone(),
            initial_inputs,
            initial_is_sequence,
            &cap_arguments,
            Some(&progress),
            None,
            Some(&log_fn),
            None,
            None,
        )
        .await
        {
            Ok(result) => {
                // Several inputs must never interleave raw results on
                // stdout — force file mode.
                let effective_dir = if multi_run {
                    Some(output_dir.clone().unwrap_or_else(|| PathBuf::from(".")))
                } else {
                    output_dir.clone()
                };
                let options = capdag::orchestrator::EmitOptions {
                    output_dir: effective_dir,
                    force: force_overwrite,
                    input_stem: stem,
                };
                let mut stdout = std::io::stdout();
                if let Err(e) =
                    capdag::orchestrator::emit_terminals(&result, &options, &mut stdout)
                {
                    eprintln!("{e}");
                    error_count += 1;
                }
            }
            Err(e) => {
                eprintln!("{e}");
                error_count += 1;
            }
        }
    }

    process::exit(if error_count > 0 { 1 } else { 0 });
}

/// `capdag resolve <cap-alias-or-urn>` — resolve a cap and show which
/// registry cartridge(s) provide it, without downloading anything.
async fn cmd_resolve(args: &[String]) -> ! {
    let Some(token) = args.get(2) else {
        eprintln!("Usage: {} resolve <cap-alias-or-urn>", args[0]);
        process::exit(2);
    };
    let registry = fabric_registry_or_exit().await;
    let cap = resolve_cap_or_exit(&registry, token).await;
    println!("{}", cap.urn);

    let manager = registry_manager_or_exit(Vec::new()).await;
    let suggestions = manager.suggestions_for_cap(&cap.urn.to_string()).await;
    if suggestions.is_empty() {
        eprintln!(
            "No registry cartridge provides this cap{}.",
            if BAKED_REGISTRY_URL.is_none() {
                " (dev build: no cartridge registry baked)"
            } else {
                ""
            }
        );
        process::exit(1);
    }
    for suggestion in &suggestions {
        let detail = manager.registry_cartridge(&suggestion.cartridge_id).await;
        match detail {
            Some(info) => {
                let platform = capdag::host_platform();
                let build = info.build_for_platform(&platform);
                let binary_state = match build {
                    Some(build) if build.binary.is_some() => "signed binary available",
                    Some(_) => "NO signed binary (installer-only publish — not runnable via capdag)",
                    None => "no build for this platform",
                };
                println!(
                    "  {} v{} [{}] — {}",
                    suggestion.cartridge_id, info.version, platform, binary_state
                );
            }
            None => println!("  {} (not in this channel's registry view)", suggestion.cartridge_id),
        }
    }
    process::exit(0);
}

/// `capdag install <cap-alias-or-urn-or-cartridge-id>` — resolve, download,
/// and VERIFY a cartridge without executing anything (CI cache warm-up).
async fn cmd_install(args: &[String]) -> ! {
    let Some(token) = args.get(2) else {
        eprintln!(
            "Usage: {} install <cap-alias-or-urn-or-cartridge-id>",
            args[0]
        );
        process::exit(2);
    };
    let manager = registry_manager_or_exit(Vec::new()).await;

    // A token with ':' is a cap URN; a bare token could be an alias OR a
    // cartridge id — try the registry's cartridge ids first (exact), then
    // the fabric alias route.
    let cartridge_id: String = if token.contains(':') || manager.registry_cartridge(token).await.is_none() {
        let registry = fabric_registry_or_exit().await;
        let cap = resolve_cap_or_exit(&registry, token).await;
        let suggestions = manager.suggestions_for_cap(&cap.urn.to_string()).await;
        let Some(first) = suggestions.first() else {
            eprintln!("No registry cartridge provides cap {}", cap.urn);
            process::exit(1);
        };
        first.cartridge_id.clone()
    } else {
        token.clone()
    };

    match manager.get_cartridge_path(&cartridge_id).await {
        Ok(path) => {
            eprintln!("Installed and verified: {cartridge_id}");
            println!("{}", path.display());
            process::exit(0);
        }
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    }
}
