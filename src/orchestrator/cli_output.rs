//! CLI output emission — the pipe-discipline contract shared by
//! `capdag run <machine>` and the single-cap mode.
//!
//! - Exactly ONE terminal, scalar, single item, and no `--output` dir given
//!   → the item's raw bytes go to STDOUT (logs/progress are on stderr), so
//!   `capdag pdf2summary doc.pdf | wc -w` behaves like a Unix tool.
//! - Anything else (sequence output, several terminals, or an explicit
//!   `--output`) → each item is written as a FILE in the output directory
//!   (default: the current directory), named
//!   `{input_stem}.{output_node_id}[.{index}].{ext}`, and every written
//!   absolute path is printed to stdout, one per line.
//! - An existing file is a hard error unless `--force` — silent overwrites
//!   destroy user data.
//!
//! The extension derives deterministically from the terminal's media URN —
//! `ext=<v>` wins (the declared file type), else `fmt=<v>` (serialization),
//! else `enc=utf-8` ⇒ `txt`, else `bin`. This is the defined naming scheme
//! of the CLI, not a fallback chain: every media URN maps to exactly one
//! name by these rules.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::orchestrator::execute_plan::PipelineResult;
use crate::urn::media_urn::MediaUrn;

/// Options for [`emit_terminals`].
pub struct EmitOptions {
    /// Where files go. `None` = stdout is allowed for the single-scalar
    /// case and files default to the current directory otherwise.
    pub output_dir: Option<PathBuf>,
    /// Overwrite existing files instead of hard-erroring.
    pub force: bool,
    /// Stem for produced file names — the input file's stem, or `stdin`.
    pub input_stem: String,
}

/// File extension for a terminal's media URN, per the naming scheme above.
pub fn extension_for_media(media_urn: &str) -> String {
    let Ok(parsed) = MediaUrn::from_string(media_urn) else {
        return "bin".to_string();
    };
    if let Some(ext) = parsed.get_tag("ext") {
        return ext.to_string();
    }
    if let Some(fmt) = parsed.get_tag("fmt") {
        return fmt.to_string();
    }
    if parsed.get_tag("enc").is_some() {
        return "txt".to_string();
    }
    "bin".to_string()
}

/// Emit a pipeline result per the pipe-discipline contract. Returns the
/// absolute paths of files written (empty when the result went to stdout).
/// `stdout` is injected so tests can capture it.
pub fn emit_terminals(
    result: &PipelineResult,
    options: &EmitOptions,
    stdout: &mut dyn Write,
) -> Result<Vec<PathBuf>, String> {
    // The stdout fast path: one terminal, scalar, one item, no explicit dir.
    if options.output_dir.is_none() && result.terminals.len() == 1 {
        let terminal = &result.terminals[0];
        if !terminal.is_sequence && terminal.items.len() == 1 {
            stdout
                .write_all(&terminal.items[0].data)
                .map_err(|e| format!("failed to write result to stdout: {e}"))?;
            stdout
                .flush()
                .map_err(|e| format!("failed to flush stdout: {e}"))?;
            return Ok(Vec::new());
        }
    }

    let dir = options
        .output_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("failed to create output dir {dir:?}: {e}"))?;

    let mut written: Vec<PathBuf> = Vec::new();
    for terminal in &result.terminals {
        let ext = extension_for_media(&terminal.media_urn);
        let multi_item = terminal.is_sequence || terminal.items.len() > 1;
        for item in &terminal.items {
            let name = if multi_item {
                format!(
                    "{}.{}.{}.{}",
                    options.input_stem, terminal.output_node_id, item.index, ext
                )
            } else {
                format!("{}.{}.{}", options.input_stem, terminal.output_node_id, ext)
            };
            let path = dir.join(&name);
            if path.exists() && !options.force {
                return Err(format!(
                    "refusing to overwrite existing file {path:?} (pass --force to allow)"
                ));
            }
            std::fs::write(&path, &item.data)
                .map_err(|e| format!("failed to write {path:?}: {e}"))?;
            let absolute = absolutize(&path);
            writeln!(stdout, "{}", absolute.display())
                .map_err(|e| format!("failed to report written path: {e}"))?;
            written.push(absolute);
        }
    }
    stdout
        .flush()
        .map_err(|e| format!("failed to flush stdout: {e}"))?;
    Ok(written)
}

