//! capdag: Machine notation DAG executor for Cap pipelines
//!
//! A unified CLI for executing and validating machine notation pipelines.

use capdag::machine::parse_machine_with_node_names;
use capdag::orchestrator::{
    build_plans_from_notation, execute_plan, parse_machine_to_cap_dag, DevBinRuntime, EngineRuntime,
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
        "Usage: {} [options] <machine-file> [input-paths...]\n\n\
         Execute a machine notation pipeline on input files.\n\n\
         Options:\n\
           --mermaid                Output Mermaid diagram code and exit\n\
           --gen-values             Output a values JSON template for the machine and exit\n\
           --dev-bins <binary> ...  Use local cartridge binaries\n\
           --values <file.json>     Argument values per node\n\
           --trace <file.trace>     Write a per-segment bifaci protocol trace (JSONL)\n\
           --help                   Show this help\n\n\
         Input paths can be:\n\
           - Single file:   /path/to/file.pdf\n\
           - Directory:     /path/to/pdfs/\n\
           - Glob pattern:  /path/to/*.pdf\n\n\
         Examples:\n\
           {} --gen-values pipeline.machine > values.json\n\
           {} --mermaid pipeline.machine\n\
           {} pipeline.machine /tmp/test.pdf\n\
           {} --values values.json pipeline.machine /tmp/pdfs/\n\
           {} --dev-bins ./pdfcartridge pipeline.machine /tmp/*.pdf",
        program, program, program, program, program, program
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

    // Parse arguments
    let mut dev_binaries = Vec::new();
    let mut mermaid_mode = false;
    let mut gen_values_mode = false;
    let mut values_file: Option<String> = None;
    let mut trace_file: Option<String> = None;
    let mut arg_idx = 1;

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

    // Set up cartridge directory
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let cartridge_dir = home.join(".capdag").join("cartridges");

    // Registry URL
    let registry_url = "https://cartridges.machinefabric.com/manifest".to_string();

    // Bundled providers shipped beside this CLI binary (the capdag executor's
    // own `providers/` tree, staged by its build with baked content hashes —
    // the same arrangement as the engine). Present only in a packaged build;
    // absent for a bare `cargo run`, in which case there are no bundled
    // providers and only ~/.capdag/cartridges + --dev-bins apply. discover_
    // cartridges verifies each bundled provider against the baked hash.
    let bundled_providers_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("providers")))
        .filter(|dir| dir.is_dir());

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

    // The reference runtime: hosts cartridges by spawning them per segment via
    // execute_dag, keeps output in memory, and fails hard on any ForEach body
    // failure. execute_plan drives the ForEach/Collect decomposition on top of it.
    let runtime: Arc<dyn EngineRuntime> = Arc::new(DevBinRuntime {
        cartridge_dir: cartridge_dir.clone(),
        registry_url: registry_url.clone(),
        channel: BUILD_CHANNEL,
        fabric_manifest_version: capdag::FABRIC_MANIFEST_VERSION,
        dev_binaries: dev_binaries.clone(),
        bundled_providers_dir: bundled_providers_dir.clone(),
        fabric_registry: registry.clone(),
        trace_sink,
    });

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
                    let strand_label = if plans.len() > 1 {
                        format!(" [strand {}]", idx)
                    } else {
                        String::new()
                    };
                    // A fan-out machine has several terminals (one per Output node).
                    for terminal in &result.terminals {
                        eprintln!(
                            "Result{} [{}]: media={} sequence={} items={}",
                            strand_label,
                            terminal.output_node_id,
                            terminal.media_urn,
                            terminal.is_sequence,
                            terminal.items.len()
                        );
                        for item in &terminal.items {
                            let preview_len = item.data.len().min(80);
                            match std::str::from_utf8(&item.data[..preview_len]) {
                                Ok(text) => eprintln!(
                                    "  [{}] {} bytes: {}",
                                    item.index,
                                    item.data.len(),
                                    text.replace('\n', " ")
                                ),
                                Err(_) => eprintln!(
                                    "  [{}] {} bytes (binary)",
                                    item.index,
                                    item.data.len()
                                ),
                            }
                        }
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
}
