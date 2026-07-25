//! capdag: Machine notation DAG executor for Cap pipelines
//!
//! A unified CLI for executing and validating machine notation pipelines.

use capdag::machine::parse_machine_with_node_names;
use capdag::orchestrator::{
    build_plans_from_notation, execute_plan, CliRuntime, EngineRuntime,
};
use capdag::{
    CapProgressFn, CartridgeChannel, ExecutionNodeType, FabricRegistry, PipelineLogFn,
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
/// build (dev-bins + bundled cartridges only; registry downloads are
/// disabled), `Some(url)` = a product build bound to that registry. Never a
/// hardcoded literal — the URL is part of the build identity.
const BAKED_REGISTRY_URL: Option<&str> =
    capdag::registry_url_from_build_env(option_env!("MFR_CARTRIDGE_REGISTRY_URL"));

/// Fabric registry origin (caps / media / aliases — the layer aliases like `disbind-pdf`
/// live in), baked at build time from the environment `dx capdag-bundle`'s
/// `select_fabric_target` exports (`https://fabric.capdag.com` for prod,
/// `https://fabric-staging.capdag.com` for staging). A shipped binary has no such env at
/// runtime, so `main` seeds the process env from these before any fabric-registry
/// construction — otherwise every fabric/schema reader would fall back to the prod default
/// and a staging build would resolve caps/aliases against prod fabric. The cartridge and
/// fabric registries move together (build.rs fails a product build that bakes one without
/// the other). `None` only for a bare `cargo run` dev build.
const BAKED_FABRIC_REGISTRY_URL: Option<&str> = option_env!("CDG_FABRIC_REGISTRY_URL");
const BAKED_FABRIC_SCHEMA_URL: Option<&str> = option_env!("CDG_SCHEMA_BASE_URL");

/// The per-user cartridge install root: `~/.capdag/cartridges`, in the same
/// `{registry_slug}/{channel}/{name}/{version}/` tree every host uses.
fn user_cartridge_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".capdag").join("cartridges")
}

/// Bundled cartridges shipped beside this CLI binary (the executor's own
/// `bundled-cartridges/` tree, staged by `dx capdag-bundle` with baked content
/// hashes). Present only in a packaged build; absent for a bare `cargo run`.
///
/// `current_exe()` is canonicalized so a launcher SYMLINK resolves to the real
/// binary before we look for `bundled-cartridges/` beside it. OS package installs put the
/// whole bundle under a prefix (`/opt/capdag`, Homebrew `libexec`,
/// `%ProgramFiles%\capdag`) and expose only a symlink on PATH
/// (`/usr/bin/capdag`, `bin/capdag`); without canonicalization the cartridges
/// tree would be searched beside the symlink (e.g. `/usr/bin/bundled-cartridges`) and
/// discovery — hence baked-hash verification — would fail. Linux already
/// resolves `/proc/self/exe`, but macOS/Windows need the explicit canonicalize.
fn bundled_cartridges_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| bundled_cartridges_dir_for_exe(&exe))
}

/// Resolve the `bundled-cartridges/` tree beside a launcher path, following symlinks.
/// Split out from `bundled_cartridges_dir` so the symlink-resolution invariant
/// every OS package depends on is unit-testable without mutating the process's
/// real `current_exe()`.
fn bundled_cartridges_dir_for_exe(exe: &std::path::Path) -> Option<PathBuf> {
    std::fs::canonicalize(exe)
        .ok()
        .and_then(|real| real.parent().map(|dir| dir.join("bundled-cartridges")))
        .filter(|dir| dir.is_dir())
}

