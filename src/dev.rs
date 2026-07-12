//! Cartridge-development support for the `capdag` CLI.
//!
//! This module backs three developer commands and the local-manifest run path:
//!
//! - [`scaffold_python_cartridge`] — `capdag new <name> --python`: write a
//!   fresh, runnable Python cartridge project (one custom cap, one Op, one
//!   manifest) into a new directory.
//! - [`stage_dev_cartridge`] — `capdag dev-install <project-dir>`: read the
//!   project's manifest, then copy it under the per-user cartridge root's
//!   reserved `dev` slug so the capdag host (and any other host pointed at that
//!   root) discovers it. Re-running overwrites the same version directory — the
//!   update step of the edit/reinstall loop.
//! - [`find_dev_cap_by_alias`] + [`check_no_fabric_conflict`] — the local-manifest
//!   run path: when `capdag <alias>` names a cap the fabric does NOT define, we
//!   fall back to a locally dev-installed cartridge's OWN manifest and run that
//!   cap through the full bifaci host — **as long as the cap does not conflict
//!   with the fabric** (no alias of it already means a different cap upstream).
//!   A dev cap never needs to be published to be developed and run locally.
//!
//! The on-disk layout mirrors every other host exactly:
//! `{user_cartridge_dir}/dev/v{CARTRIDGE_REGISTRY_VERSION}/{channel}/{name}/{version}/`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::bifaci::cartridge_repo::CartridgeChannel;
use crate::bifaci::cartridge_slug::DEV_SLUG;
use crate::bifaci::manifest::CapManifest;
use crate::cap::definition::Cap;
use crate::fabric::registry::FabricRegistry;

/// The entry-point filename a `--python` scaffold produces and `dev-install`
/// expects. The host execs this file directly (it carries a `#!/usr/bin/env
/// python3` shebang and is made executable), so the cap runs with whatever
/// `python3` + `capdag` the developer's environment provides.
pub const PYTHON_ENTRY: &str = "cartridge.py";

/// Errors from the cartridge-development commands. Each variant is actionable —
/// it names the file, entry, or conflicting alias so the developer can fix it.
#[derive(Debug)]
pub enum DevError {
    Io(String),
    InvalidName(String),
    AlreadyExists(PathBuf),
    NoEntry(PathBuf),
    ManifestSpawn { entry: PathBuf, source: String },
    ManifestFailed { entry: PathBuf, code: Option<i32>, stderr: String },
    ManifestParse { entry: PathBuf, source: String },
    NotDev { registry_url: String },
    FabricConflict { alias: String, dev_urn: String, fabric_urn: String },
}

impl std::fmt::Display for DevError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DevError::Io(m) => write!(f, "{m}"),
            DevError::InvalidName(n) => write!(
                f,
                "invalid cartridge name '{n}': use a lowercase, path-safe name \
                 matching [a-z0-9] with '-' or '_' separators (e.g. sentiment-tagger)"
            ),
            DevError::AlreadyExists(p) => {
                write!(f, "'{}' already exists — pick a new name or remove it first", p.display())
            }
            DevError::NoEntry(p) => write!(
                f,
                "no cartridge entry '{}' found in the project — expected a `{PYTHON_ENTRY}` \
                 file (create the project with `capdag new`)",
                p.display()
            ),
            DevError::ManifestSpawn { entry, source } => write!(
                f,
                "could not run the cartridge entry '{}' to read its manifest: {source}. \
                 Make sure it is executable and its dependencies (capdag) are importable.",
                entry.display()
            ),
            DevError::ManifestFailed { entry, code, stderr } => write!(
                f,
                "the cartridge entry '{}' exited with {} when asked for its manifest:\n{}",
                entry.display(),
                code.map(|c| format!("code {c}")).unwrap_or_else(|| "a signal".to_string()),
                stderr.trim()
            ),
            DevError::ManifestParse { entry, source } => write!(
                f,
                "the cartridge entry '{}' printed a manifest capdag could not parse: {source}",
                entry.display()
            ),
            DevError::NotDev { registry_url } => write!(
                f,
                "this project declares registry_url='{registry_url}' — `dev-install` only \
                 installs DEV cartridges (registry_url must be null). Publish it through the \
                 cartridge registry instead, or set registry_url to null for local development."
            ),
            DevError::FabricConflict { alias, dev_urn, fabric_urn } => write!(
                f,
                "dev cap '{dev_urn}' claims alias '{alias}', but the fabric already maps that \
                 alias to a different cap '{fabric_urn}'. A dev cartridge may declare caps the \
                 fabric does not know, but its aliases must not collide with the fabric. Rename \
                 the dev cap's alias."
            ),
        }
    }
}

