//! Texture-diet integration tests for runtime GLB assets.
//!
//! **Characterization** (PASS before diet): valid GLB v2, one animation
//! (72 channels, 161 keyframes, ~5.37s), correct counts, valid bounds.
//!
//! **Structural regression** (RED on `image/png`, GREEN after diet): every
//! embedded image is `image/webp`, buffer byteLength consistent with BIN
//! chunk (≤3 padding bytes).

use serde::Deserialize;

// ---------------------------------------------------------------------------
// Typed GLB / GLTF mirror — only fields consumed by these tests
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct GlbHeader {
    magic: u32,
    version: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GltfRoot {
    buffers: Vec<GltfBuffer>,
    buffer_views: Vec<GltfBufferView>,
    images: Vec<GltfImage>,
    animations: Vec<GltfAnimation>,
    accessors: Vec<GltfAccessor>,
    #[serde(default)]
    nodes: Vec<serde::de::IgnoredAny>,
    #[serde(default)]
    skins: Vec<serde::de::IgnoredAny>,
    #[serde(default)]
    meshes: Vec<serde::de::IgnoredAny>,
}

impl std::fmt::Debug for GltfRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GltfRoot")
            .field("buffers", &self.buffers)
            .field("buffer_views", &self.buffer_views)
            .field("images", &self.images)
            .field("animations", &self.animations)
            .field("accessors", &self.accessors)
            .field("nodes", &self.nodes.len())
            .field("skins", &self.skins.len())
            .field("meshes", &self.meshes.len())
            .finish()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GltfBuffer {
    byte_length: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GltfBufferView {
    #[serde(default)]
    byte_offset: u64,
    byte_length: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GltfAccessor {
    #[serde(default)]
    buffer_view: Option<u64>,
    #[serde(rename = "type")]
    acc_type: String,
    count: u64,
    #[serde(default)]
    max: Option<Vec<f64>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GltfImage {
    mime_type: String,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GltfAnimation {
    #[serde(default)]
    name: Option<String>,
    channels: Vec<GltfAnimationChannel>,
    samplers: Vec<GltfAnimationSampler>,
}

#[derive(Debug, Deserialize)]
struct GltfAnimationChannel {
    sampler: u64,
}

#[derive(Debug, Deserialize)]
struct GltfAnimationSampler {
    input: u64,
    output: u64,
}

struct ParsedGlb {
    header: GlbHeader,
    root: GltfRoot,
    bin_chunk: Vec<u8>,
}

fn parse_glb(path: &str) -> ParsedGlb {
    // Given a path to a GLB file on disk
    let data = std::fs::read(path).unwrap_or_else(|e| panic!("Failed to read {path}: {e}"));
    assert!(data.len() >= 12, "{path}: too short for GLB header");

    // When parsed, the binary header is valid
    let magic = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let version = u32::from_le_bytes(data[4..8].try_into().unwrap());

    let mut off: usize = 12;
    let mut root: Option<GltfRoot> = None;
    let mut bin_chunk: Option<Vec<u8>> = None;

    while off + 8 <= data.len() {
        let chunk_len = u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize;
        let chunk_type = u32::from_le_bytes(data[off + 4..off + 8].try_into().unwrap());
        let chunk_data = &data[off + 8..off + 8 + chunk_len];
        match chunk_type {
            0x4E4F534A => {
                let json_str = std::str::from_utf8(chunk_data)
                    .unwrap_or_else(|e| panic!("JSON chunk not valid UTF-8: {e}"));
                root = Some(
                    serde_json::from_str(json_str)
                        .unwrap_or_else(|e| panic!("Failed to parse GLTF JSON: {e}")),
                );
            }
            0x004E4942 => bin_chunk = Some(chunk_data.to_vec()),
            _ => {}
        }
        off += 8 + chunk_len;
    }
    ParsedGlb {
        header: GlbHeader { magic, version },
        root: root.expect("No JSON chunk found in GLB"),
        bin_chunk: bin_chunk.expect("No BIN chunk found in GLB"),
    }
}

/// Crate-relative path (unit tests run from `crates/game_client/`).
const WAVE_GLB: &str = "../../assets/models/fox/wave.glb";

// =========================================================================
// Characterization — PASS on original wave.glb
// =========================================================================

#[test]
fn wave_glb_v2_characterization() {
    // Given the runtime wave animation GLB file
    let ParsedGlb {
        header,
        root,
        bin_chunk,
    } = parse_glb(WAVE_GLB);

    // Then it is a valid GLB v2 file
    assert_eq!(header.magic, 0x46546C67, "GLB magic");
    assert_eq!(header.version, 2, "GLB version");

    // And it has exactly one animation with expected properties
    assert_eq!(root.animations.len(), 1, "animation count");
    let anim = &root.animations[0];
    assert_eq!(
        anim.name.as_deref(),
        Some("Armature|Big_Wave_Hello|baselayer")
    );
    assert_eq!(anim.channels.len(), 72, "channel count");
    assert_eq!(anim.samplers.len(), 72, "sampler count");

    // And the time accessor matches 161 keyframes over ~5.37s
    let time_acc = &root.accessors[anim.samplers[0].input as usize];
    assert_eq!(time_acc.count, 161, "keyframe count");
    assert_eq!(time_acc.acc_type, "SCALAR");
    if let Some(ref max) = time_acc.max {
        assert!(
            (max[0] - 5.366_666_666_666_666_f64).abs() < 0.001,
            "duration"
        );
    }

    // And node/skin/mesh/accessor/bufferView counts are stable
    assert_eq!(root.nodes.len(), 26, "node count");
    assert_eq!(root.skins.len(), 1, "skin count");
    assert_eq!(root.meshes.len(), 1, "mesh count");
    assert_eq!(root.accessors.len(), 81, "accessor count");
    assert_eq!(root.buffer_views.len(), 82, "bufferView count");

    // And all accessor buffer_view indices are within range
    for (i, acc) in root.accessors.iter().enumerate() {
        if let Some(bv) = acc.buffer_view {
            assert!(
                (bv as usize) < root.buffer_views.len(),
                "accessor[{i}] bufferView {bv} out of bounds"
            );
        }
    }

    // And all bufferView offset+length fits within BIN chunk
    let bin_len = bin_chunk.len();
    for (i, bv) in root.buffer_views.iter().enumerate() {
        let end = bv.byte_offset + bv.byte_length;
        assert!(
            end <= bin_len as u64,
            "bufferView[{i}]: offset+length {end} > BIN len {bin_len}"
        );
    }
}

#[test]
fn wave_glb_channel_sampler_consistency() {
    // Given the runtime wave animation file
    let ParsedGlb { root, .. } = parse_glb(WAVE_GLB);
    let anim = &root.animations[0];
    let n_acc = root.accessors.len();

    // Then every channel's sampler index is valid
    for (ci, ch) in anim.channels.iter().enumerate() {
        assert!(
            (ch.sampler as usize) < anim.samplers.len(),
            "channel[{ci}] sampler {} out of range (max {})",
            ch.sampler,
            anim.samplers.len() - 1
        );
    }

    // And every sampler's input/output accessor is within bounds
    for (si, sam) in anim.samplers.iter().enumerate() {
        assert!(
            (sam.input as usize) < n_acc,
            "sampler[{si}] input accessor {} out of range (max {})",
            sam.input,
            n_acc - 1
        );
        assert!(
            (sam.output as usize) < n_acc,
            "sampler[{si}] output accessor {} out of range (max {})",
            sam.output,
            n_acc - 1
        );
    }
}

// =========================================================================
// Structural regression — RED on image/png, GREEN after diet
// =========================================================================

#[test]
fn wave_glb_images_are_webp() {
    // Given the runtime wave animation file
    let ParsedGlb { root, .. } = parse_glb(WAVE_GLB);

    // Then every embedded image must use WebP format
    assert!(!root.images.is_empty(), "should have at least one image");
    for (i, img) in root.images.iter().enumerate() {
        assert_eq!(
            img.mime_type, "image/webp",
            "image[{i}] ({:?}) MIME should be image/webp, got {}",
            img.name, img.mime_type
        );
    }
}

#[test]
fn wave_glb_buffer_byte_length_matches_bin() {
    // Given the runtime wave animation file
    let ParsedGlb {
        root, bin_chunk, ..
    } = parse_glb(WAVE_GLB);
    let bin_len = bin_chunk.len() as u64;

    // Then every buffer's declared byteLength is ≤ actual BIN chunk length
    // and the difference (padding) is at most 3 bytes.
    for (i, buf) in root.buffers.iter().enumerate() {
        let decl = buf.byte_length;
        assert!(
            decl <= bin_len,
            "buffer[{i}] byteLength {decl} > BIN chunk length {bin_len}"
        );
        let pad = bin_len - decl;
        assert!(
            pad <= 3,
            "buffer[{i}] BIN ({bin_len}) exceeds declared ({decl}) by {pad} (max 3)"
        );
    }
}
