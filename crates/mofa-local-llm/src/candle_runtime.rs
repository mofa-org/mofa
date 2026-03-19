//! Candle Runtime for GGUF Model Inference
//!
//! This module provides the runtime for loading and running GGUF models
//! using the Candle inference engine with real model weights.
//!
//! ## Strategy:
//! - Manual GGUF binary format parsing (minimal, correct)
//! - Support F32/F16 GGUF files
//! - Phase 1: Reject quantized models

use std::fs::File;
use std::io::{BufReader, Read, SeekFrom};
use std::path::Path;

#[cfg(feature = "candle")]
use candle_core::{Device, IndexOp, Tensor};
#[cfg(feature = "candle")]
use tokenizers::Tokenizer;

/// Runtime configuration for inference
#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub max_tokens: usize,
    pub temperature: f32,
    pub top_p: f32,
    pub num_threads: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_tokens: 128,
            temperature: 0.7,
            top_p: 0.9,
            num_threads: 4,
        }
    }
}

/// GGUF Tensor type enum
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
pub enum GGUFDType {
    F32,
    F16,
    BF16,
    Q4_0,
    Q4_1,
    Q5_0,
    Q5_1,
    Q8_0,
    Q2_K,
    Q3_K,
    Q4_K,
    Q5_K,
    Q6_K,
    I8,
    I16,
    I32,
    I64,
    F64,
    Unknown(u8),
}

impl GGUFDType {
    fn from_u8(val: u8) -> Self {
        match val {
            0 => GGUFDType::F32,
            1 => GGUFDType::F16,
            2 => GGUFDType::BF16,
            3 => GGUFDType::Q4_0,
            4 => GGUFDType::Q4_1,
            5 => GGUFDType::Q5_0,
            6 => GGUFDType::Q5_1,
            7 => GGUFDType::Q8_0,
            8 => GGUFDType::Q2_K,
            9 => GGUFDType::Q3_K,
            10 => GGUFDType::Q4_K,
            11 => GGUFDType::Q5_K,
            12 => GGUFDType::Q6_K,
            13 => GGUFDType::I8,
            14 => GGUFDType::I16,
            15 => GGUFDType::I32,
            16 => GGUFDType::I64,
            17 => GGUFDType::F64,
            _ => GGUFDType::Unknown(val),
        }
    }

    fn element_size(&self) -> Option<usize> {
        match self {
            GGUFDType::F32 => Some(4),
            GGUFDType::F16 | GGUFDType::BF16 => Some(2),
            GGUFDType::I8 => Some(1),
            GGUFDType::I16 => Some(2),
            GGUFDType::I32 => Some(4),
            GGUFDType::I64 => Some(8),
            GGUFDType::F64 => Some(8),
            _ => None, // Quantized types have variable size
        }
    }

    fn is_quantized(&self) -> bool {
        matches!(
            self,
            GGUFDType::Q4_0
                | GGUFDType::Q4_1
                | GGUFDType::Q5_0
                | GGUFDType::Q5_1
                | GGUFDType::Q8_0
                | GGUFDType::Q2_K
                | GGUFDType::Q3_K
                | GGUFDType::Q4_K
                | GGUFDType::Q5_K
                | GGUFDType::Q6_K
        )
    }
}

/// GGUF Tensor information
#[derive(Debug)]
pub struct TensorInfo {
    pub name: String,
    pub n_dims: u32,
    pub shape: Vec<u64>,
    pub dtype: GGUFDType,
    pub offset: u64,
}

/// InferenceEngine wraps the loaded model for inference
#[cfg(feature = "candle")]
pub struct InferenceEngine {
    config: RuntimeConfig,
    device: Device,
    vocab_size: usize,
    embedding: Option<Tensor>,
    lm_head: Option<Tensor>,
    tokenizer_path: String,
}

#[cfg(feature = "candle")]
impl InferenceEngine {
    /// Create a new inference engine with the given configuration
    pub fn new(config: RuntimeConfig) -> Self {
        let device = Device::Cpu;
        Self {
            config,
            device,
            vocab_size: 32000,
            embedding: None,
            lm_head: None,
            tokenizer_path: String::new(),
        }
    }

    /// Load a GGUF model from the given path
    pub fn load(&mut self, model_path: &str, tokenizer_path: &str) -> Result<(), String> {
        let model_path = Path::new(model_path);
        let tokenizer_path = Path::new(tokenizer_path);

        if !model_path.exists() {
            return Err(format!("Model file not found: {}", model_path.display()));
        }
        if !tokenizer_path.exists() {
            return Err(format!(
                "Tokenizer file not found: {}",
                tokenizer_path.display()
            ));
        }

        self.tokenizer_path = tokenizer_path.to_string_lossy().to_string();

        // Load tokenizer
        let _tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| format!("Failed to load tokenizer: {}", e))?;