/// The stderr progress/log hooks shared by every execution mode.
fn progress_hooks() -> (CapProgressFn, PipelineLogFn) {
    let progress: CapProgressFn = Arc::new(|p: f32, cap_urn: &str, msg: &str| {
        eprintln!("  [{:5.1}%] {} {}", p * 100.0, cap_urn, msg);
    });
    let log_fn: PipelineLogFn = Arc::new(|record| {
        let meta_suffix = record
            .meta
            .as_ref()
            .map(|meta| format!(" [meta {:?}]", meta))
            .unwrap_or_default();
        let step_token = record.step_token_id.as_deref().unwrap_or("machine");
        let cap_urn = record.cap_urn.as_deref().unwrap_or("machine");
        let body_suffix = record
            .body_index
            .map(|index| format!(" body={index}"))
            .unwrap_or_default();
        let arg_suffix = record
            .arg_urn
            .as_deref()
            .map(|arg_urn| format!(" arg='{arg_urn}'"))
            .unwrap_or_default();
        eprintln!(
            "  [log:{}{} step='{}'{}]{} {} {}",
            record.level,
            body_suffix,
            step_token,
            arg_suffix,
            meta_suffix,
            cap_urn,
            record.message
        );
    });
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
           {p} plan <files...> [--to <t>]... [plan options]          Plan machines for a file set (multi-source aware)\n\
           {p} dag-viz <machine-file> [--mermaid|--dot]              Render the execution plan as a diagram\n\
           {p} find <cap-alias-or-urn>                               Show the providing cartridge(s)\n\
           {p} resolve [--no-cache] <cap-alias-or-urn>...            Print cap definition JSON (array for >1)\n\
           {p} cache [clear|refresh]                                 Invalidate/renew the local fabric cache\n\
           {p} install <cap-alias-or-urn-or-cartridge-id>            Download + verify without running\n\
           {p} new <name> [--python] [-o <dir>]                      Scaffold a new cartridge project\n\
           {p} dev-install <project-dir>                             Install/update a dev cartridge under the dev slug\n\n\
         Single-cap mode drives the cap's OWN declared interface — exactly like\n\
         invoking the cartridge directly, except the cap runs inside a full bifaci\n\
         host with the bundled cartridges (data/fetch/model cartridges) registered,\n\
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
           --dev-bins <binary> ...  Use local cartridge binaries\n\
           --trace <file.trace>     Write a per-segment bifaci protocol trace (JSONL)\n\
           --help                   Show this help; after a cap name, that cap's interface\n\
                                    (input / required options / options)\n\n\
         Plan options (capdag plan — the unified configurable planner):\n\
           --to <ext|media-urn>     Target (repeatable ⇒ multi-target). Omit to DISCOVER targets\n\
           --converge <auto|combine|independent>\n\
           --where <auto|earliest|latest|source|target|depth=N>   Convergence location\n\
           --mechanism <any|generalize|collect|merge>\n\
           --rank <intent|shortest|cost>\n\
           --depth <N> / --max-paths <N> / --max <N>              Search bounds\n\
           --pick <rank>            Choose a candidate (default 0, the magic pick)\n\
           --save <file.machine>    Write the chosen candidate's notation\n\
           --run                    Execute the chosen candidate on the input files\n\n\
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
    // Bind the CLI to the fabric registry origin (caps/media/aliases) it was built for.
    // A shipped binary has no runtime env, so seed the process env from the build-baked
    // value BEFORE any fabric-registry construction, unless the user has explicitly
    // overridden it (a runtime env var always wins). Without this a `--staging` build
    // resolves aliases like `disbind-pdf` against the prod fabric default. Schema base is
    // only seeded alongside the fabric URL — never pair a runtime fabric URL with a baked
    // schema URL.
    if std::env::var_os("CDG_FABRIC_REGISTRY_URL").is_none() {
        if let Some(url) = BAKED_FABRIC_REGISTRY_URL {
            std::env::set_var("CDG_FABRIC_REGISTRY_URL", url);
            if std::env::var_os("CDG_SCHEMA_BASE_URL").is_none() {
                if let Some(schema) = BAKED_FABRIC_SCHEMA_URL {
                    std::env::set_var("CDG_SCHEMA_BASE_URL", schema);
                }
            }
        }
    }

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage(&args[0]);
        process::exit(1);
    }

    // `hash-cartridge-dir <dir>` — print the deterministic content hash of a
    // cartridge version directory and exit. This is the SINGLE source of truth
    // for cartridge-directory hashing: the bundle build scripts
    // (build-engine-bundle.sh/.ps1) call this to compute the bundled-cartridge
    // hashes they bake into the engine via MFR_BUNDLED_CARTRIDGE_HASHES, so the
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
        "plan" => cmd_plan(&args).await,
        "dag-viz" => cmd_dag_viz(&args).await,
        "find" => cmd_find(&args).await,
        "resolve" => cmd_resolve(&args).await,
        "cache" => cmd_cache(&args).await,
        "install" => cmd_install(&args).await,
        "new" => cmd_new(&args).await,
        "dev-install" => cmd_dev_install(&args).await,
        "--help" | "-h" | "help" => {
            print_usage(&args[0]);
            process::exit(0);
        }
        // The RELEASE version (capdag/version.txt), baked by build.rs — this is
        // what the OS packages and the Homebrew `test` assert against, not the
        // unrelated Cargo crate version.
        "--version" | "-V" | "version" => {
            println!("capdag {}", env!("CAPDAG_VERSION"));
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

/// `capdag dag-viz <machine-file> [--mermaid|--dot]` — render the machine's
/// execution plan(s) as a diagram. This walks the SAME planner output the
/// engine executes (`build_plans_from_notation`), so it faithfully expresses
/// everything machine notation can model — ForEach fan-out, Collect/Merge
/// fan-in, Split, input slots, outputs, and every typed edge — not the old
/// flat cap-to-cap view. `--mermaid` (default) or `--dot` selects the format.
async fn cmd_dag_viz(args: &[String]) -> ! {
    let mut want_dot = false;
    let mut machine_file: Option<&str> = None;
    for a in &args[2..] {
        match a.as_str() {
            "--mermaid" => want_dot = false,
            "--dot" => want_dot = true,
            "--help" | "-h" => {
                print_usage(&args[0]);
                process::exit(0);
            }
            other if other.starts_with('-') => {
                eprintln!("Unknown dag-viz option '{other}'.");
                eprintln!("Usage: {} dag-viz <machine-file> [--mermaid|--dot]", args[0]);
                process::exit(2);
            }
            path => {
                if machine_file.is_some() {
                    eprintln!("dag-viz takes a single machine file.");
                    process::exit(2);
                }
                machine_file = Some(path);
            }
        }
    }
    let Some(machine_file) = machine_file else {
        eprintln!("Usage: {} dag-viz <machine-file> [--mermaid|--dot]", args[0]);
        process::exit(2);
    };

    let notation = match fs::read_to_string(machine_file) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading machine file '{}': {}", machine_file, e);
            process::exit(1);
        }
    };
    let registry = match FabricRegistry::new().await {
        Ok(reg) => Arc::new(reg),
        Err(e) => {
            eprintln!("Error creating FabricRegistry: {}", e);
            process::exit(1);
        }
    };
    let plans = match build_plans_from_notation(&notation, registry.clone()).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Validation failed: {}", e);
            process::exit(1);
        }
    };
    if want_dot {
        println!("{}", capdag::planner::plans_to_dot(&plans));
    } else {
        println!("{}", capdag::planner::plans_to_mermaid(&plans));
    }
    process::exit(0);
}

/// `capdag run <machine-file> [inputs…]` — execute a .machine pipeline.
async fn cmd_run(args: &[String]) -> ! {
    // Parse arguments
    let mut dev_binaries = Vec::new();
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

    execute_notation(
        notation,
        machine_file,
        &args[arg_idx..],
        dev_binaries,
        trace_file,
        output_dir,
        force_overwrite,
    )
    .await
}