impl std::error::Error for DevError {}

fn io_err(context: &str, e: std::io::Error) -> DevError {
    DevError::Io(format!("{context}: {e}"))
}

// ---------------------------------------------------------------------------
// Scaffold — `capdag new <name> --python`
// ---------------------------------------------------------------------------

/// Validate a cartridge project name: a path-safe, lowercase identifier
/// (`[a-z0-9]` with `-`/`_` separators). This is the manifest name, the on-disk
/// folder, and — in the scaffold — the seed for the example cap's alias and
/// URN tags, so it must be a clean slug.
pub fn valid_cartridge_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().next().is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// The Python cartridge source for a fresh scaffold. `__CARTRIDGE_NAME__` is
/// substituted with the project name so a cap's alias/URN are unique per
/// project (they will not collide with the fabric or another scaffold). The
/// example classifies text as positive/neutral/negative; edit `classify()` and
/// the URNs to build your own capability.
pub fn python_cartridge_source(name: &str) -> String {
    PYTHON_CARTRIDGE_TEMPLATE.replace("__CARTRIDGE_NAME__", name)
}

const PYTHON_CARTRIDGE_TEMPLATE: &str = r##"#!/usr/bin/env python3
"""__CARTRIDGE_NAME__ — a MachineFabric cartridge (Python), scaffolded by `capdag new`.

Reads UTF-8 text on stdin and emits a single tag word: `positive`,
`neutral`, or `negative`. This is the smallest useful shape of a cartridge:
one custom cap, one Op, one manifest, one main(). Replace `classify()` and
the input/output media URNs to build your own capability.

Develop it with:
    capdag dev-install .          # install/update under the local `dev` slug
    echo "I love this" | capdag __CARTRIDGE_NAME__
    # edit classify(), then re-run `capdag dev-install .` to update
"""

from capdag.bifaci.cartridge_runtime import (
    CartridgeRuntime,
    Request,
    WET_KEY_REQUEST,
)
from capdag.bifaci.manifest import CapManifest, default_group
from capdag.cap.definition import (
    Cap,
    CapArg,
    CapOutput,
    PositionSource,
    StdinSource,
)
from capdag.standard.caps import CAP_IDENTITY
from capdag.urn.cap_urn import CapUrn, CapUrnBuilder
from ops import DryContext, Op, OpMetadata, WetContext


# --- Domain logic — pure Python, no MachineFabric awareness. ----------------

POSITIVE_WORDS = {
    "good", "great", "love", "happy", "excellent",
    "wonderful", "amazing", "fantastic", "delightful",
}
NEGATIVE_WORDS = {
    "bad", "terrible", "hate", "sad", "awful",
    "disappointing", "horrible", "miserable", "broken",
}


def classify(text: str) -> str:
    """Return one of `positive`, `neutral`, `negative` for the input.

    Case-insensitive whole-word match against two small word lists. Replace
    this with a real model when you graduate from `getting started`.
    """
    tokens = {t.strip(".,!?;:").lower() for t in text.split()}
    pos = len(tokens & POSITIVE_WORDS)
    neg = len(tokens & NEGATIVE_WORDS)
    if pos > neg:
        return "positive"
    if neg > pos:
        return "negative"
    return "neutral"