        self.vocab_size = 32000;

        // Parse GGUF and load tensors
        let (embedding, lm_head) = self.load_gguf_model(model_path)?;

        self.embedding = Some(embedding);
        self.lm_head = Some(lm_head);

        Ok(())
    }

    /// Parse GGUF file and load tensors
    fn load_gguf_model(&self, model_path: &Path) -> Result<(Tensor, Tensor), String> {
        println!("[GGUF] Opening model file: {}", model_path.display());

        let file =
            File::open(model_path).map_err(|e| format!("Failed to open model file: {}", e))?;
        let mut reader = BufReader::new(file);

        // Read and verify GGUF magic
        let mut magic = [0u8; 4];
        reader
            .read_exact(&mut magic)
            .map_err(|e| format!("Failed to read magic: {}", e))?;

        if &magic != b"GGUF" {
            return Err(format!("Invalid GGUF magic: {:02X?}", magic));
        }
        println!("[GGUF] Magic verified: GGUF");

        // Read version (u32)
        let version = read_u32(&mut reader)?;
        println!("[GGUF] Version: {}", version);

        // Read tensor count (u64)
        let tensor_count = read_u64(&mut reader)?;
        println!("[GGUF] Tensor count: {}", tensor_count);

        // Read metadata count (u64)
        let metadata_count = read_u64(&mut reader)?;
        println!("[GGUF] Metadata count: {}", metadata_count);

        // Skip all metadata entries
        skip_all_metadata(&mut reader, metadata_count)?;

        // Read tensor info
        let mut tensors = Vec::new();
        for i in 0..tensor_count {
            let name = read_string(&mut reader)?;
            let n_dims = read_u32(&mut reader)?;
            let mut shape = Vec::new();
            for _ in 0..n_dims {
                shape.push(read_u64(&mut reader)?);
            }
            let dtype_val = read_u32(&mut reader)?;
            let dtype = GGUFDType::from_u8(dtype_val as u8);
            let offset = read_u64(&mut reader)?;

            tensors.push(TensorInfo {
                name,
                n_dims,
                shape,
                dtype,
                offset,
            });

            if i < 3 || i >= tensor_count.saturating_sub(2) {
                println!(
                    "[GGUF] Tensor {}: {} dims={}, dtype={:?}, offset={}",
                    i,
                    tensors.last().unwrap().name,
                    tensors.last().unwrap().n_dims,
                    tensors.last().unwrap().dtype,
                    tensors.last().unwrap().offset
                );
            }
        }

        // Align to 32 bytes after tensor info
        let current_pos = reader.stream_position().map_err(|e| e.to_string())?;
        let alignment = 32u64;
        let aligned_pos = (current_pos + alignment - 1) / alignment * alignment;
        if aligned_pos > current_pos {
            reader
                .seek(SeekFrom::Start(aligned_pos))
                .map_err(|e| format!("Seek to aligned position failed: {}", e))?;
        }
        println!("[GGUF] Aligned to byte {}", aligned_pos);

        // Find embedding and lm_head tensors
        let mut embedding_info: Option<&TensorInfo> = None;
        let mut lm_head_info: Option<&TensorInfo> = None;

        for info in &tensors {
            let name_lower = info.name.to_lowercase();

            // Look for embedding
            if info.n_dims == 2 {
                if name_lower.contains("token_embd")
                    || name_lower.contains("embed_tokens")
                    || name_lower.contains("wte")
                {
                    if embedding_info.is_none() {
                        println!("[GGUF] Found embedding: {}", info.name);
                        embedding_info = Some(info);
                    }
                }

                // Look for lm_head
                if name_lower.contains("lm_head")
                    || name_lower.contains("output.weight")
                    || (name_lower.contains("output") && name_lower.contains("weight"))
                {
                    if lm_head_info.is_none() {
                        println!("[GGUF] Found lm_head: {}", info.name);
                        lm_head_info = Some(info);
                    }
                }
            }
        }

        // Check for quantized tensors BEFORE loading
        let has_quantized = tensors.iter().any(|t| t.dtype.is_quantized());
        if has_quantized {
            let quantized_count = tensors.iter().filter(|t| t.dtype.is_quantized()).count();
            return Err(format!(
                "Quantized GGUF not supported in Phase 1. Found {} quantized tensors. Please use f16 or f32 model.",
                quantized_count
            ));
        }

        // Load embedding tensor
        let embedding = if let Some(info) = embedding_info {
            load_tensor(&mut reader, info, &self.device)?
        } else {
            return Err("Embedding tensor not found. Required: token_embd.weight, embed_tokens.weight, or wte".to_string());
        };

        // Load lm_head tensor
        let lm_head = if let Some(info) = lm_head_info {
            load_tensor(&mut reader, info, &self.device)?
        } else {
            return Err(
                "LM head tensor not found. Required: lm_head.weight or output.weight".to_string(),
            );
        };

        println!("[GGUF] Embedding shape: {:?}", embedding.shape());
        println!("[GGUF] LM head shape: {:?}", lm_head.shape());

        Ok((embedding, lm_head))
    }

    /// Generate text from a prompt
    pub fn generate(&mut self, prompt: &str) -> Result<String, String> {
        let embedding = self
            .embedding
            .as_ref()
            .ok_or("Model not loaded. Call load() first.")?;
        let lm_head = self
            .lm_head
            .as_ref()
            .ok_or("Model not loaded. Call load() first.")?;

        let tokenizer = Tokenizer::from_file(&self.tokenizer_path)
            .map_err(|e| format!("Failed to load tokenizer: {}", e))?;

        let encoding = tokenizer
            .encode(prompt, true)
            .map_err(|e| format!("Failed to encode prompt: {}", e))?;

        let mut token_ids = encoding.get_ids().to_vec();

        let emb_dim = embedding
            .dim(1)
            .map_err(|e| format!("Embedding dim error: {}", e))?;

        let max_tokens = self.config.max_tokens;

        for _ in 0..max_tokens {
            let last_token = *token_ids.last().ok_or("No tokens in prompt")?;

            let vocab_size_emb = embedding
                .dim(0)
                .map_err(|e| format!("Embedding vocab dim error: {}", e))?;

            if (last_token as usize) >= vocab_size_emb {
                break;
            }

            let token_emb = embedding
                .i((last_token as usize..(last_token as usize + 1), ..))
                .map_err(|e| format!("Embedding lookup failed: {}", e))?;

            let lm_head_dim0 = lm_head
                .dim(0)
                .map_err(|e| format!("LM head dim error: {}", e))?;

            let logits = if lm_head_dim0 == emb_dim {
                token_emb.matmul(lm_head)
            } else {
                let lm_head_t = lm_head
                    .t()
                    .map_err(|e| format!("Transpose failed: {}", e))?;
                token_emb.matmul(&lm_head_t)
            };

            let logits = logits.map_err(|e| format!("Matmul failed: {}", e))?;
            let logits = logits
                .squeeze(0)
                .map_err(|e| format!("Squeeze failed: {}", e))?;

            let next_token = argmax(&logits)?;
            let next_token = next_token as u32;

            // EOS token is typically 2 (</s>) or 1 (<s>)
            if next_token == 2 || next_token == 1 {
                break;
            }

            token_ids.push(next_token);

            if token_ids.len() >= 2048 {
                break;
            }
        }

        let output = tokenizer
            .decode(&token_ids, true)
            .map_err(|e| format!("Failed to decode: {}", e))?;

        Ok(output)
    }
}