/// Execute machine notation against a set of input files — the shared engine
/// behind `capdag run <file.machine>` and `capdag plan --run`.
///
/// Single-input machines run once per input file (the historical behavior).
/// A machine with a MULTI-anchor strand (a planner convergence candidate)
/// binds the whole file set across its input slots by media type: each file's
/// detected media must conform to exactly one slot at minimum specificity
/// distance (ties are a hard error — never positional guessing), every slot
/// must receive at least one file, and a slot receiving several files gets
/// them as one sequence.
async fn execute_notation(
    notation: String,
    machine_label: &str,
    input_args: &[String],
    dev_binaries: Vec<PathBuf>,
    trace_file: Option<String>,
    output_dir: Option<PathBuf>,
    force_overwrite: bool,
) -> ! {
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

    // Build execution plans through the single ForEach/Collect-aware front-end — the
    // same planner path the engine runs. One plan per connected strand.
    let plans = match build_plans_from_notation(&notation, registry.clone()).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Validation failed: {}", e);
            process::exit(1);
        }
    };

    // Find input nodes automatically
    let input_nodes = find_input_nodes(&notation, registry.as_ref());
    if input_nodes.is_empty() {
        eprintln!("No input nodes found in machine notation");
        process::exit(1);
    }

    // Collect all input paths and expand them
    let mut all_files: Vec<PathBuf> = Vec::new();
    for arg in input_args {
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
    eprintln!("Machine: {}", machine_label);
    eprintln!("Input node(s): {}", input_nodes.join(", "));
    eprintln!("Strands (plans): {}", plans.len());
    eprintln!("Input files: {}", all_files.len());
    for f in &all_files {
        eprintln!("  - {}", f.display());
    }

    let cartridge_dir = user_cartridge_dir();

    let registry_url: Option<String> = BAKED_REGISTRY_URL.map(str::to_string);

    let bundled_cartridges_dir = bundled_cartridges_dir();

    // The executor speaks `cap_arguments` (raw per-node arg-stream bytes). A
    // `.machine` run supplies every argument through data-flow edges and input
    // files, so the CLI passes no extra per-node argument streams here.
    let cap_arguments: HashMap<String, Vec<(String, Vec<u8>)>> = HashMap::new();

    eprintln!("\n=== Executing ===\n");
    if !dev_binaries.is_empty() {
        eprintln!("Dev mode: {} local binaries", dev_binaries.len());
        for bin in &dev_binaries {
            eprintln!("  - {}", bin.display());
        }
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
        bundled_cartridges_dir.clone(),
        registry.clone(),
        trace_sink,
    ));

    let (progress, log_fn) = progress_hooks();

    // Process each file
    let mut success_count = 0;
    let mut error_count = 0;

    let input_slots_of = |plan: &capdag::planner::MachinePlan| -> Vec<(String, capdag::MediaUrn)> {
        let mut slots: Vec<(String, capdag::MediaUrn)> = plan
            .nodes
            .iter()
            .filter_map(|(id, n)| match &n.node_type {
                ExecutionNodeType::InputSlot { expected_media_urn, .. } => {
                    match capdag::MediaUrn::from_string(expected_media_urn) {
                        Ok(u) => Some((id.clone(), u)),
                        Err(e) => {
                            eprintln!(
                                "input slot '{id}' declares an invalid media URN \
                                 '{expected_media_urn}': {e}"
                            );
                            process::exit(1);
                        }
                    }
                }
                _ => None,
            })
            .collect();
        slots.sort_by(|a, b| a.0.cmp(&b.0));
        slots
    };

    let multi_anchor = plans.iter().any(|p| input_slots_of(p).len() > 1);

    if multi_anchor {
        // ── Multi-anchor machine: bind the WHOLE file set across all slots by
        // media type, then run each plan exactly once. ──
        eprintln!("--- Multi-anchor machine: binding {} files by media type ---", all_files.len());
        eprintln!("Run: {}", notation);

        // Detect each file's media once.
        let mut file_media: Vec<(PathBuf, capdag::MediaUrn, Vec<u8>)> =
            Vec::with_capacity(all_files.len());
        for file in &all_files {
            let resolved = match capdag::detect_file_with_fabric_registry(file, registry.clone()) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to detect the media type of '{}': {e}", file.display());
                    process::exit(1);
                }
            };
            let media = match capdag::MediaUrn::from_string(&resolved.media_urn) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Detected an invalid media URN '{}': {e}", resolved.media_urn);
                    process::exit(1);
                }
            };
            let bytes = match fs::read(file) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Error reading input file '{}': {}", file.display(), e);
                    process::exit(1);
                }
            };
            file_media.push((file.clone(), media, bytes));
        }

        // All slots across all plans. Each file must bind to exactly one slot:
        // the conforming slot at minimum specificity distance; a tie between
        // distinct slots is ambiguous and fails hard.
        let all_slots: Vec<(usize, String, capdag::MediaUrn)> = plans
            .iter()
            .enumerate()
            .flat_map(|(pi, p)| {
                input_slots_of(p).into_iter().map(move |(id, urn)| (pi, id, urn))
            })
            .collect();
        let mut slot_files: HashMap<String, Vec<Vec<u8>>> = HashMap::new();
        for (file, media, bytes) in &file_media {
            let mut best: Option<(usize, i64)> = None; // (slot index, distance)
            let mut tied = false;
            for (i, (_, _, slot_urn)) in all_slots.iter().enumerate() {
                if !media.conforms_to(slot_urn).unwrap_or(false) {
                    continue;
                }
                let dist = media.specificity() as i64 - slot_urn.specificity() as i64;
                match &best {
                    None => best = Some((i, dist)),
                    Some((_, bd)) if dist < *bd => {
                        best = Some((i, dist));
                        tied = false;
                    }
                    Some((bi, bd)) if dist == *bd => {
                        // Same slot media on two plans would be distinct slots;
                        // an equal-distance second slot is a real ambiguity.
                        if *bi != i {
                            tied = true;
                        }
                    }
                    _ => {}
                }
            }
            let Some((slot_idx, _)) = best else {
                eprintln!(
                    "'{}' (detected {}) does not conform to any input anchor of this machine",
                    file.display(),
                    media
                );
                process::exit(1);
            };
            if tied {
                eprintln!(
                    "'{}' (detected {}) conforms to several input anchors at equal specificity \
                     — the binding is ambiguous",
                    file.display(),
                    media
                );
                process::exit(1);
            }
            slot_files
                .entry(all_slots[slot_idx].1.clone())
                .or_default()
                .push(bytes.clone());
        }
        for (_, slot_id, slot_urn) in &all_slots {
            if !slot_files.contains_key(slot_id) {
                eprintln!(
                    "input anchor '{slot_id}' ({slot_urn}) received no file — every anchor \
                     of a multi-anchor machine needs an input"
                );
                process::exit(1);
            }
        }

        let mut run_failed = false;
        for (idx, plan) in plans.iter().enumerate() {
            let mut initial_inputs: HashMap<String, Vec<u8>> = HashMap::new();
            let mut initial_is_sequence: HashMap<String, bool> = HashMap::new();
            for (slot_id, _) in input_slots_of(plan) {
                let files = slot_files.get(&slot_id).expect("verified above");
                if files.len() == 1 {
                    initial_inputs.insert(slot_id.clone(), files[0].clone());
                    initial_is_sequence.insert(slot_id, false);
                } else {
                    let seq = match capdag::orchestrator::cbor_util::wrap_raw_items_as_cbor_sequence(files) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("failed to assemble the {}-file sequence for '{slot_id}': {e}", files.len());
                            process::exit(1);
                        }
                    };
                    initial_inputs.insert(slot_id.clone(), seq);
                    initial_is_sequence.insert(slot_id, true);
                }
            }

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
                    let stem = all_files[0]
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "input".to_string());
                    let options = capdag::orchestrator::EmitOptions {
                        output_dir: Some(output_dir.clone().unwrap_or_else(|| PathBuf::from("."))),
                        force: force_overwrite,
                        input_stem: if plans.len() > 1 {
                            format!("{stem}.combined.strand{idx}")
                        } else {
                            format!("{stem}.combined")
                        },
                    };
                    let mut stdout = std::io::stdout();
                    if let Err(e) =
                        capdag::orchestrator::emit_terminals(&result, &options, &mut stdout)
                    {
                        eprintln!("{e}");
                        run_failed = true;
                    }
                }
                Err(e) => {
                    eprintln!("{}", e);
                    run_failed = true;
                }
            }
        }
        if run_failed {
            error_count += 1;
        } else {
            success_count += 1;
        }
    } else {
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
                let input_slot_id = input_slots_of(plan)
                    .first()
                    .map(|(id, _)| id.clone())
                    .unwrap_or_else(|| {
                        eprintln!("strand {idx} has no input anchor");
                        process::exit(1);
                    });
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
    }

    eprintln!("=== Summary ===");
    if multi_anchor {
        eprintln!("Bound files: {}", all_files.len());
    } else {
        eprintln!("Processed: {}", all_files.len());
    }
    eprintln!("Success: {}", success_count);
    eprintln!("Errors: {}", error_count);

    if error_count > 0 {
        process::exit(1);
    }
    process::exit(0);
}