# --- Op — implements the cap. -----------------------------------------------

class TagOp(Op):
    async def perform(self, dry: DryContext, wet: WetContext) -> None:
        req: Request = wet.get_required(WET_KEY_REQUEST)
        # Drain the (finite) input stream(s) and decode as UTF-8 text.
        text = req.take_input().collect_all_bytes().decode("utf-8")
        # emit_cbor writes one CHUNK frame; the runtime emits END for us.
        req.emitter().emit_cbor(classify(text))

    def metadata(self) -> OpMetadata:
        return (
            OpMetadata.builder("TagOp")
            .description("Classify text as positive / neutral / negative")
            .build()
        )


# --- URN + manifest. Media/cap URNs are seeded from the project name so they -
#     are unique per project and never collide with the published fabric. -----

IN_MEDIA = "media:enc=utf-8;__CARTRIDGE_NAME__-input"
OUT_MEDIA = "media:enc=utf-8;__CARTRIDGE_NAME__-tag"


def _cap_urn() -> CapUrn:
    """Build the cap URN ONCE via the builder, so the string we register with
    matches the runtime's canonical (alphabetically-sorted) byte form."""
    return (
        CapUrnBuilder()
        .marker("__CARTRIDGE_NAME__")
        .in_spec(IN_MEDIA)
        .out_spec(OUT_MEDIA)
        .build()
    )


CAP_URN: str = _cap_urn().to_string()


def build_manifest() -> CapManifest:
    cap = Cap(_cap_urn(), "__CARTRIDGE_NAME__", ["__CARTRIDGE_NAME__"])
    cap.cap_description = "Classify a piece of text as positive, neutral, or negative."
    cap.args = [
        CapArg(
            media_urn=IN_MEDIA,
            required=True,
            sources=[StdinSource(IN_MEDIA), PositionSource(0)],
            arg_description="UTF-8 text to classify.",
        )
    ]
    cap.output = CapOutput(
        media_urn=OUT_MEDIA,
        output_description="One of the literal strings 'positive', 'neutral', or 'negative'.",
    )

    # Every cartridge advertises CAP_IDENTITY; the runtime auto-registers its handler.
    identity = Cap(CapUrn.from_string(CAP_IDENTITY), "Identity", ["identity"])

    return CapManifest(
        name="__CARTRIDGE_NAME__",
        version="0.1.0",
        channel="nightly",          # 'nightly' or 'release'; nightly for dev.
        registry_url=None,           # None => dev cartridge (installed locally).
        description="Classify a piece of text as positive, neutral, or negative.",
        cap_groups=[default_group([identity, cap])],
    )


def main() -> None:
    runtime = CartridgeRuntime.with_manifest(build_manifest())
    runtime.register_op_type(CAP_URN, TagOp)
    runtime.run()


if __name__ == "__main__":
    main()
"##;

fn readme_source(name: &str) -> String {
    format!(
        "# {name}\n\n\
         A MachineFabric cartridge scaffolded by `capdag new`. It reads UTF-8 text on\n\
         stdin and emits `positive`, `neutral`, or `negative`.\n\n\
         ## Develop\n\n\
         ```bash\n\
         # 1. Install capdag (the cartridge runtime) so `{entry}` can import it:\n\
         pip install capdag\n\n\
         # 2. Install this cartridge under the local `dev` slug:\n\
         capdag dev-install .\n\n\
         # 3. Run your cap through the capdag host:\n\
         echo \"I love this\" | capdag {name}\n\
         # => positive\n\n\
         # 4. Edit classify() in {entry}, then re-run step 2 to update the install:\n\
         capdag dev-install .\n\
         ```\n\n\
         The cap is a *dev* cap: it is not published to the fabric, so you can develop\n\
         and run it locally as long as its alias (`{name}`) does not collide with a\n\
         published cap.\n",
        name = name,
        entry = PYTHON_ENTRY,
    )
}