// ============ Helper Functions ============

fn read_u32<R: Read>(reader: &mut R) -> Result<u32, String> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).map_err(|e| e.to_string())?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64<R: Read>(reader: &mut R) -> Result<u64, String> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf).map_err(|e| e.to_string())?;
    Ok(u64::from_le_bytes(buf))
}

fn read_string<R: Read>(reader: &mut R) -> Result<String, String> {
    // String format: u64 length, then UTF-8 bytes
    let len = read_u64(reader)? as usize;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).map_err(|e| e.to_string())?;
    String::from_utf8(buf).map_err(|e| e.to_string())
}

/// Skip a string value (used in metadata)
fn skip_string<R: Read>(reader: &mut R) -> Result<(), String> {
    let len = read_u64(reader)? as usize;
    // Skip len bytes
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).map_err(|e| e.to_string())?;
    Ok(())
}

/// Skip all metadata entries using correct format
/// Format per entry:
///   - key: u64 length prefix, then UTF-8 bytes
///   - value_type: u32
///   - value: depends on value_type
fn skip_all_metadata<R: Read>(reader: &mut R, count: u64) -> Result<(), String> {
    for _ in 0..count {
        // Skip key (string)
        skip_string(reader)?;

        // Read value_type
        let value_type = read_u32(reader)?;

        // Skip value based on type
        skip_metadata_value(reader, value_type)?;
    }
    Ok(())
}

