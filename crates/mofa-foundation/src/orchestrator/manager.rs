//! Orchestrator Manager
//!
//! Handles the selection and lifecycle of the underlying ModelOrchestrator backend
//! based on hardware discovery capabilities.

use crate::orchestrator::discovery::{ArchType, HardwareCapability, OsType, detect_hardware};
use crate::orchestrator::{MockOrchestrator, ModelOrchestrator};

/// Defines the underlying backend to route requests to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendType {
    /// Apple Silicon Native (MLX)
    Mlx,
    /// HuggingFace Candle (Cross-Platform / Windows Focus)
    Candle,
    /// ONNX Runtime (CPU/GPU)
    Onnx,
    /// Mock Backend for testing
    Mock,
}

/// The ModelManager is responsible for managing the active orchestrator backend.
pub struct ModelManager {
    capability: HardwareCapability,
    active_backend: BackendType,
    orchestrator: Box<dyn ModelOrchestrator>,
}

impl Default for ModelManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelManager {
    /// Initializes a new ModelManager, automatically detecting the hardware
    /// and assigning the most optimal backend.
    pub fn new() -> Self {
        let capability = detect_hardware();
        let backend = Self::determine_optimal_backend(&capability);

        // For this phase, we act as a router but only instantiate the Mock provider
        // Real providers will be instantiated in future phases.
        let orchestrator: Box<dyn ModelOrchestrator> = match backend {
            _ => Box::new(MockOrchestrator::new()),
        };

        Self {
            capability,
            active_backend: backend,
            orchestrator,
        }
    }

    /// Determines the best backend framework based on hardware specifics.
    fn determine_optimal_backend(cap: &HardwareCapability) -> BackendType {
        match (&cap.os, &cap.arch) {
            // Apple Silicon Devices default to MLX
            (OsType::MacOS, ArchType::Aarch64) => BackendType::Mlx,

            // Windows and Linux machines default to Candle (unless forced to ONNX)
            (OsType::Windows, _) | (OsType::Linux, _) => BackendType::Candle,

            // Fallback for everything else
            _ => BackendType::Mock,
        }
    }

    /// Returns the currently active backend type.
    pub fn active_backend(&self) -> &BackendType {
        &self.active_backend
    }

    /// Returns the detected hardware capabilities.
    pub fn capabilities(&self) -> &HardwareCapability {
        &self.capability
    }

    /// Provides mutable access to the underlying orchestrator trait object
    pub fn orchestrator_mut(&mut self) -> &mut dyn ModelOrchestrator {
        self.orchestrator.as_mut()
    }

    /// Provides reference access to the underlying orchestrator trait object
    pub fn orchestrator(&self) -> &dyn ModelOrchestrator {
        self.orchestrator.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_initialization() {
        let manager = ModelManager::new();

        println!("Detected OS: {:?}", manager.capabilities().os);
        println!("Detected Arch: {:?}", manager.capabilities().arch);
        println!("Selected Backend: {:?}", manager.active_backend());

        // Assert that a backend was selected (even if Mock)
        assert!(
            *manager.active_backend() == BackendType::Mlx
                || *manager.active_backend() == BackendType::Candle
                || *manager.active_backend() == BackendType::Mock
        );
    }

    #[test]
    fn test_optimal_backend_selection() {
        // Test Mac
        let mac_cap = HardwareCapability {
            os: OsType::MacOS,
            arch: ArchType::Aarch64,
        };
        assert_eq!(
            ModelManager::determine_optimal_backend(&mac_cap),
            BackendType::Mlx
        );

        // Test Windows
        let win_cap = HardwareCapability {
            os: OsType::Windows,
            arch: ArchType::X86_64,
        };
        assert_eq!(
            ModelManager::determine_optimal_backend(&win_cap),
            BackendType::Candle
        );

        // Test Linux
        let lin_cap = HardwareCapability {
            os: OsType::Linux,
            arch: ArchType::X86_64,
        };
        assert_eq!(
            ModelManager::determine_optimal_backend(&lin_cap),
            BackendType::Candle
        );
    }
}
