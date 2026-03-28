pub mod types;
pub mod provider;
pub mod streaming;
/// Capability Discovery Protocol — structured backend capability manifests
/// and an in-memory registry for zero-network routing decisions.
pub mod cdp;

pub use types::*;
pub use provider::*;
pub use streaming::*;
pub use cdp::{
    CapabilityFilter,
    CapabilityManifest,
    CapabilityManifestBuilder,
    CapabilityRegistry,
    CdpError,
    HardwareClass,
    Modality,
    ModelEntry,
    ModelEntryBuilder,
    ToolSchemaFormat,
};
