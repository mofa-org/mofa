//! Hardware detection for Linux inference backends
//!
//! Detects available compute backends in priority order:
//! CUDA (NVIDIA) → ROCm (AMD) → Vulkan (cross-vendor) → CPU
//!
//! Detection uses filesystem probes and process checks rather than
//! linking to GPU libraries at compile time, keeping the crate
//! lightweight regardless of which features are enabled.

use serde::{Deserialize, Serialize};
use std::path::Path;
use sysinfo::{MemoryRefreshKind, RefreshKind, System};

/// Available compute backends for Linux inference
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ComputeBackend {
    /// NVIDIA CUDA — highest throughput on NVIDIA hardware
    Cuda,
    /// AMD ROCm — optimized for AMD Radeon and Instinct GPUs
    Rocm,
    /// Vulkan compute — cross-vendor fallback for any Vulkan-capable GPU
    Vulkan,
    /// CPU-only — always available, lowest throughput
    Cpu,
}

impl std::fmt::Display for ComputeBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComputeBackend::Cuda => write!(f, "CUDA"),
            ComputeBackend::Rocm => write!(f, "ROCm"),
            ComputeBackend::Vulkan => write!(f, "Vulkan"),
            ComputeBackend::Cpu => write!(f, "CPU"),
        }
    }
}

/// Information about the detected hardware environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    /// Best available compute backend
    pub backend: ComputeBackend,
    /// All backends detected on this system
    pub available_backends: Vec<ComputeBackend>,
    /// Estimated VRAM in bytes (0 for CPU)
    pub vram_bytes: u64,
    /// Total system RAM in bytes
    pub total_ram_bytes: u64,
    /// Available system RAM in bytes at detection time
    pub available_ram_bytes: u64,
    /// Number of logical CPU cores
    pub cpu_cores: usize,
}

impl HardwareInfo {
    /// Detect available hardware and return the best configuration.
    ///
    /// Runs synchronously — call from a blocking context or `spawn_blocking`.
    pub fn detect() -> Self {
        let mut available = Vec::new();

        let cuda_vram = detect_cuda();
        if cuda_vram.is_some() {
            available.push(ComputeBackend::Cuda);
        }

        let rocm_vram = detect_rocm();
        if rocm_vram.is_some() {
            available.push(ComputeBackend::Rocm);
        }

        if detect_vulkan() {
            available.push(ComputeBackend::Vulkan);
        }

        available.push(ComputeBackend::Cpu);

        let backend = available[0].clone();
        let vram_bytes = cuda_vram.or(rocm_vram).unwrap_or(0);

        let mut sys = System::new_with_specifics(
            RefreshKind::new().with_memory(MemoryRefreshKind::everything()),
        );
        sys.refresh_memory();

        Self {
            backend,
            available_backends: available,
            vram_bytes,
            total_ram_bytes: sys.total_memory(),
            available_ram_bytes: sys.available_memory(),
            cpu_cores: num_cpus(),
        }
    }
}

// ============================================================================
// Backend detection helpers
// ============================================================================

/// Returns estimated VRAM in bytes if CUDA is available, None otherwise.
///
/// Detection strategy:
/// 1. Check for `/dev/nvidia0` device node (kernel module loaded)
/// 2. Try `nvidia-smi --query-gpu=memory.total --format=csv,noheader,nounits`
///    to get actual VRAM
fn detect_cuda() -> Option<u64> {
    if !Path::new("/dev/nvidia0").exists() {
        return None;
    }

    // Try to read VRAM from nvidia-smi
    let output = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;

    if !output.status.success() {
        // Device node exists but nvidia-smi failed — still report CUDA with unknown VRAM
        return Some(0);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // nvidia-smi returns MiB
    let mib: u64 = stdout.trim().lines().next()?.trim().parse().ok()?;
    Some(mib * 1024 * 1024)
}

/// Returns estimated VRAM in bytes if ROCm is available, None otherwise.
///
/// Detection strategy:
/// 1. Check for `/dev/kfd` (AMD Kernel Fusion Driver — required for ROCm)
/// 2. Try `rocm-smi --showmeminfo vram --csv` to get actual VRAM
fn detect_rocm() -> Option<u64> {
    if !Path::new("/dev/kfd").exists() {
        return None;
    }

    let output = std::process::Command::new("rocm-smi")
        .args(["--showmeminfo", "vram", "--csv"])
        .output()
        .ok()?;

    if !output.status.success() {
        return Some(0);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // rocm-smi CSV: GPU,VRAM Total Memory (B),VRAM Used Memory (B)
    for line in stdout.lines().skip(1) {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 2 {
            if let Ok(bytes) = parts[1].trim().parse::<u64>() {
                return Some(bytes);
            }
        }
    }

    Some(0)
}

/// Returns true if any Vulkan-capable GPU is available.
///
/// Detection strategy:
/// 1. Check for `/dev/dri/renderD128` (DRM render node — present for any GPU with Vulkan support)
/// 2. Try `vulkaninfo --summary` as a secondary check
fn detect_vulkan() -> bool {
    if Path::new("/dev/dri/renderD128").exists() {
        return true;
    }

    std::process::Command::new("vulkaninfo")
        .arg("--summary")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_display() {
        assert_eq!(ComputeBackend::Cuda.to_string(), "CUDA");
        assert_eq!(ComputeBackend::Rocm.to_string(), "ROCm");
        assert_eq!(ComputeBackend::Vulkan.to_string(), "Vulkan");
        assert_eq!(ComputeBackend::Cpu.to_string(), "CPU");
    }

    #[test]
    fn test_backend_equality() {
        assert_eq!(ComputeBackend::Cuda, ComputeBackend::Cuda);
        assert_ne!(ComputeBackend::Cuda, ComputeBackend::Rocm);
        assert_ne!(ComputeBackend::Rocm, ComputeBackend::Vulkan);
    }

    #[test]
    fn test_backend_serde_roundtrip() {
        for backend in [
            ComputeBackend::Cuda,
            ComputeBackend::Rocm,
            ComputeBackend::Vulkan,
            ComputeBackend::Cpu,
        ] {
            let json = serde_json::to_string(&backend).expect("serialize");
            let back: ComputeBackend = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(backend, back);
        }
    }

    #[test]
    fn test_detect_always_includes_cpu() {
        let info = HardwareInfo::detect();
        assert!(
            info.available_backends.contains(&ComputeBackend::Cpu),
            "CPU must always be in available backends"
        );
    }

    #[test]
    fn test_detect_backend_is_first_available() {
        let info = HardwareInfo::detect();
        assert_eq!(
            info.backend,
            info.available_backends[0],
            "best backend must be first in available list"
        );
    }

    #[test]
    fn test_detect_ram_nonzero() {
        let info = HardwareInfo::detect();
        assert!(info.total_ram_bytes > 0, "total RAM must be > 0");
        assert!(info.cpu_cores > 0, "cpu cores must be > 0");
    }

    #[test]
    fn test_hardware_info_serde_roundtrip() {
        let info = HardwareInfo::detect();
        let json = serde_json::to_string(&info).expect("serialize");
        let back: HardwareInfo = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(info.backend, back.backend);
        assert_eq!(info.total_ram_bytes, back.total_ram_bytes);
    }
}