/// Build the FabricRegistry or exit with the error.
async fn fabric_registry_or_exit() -> Arc<FabricRegistry> {
    fabric_registry_or_exit_with_bypass(false).await
}

/// Construct the fabric registry, optionally bypassing every on-disk cache so
/// the manifest and all cap bodies are fetched fresh (correct against a
/// mutable channel like staging that re-publishes the same manifest version).
async fn fabric_registry_or_exit_with_bypass(bypass_cache: bool) -> Arc<FabricRegistry> {
    let config = capdag::RegistryConfig::default().with_bypass_cache(bypass_cache);
    match FabricRegistry::with_config(config).await {
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

/// Resolve a cap for single-cap mode with a local dev fallback: try the fabric
/// first; if the token names a cap the fabric does NOT define, fall back to a
/// locally dev-installed cartridge's OWN manifest (run by alias). A dev cap is
/// accepted only if it does not conflict with the fabric — no alias of it may
/// already mean a different cap upstream. On acceptance the cap is injected into
/// the registry's in-memory cache so the rest of the pipeline plans and routes
/// it exactly like any fabric cap. This is what lets a brand-new cap be run
/// through the full capdag host before it is ever published.
async fn resolve_cap_or_dev_or_exit(
    registry: &FabricRegistry,
    token: &str,
) -> (capdag::Cap, Option<PathBuf>) {
    let cap_ref = match capdag::orchestrator::classify_cap_token(token) {
        Ok(capdag::orchestrator::CapToken::Urn(urn)) => urn,
        Ok(capdag::orchestrator::CapToken::Alias(alias)) => alias,
        Err(e) => {
            eprintln!("{e}");
            process::exit(2);
        }
    };
    if let Ok(cap) = registry.get_cap(&cap_ref).await {
        return (cap, None);
    }
    // Not in the fabric — is it a locally dev-installed cap? Dev caps are run by
    // their alias.
    match capdag::dev::find_dev_cap_by_alias(&user_cartridge_dir(), &cap_ref) {
        Ok(Some((cap, dir))) => {
            if let Err(e) = capdag::dev::check_no_fabric_conflict(registry, &cap).await {
                eprintln!("{e}");
                process::exit(1);
            }
            eprintln!(
                "  [dev] '{token}' is not published in the fabric; running the local dev \
                 cartridge at {}",
                dir.display()
            );
            // Inject so the planner and arg mapper resolve the cap's URN uniformly;
            // return the install dir so the runtime hosts that dev cartridge.
            registry.add_caps_to_cache(vec![cap.clone()]);
            (cap, Some(dir))
        }
        Ok(None) => {
            eprintln!(
                "Error resolving cap '{token}': not defined in the fabric, and no dev cartridge \
                 installed under the local `dev` slug advertises it. Publish the cap, or run \
                 `capdag dev-install <project>` on a cartridge that provides it."
            );
            process::exit(1);
        }
        Err(e) => {
            eprintln!("Error scanning local dev cartridges for '{token}': {e}");
            process::exit(1);
        }
    }
}

/// Parse a `--to` target into a media URN. A value containing ':' is taken as a
/// full media URN; a bare token (e.g. `png`) is the file-extension shorthand
/// `media:ext=<token>`. Exits on a malformed value.
/// `capdag plan <files...> [--to <t>]... [options]` — the unified configurable
/// planner over a file set (docs/planner-configuration-space.md).
///
/// Without `--to`, DISCOVERS the reachable targets for the set (convergent
/// targets first, tagged with their apex). With one or more `--to`, plans
/// ranked candidate machines; `--pick`/`--save`/`--run` choose, persist, and
/// execute a candidate through the same execution engine as `capdag run`.
async fn cmd_plan(args: &[String]) -> ! {
    use capdag::planner as p;

    let mut to_targets: Vec<String> = Vec::new();
    let mut converge = "auto".to_string();
    let mut location = "auto".to_string();
    let mut mechanism = "any".to_string();
    let mut rank = "intent".to_string();
    let mut max_depth = p::PlanRequest::DEFAULT_MAX_DEPTH;
    let mut max_paths = p::PlanRequest::DEFAULT_MAX_PATHS;
    let mut max_candidates = p::PlanRequest::DEFAULT_MAX_CANDIDATES;
    let mut pick: usize = 0;
    let mut save: Option<String> = None;
    let mut run_after = false;
    let mut output_dir: Option<PathBuf> = None;
    let mut force = false;
    let mut dev_binaries: Vec<PathBuf> = Vec::new();
    let mut trace_file: Option<String> = None;
    let mut file_args: Vec<String> = Vec::new();
    let mut configured = false; // any explicitly-set knob ⇒ Configured mode

    let take_value = |args: &[String], i: &mut usize, flag: &str| -> String {
        *i += 1;
        match args.get(*i) {
            Some(v) => v.clone(),
            None => {
                eprintln!("{flag} requires a value");
                process::exit(2);
            }
        }
    };

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--to" => to_targets.push(take_value(args, &mut i, "--to")),
            "--converge" => {
                converge = take_value(args, &mut i, "--converge");
                configured = true;
            }
            "--where" => {
                location = take_value(args, &mut i, "--where");
                configured = true;
            }
            "--mechanism" => {
                mechanism = take_value(args, &mut i, "--mechanism");
                configured = true;
            }
            "--rank" => rank = take_value(args, &mut i, "--rank"),
            "--depth" => {
                max_depth = parse_usize_or_exit(&take_value(args, &mut i, "--depth"), "--depth")
            }
            "--max-paths" => {
                max_paths =
                    parse_usize_or_exit(&take_value(args, &mut i, "--max-paths"), "--max-paths")
            }
            "--max" => {
                max_candidates = parse_usize_or_exit(&take_value(args, &mut i, "--max"), "--max")
            }
            "--pick" => pick = parse_usize_or_exit(&take_value(args, &mut i, "--pick"), "--pick"),
            "--save" => save = Some(take_value(args, &mut i, "--save")),
            "--run" => run_after = true,
            "-o" | "--output" => output_dir = Some(PathBuf::from(take_value(args, &mut i, "--output"))),
            "--force" => force = true,
            "--trace" => trace_file = Some(take_value(args, &mut i, "--trace")),
            "--dev-bins" => {
                i += 1;
                while i < args.len() && !args[i].starts_with("--") {
                    let expanded = expand_dev_binary_path(&args[i]);
                    if expanded.is_empty() {
                        eprintln!("No executables found in: {}", args[i]);
                        process::exit(1);
                    }
                    dev_binaries.extend(expanded);
                    i += 1;
                }
                continue;
            }
            "--help" | "-h" => {
                print_usage(&args[0]);
                process::exit(0);
            }
            tok if tok.starts_with('-') => {
                eprintln!("Unknown plan option '{tok}'.");
                process::exit(2);
            }
            tok => file_args.push(tok.to_string()),
        }
        i += 1;
    }

    let mut files: Vec<PathBuf> = Vec::new();
    for f in &file_args {
        files.extend(expand_input_path(f));
    }
    if files.is_empty() {
        eprintln!("capdag plan needs at least one input file");
        process::exit(2);
    }
    files.sort();

    let registry = match FabricRegistry::new().await {
        Ok(reg) => Arc::new(reg),
        Err(e) => {
            eprintln!("Error creating FabricRegistry: {}", e);
            process::exit(1);
        }
    };

    // Detect each file's media and group equal types: N same-typed files form
    // ONE sequence anchor; distinct types are distinct anchors.
    let mut groups: Vec<(capdag::MediaUrn, usize)> = Vec::new();
    for file in &files {
        let resolved = match capdag::detect_file_with_fabric_registry(file, registry.clone()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to detect the media type of '{}': {e}", file.display());
                process::exit(1);
            }
        };
        let media = match capdag::MediaUrn::from_string(&resolved.media_urn) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Detected an invalid media URN '{}': {e}", resolved.media_urn);
                process::exit(1);
            }
        };
        match groups.iter_mut().find(|(m, _)| m.is_equivalent(&media).unwrap_or(false)) {
            Some((_, count)) => *count += 1,
            None => groups.push((media, 1)),
        }
    }
    let sources: Vec<p::SourceSpec> = groups
        .iter()
        .map(|(media, count)| {
            if *count > 1 {
                p::SourceSpec::sequence(media.clone())
            } else {
                p::SourceSpec::single(media.clone())
            }
        })
        .collect();
    eprintln!("Sources ({} files):", files.len());
    for (media, count) in &groups {
        eprintln!("  - {media}  ×{count}");
    }

    // Build the live cap graph from the fabric cache: all caps + the bookend
    // set (media defs with at least one file extension).
    let caps = match registry.get_cached_caps().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load caps from the fabric cache: {e}");
            process::exit(1);
        }
    };
    let media_defs = match registry.get_cached_media_defs().await {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to load media defs from the fabric cache: {e}");
            process::exit(1);
        }
    };
    let bookends: std::collections::HashSet<capdag::MediaUrn> = media_defs
        .iter()
        .filter(|d| !d.extensions.is_empty())
        .filter_map(|d| capdag::MediaUrn::from_string(&d.urn).ok())
        .collect();
    let mut fab = capdag::planner::LiveCapFab::new();
    fab.sync_from_caps(&caps, &bookends);

    // ── Discover mode: no --to ──
    if to_targets.is_empty() {
        let discovered = match fab.discover_convergent_targets(&sources, max_depth) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("{e}");
                process::exit(1);
            }
        };
        for dead in &discovered.dead_end_sources {
            eprintln!("dead end: '{dead}' reaches no target — it will be left untouched");
        }
        let targets = discovered.targets;
        if targets.is_empty() {
            eprintln!("No reachable targets for this file set.");
            process::exit(1);
        }
        println!("Reachable targets:");
        for t in &targets {
            let title = registry
                .get_cached_media_def(&t.media_def.to_string())
                .map(|d| d.title)
                .unwrap_or_else(|| t.display_name.clone());
            match &t.apex {
                Some(apex) => println!(
                    "  combine all → {title}  [{}]  (via {} at depth {}, ~{} steps)",
                    t.media_def, apex.media_urn, apex.depth, t.min_total_steps
                ),
                None => println!(
                    "  convert each → {title}  [{}]  (~{} steps)",
                    t.media_def, t.min_total_steps
                ),
            }
        }
        eprintln!("\nPick one with: capdag plan <files...> --to <target>");
        process::exit(0);
    }

    // ── Plan mode ──
    let target_urns: Vec<capdag::MediaUrn> =
        to_targets.iter().map(|t| parse_target_media_or_exit(t)).collect();

    let presence = match converge.as_str() {
        "auto" => p::ConvergencePresence::Auto,
        "combine" => p::ConvergencePresence::Converged,
        "independent" => p::ConvergencePresence::Independent,
        other => {
            eprintln!("--converge must be auto|combine|independent, got '{other}'");
            process::exit(2);
        }
    };
    let location = match location.as_str() {
        "auto" => p::ConvergenceLocation::Auto,
        "earliest" => p::ConvergenceLocation::Earliest,
        "latest" => p::ConvergenceLocation::Latest,
        "source" => p::ConvergenceLocation::AtSource,
        "target" => p::ConvergenceLocation::AtTarget,
        other => match other.strip_prefix("depth=") {
            Some(n) => p::ConvergenceLocation::AtDepth(parse_usize_or_exit(n, "--where depth=")),
            None => {
                eprintln!("--where must be auto|earliest|latest|source|target|depth=N, got '{other}'");
                process::exit(2);
            }
        },
    };
    let mechanism = match mechanism.as_str() {
        "any" => p::ConvergenceMechanism::Any,
        "generalize" => p::ConvergenceMechanism::Generalize,
        "collect" => p::ConvergenceMechanism::Collect,
        "merge" => p::ConvergenceMechanism::Merge,
        other => {
            eprintln!("--mechanism must be any|generalize|collect|merge, got '{other}'");
            process::exit(2);
        }
    };
    let ranking = match rank.as_str() {
        "intent" => p::RankPolicy::Intent,
        "shortest" => p::RankPolicy::Shortest,
        "cost" => p::RankPolicy::Cost,
        other => {
            eprintln!("--rank must be intent|shortest|cost, got '{other}'");
            process::exit(2);
        }
    };

    let request = p::PlanRequest {
        sources,
        targets: p::TargetSpec::Exact(target_urns),
        convergence: p::ConvergencePolicy {
            presence,
            location,
            mechanism,
            at_type: None,
            arity: p::ConvergenceArity::Auto,
        },
        divergence: p::DivergencePolicy::default(),
        ranking,
        search: p::SearchDirection::Auto,
        mode: if configured { p::PlanMode::Configured } else { p::PlanMode::Auto },
        max_depth,
        max_paths,
        max_candidates,
    };

    let outcome = match fab.plan(&request, registry.as_ref()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };
    for dead in &outcome.dead_end_sources {
        eprintln!("dead end: '{dead}' reaches no target — it will be left untouched");
    }
    let candidates = outcome.candidates;

    println!("Candidates ({}):", candidates.len());
    for c in &candidates {
        println!(
            "  [{}] {}  ({} steps{}{})",
            c.rank,
            c.label,
            c.cost.cap_steps,
            if c.profile.converged { ", combined result" } else { "" },
            if c.profile.diverged { ", fan-out" } else { "" },
        );
        println!("      {}", c.notation);
    }

    let Some(chosen) = candidates.iter().find(|c| c.rank == pick) else {
        eprintln!("--pick {pick} is out of range (0..{})", candidates.len());
        process::exit(2);
    };

    if let Some(path) = &save {
        if let Err(e) = fs::write(path, &chosen.notation) {
            eprintln!("Failed to write '{path}': {e}");
            process::exit(1);
        }
        eprintln!("Saved candidate [{pick}] to {path}");
    }

    if run_after {
        let file_strings: Vec<String> =
            files.iter().map(|f| f.to_string_lossy().into_owned()).collect();
        execute_notation(
            chosen.notation.clone(),
            &format!("plan candidate [{pick}]"),
            &file_strings,
            dev_binaries,
            trace_file,
            output_dir,
            force,
        )
        .await
    }
    process::exit(0);
}