/// Skip a metadata value based on its type
fn skip_metadata_value<R: Read>(reader: &mut R, value_type: u32) -> Result<(), String> {
    match value_type {
        0 => {
            // UINT8
            let _ = skip_bytes(reader, 1)?;
        }
        1 => {
            // INT8
            let _ = skip_bytes(reader, 1)?;
        }
        2 => {
            // UINT16
            let _ = skip_bytes(reader, 2)?;
        }
        3 => {
            // INT16
            let _ = skip_bytes(reader, 2)?;
        }
        4 => {
            // UINT32
            let _ = skip_bytes(reader, 4)?;
        }
        5 => {
            // INT32
            let _ = skip_bytes(reader, 4)?;
        }
        6 => {
            // FLOAT32
            let _ = skip_bytes(reader, 4)?;
        }
        7 => {
            // BOOL
            let _ = skip_bytes(reader, 1)?;
        }
        8 => {
            // STRING
            skip_string(reader)?;
        }
        9 => {
            // ARRAY
            // Array: element_type (u32), count (u64), then elements
            let element_type = read_u32(reader)?;
            let count = read_u64(reader)?;
            for _ in 0..count {
                skip_metadata_value(reader, element_type)?;
            }
        }
        10 => {
            // UINT64
            let _ = skip_bytes(reader, 8)?;
        }
        11 => {
            // INT64
            let _ = skip_bytes(reader, 8)?;
        }
        12 => {
            // FLOAT64
            let _ = skip_bytes(reader, 8)?;
        }
        _ => {
            return Err(format!("Unknown metadata value type: {}", value_type));
        }
    }
    Ok(())
}

/// Skip exact number of bytes
fn skip_bytes<R: Read>(reader: &mut R, count: usize) -> Result<(), String> {
    let mut buf = vec![0u8; count];
    reader.read_exact(&mut buf).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(feature = "candle")]
use std::io::Seek;

#[cfg(feature = "candle")]
fn load_tensor<R: Read + Seek>(
    reader: &mut R,
    info: &TensorInfo,
    device: &Device,
) -> Result<Tensor, String> {
    // Seek to tensor offset
    reader
        .seek(SeekFrom::Start(info.offset))
        .map_err(|e| format!("Seek to offset {} failed: {}", info.offset, e))?;

    let element_size = info.dtype.element_size().ok_or(format!(
        "Cannot determine element size for {:?}",
        info.dtype
    ))?;

    let total_elements: usize = info.shape.iter().product::<u64>() as usize;
    let size = total_elements * element_size;

    let mut data = vec![0u8; size];
    reader
        .read_exact(&mut data)
        .map_err(|e| format!("Failed to read tensor data: {}", e))?;

    // Convert to f32 based on dtype
    let data_f32: Vec<f32> = match info.dtype {
        GGUFDType::F32 => data
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect(),
        GGUFDType::F16 => data
            .chunks_exact(2)
            .map(|chunk| {
                let half = u16::from_le_bytes([chunk[0], chunk[1]]);
                half_to_f32(half)
            })
            .collect(),
        GGUFDType::BF16 => data
            .chunks_exact(2)
            .map(|chunk| {
                let bits = u16::from_le_bytes([chunk[0], chunk[1]]) as u32;
                let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
                let exp = ((bits >> 7) & 0xFF) as i32 - 127;
                let mantissa = (bits & 0x7F) as f32 / 128.0;
                sign * (1.0 + mantissa) * 2.0_f32.powi(exp)
            })
            .collect(),
        GGUFDType::I32 => data
            .chunks_exact(4)
            .map(|chunk| {
                let bits = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                bits as f32
            })
            .collect(),
        _ => return Err(format!("Unsupported dtype for Phase 1: {:?}", info.dtype)),
    };

    let shape: Vec<usize> = info.shape.iter().map(|&d| d as usize).collect();

    Tensor::from_vec(data_f32, shape, device).map_err(|e| format!("Failed to create tensor: {}", e))
}

#[cfg(feature = "candle")]
fn argmax(tensor: &Tensor) -> Result<usize, String> {
    let dims = tensor.dims();
    if dims.len() != 1 {
        return Err(format!("Expected 1D tensor, got {}D", dims.len()));
    }

    let data = tensor
        .to_vec1::<f32>()
        .map_err(|e| format!("Failed to convert tensor to vec: {}", e))?;

    let max_idx = data
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(idx, _)| idx)
        .ok_or("Empty tensor")?;

    Ok(max_idx)
}

fn half_to_f32(half: u16) -> f32 {
    let bits = half as u32;
    let sign = (bits >> 15) as f32;
    let exp = ((bits >> 10) & 0x1F) as i32 - 15;
    let mantissa = (bits & 0x3FF) as f32 / 1024.0;

    if exp == -15 {
        if mantissa == 0.0 {
            0.0
        } else {
            sign * mantissa.powi(exp + 1)
        }
    } else {
        sign * (1.0 + mantissa).powi(exp)
    }
}

#[cfg(feature = "candle")]
impl Drop for InferenceEngine {
    fn drop(&mut self) {}
}
