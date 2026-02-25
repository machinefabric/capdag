//! Real-world multi-cartridge chain tests for macino
//!
//! Unlike the testcartridge integration tests (which use synthetic test caps),
//! these tests exercise real cartridges (pdfcartridge, txtcartridge, modelcartridge,
//! candlecartridge, ggufcartridge) through multi-step pipelines with real input data.
//!
//! Prerequisites:
//! - Cartridge binaries must be pre-built (`cargo build --release` in each cartridge dir)
//! - ML-dependent tests require pre-downloaded models
//! - Tests skip with a clear message when binaries are missing

use capns::{Cap, CapUrn, CapUrnBuilder};
use macino::{
    executor::{execute_dag, NodeData},
    parse_dot_to_cap_dag, CapRegistryTrait, ParseOrchestrationError,
};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use tempfile::TempDir;

// =============================================================================
// Cap URN Builders — mirror the exact builder calls in each cartridge
// =============================================================================

// -- pdfcartridge caps (matches standard/caps.rs helpers with MEDIA_PDF input) --

fn pdf_generate_thumbnail() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "generate_thumbnail")
        .in_spec("media:pdf")
        .out_spec("media:image;png;thumbnail")
        .build()
        .expect("pdf generate_thumbnail URN")
}

fn pdf_extract_metadata() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "extract_metadata")
        .in_spec("media:pdf")
        .out_spec("media:file-metadata;textable;form=map")
        .build()
        .expect("pdf extract_metadata URN")
}

fn pdf_disbind() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "disbind")
        .in_spec("media:pdf")
        .out_spec("media:disbound-page;textable;form=list")
        .build()
        .expect("pdf disbind URN")
}

fn pdf_extract_outline() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "extract_outline")
        .in_spec("media:pdf")
        .out_spec("media:document-outline;textable;form=map")
        .build()
        .expect("pdf extract_outline URN")
}

// -- txtcartridge caps (matches standard/caps.rs helpers with MEDIA_MD input) --

fn md_generate_thumbnail() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "generate_thumbnail")
        .in_spec("media:md;textable")
        .out_spec("media:image;png;thumbnail")
        .build()
        .expect("md generate_thumbnail URN")
}

fn md_extract_metadata() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "extract_metadata")
        .in_spec("media:md;textable")
        .out_spec("media:file-metadata;textable;form=map")
        .build()
        .expect("md extract_metadata URN")
}

fn md_extract_outline() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "extract_outline")
        .in_spec("media:md;textable")
        .out_spec("media:document-outline;textable;form=map")
        .build()
        .expect("md extract_outline URN")
}

// -- candlecartridge caps (matches candlecartridge/src/main.rs builders) --

fn candle_text_embeddings() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "generate_embeddings")
        .solo_tag("ml-model")
        .solo_tag("candle")
        .in_spec("media:textable;form=scalar")
        .out_spec("media:embedding-vector;textable;form=map")
        .build()
        .expect("candle text embeddings URN")
}

fn candle_embeddings_dimensions() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "embeddings_dimensions")
        .solo_tag("ml-model")
        .solo_tag("candle")
        .in_spec("media:model-spec;textable;form=scalar")
        .out_spec("media:model-dim;integer;textable;numeric;form=scalar")
        .build()
        .expect("candle embeddings_dimensions URN")
}

fn candle_image_embeddings() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "generate_image_embeddings")
        .solo_tag("ml-model")
        .solo_tag("candle")
        .in_spec("media:image;png")  // no bytes tag — retired
        .out_spec("media:embedding-vector;textable;form=map")
        .build()
        .expect("candle image embeddings URN")
}

fn candle_caption() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "generate_caption")
        .solo_tag("ml-model")
        .solo_tag("candle")
        .in_spec("media:image;png")  // no bytes tag — retired
        .out_spec("media:image-caption;textable;form=map")
        .build()
        .expect("candle caption URN")
}

fn candle_transcribe() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "transcribe")
        .solo_tag("ml-model")
        .solo_tag("candle")
        .in_spec("media:audio;wav;speech")  // no bytes tag — retired
        .out_spec("media:transcription;textable;form=map")
        .build()
        .expect("candle transcribe URN")
}

// -- modelcartridge caps (matches modelcartridge/src/main.rs) --

fn model_availability() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "model-availability")
        .in_spec("media:model-spec;textable;form=scalar")
        .out_spec("media:model-availability;textable;form=map")
        .build()
        .expect("model-availability URN")
}

fn model_status() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "model-status")
        .in_spec("media:model-spec;textable;form=scalar")
        .out_spec("media:model-status;textable;form=map")
        .build()
        .expect("model-status URN")
}

fn model_contents() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "model-contents")
        .in_spec("media:model-spec;textable;form=scalar")
        .out_spec("media:model-contents;textable;form=map")
        .build()
        .expect("model-contents URN")
}

fn model_path() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "model-path")
        .in_spec("media:model-spec;textable;form=scalar")
        .out_spec("media:model-path;textable;form=map")
        .build()
        .expect("model-path URN")
}

fn model_download() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "download-model")
        .in_spec("media:model-spec;textable;form=scalar")
        .out_spec("media:download-result;textable;form=map")
        .build()
        .expect("model download URN")
}

// =============================================================================
// CartridgeRegistry — populated from the cap URN builders above
// =============================================================================

struct CartridgeRegistry {
    caps: HashMap<String, Cap>,
}

impl CartridgeRegistry {
    fn new() -> Self {
        Self {
            caps: HashMap::new(),
        }
    }

    fn register(&mut self, urn: CapUrn) {
        let op = urn
            .get_tag("op")
            .map(|s| s.to_string())
            .unwrap_or_default();
        let cap = Cap {
            urn: urn.clone(),
            title: format!("Cap {}", op),
            cap_description: None,
            metadata: HashMap::new(),
            command: "cartridge".to_string(),
            media_specs: vec![],
            args: vec![],
            output: None,
            metadata_json: None,
            registered_by: None,
        };
        self.caps.insert(urn.to_string(), cap);
    }
}

#[async_trait::async_trait]
impl CapRegistryTrait for CartridgeRegistry {
    async fn lookup(&self, urn: &str) -> Result<Cap, ParseOrchestrationError> {
        let normalized = CapUrn::from_string(urn)
            .map_err(|e| ParseOrchestrationError::CapUrnParseError(format!("{:?}", e)))?
            .to_string();

        self.caps
            .iter()
            .find(|(k, _)| {
                if let Ok(k_norm) = CapUrn::from_string(k) {
                    k_norm.to_string() == normalized
                } else {
                    false
                }
            })
            .map(|(_, v)| v.clone())
            .ok_or_else(|| ParseOrchestrationError::CapNotFound {
                cap_urn: urn.to_string(),
            })
    }
}

// =============================================================================
// Binary Discovery
// =============================================================================

/// Find the most recent release binary for a cartridge.
/// Looks for both unversioned (e.g., `pdfcartridge`) and versioned (e.g., `pdfcartridge-0.93.6217`)
/// names in the cartridge's `target/release/` directory.
fn find_cartridge_binary(name: &str) -> Option<PathBuf> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;
    let release_dir = PathBuf::from(&manifest_dir)
        .parent()? // capns/
        .parent()? // filegrind/
        .join(name)
        .join("target")
        .join("release");

    if !release_dir.exists() {
        return None;
    }

    // Try exact name first
    let exact = release_dir.join(name);
    if exact.is_file() {
        return Some(exact);
    }

    // Try versioned names: find most recent file matching <name>-*
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(&release_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|f| f.to_str())
                    .map_or(false, |f| {
                        f.starts_with(&format!("{}-", name))
                            && !f.ends_with(".d")
                            && !f.ends_with(".dSYM")
                    })
        })
        .collect();

    // Sort by modification time, most recent first
    candidates.sort_by(|a, b| {
        b.metadata()
            .and_then(|m| m.modified())
            .ok()
            .cmp(&a.metadata().and_then(|m| m.modified()).ok())
    });

    candidates.into_iter().next()
}

/// Require specific cartridge binaries. Returns paths or prints skip message and returns None.
fn require_binaries(names: &[&str]) -> Option<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for &name in names {
        match find_cartridge_binary(name) {
            Some(path) => {
                eprintln!("[CartridgeTest] Found {}: {:?}", name, path);
                paths.push(path);
            }
            None => {
                eprintln!(
                    "SKIPPED: {} binary not found. Build with: cd ../../{} && cargo build --release",
                    name, name
                );
                return None;
            }
        }
    }
    Some(paths)
}

// =============================================================================
// Test Fixture Generators
// =============================================================================

/// Generate a minimal valid PDF with one blank page.
fn generate_test_pdf() -> Vec<u8> {
    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.0\n");

    let obj1_start = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    let obj2_start = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    let obj3_start = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
    );

    let xref_start = pdf.len();
    pdf.extend_from_slice(b"xref\n0 4\n");
    // Each xref entry is exactly 20 bytes: offset(10) + space + gen(5) + space + keyword + space + LF
    pdf.extend_from_slice(format!("{:010} 65535 f \n", 0).as_bytes());
    pdf.extend_from_slice(format!("{:010} 00000 n \n", obj1_start).as_bytes());
    pdf.extend_from_slice(format!("{:010} 00000 n \n", obj2_start).as_bytes());
    pdf.extend_from_slice(format!("{:010} 00000 n \n", obj3_start).as_bytes());
    pdf.extend_from_slice(b"trailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n");
    pdf.extend_from_slice(format!("{}\n%%EOF\n", xref_start).as_bytes());

    pdf
}

/// Generate a simple markdown document.
fn generate_test_markdown() -> Vec<u8> {
    b"# Test Document\n\n## Section One\n\nThis is a test document for macino integration tests.\n\n## Section Two\n\nMore content here.\n".to_vec()
}

/// CRC32 computation for PNG chunks.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Build a PNG chunk with correct CRC.
fn png_chunk(chunk_type: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = Vec::new();
    chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
    chunk.extend_from_slice(chunk_type);
    chunk.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(chunk_type);
    crc_input.extend_from_slice(data);
    chunk.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    chunk
}

/// Adler32 checksum for zlib.
fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// Wrap raw data in a zlib container using stored (uncompressed) deflate blocks.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    // CMF=0x78 (deflate, 32K window), FLG=0x01 (check: (0x78*256+0x01) % 31 == 0)
    out.push(0x78);
    out.push(0x01);

    // Split into stored blocks (max 65535 bytes each)
    let mut offset = 0;
    while offset < data.len() {
        let remaining = data.len() - offset;
        let block_size = remaining.min(65535);
        let is_final = offset + block_size >= data.len();

        out.push(if is_final { 0x01 } else { 0x00 });
        out.extend_from_slice(&(block_size as u16).to_le_bytes());
        out.extend_from_slice(&(!(block_size as u16)).to_le_bytes());
        out.extend_from_slice(&data[offset..offset + block_size]);

        offset += block_size;
    }

    // Handle empty data: emit one final empty stored block
    if data.is_empty() {
        out.push(0x01);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(!0u16).to_le_bytes());
    }

    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// Generate a valid PNG image (32x32 solid color, RGB).