fn parse_usize_or_exit(value: &str, flag: &str) -> usize {
    match value.parse::<usize>() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("{flag} requires a non-negative integer, got '{value}'");
            process::exit(2);
        }
    }
}

fn parse_target_media_or_exit(t: &str) -> capdag::MediaUrn {
    let s = if t.contains(':') {
        t.to_string()
    } else {
        format!("media:ext={t}")
    };
    match capdag::MediaUrn::from_string(&s) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Invalid --to target '{t}': {e}");
            process::exit(2);
        },
    }
}

/// Narrow an abstract cap to its concrete specialization by detecting the input
/// file's media type (and honouring `--to`), or exit with an actionable error.
async fn narrow_abstract_or_exit(
    registry: &Arc<FabricRegistry>,
    abstract_cap: capdag::Cap,
    cap_tokens: &[String],
    to_target: Option<&str>,
) -> capdag::Cap {
    // Find the input FILE among the positional tokens — the first token that
    // expands to at least one existing file. Abstract narrowing needs a
    // concrete input to detect media from; piped stdin has no path/extension
    // and therefore cannot be narrowed (fail hard rather than guess).
    let mut input_path: Option<PathBuf> = None;
    for tok in cap_tokens {
        if tok.starts_with('-') {
            continue;
        }
        if let Some(first) = expand_input_path(tok).into_iter().next() {
            input_path = Some(first);
            break;
        }
    }
    let Some(path) = input_path else {
        eprintln!(
            "'{}' is an abstract cap — it needs an input FILE to detect the media type and narrow to a concrete cap. Provide a file path (piped stdin cannot be narrowed).",
            abstract_cap.primary_alias()
        );
        process::exit(2);
    };

    let resolved = match capdag::detect_file_with_fabric_registry(&path, registry.clone()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to detect the media type of '{}': {e}", path.display());
            process::exit(1);
        },
    };
    let input_media = match capdag::MediaUrn::from_string(&resolved.media_urn) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Detected an invalid media URN '{}': {e}", resolved.media_urn);
            process::exit(1);
        },
    };

    let target_media = to_target.map(parse_target_media_or_exit);

    match registry
        .narrow_abstract_cap(&abstract_cap.urn, &input_media, target_media.as_ref())
        .await
    {
        Ok(concrete_urn) => match registry.get_cap(&concrete_urn.to_string()).await {
            Ok(concrete) => {
                eprintln!(
                    "{} → {} (input {})",
                    abstract_cap.primary_alias(),
                    concrete.primary_alias(),
                    input_media
                );
                concrete
            },
            Err(e) => {
                eprintln!("Narrowed to '{concrete_urn}' but that cap is not in the registry: {e}");
                process::exit(1);
            },
        },
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        },
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
        // The CLI seeds CDG_FABRIC_REGISTRY_URL from BAKED_FABRIC_REGISTRY_URL at
        // startup, so this resolves to the fabric this build resolves caps against.
        capdag::RegistryConfig::default().registry_base_url,
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
/// missing) and hosted on the shared switch BESIDE the bundled cartridges, so
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
    // Target output for narrowing an ABSTRACT cap (e.g. `convert-image` needs a
    // target format). Ignored (and rejected) for concrete caps.
    let mut to_target: Option<String> = None;
    // `capdag <cap> --help` shows THAT CAP's declared interface (input /
    // required options / options), not the generic usage — deferred until the
    // cap token is resolved below.
    let mut show_cap_help = false;
    let mut idx = 2usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--help" | "-h" => show_cap_help = true,
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
            "--to" => {
                idx += 1;
                let Some(t) = args.get(idx) else {
                    eprintln!("--to requires a target (an extension like `png`, a media URN, or `media:...`)");
                    process::exit(2);
                };
                to_target = Some(t.clone());
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
    let (resolved_cap, dev_dir) = resolve_cap_or_dev_or_exit(&registry, cap_token).await;
    // A dev cap's cartridge is hosted by feeding its install dir to the runtime
    // as a dev binary (the same path `--dev-bins` uses); its cartridge.json
    // resolves the entry point.
    if let Some(dir) = dev_dir {
        dev_binaries.push(dir);
    }

    // Per-cap help: the cap's own interface, structured by argument role
    // (input / required options / options). An abstract cap is rendered
    // as-is — narrowing needs an input, and help must work without one.
    if show_cap_help {
        match capdag::orchestrator::render_cap_interface(&resolved_cap) {
            Ok(text) => {
                eprint!("{text}");
                process::exit(0);
            }
            Err(e) => {
                eprintln!("{e}");
                process::exit(1);
            }
        }
    }

    // Alias/URN resolution answered "which cap does this name mean?" (an
    // is_equivalent question). If it named an ABSTRACT cap, we now answer the
    // dispatch question — "which concrete cap handles THIS input?" — by
    // detecting the input media and narrowing via is_dispatchable. Concrete
    // caps run as-is; `--to` is only meaningful for the abstract case.
    let cap = if resolved_cap.is_abstract() {
        narrow_abstract_or_exit(&registry, resolved_cap, &cap_tokens, to_target.as_deref()).await
    } else {
        if to_target.is_some() {
            eprintln!(
                "--to is only valid for an abstract (generic) cap; '{cap_token}' resolves to a concrete cap"
            );
            process::exit(2);
        }
        resolved_cap
    };

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
        bundled_cartridges_dir(),
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

/// `capdag resolve [--no-cache] <cap-alias-or-urn>` — print the canonical cap
/// definition JSON for a single cap, resolved through the baked fabric registry
/// (the same registry every mirror uses). Cartridges use this to (re)generate
/// the cap-def snapshots they embed and implement: the printed JSON deserializes straight
/// back into a `Cap`, carrying the aliases, args, and output as the fabric
/// defines them. Resolution uses the alias/URN boundary (a media alias fails
/// hard); an abstract cap is dumped as-is (cartridges only ever snapshot the
/// concrete caps they implement).
async fn cmd_resolve(args: &[String]) -> ! {
    // `--no-cache` forces a fresh fetch against the live fabric (skips the
    // version-keyed on-disk cache, which is stale on a mutable channel).
    let no_cache = args[2..].iter().any(|a| a == "--no-cache");
    // Accept ONE or MANY cap tokens. A single token prints the cap def object;
    // several tokens print a JSON ARRAY of cap defs, in order — one process, one
    // registry, one manifest read. Cartridge snapshot generation resolves a
    // cartridge's whole cap-aliases.txt in a single batched call this way,
    // instead of spawning `capdag` once per alias.
    let tokens: Vec<&str> = args[2..]
        .iter()
        .filter(|a| !a.starts_with('-'))
        .map(|s| s.as_str())
        .collect();
    if tokens.is_empty() {
        eprintln!("Usage: {} resolve [--no-cache] <cap-alias-or-urn>...", args[0]);
        process::exit(2);
    }
    let registry = fabric_registry_or_exit_with_bypass(no_cache).await;

    let json = if tokens.len() == 1 {
        let cap = resolve_cap_or_exit(&registry, tokens[0]).await;
        serde_json::to_string_pretty(&cap)
    } else {
        let mut caps: Vec<capdag::Cap> = Vec::with_capacity(tokens.len());
        for token in &tokens {
            caps.push(resolve_cap_or_exit(&registry, token).await);
        }
        serde_json::to_string_pretty(&caps)
    };
    match json {
        Ok(json) => {
            println!("{json}");
            process::exit(0);
        }
        Err(e) => {
            eprintln!("Failed to serialize cap def(s): {e}");
            process::exit(1);
        }
    }
}

/// `capdag cache clear|refresh` — invalidate the local fabric cache for the
/// active registry. `clear` purges (in-memory + on-disk, manifest included);
/// `refresh` (the default) purges and then re-fetches the manifest so the next
/// command starts from a renewed cache. Use this after a channel re-publishes
/// under the same manifest version and the version-keyed cache is stale.
async fn cmd_cache(args: &[String]) -> ! {
    let sub = args.get(2).map(String::as_str).unwrap_or("refresh");
    let (do_refresh, ok_verb) = match sub {
        "clear" | "purge" | "invalidate" => (false, "cleared"),
        "refresh" | "renew" => (true, "refreshed"),
        other => {
            eprintln!(
                "Unknown cache subcommand '{other}'. Usage: {} cache [clear|refresh]",
                args[0]
            );
            process::exit(2);
        }
    };

    // Build against the live cache (no bypass) so clear_cache targets the very
    // directory the other commands read.
    let registry = fabric_registry_or_exit().await;
    let dir = registry.cache_dir().display().to_string();
    if let Err(e) = registry.clear_cache() {
        eprintln!("Failed to clear fabric cache at {dir}: {e}");
        process::exit(1);
    }

    if do_refresh {
        // Re-fetch the manifest fresh into the now-empty cache so the renewal
        // is complete rather than lazy. A fresh bypass-mode registry pulls the
        // current manifest and writes it through.
        let _ = fabric_registry_or_exit_with_bypass(true).await;
    }
    println!("Fabric cache {ok_verb}: {dir}");
    process::exit(0);
}

/// `capdag find <cap-alias-or-urn>` — resolve a cap and show which registry
/// cartridge(s) provide it, without downloading anything.
async fn cmd_find(args: &[String]) -> ! {
    let Some(token) = args.get(2) else {
        eprintln!("Usage: {} find <cap-alias-or-urn>", args[0]);
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

/// `capdag new <name> [--python] [-o <dir>]` — scaffold a fresh cartridge
/// project. Python is the only (and default) kind today. Writes a runnable
/// `cartridge.py`, a README, and a `.gitignore` into `<dir>/<name>/`.
async fn cmd_new(args: &[String]) -> ! {
    let mut name: Option<&str> = None;
    let mut parent = PathBuf::from(".");
    let mut idx = 2usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--python" => {} // the only kind; explicit is fine.
            "-o" | "--output" => {
                idx += 1;
                let Some(dir) = args.get(idx) else {
                    eprintln!("--output requires a directory path");
                    process::exit(2);
                };
                parent = PathBuf::from(dir);
            }
            other if other.starts_with("--") => {
                eprintln!("Unknown option '{other}' for `new` (only --python is supported).");
                process::exit(2);
            }
            other if name.is_none() => name = Some(other),
            other => {
                eprintln!("Unexpected argument '{other}' for `new`.");
                process::exit(2);
            }
        }
        idx += 1;
    }
    let Some(name) = name else {
        eprintln!("Usage: {} new <name> [--python] [-o <dir>]", args[0]);
        process::exit(2);
    };

    match capdag::dev::scaffold_python_cartridge(name, &parent) {
        Ok(project_dir) => {
            eprintln!("Scaffolded Python cartridge '{name}' at {}", project_dir.display());
            eprintln!("Next:");
            eprintln!("  pip install capdag            # the cartridge runtime");
            eprintln!("  cd {}", project_dir.display());
            eprintln!("  capdag dev-install .          # install under the local `dev` slug");
            eprintln!("  echo \"I love this\" | capdag {name}");
            println!("{}", project_dir.display());
            process::exit(0);
        }
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    }
}

/// `capdag dev-install <project-dir>` — install (or update) a dev cartridge
/// under the per-user cartridge root's `dev` slug so the capdag host discovers
/// it. Reads the project's manifest, verifies none of its caps conflict with
/// the fabric, then stages it. Re-running overwrites the same version directory
/// — the update step of the edit/reinstall loop.
async fn cmd_dev_install(args: &[String]) -> ! {
    let project_dir = PathBuf::from(args.get(2).map(String::as_str).unwrap_or("."));

    let entry = match capdag::dev::project_entry(&project_dir) {
        Ok(entry) => entry,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };
    let manifest = match capdag::dev::read_entry_manifest(&entry) {
        Ok(manifest) => manifest,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };

    // A dev cartridge may declare caps the fabric does not know, but its aliases
    // must not collide with the fabric. Check every declared cap up front so a
    // conflict is reported before anything is written to disk.
    let registry = fabric_registry_or_exit().await;
    for group in &manifest.cap_groups {
        for cap in &group.caps {
            if let Err(e) = capdag::dev::check_no_fabric_conflict(&registry, cap).await {
                eprintln!("{e}");
                process::exit(1);
            }
        }
    }

    match capdag::dev::stage_dev_cartridge(
        &project_dir,
        &manifest,
        &user_cartridge_dir(),
        capdag::FABRIC_MANIFEST_VERSION,
    ) {
        Ok(version_dir) => {
            eprintln!(
                "Installed dev cartridge '{}' v{} ({}) at {}",
                manifest.name,
                manifest.version,
                manifest.channel.as_str(),
                version_dir.display()
            );
            // Hint the run command using the first non-identity cap alias.
            let run_alias = manifest
                .cap_groups
                .iter()
                .flat_map(|g| g.caps.iter())
                .filter(|c| !c.get_aliases().iter().any(|a| a == "identity"))
                .find_map(|c| c.get_aliases().first().cloned());
            if let Some(alias) = run_alias {
                eprintln!("Run it:  echo \"...\" | {} {alias}", args[0]);
            }
            println!("{}", version_dir.display());
            process::exit(0);
        }
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    }
}

#[cfg(test)]
mod packaging_tests {
    //! Invariants the OS packages (deb/rpm/Homebrew) depend on.
    use super::bundled_cartridges_dir_for_exe;
    use std::fs;

    /// TEST1902: a launcher SYMLINK — the packaging pattern (`/usr/bin/capdag`
    /// → `/opt/capdag/capdag`, Homebrew `bin/capdag` → `libexec/capdag`) — must
    /// resolve to the REAL bundle's `bundled-cartridges/`, not a `bundled-cartridges/` beside the
    /// symlink. This fails if `bundled_cartridges_dir_for_exe` stops canonicalizing.
    #[cfg(unix)]
    #[test]
    fn test1902_cartridges_resolve_through_launcher_symlink() {
        use std::os::unix::fs::symlink;
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("opt/capdag");
        fs::create_dir_all(real.join("bundled-cartridges")).unwrap();
        fs::write(real.join("capdag"), b"binary").unwrap();
        let bindir = tmp.path().join("usr/bin");
        fs::create_dir_all(&bindir).unwrap();
        let link = bindir.join("capdag");
        symlink(real.join("capdag"), &link).unwrap();

        let got = bundled_cartridges_dir_for_exe(&link)
            .expect("bundled-cartridges/ must resolve through the launcher symlink");
        assert_eq!(
            fs::canonicalize(&got).unwrap(),
            fs::canonicalize(real.join("bundled-cartridges")).unwrap(),
        );
    }

    /// TEST1903: no `bundled-cartridges/` beside the binary ⇒ `None` (a bare `cargo`
    /// build / unpackaged binary — not an error, discovery just skips it).
    #[test]
    fn test1903_no_bundled_cartridges_dir_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("capdag"), b"binary").unwrap();
        assert!(bundled_cartridges_dir_for_exe(&tmp.path().join("capdag")).is_none());
    }
}