const GITIGNORE_SOURCE: &str = "\
.venv/
__pycache__/
*.pyc
.pytest_cache/
cartridge.json
";

/// Scaffold a new Python cartridge project directory named `name` under
/// `parent_dir`. Returns the created project directory. Fails hard if the name
/// is not path-safe or the target already exists (never overwrites existing
/// work).
pub fn scaffold_python_cartridge(name: &str, parent_dir: &Path) -> Result<PathBuf, DevError> {
    if !valid_cartridge_name(name) {
        return Err(DevError::InvalidName(name.to_string()));
    }
    let project_dir = parent_dir.join(name);
    if project_dir.exists() {
        return Err(DevError::AlreadyExists(project_dir));
    }
    fs::create_dir_all(&project_dir)
        .map_err(|e| io_err(&format!("creating project dir '{}'", project_dir.display()), e))?;

    let entry_path = project_dir.join(PYTHON_ENTRY);
    fs::write(&entry_path, python_cartridge_source(name))
        .map_err(|e| io_err(&format!("writing '{}'", entry_path.display()), e))?;
    make_executable(&entry_path)?;

    fs::write(project_dir.join("README.md"), readme_source(name))
        .map_err(|e| io_err("writing README.md", e))?;
    fs::write(project_dir.join(".gitignore"), GITIGNORE_SOURCE)
        .map_err(|e| io_err("writing .gitignore", e))?;

    Ok(project_dir)
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), DevError> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)
        .map_err(|e| io_err(&format!("stat '{}'", path.display()), e))?
        .permissions();
    perms.set_mode(perms.mode() | 0o755);
    fs::set_permissions(path, perms)
        .map_err(|e| io_err(&format!("chmod +x '{}'", path.display()), e))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), DevError> {
    // On Windows the host launches the entry through its file association /
    // launcher; there is no executable bit to set.
    Ok(())
}

// ---------------------------------------------------------------------------
// Manifest reading — run `<entry> manifest` and parse the CapManifest JSON.
// ---------------------------------------------------------------------------