fn generate_test_png(width: u32, height: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
    let mut png = Vec::new();

    // PNG signature
    png.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);

    // IHDR
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(2); // color type: RGB
    ihdr.push(0); // compression: deflate
    ihdr.push(0); // filter: adaptive
    ihdr.push(0); // interlace: none
    png.extend_from_slice(&png_chunk(b"IHDR", &ihdr));

    // Raw pixel data: filter_byte(0) + RGB per pixel, per row
    let mut raw = Vec::with_capacity((1 + width as usize * 3) * height as usize);
    for _ in 0..height {
        raw.push(0); // filter: None
        for _ in 0..width {
            raw.extend_from_slice(&[r, g, b]);
        }
    }

    // IDAT: zlib-compressed pixel data
    let compressed = zlib_stored(&raw);
    png.extend_from_slice(&png_chunk(b"IDAT", &compressed));

    // IEND
    png.extend_from_slice(&png_chunk(b"IEND", &[]));

    png
}

/// Generate a minimal WAV file (16kHz 16-bit mono PCM, 0.1s silence).
fn generate_test_wav() -> Vec<u8> {
    let sample_rate: u32 = 16000;
    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let num_samples: u32 = sample_rate / 10; // 0.1 seconds
    let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_size = num_samples * block_align as u32;
    let file_size = 36 + data_size;

    let mut wav = Vec::with_capacity(44 + data_size as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&file_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    wav.extend_from_slice(&num_channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    // Silence: all zeros
    wav.resize(wav.len() + data_size as usize, 0);
    wav
}

// =============================================================================
// DOT Construction Helpers
// =============================================================================

/// Escape a cap URN string for use inside a DOT label attribute.
/// The URN's internal double quotes become escaped quotes in the DOT string.
fn escape_for_dot(cap_urn: &str) -> String {
    cap_urn.replace('"', "\\\"")
}

/// Build a DOT edge line from node names and a cap URN.
fn dot_edge(from: &str, to: &str, cap_urn: &CapUrn) -> String {
    format!(
        "        {} -> {} [label=\"{}\"];",
        from,
        to,
        escape_for_dot(&cap_urn.to_string())
    )
}

/// Build a complete DOT digraph from a name and edge lines.
fn dot_graph(name: &str, edges: &[String]) -> String {
    format!(
        "    digraph {} {{\n{}\n    }}",
        name,
        edges.join("\n")
    )
}

// =============================================================================
// ML Model Specs (matching candlecartridge defaults)
// =============================================================================

const MODEL_BERT: &str = "hf:sentence-transformers/all-MiniLM-L6-v2?include=*.json,*.safetensors";
const MODEL_CLIP: &str = "hf:openai/clip-vit-base-patch32?include=*.json,*.safetensors,pytorch_model.bin";
const MODEL_BLIP: &str = "hf:Salesforce/blip-image-captioning-large?include=*.json,*.safetensors";
const MODEL_WHISPER: &str = "hf:openai/whisper-base?include=*.json,*.safetensors";

// =============================================================================
// Model Pre-Download
// =============================================================================

/// Pre-download ML models via modelcartridge before running ML tests.
/// This prevents ML cartridges from hanging on peer model-download requests
/// during DAG execution. Runs a single-edge DAG: model_spec → download-model.
async fn ensure_model_downloaded(model_spec: &str, modelcartridge_bin: &PathBuf) {
    eprintln!(
        "[PreDownload] Ensuring model is available: {}",
        model_spec
    );

    let download_urn = model_download();
    let mut registry = CartridgeRegistry::new();
    registry.register(download_urn.clone());

    let dot = dot_graph(
        "pre_download",
        &[dot_edge("model", "result", &download_urn)],
    );

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Pre-download DAG parse failed");

    let temp = TempDir::new().expect("temp dir");
    let plugin_dir = temp.path().join("plugins");
    std::fs::create_dir_all(&plugin_dir).expect("plugin dir");

    let mut inputs = HashMap::new();
    inputs.insert("model".to_string(), NodeData::Text(model_spec.to_string()));

    match execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        vec![modelcartridge_bin.clone()],
    )
    .await
    {
        Ok(outputs) => {
            if let Some(NodeData::Bytes(b)) = outputs.get("result") {
                let result = String::from_utf8_lossy(b);
                eprintln!("[PreDownload] Result: {}", &result[..result.len().min(200)]);
            }
        }
        Err(e) => {
            eprintln!("[PreDownload] Warning: model download failed: {}", e);
        }
    }
}

// =============================================================================
// Test Setup
// =============================================================================

fn setup_test_env(dev_binaries: Vec<PathBuf>) -> (TempDir, PathBuf, Vec<PathBuf>) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let plugin_dir = temp_dir.path().join("plugins");
    std::fs::create_dir_all(&plugin_dir).expect("Failed to create plugin dir");
    (temp_dir, plugin_dir, dev_binaries)
}

fn extract_bytes(outputs: &HashMap<String, NodeData>, node: &str) -> Vec<u8> {
    match outputs.get(node).unwrap_or_else(|| panic!("Missing node '{}'", node)) {
        NodeData::Bytes(b) => b.clone(),
        other => panic!("Expected Bytes at node '{}', got {:?}", node, other),
    }
}

fn extract_text(outputs: &HashMap<String, NodeData>, node: &str) -> String {
    let bytes = extract_bytes(outputs, node);
    String::from_utf8(bytes).unwrap_or_else(|_| panic!("Invalid UTF-8 at node '{}'", node))
}

// =============================================================================
// Scenario 1: PDF Document Intelligence (3 caps, fan-out)
// pdfcartridge: extract_metadata + extract_outline + generate_thumbnail
// =============================================================================

