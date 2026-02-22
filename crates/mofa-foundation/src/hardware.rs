use std::env::consts;

/// Represents the operating system of the host machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OsClassification {
    MacOS,
    Windows,
    Linux,
    Other(String),
}

/// Represents the CPU family/architecture of the host machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CpuFamily {
    AppleSilicon,
    X86_64,
    Arm,
    Other(String),
}

/// Holds information about the host environment's hardware capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareCapability {
    pub os: OsClassification,
    pub cpu_family: CpuFamily,
    pub gpu_available: bool,
}

/// Detects the host machine's hardware capabilities dynamically.
pub fn detect_hardware() -> HardwareCapability {
    let os = match consts::OS {
        "macos" => OsClassification::MacOS,
        "windows" => OsClassification::Windows,
        "linux" => OsClassification::Linux,
        other => OsClassification::Other(other.to_string()),
    };

    let cpu_family = match consts::ARCH {
        "x86_64" => CpuFamily::X86_64,
        "aarch64" => {
            if os == OsClassification::MacOS {
                CpuFamily::AppleSilicon
            } else {
                CpuFamily::Arm
            }
        }
        "arm" => CpuFamily::Arm,
        other => CpuFamily::Other(other.to_string()),
    };

    // Basic stubs/checks for GPU availability
    let gpu_available = match os {
        OsClassification::MacOS => {
            // Apple Silicon inherently has Metal acceleration available.
            cpu_family == CpuFamily::AppleSilicon
        }
        OsClassification::Windows | OsClassification::Linux => {
            // A basic stub: check if nvidia-smi command is available in the path
            // This is a naive check to be expanded later.
            std::process::Command::new("nvidia-smi")
                .arg("--version")
                .output()
                .is_ok()
        }
        _ => false,
    };

    HardwareCapability {
        os,
        cpu_family,
        gpu_available,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_hardware() {
        let hardware = detect_hardware();
        println!("Detected Hardware Configuration: {:#?}", hardware);

        // Assert that the fields have been populated with something meaningful.
        // We can't do exact asserts since this runs on different CI/dev environments,
        // but it shouldn't be unknown/other usually (unless we are testing on an exotic arch).

        match hardware.os {
            OsClassification::MacOS | OsClassification::Windows | OsClassification::Linux => (),
            OsClassification::Other(_) => {
                // Warning, unexpected OS but not a failure condition technically
            }
        }

        match hardware.cpu_family {
            CpuFamily::AppleSilicon | CpuFamily::X86_64 | CpuFamily::Arm => (),
            CpuFamily::Other(_) => {
                // Warning, unexpected CPU family
            }
        }

        // GPU availability is environment-dependent, we just ensure it doesn't panic
        let _ = hardware.gpu_available;
    }
}