fn absolutize(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::execute_plan::{OutputItem, TerminalOutput};

    fn terminal(node: &str, media: &str, is_sequence: bool, items: Vec<&[u8]>) -> TerminalOutput {
        TerminalOutput {
            output_node_id: node.to_string(),
            items: items
                .into_iter()
                .enumerate()
                .map(|(index, data)| OutputItem {
                    index,
                    data: data.to_vec(),
                })
                .collect(),
            is_sequence,
            media_urn: media.to_string(),
            writer_results: Vec::new(),
        }
    }

    fn result_of(terminals: Vec<TerminalOutput>) -> PipelineResult {
        PipelineResult {
            terminals,
            body_outcomes: Vec::new(),
        }
    }

    // TEST8042: the emission contract — single scalar terminal streams raw
    // bytes to stdout; sequences/multi-terminal results write files with the
    // contract names and list their paths on stdout; an existing file
    // without --force is a hard error and --force overwrites.
    #[test]
    fn test8042_emit_terminals_contract() {
        // 1. Single scalar → raw stdout bytes, no files.
        let result = result_of(vec![terminal(
            "output",
            "media:enc=utf-8;summary",
            false,
            vec![b"the summary text"],
        )]);
        let mut stdout: Vec<u8> = Vec::new();
        let written = emit_terminals(
            &result,
            &EmitOptions {
                output_dir: None,
                force: false,
                input_stem: "doc".to_string(),
            },
            &mut stdout,
        )
        .unwrap();
        assert!(written.is_empty());
        assert_eq!(stdout, b"the summary text");

        // 2. Sequence → one file per item, named {stem}.{node}.{index}.{ext},
        //    paths listed on stdout.
        let dir = tempfile::tempdir().unwrap();
        let result = result_of(vec![terminal(
            "output",
            "media:ext=png;image",
            true,
            vec![b"png0", b"png1"],
        )]);
        let mut stdout: Vec<u8> = Vec::new();
        let written = emit_terminals(
            &result,
            &EmitOptions {
                output_dir: Some(dir.path().to_path_buf()),
                force: false,
                input_stem: "doc".to_string(),
            },
            &mut stdout,
        )
        .unwrap();
        assert_eq!(written.len(), 2);
        assert!(written[0].ends_with("doc.output.0.png"), "{written:?}");
        assert!(written[1].ends_with("doc.output.1.png"), "{written:?}");
        assert_eq!(std::fs::read(&written[0]).unwrap(), b"png0");
        let listing = String::from_utf8(stdout).unwrap();
        assert_eq!(listing.lines().count(), 2, "one path per line: {listing}");

        // 3. Multi-terminal (fan-out) → files even for scalars, one per
        //    terminal, extension from each terminal's own media.
        let dir = tempfile::tempdir().unwrap();
        let result = result_of(vec![
            terminal("thumb", "media:ext=png;image", false, vec![b"png"]),
            terminal("text", "media:enc=utf-8", false, vec![b"txt"]),
        ]);
        let mut stdout: Vec<u8> = Vec::new();
        let written = emit_terminals(
            &result,
            &EmitOptions {
                output_dir: Some(dir.path().to_path_buf()),
                force: false,
                input_stem: "doc".to_string(),
            },
            &mut stdout,
        )
        .unwrap();
        assert_eq!(written.len(), 2);
        assert!(written[0].ends_with("doc.thumb.png"), "{written:?}");
        assert!(written[1].ends_with("doc.text.txt"), "{written:?}");

        // 4. Existing file without --force = hard error; with --force it
        //    overwrites.
        let mut stdout: Vec<u8> = Vec::new();
        let err = emit_terminals(
            &result,
            &EmitOptions {
                output_dir: Some(dir.path().to_path_buf()),
                force: false,
                input_stem: "doc".to_string(),
            },
            &mut stdout,
        )
        .unwrap_err();
        assert!(err.contains("refusing to overwrite"), "{err}");
        let mut stdout: Vec<u8> = Vec::new();
        emit_terminals(
            &result,
            &EmitOptions {
                output_dir: Some(dir.path().to_path_buf()),
                force: true,
                input_stem: "doc".to_string(),
            },
            &mut stdout,
        )
        .expect("--force must overwrite");

        // 5. Extension derivation: ext= wins over fmt=; enc= alone ⇒ txt;
        //    no axis ⇒ bin.
        assert_eq!(extension_for_media("media:ext=pdf;fmt=json"), "pdf");
        assert_eq!(extension_for_media("media:fmt=ndjson;record"), "ndjson");
        assert_eq!(extension_for_media("media:enc=utf-8;summary"), "txt");
        assert_eq!(extension_for_media("media:embedding-vector"), "bin");
    }
}