// TEST014: PDF fan-out produces metadata, outline, and thumbnail from a single PDF input
#[tokio::test]
async fn test014_pdf_document_intelligence() {
    let dev_binaries = match require_binaries(&["pdfcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let metadata_urn = pdf_extract_metadata();
    let outline_urn = pdf_extract_outline();
    let thumbnail_urn = pdf_generate_thumbnail();
    registry.register(metadata_urn.clone());
    registry.register(outline_urn.clone());
    registry.register(thumbnail_urn.clone());

    let dot = dot_graph(
        "pdf_document_intelligence",
        &[
            dot_edge("pdf_input", "metadata", &metadata_urn),
            dot_edge("pdf_input", "outline", &outline_urn),
            dot_edge("pdf_input", "thumbnail", &thumbnail_urn),
        ],
    );
    eprintln!("[TEST014] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");
    assert_eq!(graph.edges.len(), 3);

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert("pdf_input".to_string(), NodeData::Bytes(generate_test_pdf()));

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    // Verify metadata is JSON with expected keys
    let metadata_text = extract_text(&outputs, "metadata");
    eprintln!("[TEST014] metadata: {}", &metadata_text[..metadata_text.len().min(200)]);
    assert!(
        metadata_text.contains("page_count") || metadata_text.contains("pages"),
        "Metadata should contain page information"
    );

    // Verify outline is JSON
    let outline_text = extract_text(&outputs, "outline");
    eprintln!("[TEST014] outline: {}", &outline_text[..outline_text.len().min(200)]);
    // Outline might be empty for a blank PDF, but should be valid
    assert!(!outline_text.is_empty(), "Outline should not be empty");

    // Verify thumbnail is PNG (starts with PNG signature)
    let thumbnail_bytes = extract_bytes(&outputs, "thumbnail");
    eprintln!("[TEST014] thumbnail: {} bytes", thumbnail_bytes.len());
    assert!(
        thumbnail_bytes.len() >= 8 && thumbnail_bytes[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        "Thumbnail should be valid PNG (signature check)"
    );
}

// =============================================================================
// Scenario 2: PDF Thumbnail to Image Embedding (2 caps, linear chain)
// pdfcartridge → candlecartridge (requires parser fix for media URN compatibility)
// =============================================================================

// TEST015: Cross-cartridge chain: PDF thumbnail piped to CLIP image embedding
#[tokio::test]
async fn test015_pdf_thumbnail_to_image_embedding() {
    // modelcartridge required: candlecartridge sends peer requests for model downloading
    let dev_binaries = match require_binaries(&["pdfcartridge", "candlecartridge", "modelcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let thumbnail_urn = pdf_generate_thumbnail();
    let img_embed_urn = candle_image_embeddings();
    registry.register(thumbnail_urn.clone());
    registry.register(img_embed_urn.clone());

    // Pre-download CLIP model needed for image embeddings
    let modelcartridge_bin = &dev_binaries.iter().find(|p| {
        p.to_str().map_or(false, |s| s.contains("modelcartridge"))
    }).expect("modelcartridge binary required").clone();
    ensure_model_downloaded(MODEL_CLIP, modelcartridge_bin).await;

    // This chain requires the parser fix: thumbnail outputs media:image;png;thumbnail
    // and image_embeddings inputs media:image;png — compatible via accepts()
    let dot = dot_graph(
        "pdf_thumbnail_to_image_embedding",
        &[
            dot_edge("pdf_input", "thumbnail", &thumbnail_urn),
            dot_edge("thumbnail", "embedding", &img_embed_urn),
        ],
    );
    eprintln!("[TEST015] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed — did the media URN compatibility fix work?");
    assert_eq!(graph.edges.len(), 2);

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert("pdf_input".to_string(), NodeData::Bytes(generate_test_pdf()));

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    // Verify embedding output is JSON with embedding vector
    let embedding_text = extract_text(&outputs, "embedding");
    eprintln!(
        "[TEST015] embedding: {}",
        &embedding_text[..embedding_text.len().min(200)]
    );
    assert!(
        embedding_text.contains("embeddings") || embedding_text.contains("embedding"),
        "Embedding output should contain embedding vector data"
    );
}

// =============================================================================
// Scenario 3: PDF Full Intelligence Pipeline (5 caps, fan-out + chain)
// pdfcartridge ×3 + candlecartridge: metadata + outline + thumbnail → image_embeddings
// =============================================================================

// TEST016: Complete PDF intelligence pipeline with cross-cartridge image embedding
#[tokio::test]
async fn test016_pdf_full_intelligence_pipeline() {
    // modelcartridge required: candlecartridge sends peer requests for model downloading
    let dev_binaries = match require_binaries(&["pdfcartridge", "candlecartridge", "modelcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let metadata_urn = pdf_extract_metadata();
    let outline_urn = pdf_extract_outline();
    let thumbnail_urn = pdf_generate_thumbnail();
    let img_embed_urn = candle_image_embeddings();
    registry.register(metadata_urn.clone());
    registry.register(outline_urn.clone());
    registry.register(thumbnail_urn.clone());
    registry.register(img_embed_urn.clone());

    // Pre-download CLIP model needed for image embeddings
    let modelcartridge_bin = &dev_binaries.iter().find(|p| {
        p.to_str().map_or(false, |s| s.contains("modelcartridge"))
    }).expect("modelcartridge binary required").clone();
    ensure_model_downloaded(MODEL_CLIP, modelcartridge_bin).await;

    let dot = dot_graph(
        "pdf_full_intelligence",
        &[
            dot_edge("pdf_input", "metadata", &metadata_urn),
            dot_edge("pdf_input", "outline", &outline_urn),
            dot_edge("pdf_input", "thumbnail", &thumbnail_urn),
            dot_edge("thumbnail", "img_embedding", &img_embed_urn),
        ],
    );
    eprintln!("[TEST016] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");
    assert_eq!(graph.edges.len(), 4);
    assert_eq!(graph.nodes.len(), 5); // pdf_input, metadata, outline, thumbnail, img_embedding

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert("pdf_input".to_string(), NodeData::Bytes(generate_test_pdf()));

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    // All 4 output nodes should have data
    assert!(outputs.contains_key("metadata"), "Missing metadata output");
    assert!(outputs.contains_key("outline"), "Missing outline output");
    assert!(outputs.contains_key("thumbnail"), "Missing thumbnail output");
    assert!(
        outputs.contains_key("img_embedding"),
        "Missing img_embedding output"
    );

    // Verify thumbnail is PNG
    let thumb = extract_bytes(&outputs, "thumbnail");
    assert!(
        thumb.len() >= 8 && thumb[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        "Thumbnail must be valid PNG"
    );

    // Verify metadata is non-empty text
    let meta = extract_text(&outputs, "metadata");
    assert!(!meta.is_empty(), "Metadata must not be empty");

    // Verify embedding has data
    let emb = extract_text(&outputs, "img_embedding");
    assert!(!emb.is_empty(), "Image embedding must not be empty");
}

// =============================================================================
// Scenario 4: Text Document Intelligence (3 caps, fan-out)
// txtcartridge: extract_metadata + extract_outline + generate_thumbnail on markdown
// =============================================================================

// TEST017: Markdown fan-out produces metadata, outline, and thumbnail
#[tokio::test]
async fn test017_text_document_intelligence() {
    let dev_binaries = match require_binaries(&["txtcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let metadata_urn = md_extract_metadata();
    let outline_urn = md_extract_outline();
    let thumbnail_urn = md_generate_thumbnail();
    registry.register(metadata_urn.clone());
    registry.register(outline_urn.clone());
    registry.register(thumbnail_urn.clone());

    let dot = dot_graph(
        "text_document_intelligence",
        &[
            dot_edge("md_input", "metadata", &metadata_urn),
            dot_edge("md_input", "outline", &outline_urn),
            dot_edge("md_input", "thumbnail", &thumbnail_urn),
        ],
    );
    eprintln!("[TEST017] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");
    assert_eq!(graph.edges.len(), 3);

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert(
        "md_input".to_string(),
        NodeData::Bytes(generate_test_markdown()),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    // Verify metadata
    let meta = extract_text(&outputs, "metadata");
    eprintln!("[TEST017] metadata: {}", &meta[..meta.len().min(200)]);
    assert!(!meta.is_empty(), "Metadata must not be empty");

    // Verify outline (markdown has headers so outline should have content)
    let outline = extract_text(&outputs, "outline");
    eprintln!("[TEST017] outline: {}", &outline[..outline.len().min(200)]);
    assert!(!outline.is_empty(), "Outline must not be empty");

    // Verify thumbnail is PNG
    let thumb = extract_bytes(&outputs, "thumbnail");
    assert!(
        thumb.len() >= 8 && thumb[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        "Thumbnail must be valid PNG"
    );
}

// =============================================================================
// Scenario 5: Multi-Format Document Processing (6 caps, parallel fan-outs)
// pdfcartridge ×3 + txtcartridge ×3: two independent fan-outs from different inputs
// =============================================================================

// TEST018: Parallel processing of PDF and markdown through independent fan-outs
#[tokio::test]
async fn test018_multi_format_document_processing() {
    let dev_binaries = match require_binaries(&["pdfcartridge", "txtcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let pdf_meta = pdf_extract_metadata();
    let pdf_outline = pdf_extract_outline();
    let pdf_thumb = pdf_generate_thumbnail();
    let md_meta = md_extract_metadata();
    let md_outline = md_extract_outline();
    let md_thumb = md_generate_thumbnail();
    registry.register(pdf_meta.clone());
    registry.register(pdf_outline.clone());
    registry.register(pdf_thumb.clone());
    registry.register(md_meta.clone());
    registry.register(md_outline.clone());
    registry.register(md_thumb.clone());

    let dot = dot_graph(
        "multi_format_processing",
        &[
            dot_edge("pdf_input", "pdf_metadata", &pdf_meta),
            dot_edge("pdf_input", "pdf_outline", &pdf_outline),
            dot_edge("pdf_input", "pdf_thumbnail", &pdf_thumb),
            dot_edge("md_input", "md_metadata", &md_meta),
            dot_edge("md_input", "md_outline", &md_outline),
            dot_edge("md_input", "md_thumbnail", &md_thumb),
        ],
    );
    eprintln!("[TEST018] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");
    assert_eq!(graph.edges.len(), 6);
    assert_eq!(graph.nodes.len(), 8); // 2 inputs + 6 outputs

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert("pdf_input".to_string(), NodeData::Bytes(generate_test_pdf()));
    inputs.insert(
        "md_input".to_string(),
        NodeData::Bytes(generate_test_markdown()),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    // All 6 output nodes should have data
    for node in &[
        "pdf_metadata",
        "pdf_outline",
        "pdf_thumbnail",
        "md_metadata",
        "md_outline",
        "md_thumbnail",
    ] {
        assert!(
            outputs.contains_key(*node),
            "Missing output node '{}'",
            node
        );
    }

    // Both thumbnails should be PNG
    for node in &["pdf_thumbnail", "md_thumbnail"] {
        let thumb = extract_bytes(&outputs, node);
        assert!(
            thumb.len() >= 8
                && thumb[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
            "{} must be valid PNG",
            node
        );
    }

    // Both metadata outputs should be non-empty
    assert!(!extract_text(&outputs, "pdf_metadata").is_empty());
    assert!(!extract_text(&outputs, "md_metadata").is_empty());
}

// =============================================================================
// Scenario 6: Model + Dimensions (2 caps, fan-out)
// modelcartridge + candlecartridge: model-spec → availability + candle_dimensions
// =============================================================================

// TEST019: Fan-out from model spec to availability check and embedding dimensions
#[tokio::test]
async fn test019_model_plus_dimensions() {
    let dev_binaries = match require_binaries(&["modelcartridge", "candlecartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let avail_urn = model_availability();
    let dim_urn = candle_embeddings_dimensions();
    registry.register(avail_urn.clone());
    registry.register(dim_urn.clone());

    // Pre-download BERT model needed for embeddings dimensions
    let modelcartridge_bin = &dev_binaries.iter().find(|p| {
        p.to_str().map_or(false, |s| s.contains("modelcartridge"))
    }).expect("modelcartridge binary required").clone();
    ensure_model_downloaded(MODEL_BERT, modelcartridge_bin).await;

    let dot = dot_graph(
        "model_plus_dimensions",
        &[
            dot_edge("model_spec", "availability", &avail_urn),
            dot_edge("model_spec", "dimensions", &dim_urn),
        ],
    );
    eprintln!("[TEST019] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");
    assert_eq!(graph.edges.len(), 2);

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert(
        "model_spec".to_string(),
        NodeData::Text(MODEL_BERT.to_string()),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    // Verify availability output
    let avail = extract_text(&outputs, "availability");
    eprintln!("[TEST019] availability: {}", avail);
    assert!(!avail.is_empty(), "Availability must not be empty");

    // Verify dimensions output (should contain 384 for MiniLM)
    let dim = extract_text(&outputs, "dimensions");
    eprintln!("[TEST019] dimensions: {}", dim);
    assert!(
        dim.contains("384"),
        "MiniLM-L6-v2 should have 384 dimensions, got: {}",
        dim
    );
}

// =============================================================================
// Scenario 7: Model Availability + Status (2 caps, fan-out)
// modelcartridge: model-spec → availability + status
// =============================================================================

// TEST020: Model spec fan-out to availability and status checks
#[tokio::test]
async fn test020_model_availability_plus_status() {
    let dev_binaries = match require_binaries(&["modelcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let avail_urn = model_availability();
    let status_urn = model_status();
    registry.register(avail_urn.clone());
    registry.register(status_urn.clone());

    let dot = dot_graph(
        "model_availability_status",
        &[
            dot_edge("model_spec", "availability", &avail_urn),
            dot_edge("model_spec", "status", &status_urn),
        ],
    );
    eprintln!("[TEST020] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");
    assert_eq!(graph.edges.len(), 2);

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert(
        "model_spec".to_string(),
        NodeData::Text(MODEL_BERT.to_string()),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    let avail = extract_text(&outputs, "availability");
    eprintln!("[TEST020] availability: {}", avail);
    assert!(!avail.is_empty());

    let status = extract_text(&outputs, "status");
    eprintln!("[TEST020] status: {}", status);
    assert!(!status.is_empty());
}

// =============================================================================
// Scenario 8: Text Embedding (1 cap, single step)
// candlecartridge: text → BERT embedding vector
// =============================================================================

// TEST021: Generate text embedding with BERT via candlecartridge
#[tokio::test]
async fn test021_text_embedding() {
    // modelcartridge required: candlecartridge sends peer requests for model downloading
    let dev_binaries = match require_binaries(&["candlecartridge", "modelcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let embed_urn = candle_text_embeddings();
    registry.register(embed_urn.clone());

    // Pre-download BERT model needed for text embeddings
    let modelcartridge_bin = &dev_binaries.iter().find(|p| {
        p.to_str().map_or(false, |s| s.contains("modelcartridge"))
    }).expect("modelcartridge binary required").clone();
    ensure_model_downloaded(MODEL_BERT, modelcartridge_bin).await;

    let dot = dot_graph(
        "text_embedding",
        &[dot_edge("text_input", "embedding", &embed_urn)],
    );
    eprintln!("[TEST021] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert(
        "text_input".to_string(),
        NodeData::Text("The quick brown fox jumps over the lazy dog.".to_string()),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    let embedding = extract_text(&outputs, "embedding");
    eprintln!(
        "[TEST021] embedding: {}",
        &embedding[..embedding.len().min(200)]
    );
    assert!(
        embedding.contains("embeddings") || embedding.contains("embedding"),
        "Output should contain embedding vector"
    );
}

// =============================================================================
// Scenario 9: Image Caption (1 cap, single step)
// candlecartridge: PNG → BLIP caption
// =============================================================================

// TEST022: Generate image caption with BLIP via candlecartridge
#[tokio::test]
async fn test022_image_caption() {
    // modelcartridge required: candlecartridge sends peer requests for model downloading
    let dev_binaries = match require_binaries(&["candlecartridge", "modelcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let caption_urn = candle_caption();
    registry.register(caption_urn.clone());

    // Pre-download BLIP model needed for image captioning
    let modelcartridge_bin = &dev_binaries.iter().find(|p| {
        p.to_str().map_or(false, |s| s.contains("modelcartridge"))
    }).expect("modelcartridge binary required").clone();
    ensure_model_downloaded(MODEL_BLIP, modelcartridge_bin).await;

    let dot = dot_graph(
        "image_caption",
        &[dot_edge("image_input", "caption", &caption_urn)],
    );
    eprintln!("[TEST022] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert(
        "image_input".to_string(),
        NodeData::Bytes(generate_test_png(32, 32, 255, 0, 0)), // 32x32 red image
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    let caption = extract_text(&outputs, "caption");
    eprintln!("[TEST022] caption: {}", caption);
    assert!(!caption.is_empty(), "Caption must not be empty");
}

// =============================================================================
// Scenario 10: Audio Transcription (1 cap, single step)
// candlecartridge: WAV → Whisper transcription
// =============================================================================

// TEST023: Transcribe audio with Whisper via candlecartridge
#[tokio::test]
async fn test023_audio_transcription() {
    // modelcartridge required: candlecartridge sends peer requests for model downloading
    let dev_binaries = match require_binaries(&["candlecartridge", "modelcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let transcribe_urn = candle_transcribe();
    registry.register(transcribe_urn.clone());

    // Pre-download Whisper model needed for audio transcription
    let modelcartridge_bin = &dev_binaries.iter().find(|p| {
        p.to_str().map_or(false, |s| s.contains("modelcartridge"))
    }).expect("modelcartridge binary required").clone();
    ensure_model_downloaded(MODEL_WHISPER, modelcartridge_bin).await;

    let dot = dot_graph(
        "audio_transcription",
        &[dot_edge("audio_input", "transcription", &transcribe_urn)],
    );
    eprintln!("[TEST023] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert(
        "audio_input".to_string(),
        NodeData::Bytes(generate_test_wav()),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    let transcription = extract_text(&outputs, "transcription");
    eprintln!("[TEST023] transcription: {}", transcription);
    // Silence might produce empty transcription or whitespace, but should not error
    assert!(
        outputs.contains_key("transcription"),
        "Transcription output node must exist"
    );
}

// =============================================================================
// Scenario 11: PDF Complete Analysis (4 caps, all pdfcartridge ops)
// pdfcartridge: extract_metadata + extract_outline + generate_thumbnail + disbind
// =============================================================================

// TEST024: All 4 pdfcartridge ops on a single PDF — full document analysis pipeline
#[tokio::test]
async fn test024_pdf_complete_analysis() {
    let dev_binaries = match require_binaries(&["pdfcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let metadata_urn = pdf_extract_metadata();
    let outline_urn = pdf_extract_outline();
    let thumbnail_urn = pdf_generate_thumbnail();
    let disbind_urn = pdf_disbind();
    registry.register(metadata_urn.clone());
    registry.register(outline_urn.clone());
    registry.register(thumbnail_urn.clone());
    registry.register(disbind_urn.clone());

    let dot = dot_graph(
        "pdf_complete_analysis",
        &[
            dot_edge("pdf_input", "metadata", &metadata_urn),
            dot_edge("pdf_input", "outline", &outline_urn),
            dot_edge("pdf_input", "thumbnail", &thumbnail_urn),
            dot_edge("pdf_input", "pages", &disbind_urn),
        ],
    );
    eprintln!("[TEST024] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");
    assert_eq!(graph.edges.len(), 4, "4 edges expected");
    assert_eq!(graph.nodes.len(), 5, "1 input + 4 outputs");

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert("pdf_input".to_string(), NodeData::Bytes(generate_test_pdf()));

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    // All 4 output nodes must exist
    for node in &["metadata", "outline", "thumbnail", "pages"] {
        assert!(outputs.contains_key(*node), "Missing output node '{}'", node);
    }

    // Metadata is JSON with page info
    let meta = extract_text(&outputs, "metadata");
    eprintln!("[TEST024] metadata: {}", &meta[..meta.len().min(200)]);
    assert!(!meta.is_empty());

    // Outline is valid (may be minimal for blank PDF)
    let outline = extract_text(&outputs, "outline");
    eprintln!("[TEST024] outline: {}", &outline[..outline.len().min(200)]);
    assert!(!outline.is_empty());

    // Thumbnail is PNG
    let thumb = extract_bytes(&outputs, "thumbnail");
    assert!(
        thumb.len() >= 8 && thumb[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        "Thumbnail must be valid PNG"
    );

    // Disbind produces page content (may be empty text for blank PDF, but should exist)
    let pages = extract_text(&outputs, "pages");
    eprintln!("[TEST024] pages: {}", &pages[..pages.len().min(200)]);
    assert!(!pages.is_empty(), "Disbind output must not be empty");
}

// =============================================================================
// Scenario 12: Model Full Inspection (4 caps, all non-download modelcartridge ops)
// modelcartridge: availability + status + contents + path
// =============================================================================

// TEST025: All 4 modelcartridge inspection ops on a single model spec
#[tokio::test]
async fn test025_model_full_inspection() {
    let dev_binaries = match require_binaries(&["modelcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let avail_urn = model_availability();
    let status_urn = model_status();
    let contents_urn = model_contents();
    let path_urn = model_path();
    registry.register(avail_urn.clone());
    registry.register(status_urn.clone());
    registry.register(contents_urn.clone());
    registry.register(path_urn.clone());

    let dot = dot_graph(
        "model_full_inspection",
        &[
            dot_edge("model_spec", "availability", &avail_urn),
            dot_edge("model_spec", "status", &status_urn),
            dot_edge("model_spec", "contents", &contents_urn),
            dot_edge("model_spec", "path", &path_urn),
        ],
    );
    eprintln!("[TEST025] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");
    assert_eq!(graph.edges.len(), 4);
    assert_eq!(graph.nodes.len(), 5);

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert(
        "model_spec".to_string(),
        NodeData::Text(MODEL_BERT.to_string()),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    for node in &["availability", "status", "contents", "path"] {
        assert!(outputs.contains_key(*node), "Missing output node '{}'", node);
    }

    // Availability should indicate local presence
    let avail = extract_text(&outputs, "availability");
    eprintln!("[TEST025] availability: {}", &avail[..avail.len().min(200)]);
    assert!(!avail.is_empty());

    // Status should have state field
    let status = extract_text(&outputs, "status");
    eprintln!("[TEST025] status: {}", &status[..status.len().min(200)]);
    assert!(!status.is_empty());

    // Contents should list model files
    let contents = extract_text(&outputs, "contents");
    eprintln!("[TEST025] contents: {}", &contents[..contents.len().min(300)]);
    assert!(!contents.is_empty());

    // Path should contain filesystem path
    let path = extract_text(&outputs, "path");
    eprintln!("[TEST025] path: {}", &path[..path.len().min(200)]);
    assert!(!path.is_empty());
}

// =============================================================================
// Scenario 13: Two-Format Full Analysis (7 caps, pdf ×4 + md ×3)
// pdfcartridge: metadata + outline + thumbnail + disbind
// txtcartridge: metadata + outline + thumbnail (no disbind — markdown has no pages)
// =============================================================================

// TEST026: 7-cap parallel analysis — all pdf ops + all md ops on two documents
#[tokio::test]
async fn test026_two_format_full_analysis() {
    let dev_binaries = match require_binaries(&["pdfcartridge", "txtcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let pdf_meta = pdf_extract_metadata();
    let pdf_outline = pdf_extract_outline();
    let pdf_thumb = pdf_generate_thumbnail();
    let pdf_disb = pdf_disbind();
    let md_meta = md_extract_metadata();
    let md_outline = md_extract_outline();
    let md_thumb = md_generate_thumbnail();
    registry.register(pdf_meta.clone());
    registry.register(pdf_outline.clone());
    registry.register(pdf_thumb.clone());
    registry.register(pdf_disb.clone());
    registry.register(md_meta.clone());
    registry.register(md_outline.clone());
    registry.register(md_thumb.clone());

    let dot = dot_graph(
        "two_format_full_analysis",
        &[
            dot_edge("pdf_input", "pdf_metadata", &pdf_meta),
            dot_edge("pdf_input", "pdf_outline", &pdf_outline),
            dot_edge("pdf_input", "pdf_thumbnail", &pdf_thumb),
            dot_edge("pdf_input", "pdf_pages", &pdf_disb),
            dot_edge("md_input", "md_metadata", &md_meta),
            dot_edge("md_input", "md_outline", &md_outline),
            dot_edge("md_input", "md_thumbnail", &md_thumb),
        ],
    );
    eprintln!("[TEST026] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");
    assert_eq!(graph.edges.len(), 7, "7 edges expected");
    assert_eq!(graph.nodes.len(), 9, "2 inputs + 7 outputs");

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert("pdf_input".to_string(), NodeData::Bytes(generate_test_pdf()));
    inputs.insert(
        "md_input".to_string(),
        NodeData::Bytes(generate_test_markdown()),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    // All 7 output nodes must exist
    let expected_nodes = [
        "pdf_metadata", "pdf_outline", "pdf_thumbnail", "pdf_pages",
        "md_metadata", "md_outline", "md_thumbnail",
    ];
    for node in &expected_nodes {
        assert!(outputs.contains_key(*node), "Missing output node '{}'", node);
    }

    // Both thumbnails must be PNG
    for node in &["pdf_thumbnail", "md_thumbnail"] {
        let thumb = extract_bytes(&outputs, node);
        assert!(
            thumb.len() >= 8 && thumb[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
            "{} must be valid PNG",
            node
        );
    }

    // All text outputs must be non-empty
    for node in &expected_nodes {
        if !node.contains("thumbnail") {
            let text = extract_text(&outputs, node);
            assert!(!text.is_empty(), "{} must not be empty", node);
        }
    }

    eprintln!(
        "[TEST026] All 7 outputs verified: {} nodes with data",
        outputs.len()
    );
}

// =============================================================================
// Scenario 14: Model + PDF Combined Pipeline (5 caps, 2 sources)
// modelcartridge ×2 + pdfcartridge ×3: model inspection + PDF analysis
// =============================================================================

// TEST027: 5-cap cross-domain pipeline — model inspection + PDF document analysis
#[tokio::test]
async fn test027_model_plus_pdf_combined() {
    let dev_binaries = match require_binaries(&["modelcartridge", "pdfcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let avail_urn = model_availability();
    let status_urn = model_status();
    let pdf_meta = pdf_extract_metadata();
    let pdf_outline = pdf_extract_outline();
    let pdf_thumb = pdf_generate_thumbnail();
    registry.register(avail_urn.clone());
    registry.register(status_urn.clone());
    registry.register(pdf_meta.clone());
    registry.register(pdf_outline.clone());
    registry.register(pdf_thumb.clone());

    let dot = dot_graph(
        "model_plus_pdf",
        &[
            dot_edge("model_spec", "availability", &avail_urn),
            dot_edge("model_spec", "status", &status_urn),
            dot_edge("pdf_input", "metadata", &pdf_meta),
            dot_edge("pdf_input", "outline", &pdf_outline),
            dot_edge("pdf_input", "thumbnail", &pdf_thumb),
        ],
    );
    eprintln!("[TEST027] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");
    assert_eq!(graph.edges.len(), 5);
    assert_eq!(graph.nodes.len(), 7); // 2 inputs + 5 outputs

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert(
        "model_spec".to_string(),
        NodeData::Text(MODEL_BERT.to_string()),
    );
    inputs.insert("pdf_input".to_string(), NodeData::Bytes(generate_test_pdf()));

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    for node in &["availability", "status", "metadata", "outline", "thumbnail"] {
        assert!(outputs.contains_key(*node), "Missing output node '{}'", node);
    }

    // Model outputs
    let avail = extract_text(&outputs, "availability");
    eprintln!("[TEST027] availability: {}", &avail[..avail.len().min(200)]);
    assert!(!avail.is_empty());

    let status = extract_text(&outputs, "status");
    eprintln!("[TEST027] status: {}", &status[..status.len().min(200)]);
    assert!(!status.is_empty());

    // PDF outputs
    let thumb = extract_bytes(&outputs, "thumbnail");
    assert!(
        thumb.len() >= 8 && thumb[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        "Thumbnail must be valid PNG"
    );

    assert!(!extract_text(&outputs, "metadata").is_empty());
    assert!(!extract_text(&outputs, "outline").is_empty());

    eprintln!("[TEST027] 5-cap cross-domain pipeline complete");
}

// =============================================================================
// Scenario 15: Three-Cartridge 6-Cap Pipeline (model + pdf + txt)
// modelcartridge ×2 + pdfcartridge ×2 + txtcartridge ×2: 3 sources, 6 caps
// =============================================================================

// TEST028: 6-cap three-cartridge pipeline — model + PDF + markdown analysis
#[tokio::test]
async fn test028_three_cartridge_pipeline() {
    let dev_binaries = match require_binaries(&["modelcartridge", "pdfcartridge", "txtcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let avail_urn = model_availability();
    let status_urn = model_status();
    let pdf_meta = pdf_extract_metadata();
    let pdf_thumb = pdf_generate_thumbnail();
    let md_meta = md_extract_metadata();
    let md_thumb = md_generate_thumbnail();
    registry.register(avail_urn.clone());
    registry.register(status_urn.clone());
    registry.register(pdf_meta.clone());
    registry.register(pdf_thumb.clone());
    registry.register(md_meta.clone());
    registry.register(md_thumb.clone());

    let dot = dot_graph(
        "three_cartridge_pipeline",
        &[
            dot_edge("model_spec", "availability", &avail_urn),
            dot_edge("model_spec", "status", &status_urn),
            dot_edge("pdf_input", "pdf_metadata", &pdf_meta),
            dot_edge("pdf_input", "pdf_thumbnail", &pdf_thumb),
            dot_edge("md_input", "md_metadata", &md_meta),
            dot_edge("md_input", "md_thumbnail", &md_thumb),
        ],
    );
    eprintln!("[TEST028] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");
    assert_eq!(graph.edges.len(), 6);
    assert_eq!(graph.nodes.len(), 9); // 3 inputs + 6 outputs

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert(
        "model_spec".to_string(),
        NodeData::Text(MODEL_BERT.to_string()),
    );
    inputs.insert("pdf_input".to_string(), NodeData::Bytes(generate_test_pdf()));
    inputs.insert(
        "md_input".to_string(),
        NodeData::Bytes(generate_test_markdown()),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    let expected = [
        "availability", "status",
        "pdf_metadata", "pdf_thumbnail",
        "md_metadata", "md_thumbnail",
    ];
    for node in &expected {
        assert!(outputs.contains_key(*node), "Missing output node '{}'", node);
    }

    // Both thumbnails are PNG
    for node in &["pdf_thumbnail", "md_thumbnail"] {
        let thumb = extract_bytes(&outputs, node);
        assert!(
            thumb.len() >= 8 && thumb[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
            "{} must be valid PNG",
            node
        );
    }

    // All text outputs non-empty
    for node in &["availability", "status", "pdf_metadata", "md_metadata"] {
        assert!(!extract_text(&outputs, node).is_empty(), "{} must not be empty", node);
    }

    eprintln!(
        "[TEST028] 6-cap three-cartridge pipeline complete: {} outputs",
        outputs.len()
    );
}

// =============================================================================
// Additional Cap URN Builders
// =============================================================================

// -- txtcartridge txt format (matches txtcartridge's plain-text media type) --

fn txt_generate_thumbnail() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "generate_thumbnail")
        .in_spec("media:txt;textable")
        .out_spec("media:image;png;thumbnail")
        .build()
        .expect("txt generate_thumbnail URN")
}

fn txt_extract_metadata() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "extract_metadata")
        .in_spec("media:txt;textable")
        .out_spec("media:file-metadata;textable;form=map")
        .build()
        .expect("txt extract_metadata URN")
}

fn txt_extract_outline() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "extract_outline")
        .in_spec("media:txt;textable")
        .out_spec("media:document-outline;textable;form=map")
        .build()
        .expect("txt extract_outline URN")
}

// -- txtcartridge rst format --

fn rst_generate_thumbnail() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "generate_thumbnail")
        .in_spec("media:rst;textable")
        .out_spec("media:image;png;thumbnail")
        .build()
        .expect("rst generate_thumbnail URN")
}

fn rst_extract_metadata() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "extract_metadata")
        .in_spec("media:rst;textable")
        .out_spec("media:file-metadata;textable;form=map")
        .build()
        .expect("rst extract_metadata URN")
}

fn rst_extract_outline() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "extract_outline")
        .in_spec("media:rst;textable")
        .out_spec("media:document-outline;textable;form=map")
        .build()
        .expect("rst extract_outline URN")
}

// -- txtcartridge log format --

fn log_generate_thumbnail() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "generate_thumbnail")
        .in_spec("media:log;textable")
        .out_spec("media:image;png;thumbnail")
        .build()
        .expect("log generate_thumbnail URN")
}

fn log_extract_metadata() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "extract_metadata")
        .in_spec("media:log;textable")
        .out_spec("media:file-metadata;textable;form=map")
        .build()
        .expect("log extract_metadata URN")
}

fn log_extract_outline() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "extract_outline")
        .in_spec("media:log;textable")
        .out_spec("media:document-outline;textable;form=map")
        .build()
        .expect("log extract_outline URN")
}

// -- modelcartridge list-models --

fn model_list_models() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "list-models")
        .in_spec("media:model-repo;textable;form=map")
        .out_spec("media:model-list;textable;form=map")
        .build()
        .expect("model list-models URN")
}

// -- ggufcartridge caps (mirrors exact builder calls in ggufcartridge/src/main.rs) --

fn gguf_embeddings_dimensions() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "embeddings_dimensions")
        .solo_tag("ml-model")
        .solo_tag("gguf")
        .in_spec("media:model-spec;textable;form=scalar")
        .out_spec("media:integer;textable;numeric;form=scalar")
        .build()
        .expect("gguf embeddings_dimensions URN")
}

fn gguf_llm_model_info() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "llm_model_info")
        .solo_tag("llm")
        .solo_tag("ml-model")
        .solo_tag("gguf")
        .in_spec("media:llm-generation-request;json;form=map")
        .out_spec("media:llm-model-info;json;form=map")
        .build()
        .expect("gguf llm_model_info URN")
}

fn gguf_llm_vocab() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "llm_vocab")
        .solo_tag("llm")
        .solo_tag("ml-model")
        .solo_tag("gguf")
        .in_spec("media:llm-generation-request;json;form=map")
        .out_spec("media:llm-vocab-response;json;form=map")
        .build()
        .expect("gguf llm_vocab URN")
}

fn gguf_llm_inference() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "llm_inference")
        .solo_tag("llm")
        .solo_tag("ml-model")
        .solo_tag("gguf")
        .in_spec("media:llm-generation-request;json;form=map")
        .out_spec("media:llm-text-stream;ndjson;streaming")
        .build()
        .expect("gguf llm_inference URN")
}

fn gguf_llm_inference_constrained() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "llm_inference_constrained")
        .solo_tag("constrained")
        .solo_tag("llm")
        .solo_tag("ml-model")
        .solo_tag("gguf")
        .in_spec("media:llm-generation-request;json;form=map")
        .out_spec("media:llm-text-stream;ndjson;streaming")
        .build()
        .expect("gguf llm_inference_constrained URN")
}

fn gguf_generate_embeddings() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "generate_embeddings")
        .solo_tag("ml-model")
        .solo_tag("gguf")
        .in_spec("media:textable;form=scalar")
        .out_spec("media:embedding-vector;textable;form=map")
        .build()
        .expect("gguf generate_embeddings URN")
}

fn gguf_analyze_image() -> CapUrn {
    CapUrnBuilder::new()
        .tag("op", "analyze_image")
        .solo_tag("vision")
        .solo_tag("ml-model")
        .solo_tag("gguf")
        .in_spec("media:image;png")  // bytes tag retired
        .out_spec("media:llm-text-stream;ndjson;streaming")
        .build()
        .expect("gguf analyze_image URN")
}

// =============================================================================
// Additional Model Constants (GGUF)
// =============================================================================

/// Small GGUF embedding model (~84MB) for embedding tests
const MODEL_GGUF_EMBED: &str =
    "hf:nomic-ai/nomic-embed-text-v1.5-GGUF?include=nomic-embed-text-v1.5.Q4_K_M.gguf";

/// Small 0.5B GGUF LLM for generation tests (~320MB)
const MODEL_GGUF_LLM: &str =
    "hf:bartowski/Qwen2.5-0.5B-Instruct-GGUF?include=Qwen2.5-0.5B-Instruct-Q4_K_M.gguf";

/// Small GGUF vision model for image analysis tests (~1.8GB, test skips if not present)
const MODEL_GGUF_VISION: &str =
    "hf:vikhyatk/moondream2?include=moondream2-mmproj-f16.gguf,moondream2-text-model-f16.gguf";

// =============================================================================
// Additional Test Fixtures
// =============================================================================

/// Generate a simple plain-text document.
fn generate_test_txt() -> Vec<u8> {
    b"Hello World\n\nThis is a plain text document.\nIt has multiple lines of content for testing.\n\nAnother paragraph here.".to_vec()
}

/// Generate a reStructuredText document with headers.
fn generate_test_rst() -> Vec<u8> {
    b"Test RST Document\n=================\n\nSection One\n-----------\n\nThis is a reStructuredText document.\nIt has proper RST formatting.\n\nSection Two\n-----------\n\nMore content in section two.\n".to_vec()
}

/// Generate a log file with timestamped entries.
fn generate_test_log() -> Vec<u8> {
    b"2024-01-01 12:00:00 INFO Application started\n2024-01-01 12:00:01 INFO Processing files\n2024-01-01 12:00:02 WARN Low memory warning\n2024-01-01 12:00:03 ERROR File not found: /tmp/test.txt\n2024-01-01 12:00:04 INFO Shutdown complete\n".to_vec()
}

/// Build a minimal LLM generation request as JSON bytes.
/// model_spec and prompt are the only required fields for ggufcartridge LLM caps.
fn build_llm_request(model_spec: &str, prompt: &str) -> Vec<u8> {
    // Escape special JSON chars in model_spec and prompt (HF specs use ? * / : which are safe)
    let ms = model_spec.replace('\\', "\\\\").replace('"', "\\\"");
    let pr = prompt.replace('\\', "\\\\").replace('"', "\\\"");
    format!(r#"{{"model_spec":"{ms}","prompt":"{pr}"}}"#).into_bytes()
}

/// Build an LLM generation request with a JSON schema constraint.
fn build_llm_constrained_request(model_spec: &str, prompt: &str) -> Vec<u8> {
    let ms = model_spec.replace('\\', "\\\\").replace('"', "\\\"");
    let pr = prompt.replace('\\', "\\\\").replace('"', "\\\"");
    // Minimal JSON schema: object with a single "result" string field
    format!(
        r#"{{"model_spec":"{ms}","prompt":"{pr}","json_schema":{{"type":"object","properties":{{"result":{{"type":"string"}}}},"required":["result"]}}}}"#
    ).into_bytes()
}

// =============================================================================
// Scenario 16: txtcartridge Plain Text Format (3 caps, fan-out)
// txtcartridge: extract_metadata + extract_outline + generate_thumbnail on .txt
// =============================================================================

// TEST029: Plain text fan-out produces metadata, outline, and thumbnail from txt input
#[tokio::test]
async fn test029_txt_document_intelligence() {
    let dev_binaries = match require_binaries(&["txtcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let metadata_urn = txt_extract_metadata();
    let outline_urn = txt_extract_outline();
    let thumbnail_urn = txt_generate_thumbnail();
    registry.register(metadata_urn.clone());
    registry.register(outline_urn.clone());
    registry.register(thumbnail_urn.clone());

    let dot = dot_graph(
        "txt_document_intelligence",
        &[
            dot_edge("txt_input", "metadata", &metadata_urn),
            dot_edge("txt_input", "outline", &outline_urn),
            dot_edge("txt_input", "thumbnail", &thumbnail_urn),
        ],
    );
    eprintln!("[TEST029] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");
    assert_eq!(graph.edges.len(), 3);

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert("txt_input".to_string(), NodeData::Bytes(generate_test_txt()));

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    let meta = extract_text(&outputs, "metadata");
    eprintln!("[TEST029] metadata: {}", &meta[..meta.len().min(200)]);
    assert!(!meta.is_empty(), "txt metadata must not be empty");

    let outline = extract_text(&outputs, "outline");
    eprintln!("[TEST029] outline: {}", &outline[..outline.len().min(200)]);
    assert!(!outline.is_empty(), "txt outline must not be empty");

    let thumb = extract_bytes(&outputs, "thumbnail");
    eprintln!("[TEST029] thumbnail: {} bytes", thumb.len());
    assert!(
        thumb.len() >= 8 && thumb[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        "txt thumbnail must be valid PNG"
    );
}

// =============================================================================
// Scenario 17: txtcartridge RST Format (3 caps, fan-out)
// txtcartridge: extract_metadata + extract_outline + generate_thumbnail on .rst
// =============================================================================

// TEST030: RST document fan-out produces metadata, outline (with headers), and thumbnail
#[tokio::test]
async fn test030_rst_document_intelligence() {
    let dev_binaries = match require_binaries(&["txtcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let metadata_urn = rst_extract_metadata();
    let outline_urn = rst_extract_outline();
    let thumbnail_urn = rst_generate_thumbnail();
    registry.register(metadata_urn.clone());
    registry.register(outline_urn.clone());
    registry.register(thumbnail_urn.clone());

    let dot = dot_graph(
        "rst_document_intelligence",
        &[
            dot_edge("rst_input", "metadata", &metadata_urn),
            dot_edge("rst_input", "outline", &outline_urn),
            dot_edge("rst_input", "thumbnail", &thumbnail_urn),
        ],
    );
    eprintln!("[TEST030] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");
    assert_eq!(graph.edges.len(), 3);

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert("rst_input".to_string(), NodeData::Bytes(generate_test_rst()));

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    let meta = extract_text(&outputs, "metadata");
    eprintln!("[TEST030] metadata: {}", &meta[..meta.len().min(200)]);
    assert!(!meta.is_empty(), "rst metadata must not be empty");

    // RST has section headers — outline should have content
    let outline = extract_text(&outputs, "outline");
    eprintln!("[TEST030] outline: {}", &outline[..outline.len().min(200)]);
    assert!(!outline.is_empty(), "rst outline must not be empty");

    let thumb = extract_bytes(&outputs, "thumbnail");
    assert!(
        thumb.len() >= 8 && thumb[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        "rst thumbnail must be valid PNG"
    );
}

// =============================================================================
// Scenario 18: txtcartridge Log Format (3 caps, fan-out)
// txtcartridge: extract_metadata + extract_outline + generate_thumbnail on .log
// =============================================================================

// TEST031: Log file fan-out produces metadata, outline, and thumbnail from log input
#[tokio::test]
async fn test031_log_document_intelligence() {
    let dev_binaries = match require_binaries(&["txtcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let metadata_urn = log_extract_metadata();
    let outline_urn = log_extract_outline();
    let thumbnail_urn = log_generate_thumbnail();
    registry.register(metadata_urn.clone());
    registry.register(outline_urn.clone());
    registry.register(thumbnail_urn.clone());

    let dot = dot_graph(
        "log_document_intelligence",
        &[
            dot_edge("log_input", "metadata", &metadata_urn),
            dot_edge("log_input", "outline", &outline_urn),
            dot_edge("log_input", "thumbnail", &thumbnail_urn),
        ],
    );
    eprintln!("[TEST031] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");
    assert_eq!(graph.edges.len(), 3);

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert("log_input".to_string(), NodeData::Bytes(generate_test_log()));

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    let meta = extract_text(&outputs, "metadata");
    eprintln!("[TEST031] metadata: {}", &meta[..meta.len().min(200)]);
    assert!(!meta.is_empty(), "log metadata must not be empty");

    // Log files don't have section headers — outline may be minimal but must not error
    let outline = extract_text(&outputs, "outline");
    eprintln!("[TEST031] outline: {}", &outline[..outline.len().min(200)]);
    assert!(!outline.is_empty(), "log outline response must not be empty");

    let thumb = extract_bytes(&outputs, "thumbnail");
    assert!(
        thumb.len() >= 8 && thumb[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        "log thumbnail must be valid PNG"
    );
}

// =============================================================================
// Scenario 19: All Four Text Formats in One DAG (12 caps, 4 parallel fan-outs)
// txtcartridge: txt + rst + log + md each → metadata + outline + thumbnail
// =============================================================================

// TEST032: 12-cap DAG processing all four text formats simultaneously
#[tokio::test]
async fn test032_all_text_formats_intelligence() {
    let dev_binaries = match require_binaries(&["txtcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let txt_meta = txt_extract_metadata();
    let txt_outline = txt_extract_outline();
    let txt_thumb = txt_generate_thumbnail();
    let rst_meta = rst_extract_metadata();
    let rst_outline = rst_extract_outline();
    let rst_thumb = rst_generate_thumbnail();
    let log_meta = log_extract_metadata();
    let log_outline = log_extract_outline();
    let log_thumb = log_generate_thumbnail();
    let md_meta = md_extract_metadata();
    let md_outline = md_extract_outline();
    let md_thumb = md_generate_thumbnail();
    registry.register(txt_meta.clone());
    registry.register(txt_outline.clone());
    registry.register(txt_thumb.clone());
    registry.register(rst_meta.clone());
    registry.register(rst_outline.clone());
    registry.register(rst_thumb.clone());
    registry.register(log_meta.clone());
    registry.register(log_outline.clone());
    registry.register(log_thumb.clone());
    registry.register(md_meta.clone());
    registry.register(md_outline.clone());
    registry.register(md_thumb.clone());

    let dot = dot_graph(
        "all_text_formats",
        &[
            dot_edge("txt_input", "txt_metadata", &txt_meta),
            dot_edge("txt_input", "txt_outline", &txt_outline),
            dot_edge("txt_input", "txt_thumbnail", &txt_thumb),
            dot_edge("rst_input", "rst_metadata", &rst_meta),
            dot_edge("rst_input", "rst_outline", &rst_outline),
            dot_edge("rst_input", "rst_thumbnail", &rst_thumb),
            dot_edge("log_input", "log_metadata", &log_meta),
            dot_edge("log_input", "log_outline", &log_outline),
            dot_edge("log_input", "log_thumbnail", &log_thumb),
            dot_edge("md_input", "md_metadata", &md_meta),
            dot_edge("md_input", "md_outline", &md_outline),
            dot_edge("md_input", "md_thumbnail", &md_thumb),
        ],
    );
    eprintln!("[TEST032] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");
    assert_eq!(graph.edges.len(), 12, "12 edges expected");
    assert_eq!(graph.nodes.len(), 16, "4 inputs + 12 outputs");

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert("txt_input".to_string(), NodeData::Bytes(generate_test_txt()));
    inputs.insert("rst_input".to_string(), NodeData::Bytes(generate_test_rst()));
    inputs.insert("log_input".to_string(), NodeData::Bytes(generate_test_log()));
    inputs.insert("md_input".to_string(), NodeData::Bytes(generate_test_markdown()));

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    // All 12 output nodes must have data
    let expected_nodes = [
        "txt_metadata", "txt_outline", "txt_thumbnail",
        "rst_metadata", "rst_outline", "rst_thumbnail",
        "log_metadata", "log_outline", "log_thumbnail",
        "md_metadata", "md_outline", "md_thumbnail",
    ];
    for node in &expected_nodes {
        assert!(outputs.contains_key(*node), "Missing output node '{}'", node);
    }

    // All thumbnails must be valid PNG
    for node in &["txt_thumbnail", "rst_thumbnail", "log_thumbnail", "md_thumbnail"] {
        let thumb = extract_bytes(&outputs, node);
        assert!(
            thumb.len() >= 8 && thumb[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
            "{} must be valid PNG",
            node
        );
    }

    // All metadata outputs must be non-empty
    for node in &["txt_metadata", "rst_metadata", "log_metadata", "md_metadata"] {
        assert!(!extract_text(&outputs, node).is_empty(), "{} must not be empty", node);
    }

    eprintln!("[TEST032] All 12 outputs verified across 4 text formats");
}

// =============================================================================
// Scenario 20: modelcartridge list-models (1 cap)
// modelcartridge: model-repo → model-list
// =============================================================================

// TEST033: List all locally cached models via modelcartridge
#[tokio::test]
async fn test033_model_list_models() {
    let dev_binaries = match require_binaries(&["modelcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let list_urn = model_list_models();
    registry.register(list_urn.clone());

    let dot = dot_graph(
        "model_list",
        &[dot_edge("repo_input", "model_list", &list_urn)],
    );
    eprintln!("[TEST033] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");
    assert_eq!(graph.edges.len(), 1);

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    // Empty JSON object: list all models without filtering
    inputs.insert("repo_input".to_string(), NodeData::Bytes(b"{}".to_vec()));

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    let result = extract_text(&outputs, "model_list");
    eprintln!("[TEST033] model_list: {}", &result[..result.len().min(300)]);
    assert!(!result.is_empty(), "model list output must not be empty");
    // Should be valid JSON (either empty list or list of cached models)
    assert!(
        result.starts_with('{') || result.starts_with('['),
        "model list must be JSON, got: {}",
        &result[..result.len().min(50)]
    );
}

// =============================================================================
// Scenario 21: ggufcartridge Embedding Dimensions (1 cap)
// ggufcartridge: model-spec → embedding dimensions integer
// =============================================================================

// TEST034: Query GGUF embedding model dimensions via ggufcartridge
#[tokio::test]
async fn test034_gguf_embeddings_dimensions() {
    let dev_binaries = match require_binaries(&["ggufcartridge", "modelcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let dim_urn = gguf_embeddings_dimensions();
    registry.register(dim_urn.clone());

    let modelcartridge_bin = dev_binaries
        .iter()
        .find(|p| p.to_str().map_or(false, |s| s.contains("modelcartridge")))
        .expect("modelcartridge binary required")
        .clone();
    ensure_model_downloaded(MODEL_GGUF_EMBED, &modelcartridge_bin).await;

    let dot = dot_graph(
        "gguf_dimensions",
        &[dot_edge("model_spec", "dimensions", &dim_urn)],
    );
    eprintln!("[TEST034] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert(
        "model_spec".to_string(),
        NodeData::Text(MODEL_GGUF_EMBED.to_string()),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    let dim_text = extract_text(&outputs, "dimensions");
    eprintln!("[TEST034] dimensions: {}", dim_text);
    // Should be a positive integer (embedding dim for nomic-embed is 768)
    let dim: usize = dim_text.trim().parse()
        .unwrap_or_else(|_| panic!("Dimensions output must be a number, got: {}", dim_text));
    assert!(dim > 0, "Embedding dimensions must be positive, got: {}", dim);
}

// =============================================================================
// Scenario 22: ggufcartridge LLM Model Info (1 cap)
// ggufcartridge: llm-generation-request → llm-model-info
// =============================================================================

// TEST035: Query GGUF model metadata via llm_model_info cap
#[tokio::test]
async fn test035_gguf_llm_model_info() {
    let dev_binaries = match require_binaries(&["ggufcartridge", "modelcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let info_urn = gguf_llm_model_info();
    registry.register(info_urn.clone());

    let modelcartridge_bin = dev_binaries
        .iter()
        .find(|p| p.to_str().map_or(false, |s| s.contains("modelcartridge")))
        .expect("modelcartridge binary required")
        .clone();
    ensure_model_downloaded(MODEL_GGUF_LLM, &modelcartridge_bin).await;

    let dot = dot_graph(
        "gguf_model_info",
        &[dot_edge("request_input", "model_info", &info_urn)],
    );
    eprintln!("[TEST035] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert(
        "request_input".to_string(),
        NodeData::Bytes(build_llm_request(MODEL_GGUF_LLM, " ")),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    let info = extract_text(&outputs, "model_info");
    eprintln!("[TEST035] model_info: {}", &info[..info.len().min(300)]);
    assert!(!info.is_empty(), "Model info must not be empty");
    assert!(
        info.contains("model_spec") || info.contains("vocab_size"),
        "Model info must contain metadata fields, got: {}",
        &info[..info.len().min(200)]
    );
}

// =============================================================================
// Scenario 23: ggufcartridge LLM Vocabulary (1 cap)
// ggufcartridge: llm-generation-request → llm-vocab-response
// =============================================================================

// TEST036: Extract vocabulary tokens from a GGUF model via llm_vocab cap
#[tokio::test]
async fn test036_gguf_llm_vocab() {
    let dev_binaries = match require_binaries(&["ggufcartridge", "modelcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let vocab_urn = gguf_llm_vocab();
    registry.register(vocab_urn.clone());

    let modelcartridge_bin = dev_binaries
        .iter()
        .find(|p| p.to_str().map_or(false, |s| s.contains("modelcartridge")))
        .expect("modelcartridge binary required")
        .clone();
    ensure_model_downloaded(MODEL_GGUF_LLM, &modelcartridge_bin).await;

    let dot = dot_graph(
        "gguf_vocab",
        &[dot_edge("request_input", "vocab", &vocab_urn)],
    );
    eprintln!("[TEST036] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert(
        "request_input".to_string(),
        NodeData::Bytes(build_llm_request(MODEL_GGUF_LLM, " ")),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    let vocab = extract_text(&outputs, "vocab");
    eprintln!("[TEST036] vocab (first 300): {}", &vocab[..vocab.len().min(300)]);
    assert!(!vocab.is_empty(), "Vocab output must not be empty");
    assert!(
        vocab.contains("vocab") || vocab.contains("vocab_size"),
        "Vocab output must contain vocab data, got: {}",
        &vocab[..vocab.len().min(200)]
    );
}

// =============================================================================
// Scenario 24: ggufcartridge Model Info + Vocab Fan-out (2 caps)
// ggufcartridge: same request → model_info + vocab
// =============================================================================

// TEST037: Fan-out from one LLM request to both model_info and vocab outputs
#[tokio::test]
async fn test037_gguf_model_info_plus_vocab() {
    let dev_binaries = match require_binaries(&["ggufcartridge", "modelcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let info_urn = gguf_llm_model_info();
    let vocab_urn = gguf_llm_vocab();
    registry.register(info_urn.clone());
    registry.register(vocab_urn.clone());

    let modelcartridge_bin = dev_binaries
        .iter()
        .find(|p| p.to_str().map_or(false, |s| s.contains("modelcartridge")))
        .expect("modelcartridge binary required")
        .clone();
    ensure_model_downloaded(MODEL_GGUF_LLM, &modelcartridge_bin).await;

    let dot = dot_graph(
        "gguf_model_info_vocab",
        &[
            dot_edge("request_input", "model_info", &info_urn),
            dot_edge("request_input", "vocab", &vocab_urn),
        ],
    );
    eprintln!("[TEST037] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");
    assert_eq!(graph.edges.len(), 2);

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert(
        "request_input".to_string(),
        NodeData::Bytes(build_llm_request(MODEL_GGUF_LLM, " ")),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    assert!(outputs.contains_key("model_info"), "Missing model_info output");
    assert!(outputs.contains_key("vocab"), "Missing vocab output");

    let info = extract_text(&outputs, "model_info");
    eprintln!("[TEST037] model_info: {}", &info[..info.len().min(200)]);
    assert!(!info.is_empty());

    let vocab = extract_text(&outputs, "vocab");
    eprintln!("[TEST037] vocab (first 200): {}", &vocab[..vocab.len().min(200)]);
    assert!(!vocab.is_empty());
}

// =============================================================================
// Scenario 25: ggufcartridge LLM Text Generation (1 cap, streaming)
// ggufcartridge: llm-generation-request → llm-text-stream
// =============================================================================

// TEST038: Generate text with a small GGUF LLM via llm_inference cap
#[tokio::test]
async fn test038_gguf_llm_inference() {
    let dev_binaries = match require_binaries(&["ggufcartridge", "modelcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let infer_urn = gguf_llm_inference();
    registry.register(infer_urn.clone());

    let modelcartridge_bin = dev_binaries
        .iter()
        .find(|p| p.to_str().map_or(false, |s| s.contains("modelcartridge")))
        .expect("modelcartridge binary required")
        .clone();
    ensure_model_downloaded(MODEL_GGUF_LLM, &modelcartridge_bin).await;

    let dot = dot_graph(
        "gguf_inference",
        &[dot_edge("request_input", "generation", &infer_urn)],
    );
    eprintln!("[TEST038] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert(
        "request_input".to_string(),
        NodeData::Bytes(build_llm_request(
            MODEL_GGUF_LLM,
            "Write a single sentence about the sky.",
        )),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    let generation = extract_text(&outputs, "generation");
    eprintln!("[TEST038] generation: {}", &generation[..generation.len().min(300)]);
    assert!(!generation.is_empty(), "Generation output must not be empty");
}

// =============================================================================
// Scenario 26: ggufcartridge Constrained LLM Generation (1 cap)
// ggufcartridge: llm-generation-request (with JSON schema) → llm-text-stream
// =============================================================================

// TEST039: Generate JSON-constrained output with GGUF LLM via llm_inference_constrained cap
#[tokio::test]
async fn test039_gguf_llm_inference_constrained() {
    let dev_binaries = match require_binaries(&["ggufcartridge", "modelcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let constrained_urn = gguf_llm_inference_constrained();
    registry.register(constrained_urn.clone());

    let modelcartridge_bin = dev_binaries
        .iter()
        .find(|p| p.to_str().map_or(false, |s| s.contains("modelcartridge")))
        .expect("modelcartridge binary required")
        .clone();
    ensure_model_downloaded(MODEL_GGUF_LLM, &modelcartridge_bin).await;

    let dot = dot_graph(
        "gguf_constrained",
        &[dot_edge("request_input", "generation", &constrained_urn)],
    );
    eprintln!("[TEST039] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert(
        "request_input".to_string(),
        NodeData::Bytes(build_llm_constrained_request(
            MODEL_GGUF_LLM,
            "Describe the color blue in one word.",
        )),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    let generation = extract_text(&outputs, "generation");
    eprintln!("[TEST039] constrained generation: {}", &generation[..generation.len().min(300)]);
    assert!(!generation.is_empty(), "Constrained generation output must not be empty");
}

// =============================================================================
// Scenario 27: ggufcartridge Text Embeddings (fan-in: text + model-spec)
// ggufcartridge: text_input + model_spec → embedding vector
// The generate_embeddings cap requires both the text stream (media:textable;form=scalar)
// and the model-spec stream (media:model-spec;textable;form=scalar) simultaneously.
// Fan-in via two edges with the same cap URN to the same output node.
// =============================================================================

// TEST040: Generate GGUF text embeddings with fan-in of text and model-spec inputs
#[tokio::test]
async fn test040_gguf_generate_embeddings() {
    let dev_binaries = match require_binaries(&["ggufcartridge", "modelcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let embed_urn = gguf_generate_embeddings();
    registry.register(embed_urn.clone());

    let modelcartridge_bin = dev_binaries
        .iter()
        .find(|p| p.to_str().map_or(false, |s| s.contains("modelcartridge")))
        .expect("modelcartridge binary required")
        .clone();
    ensure_model_downloaded(MODEL_GGUF_EMBED, &modelcartridge_bin).await;

    // Fan-in: both text_input and model_spec feed into embedding via the same cap.
    // The handler reads both streams: media:textable;form=scalar AND media:model-spec;textable;form=scalar.
    let dot = dot_graph(
        "gguf_text_embedding",
        &[
            dot_edge("text_input", "embedding", &embed_urn),
            dot_edge("model_spec", "embedding", &embed_urn),
        ],
    );
    eprintln!("[TEST040] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert(
        "text_input".to_string(),
        NodeData::Text("The quick brown fox jumps over the lazy dog.".to_string()),
    );
    inputs.insert(
        "model_spec".to_string(),
        NodeData::Text(MODEL_GGUF_EMBED.to_string()),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    let embedding = extract_text(&outputs, "embedding");
    eprintln!(
        "[TEST040] embedding: {}",
        &embedding[..embedding.len().min(200)]
    );
    assert!(!embedding.is_empty(), "GGUF embedding output must not be empty");
    assert!(
        embedding.contains("embeddings") || embedding.contains("embedding"),
        "Output should contain embedding vector data"
    );
}

// =============================================================================
// Scenario 28: ggufcartridge Vision Analysis (fan-in: image + model-spec)
// ggufcartridge: image_input + model_spec → llm-text-stream (image analysis)
// =============================================================================

// TEST041: Analyze image with GGUF vision model via fan-in of image and model-spec
#[tokio::test]
async fn test041_gguf_analyze_image() {
    let dev_binaries = match require_binaries(&["ggufcartridge", "modelcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let vision_urn = gguf_analyze_image();
    registry.register(vision_urn.clone());

    let modelcartridge_bin = dev_binaries
        .iter()
        .find(|p| p.to_str().map_or(false, |s| s.contains("modelcartridge")))
        .expect("modelcartridge binary required")
        .clone();
    // Vision model is large (~1.8GB) — pre-download; test proceeds regardless of download outcome
    ensure_model_downloaded(MODEL_GGUF_VISION, &modelcartridge_bin).await;

    // Fan-in: image_input and model_spec both feed into analysis via the same cap.
    // Handler reads: media:image;png (image bytes) AND media:model-spec;textable;form=scalar.
    let dot = dot_graph(
        "gguf_vision",
        &[
            dot_edge("image_input", "analysis", &vision_urn),
            dot_edge("model_spec", "analysis", &vision_urn),
        ],
    );
    eprintln!("[TEST041] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert(
        "image_input".to_string(),
        NodeData::Bytes(generate_test_png(64, 64, 100, 149, 237)), // blue image
    );
    inputs.insert(
        "model_spec".to_string(),
        NodeData::Text(MODEL_GGUF_VISION.to_string()),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    let analysis = extract_text(&outputs, "analysis");
    eprintln!("[TEST041] analysis: {}", &analysis[..analysis.len().min(300)]);
    assert!(!analysis.is_empty(), "Vision analysis output must not be empty");
}

// =============================================================================
// Scenario 29: PDF Thumbnail → ggufcartridge Vision Analysis (cross-cartridge chain)
// pdfcartridge → candlecartridge: thumbnail output feeds into gguf vision
// =============================================================================

// TEST042: Cross-cartridge chain: PDF thumbnail piped to GGUF vision analysis
#[tokio::test]
async fn test042_pdf_thumbnail_to_gguf_vision() {
    let dev_binaries =
        match require_binaries(&["pdfcartridge", "ggufcartridge", "modelcartridge"]) {
            Some(b) => b,
            None => return,
        };

    let mut registry = CartridgeRegistry::new();
    let thumbnail_urn = pdf_generate_thumbnail();
    let vision_urn = gguf_analyze_image();
    registry.register(thumbnail_urn.clone());
    registry.register(vision_urn.clone());

    let modelcartridge_bin = dev_binaries
        .iter()
        .find(|p| p.to_str().map_or(false, |s| s.contains("modelcartridge")))
        .expect("modelcartridge binary required")
        .clone();
    ensure_model_downloaded(MODEL_GGUF_VISION, &modelcartridge_bin).await;

    // Chain: pdf → thumbnail → vision analysis (fan-in with model_spec)
    // thumbnail outputs media:image;png;thumbnail which conforms to media:image;png (vision in_spec)
    let dot = dot_graph(
        "pdf_thumbnail_to_vision",
        &[
            dot_edge("pdf_input", "thumbnail", &thumbnail_urn),
            dot_edge("thumbnail", "analysis", &vision_urn),
            dot_edge("model_spec", "analysis", &vision_urn),
        ],
    );
    eprintln!("[TEST042] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed — thumbnail→vision media URN compatibility check");
    assert_eq!(graph.edges.len(), 3);

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    inputs.insert("pdf_input".to_string(), NodeData::Bytes(generate_test_pdf()));
    inputs.insert(
        "model_spec".to_string(),
        NodeData::Text(MODEL_GGUF_VISION.to_string()),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    assert!(outputs.contains_key("thumbnail"), "Missing thumbnail output");
    assert!(outputs.contains_key("analysis"), "Missing analysis output");

    let thumb = extract_bytes(&outputs, "thumbnail");
    assert!(
        thumb.len() >= 8 && thumb[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        "Thumbnail must be valid PNG"
    );

    let analysis = extract_text(&outputs, "analysis");
    eprintln!("[TEST042] analysis: {}", &analysis[..analysis.len().min(300)]);
    assert!(!analysis.is_empty(), "Vision analysis output must not be empty");
}

// =============================================================================
// Scenario 30: All 4 ggufcartridge LLM Ops Fan-out (4 caps from same request)
// ggufcartridge: request → model_info + vocab + inference + inference_constrained
// =============================================================================

// TEST043: Fan-out from one LLM request to all 4 ggufcartridge LLM operations
#[tokio::test]
async fn test043_gguf_all_llm_ops() {
    let dev_binaries = match require_binaries(&["ggufcartridge", "modelcartridge"]) {
        Some(b) => b,
        None => return,
    };

    let mut registry = CartridgeRegistry::new();
    let info_urn = gguf_llm_model_info();
    let vocab_urn = gguf_llm_vocab();
    let infer_urn = gguf_llm_inference();
    let constrained_urn = gguf_llm_inference_constrained();
    registry.register(info_urn.clone());
    registry.register(vocab_urn.clone());
    registry.register(infer_urn.clone());
    registry.register(constrained_urn.clone());

    let modelcartridge_bin = dev_binaries
        .iter()
        .find(|p| p.to_str().map_or(false, |s| s.contains("modelcartridge")))
        .expect("modelcartridge binary required")
        .clone();
    ensure_model_downloaded(MODEL_GGUF_LLM, &modelcartridge_bin).await;

    // Fan-out: same request dispatched to all 4 LLM caps simultaneously.
    // model_info and vocab use the same request format; inference and constrained
    // inference differ only in constraint field — both use unconstrained format here.
    let dot = dot_graph(
        "gguf_all_llm_ops",
        &[
            dot_edge("request_input", "model_info", &info_urn),
            dot_edge("request_input", "vocab", &vocab_urn),
            dot_edge("request_input", "generation", &infer_urn),
            dot_edge("request_input", "constrained_generation", &constrained_urn),
        ],
    );
    eprintln!("[TEST043] DOT:\n{}", dot);

    let graph = parse_dot_to_cap_dag(&dot, &registry)
        .await
        .expect("Parse failed");
    assert_eq!(graph.edges.len(), 4);
    assert_eq!(graph.nodes.len(), 5); // 1 input + 4 outputs

    let (_temp, plugin_dir, dev_bins) = setup_test_env(dev_binaries);
    let mut inputs = HashMap::new();
    // Use constrained request so both inference caps receive valid input
    inputs.insert(
        "request_input".to_string(),
        NodeData::Bytes(build_llm_constrained_request(
            MODEL_GGUF_LLM,
            "Name one color.",
        )),
    );

    let outputs = execute_dag(
        &graph,
        plugin_dir,
        "https://filegrind.com/api/plugins".to_string(),
        inputs,
        dev_bins,
    )
    .await
    .expect("Execution failed");

    let expected = ["model_info", "vocab", "generation", "constrained_generation"];
    for node in &expected {
        assert!(outputs.contains_key(*node), "Missing output node '{}'", node);
    }

    let info = extract_text(&outputs, "model_info");
    eprintln!("[TEST043] model_info: {}", &info[..info.len().min(200)]);
    assert!(!info.is_empty());

    let vocab = extract_text(&outputs, "vocab");
    eprintln!("[TEST043] vocab (first 100): {}", &vocab[..vocab.len().min(100)]);
    assert!(!vocab.is_empty());

    let gen = extract_text(&outputs, "generation");
    eprintln!("[TEST043] generation: {}", &gen[..gen.len().min(200)]);
    assert!(!gen.is_empty());

    let cgen = extract_text(&outputs, "constrained_generation");
    eprintln!(
        "[TEST043] constrained_generation: {}",
        &cgen[..cgen.len().min(200)]
    );
    assert!(!cgen.is_empty());

    eprintln!("[TEST043] All 4 ggufcartridge LLM ops fan-out complete");
}