/// Run a cartridge entry's `manifest` subcommand and parse the printed
/// `CapManifest` JSON. Every cartridge (any language) prints the same wire
/// shape, so a Python cartridge's output deserializes into the Rust type.
pub fn read_entry_manifest(entry: &Path) -> Result<CapManifest, DevError> {
    let output = Command::new(entry)
        .arg("manifest")
        .output()
        .map_err(|e| DevError::ManifestSpawn { entry: entry.to_path_buf(), source: e.to_string() })?;
    if !output.status.success() {
        return Err(DevError::ManifestFailed {
            entry: entry.to_path_buf(),
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    serde_json::from_slice::<CapManifest>(&output.stdout)
        .map_err(|e| DevError::ManifestParse { entry: entry.to_path_buf(), source: e.to_string() })
}

/// The project's entry path (`<project_dir>/cartridge.py`), verifying it exists.
pub fn project_entry(project_dir: &Path) -> Result<PathBuf, DevError> {
    let entry = project_dir.join(PYTHON_ENTRY);
    if !entry.is_file() {
        return Err(DevError::NoEntry(entry));
    }
    Ok(entry)
}

// ---------------------------------------------------------------------------
// dev-install — stage a project under the `dev` slug.
// ---------------------------------------------------------------------------

/// The install version directory for a dev cartridge under `user_cartridge_dir`:
/// `dev/v{CARTRIDGE_REGISTRY_VERSION}/{channel}/{name}/{version}/`.
pub fn dev_version_dir(
    user_cartridge_dir: &Path,
    channel: CartridgeChannel,
    name: &str,
    version: &str,
) -> PathBuf {
    user_cartridge_dir
        .join(DEV_SLUG)
        .join(format!("v{}", crate::CARTRIDGE_REGISTRY_VERSION))
        .join(channel.as_str())
        .join(name)
        .join(version)
}

/// Copy a dev cartridge project into its `dev`-slug version directory and write
/// its `cartridge.json` install record. Overwrites any existing install of the
/// same `(name, version, channel)` — this is the "update" of the edit/reinstall
/// loop. Returns the version directory the cartridge was installed into.
///
/// `manifest` must have already been read from the project (via
/// [`read_entry_manifest`]) and verified to be a dev cartridge (`registry_url`
/// is `None`); this staging step does not itself re-run the entry.
pub fn stage_dev_cartridge(
    project_dir: &Path,
    manifest: &CapManifest,
    user_cartridge_dir: &Path,
    fabric_manifest_version: u32,
) -> Result<PathBuf, DevError> {
    if let Some(url) = &manifest.registry_url {
        return Err(DevError::NotDev { registry_url: url.clone() });
    }
    let version_dir = dev_version_dir(
        user_cartridge_dir,
        manifest.channel,
        &manifest.name,
        &manifest.version,
    );

    // Update semantics: replace the version directory wholesale so a removed
    // file in the project does not linger in a stale install.
    if version_dir.exists() {
        fs::remove_dir_all(&version_dir)
            .map_err(|e| io_err(&format!("clearing old install '{}'", version_dir.display()), e))?;
    }
    fs::create_dir_all(&version_dir)
        .map_err(|e| io_err(&format!("creating '{}'", version_dir.display()), e))?;

    copy_project_tree(project_dir, &version_dir)?;
    make_executable(&version_dir.join(PYTHON_ENTRY))?;

    let cj = crate::CartridgeJson {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        channel: manifest.channel,
        registry_url: None,
        entry: PYTHON_ENTRY.to_string(),
        installed_at: {
            use std::time::SystemTime;
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("system clock before epoch");
            format!("{}Z", now.as_secs())
        },
        installed_from: Some(crate::CartridgeInstallSource::Dev),
        source_url: String::new(),
        package_sha256: String::new(),
        package_size: 0,
        fabric_manifest_version,
    };
    cj.write_to_dir(&version_dir)
        .map_err(|e| DevError::Io(format!("writing cartridge.json: {e}")))?;

    Ok(version_dir)
}

/// Directory/file names never copied into an install (developer scratch that
/// would bloat or break the install).
fn is_ignored_project_entry(name: &str) -> bool {
    matches!(name, ".venv" | "__pycache__" | ".git" | ".pytest_cache" | "cartridge.json")
        || name.ends_with(".pyc")
}

/// Recursively copy a project tree into `dst`, skipping developer scratch.
fn copy_project_tree(src: &Path, dst: &Path) -> Result<(), DevError> {
    for entry in fs::read_dir(src)
        .map_err(|e| io_err(&format!("reading project dir '{}'", src.display()), e))?
    {
        let entry = entry.map_err(|e| io_err("reading a project entry", e))?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if is_ignored_project_entry(&name_str) {
            continue;
        }
        let from = entry.path();
        let to = dst.join(&name);
        let file_type = entry
            .file_type()
            .map_err(|e| io_err(&format!("stat '{}'", from.display()), e))?;
        if file_type.is_dir() {
            fs::create_dir_all(&to)
                .map_err(|e| io_err(&format!("creating '{}'", to.display()), e))?;
            copy_project_tree(&from, &to)?;
        } else {
            fs::copy(&from, &to)
                .map_err(|e| io_err(&format!("copying '{}'", from.display()), e))?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Local-manifest run path — resolve a dev cap by alias, guard against conflict.
// ---------------------------------------------------------------------------

/// Scan the dev-installed cartridges under `user_cartridge_dir/dev/…` and return
/// the cap whose declared `aliases` contain `alias`, along with the version
/// directory it was found in. Reads each dev cartridge's manifest by running its
/// entry's `manifest` subcommand. Returns `Ok(None)` when no dev cartridge
/// advertises the alias (the caller then reports the normal "unknown cap" error).
///
/// Alias uniqueness makes at most one match meaningful; the first match wins.
pub fn find_dev_cap_by_alias(
    user_cartridge_dir: &Path,
    alias: &str,
) -> Result<Option<(Cap, PathBuf)>, DevError> {
    let dev_root = user_cartridge_dir
        .join(DEV_SLUG)
        .join(format!("v{}", crate::CARTRIDGE_REGISTRY_VERSION));
    if !dev_root.is_dir() {
        return Ok(None);
    }
    // dev/v{N}/{channel}/{name}/{version}/
    for version_dir in walk_version_dirs(&dev_root)? {
        let cj_path = version_dir.join("cartridge.json");
        if !cj_path.is_file() {
            continue;
        }
        let bytes = fs::read(&cj_path)
            .map_err(|e| io_err(&format!("reading '{}'", cj_path.display()), e))?;
        let cj: crate::CartridgeJson = match serde_json::from_slice(&bytes) {
            Ok(cj) => cj,
            Err(_) => continue, // a malformed dev install is surfaced elsewhere; skip here.
        };
        let entry = version_dir.join(&cj.entry);
        if !entry.is_file() {
            continue;
        }
        let manifest = read_entry_manifest(&entry)?;
        for group in &manifest.cap_groups {
            for cap in &group.caps {
                if cap.get_aliases().iter().any(|a| a == alias) {
                    return Ok(Some((cap.clone(), version_dir)));
                }
            }
        }
    }
    Ok(None)
}

/// Collect every `.../{channel}/{name}/{version}/` directory three levels below
/// `dev_root` (channel → name → version).
fn walk_version_dirs(dev_root: &Path) -> Result<Vec<PathBuf>, DevError> {
    let mut out = Vec::new();
    for channel in read_subdirs(dev_root)? {
        for name in read_subdirs(&channel)? {
            for version in read_subdirs(&name)? {
                out.push(version);
            }
        }
    }
    Ok(out)
}

fn read_subdirs(dir: &Path) -> Result<Vec<PathBuf>, DevError> {
    let mut out = Vec::new();
    for entry in
        fs::read_dir(dir).map_err(|e| io_err(&format!("reading '{}'", dir.display()), e))?
    {
        let entry = entry.map_err(|e| io_err("reading a directory entry", e))?;
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            out.push(entry.path());
        }
    }
    out.sort();
    Ok(out)
}

/// Verify a dev cap does not conflict with the fabric: none of its aliases may
/// already resolve, in the fabric, to a DIFFERENT cap URN. A dev cartridge is
/// free to declare caps the fabric does not know (that is the whole point of
/// local development); it just may not hijack a name the fabric already owns for
/// something else. An alias the fabric does not define at all is fine.
pub async fn check_no_fabric_conflict(registry: &FabricRegistry, cap: &Cap) -> Result<(), DevError> {
    let dev_urn = cap.urn.to_string();
    for alias in cap.get_aliases() {
        if let Ok(target) = registry.resolve_alias(alias).await {
            // Compare canonical forms — resolve_alias returns the target URN
            // string; a dev cap providing the SAME fabric cap (e.g. identity) is
            // not a conflict.
            let fabric_urn = match crate::CapUrn::from_string(&target) {
                Ok(u) => u.to_string(),
                Err(_) => target.clone(),
            };
            if fabric_urn != dev_urn {
                return Err(DevError::FabricConflict {
                    alias: alias.clone(),
                    dev_urn,
                    fabric_urn,
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cap::definition::Cap;
    use crate::fabric::alias::StoredAlias;
    use crate::urn::cap_urn::CapUrn;

    fn temp_root(tag: &str) -> PathBuf {
        // A unique-per-test dir under the OS temp root. No Date/rand available
        // in the crate's normal build, so key on the test tag + process id.
        let base = std::env::temp_dir().join(format!("capdag-dev-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    // TEST8100: the Python scaffold writes a runnable-shaped project — the entry
    // exists, is executable, carries the current-regime API (aliases, enc=utf-8,
    // NO `command=`/`textable`), and substitutes the project name into the
    // manifest, alias, and media URNs.
    #[test]
    fn test8100_scaffold_python_cartridge_shape() {
        let root = temp_root("scaffold");
        let proj = scaffold_python_cartridge("mood-tagger", &root).unwrap();
        assert_eq!(proj, root.join("mood-tagger"));

        let entry = proj.join(PYTHON_ENTRY);
        let src = fs::read_to_string(&entry).unwrap();
        assert!(src.contains("name=\"mood-tagger\""), "manifest name substituted");
        assert!(src.contains("[\"mood-tagger\"]"), "cap alias seeded from name");
        assert!(src.contains("media:enc=utf-8;mood-tagger-input"), "input media uses enc=utf-8");
        assert!(!src.contains("command="), "no removed `command=` field");
        assert!(!src.contains("textable"), "no removed `textable` marker");
        assert!(proj.join("README.md").is_file());
        assert!(proj.join(".gitignore").is_file());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&entry).unwrap().permissions().mode();
            assert!(mode & 0o111 != 0, "entry is executable");
        }
    }

    // TEST8101: scaffolding rejects a bad name and refuses to overwrite.
    #[test]
    fn test8101_scaffold_guards() {
        let root = temp_root("guards");
        assert!(matches!(
            scaffold_python_cartridge("Bad Name", &root),
            Err(DevError::InvalidName(_))
        ));
        scaffold_python_cartridge("greeter", &root).unwrap();
        assert!(matches!(
            scaffold_python_cartridge("greeter", &root),
            Err(DevError::AlreadyExists(_))
        ));
    }

    /// Write a stub cartridge entry (a bash script) that prints a canned
    /// `CapManifest` JSON on `manifest`. Lets us exercise the capdag-side
    /// staging/parsing/resolution without the Python runtime.
    #[cfg(unix)]
    fn write_stub_entry(dir: &Path, name: &str, alias: &str, urn: &str) -> PathBuf {
        // The cap URN quotes its media specs; escape those quotes for JSON.
        let urn_json = urn.replace('"', "\\\"");
        let manifest = format!(
            r#"{{"name":"{name}","version":"0.1.0","channel":"nightly","registry_url":null,"description":"stub","cap_groups":[{{"name":"default","caps":[{{"urn":"cap:effect=none","title":"Identity","aliases":["identity"]}},{{"urn":"{urn_json}","title":"{name}","aliases":["{alias}"]}}]}}]}}"#
        );
        let script = format!("#!/usr/bin/env bash\nif [ \"$1\" = manifest ]; then\n  cat <<'EOF'\n{manifest}\nEOF\nfi\n");
        let path = dir.join(PYTHON_ENTRY);
        fs::write(&path, script).unwrap();
        make_executable(&path).unwrap();
        path
    }

    // TEST8102: read_entry_manifest + stage_dev_cartridge + find_dev_cap_by_alias
    // round-trip: a stub project installs under dev/v{N}/nightly/<name>/<ver>/,
    // writes a cartridge.json, and its custom cap is resolvable by alias.
    #[cfg(unix)]
    #[test]
    fn test8102_dev_install_and_find_by_alias() {
        let root = temp_root("install");
        let project = root.join("proj");
        fs::create_dir_all(&project).unwrap();
        let urn = "cap:greet;in=\"media:enc=utf-8\";out=\"media:enc=utf-8;greeting\"";
        write_stub_entry(&project, "greeter", "greet", urn);

        let user_dir = root.join("cartridges");
        let entry = project_entry(&project).unwrap();
        let manifest = read_entry_manifest(&entry).unwrap();
        assert_eq!(manifest.name, "greeter");
        assert!(manifest.registry_url.is_none());

        let version_dir = stage_dev_cartridge(&project, &manifest, &user_dir, 7).unwrap();
        assert!(version_dir.ends_with(format!(
            "dev/v{}/nightly/greeter/0.1.0",
            crate::CARTRIDGE_REGISTRY_VERSION
        )));
        assert!(version_dir.join("cartridge.json").is_file());
        assert!(version_dir.join(PYTHON_ENTRY).is_file());

        let found = find_dev_cap_by_alias(&user_dir, "greet").unwrap();
        let (cap, dir) = found.expect("dev cap resolvable by alias");
        assert_eq!(dir, version_dir);
        assert!(cap.get_aliases().iter().any(|a| a == "greet"));
        // An alias no dev cartridge advertises resolves to nothing.
        assert!(find_dev_cap_by_alias(&user_dir, "nope").unwrap().is_none());
    }

    // TEST8103: stage_dev_cartridge refuses a non-dev project (registry_url set).
    #[cfg(unix)]
    #[test]
    fn test8103_dev_install_rejects_published_manifest() {
        let root = temp_root("nondev");
        let project = root.join("proj");
        fs::create_dir_all(&project).unwrap();
        write_stub_entry(&project, "pub", "pub-cap", "cap:effect=none;pub");
        let entry = project_entry(&project).unwrap();
        let mut manifest = read_entry_manifest(&entry).unwrap();
        manifest.registry_url = Some("https://cartridges.example.com/v1/manifest".to_string());
        assert!(matches!(
            stage_dev_cartridge(&project, &manifest, &root.join("c"), 7),
            Err(DevError::NotDev { .. })
        ));
    }

    // TEST8104: the fabric-conflict guard — a dev cap whose alias the fabric maps
    // to a DIFFERENT cap is rejected; a brand-new alias, and a dev cap that
    // matches an existing fabric cap exactly, are both accepted.
    #[tokio::test]
    async fn test8104_fabric_conflict_guard() {
        let registry = FabricRegistry::new_for_test();
        // Seed the fabric with a cap `alpha` at a known URN, and publish its
        // alias into the warm alias cache (as the real publisher would).
        let alpha = Cap::new(
            CapUrn::from_string("cap:alpha;in=\"media:enc=utf-8\";out=\"media:enc=utf-8;alpha\"")
                .unwrap(),
            "Alpha".to_string(),
            vec!["alpha".to_string()],
        );
        let alpha_urn = alpha.urn.to_string();
        registry.add_caps_to_cache(vec![alpha.clone()]);
        registry.add_aliases_to_cache(vec![StoredAlias {
            name: "alpha".to_string(),
            target: alpha_urn.clone(),
            version: 1,
        }]);

        // A dev cap claiming `alpha` but with a DIFFERENT URN => conflict.
        let clashing = Cap::new(
            CapUrn::from_string("cap:beta;in=\"media:enc=utf-8\";out=\"media:enc=utf-8;beta\"")
                .unwrap(),
            "Clash".to_string(),
            vec!["alpha".to_string()],
        );
        assert!(matches!(
            check_no_fabric_conflict(&registry, &clashing).await,
            Err(DevError::FabricConflict { .. })
        ));

        // A brand-new alias the fabric never heard of => fine.
        let fresh = Cap::new(
            CapUrn::from_string("cap:gamma;in=\"media:enc=utf-8\";out=\"media:enc=utf-8;gamma\"")
                .unwrap(),
            "Fresh".to_string(),
            vec!["gamma".to_string()],
        );
        assert!(check_no_fabric_conflict(&registry, &fresh).await.is_ok());

        // The very same fabric cap (same alias => same URN) => not a conflict.
        assert!(check_no_fabric_conflict(&registry, &alpha).await.is_ok());
    }
}
