//! Hardware Discovery module
//!
//! Provides utilities to detect the underlying hardware of the host system,
//! such as OS and CPU architecture.

use std::env::consts;

/// Represents the host system's hardware capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareCapability {
    pub os: OsType,
    pub arch: ArchType,
}

/// Supported Operating Systems
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OsType {
    Windows,
    MacOS,
    Linux,
    Unknown,
}

/// Supported Architectures
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchType {
    X86_64,
    Aarch64, // Apple Silicon / ARM64
    Unknown,
}

/// Detects the hardware capabilities of the current host system.
///
/// Under the hood, this uses standard Rust library constants (`std::env::consts`)
/// to determine the compilation target environment at runtime.
pub fn detect_hardware() -> HardwareCapability {
    let os = match consts::OS {
        "windows" => OsType::Windows,
        "macos" => OsType::MacOS,
        "linux" => OsType::Linux,
        _ => OsType::Unknown,
    };

    let arch = match consts::ARCH {
        "x86_64" => ArchType::X86_64,
        "aarch64" => ArchType::Aarch64,
        _ => ArchType::Unknown,
    };

    HardwareCapability { os, arch }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_hardware_returns_valid_capability() {
        let capability = detect_hardware();

        // Assert that the OS is not completely unknown in a typical test environment
        assert!(
            capability.os == OsType::Windows
                || capability.os == OsType::MacOS
                || capability.os == OsType::Linux
        );

        // Assert that the architecture is recognized
        assert!(capability.arch == ArchType::X86_64 || capability.arch == ArchType::Aarch64);
    }
}
