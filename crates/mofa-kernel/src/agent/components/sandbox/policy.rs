//! Sandbox policy types
//!
//! Declarative capability and resource-limit model for sandboxed tool
//! execution. Policies are backend-agnostic: the same `SandboxPolicy` is
//! interpreted by a process-isolation backend, a wasmtime backend, or a
//! passthrough no-op backend.
//!
//! # Design
//!
//! Policies are *default-deny*: any capability not explicitly listed in the
//! `allowed_capabilities` set is refused. This matches the OWASP Tool
//! Sandbox design pattern and keeps the security posture predictable across
//! backends that may have different native default behaviors.
//!
//! ```text
//!                 ┌──────────────────────────┐
//!                 │       SandboxPolicy      │
//!                 │                          │
//!                 │  allowed_capabilities    │───► SandboxCapability set
//!                 │  fs_allow_list           │───► Vec<PathPattern>
//!                 │  net_allow_list          │───► Vec<NetEndpoint>
//!                 │  env_allow_list          │───► Vec<String>
//!                 │  subprocess_allow_list   │───► Vec<String>
//!                 │  resource_limits         │───► SandboxResourceLimits
//!                 └──────────────────────────┘
//! ```

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use super::error::{SandboxError, SandboxResult};

/// SandboxCapability categories that a sandboxed tool may request.
///
/// A sandbox *policy* lists which of these are granted; anything not listed
/// is denied. Capabilities are deliberately coarse-grained — the fine-grained
/// allow-lists (`fs_allow_list`, `net_allow_list`, etc.) further constrain
/// *what* may be accessed within a granted capability.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[non_exhaustive]
pub enum SandboxCapability {
    /// Read-only filesystem access to paths in `fs_allow_list`.
    FsRead,
    /// Read-write filesystem access to paths in `fs_allow_list`.
    FsWrite,
    /// Outbound network to endpoints in `net_allow_list`.
    Net,
    /// Read process environment variables in `env_allow_list`.
    EnvRead,
    /// Spawn subprocesses whose program name is in `subprocess_allow_list`.
    Subprocess,
    /// Read-only CPU-bound compute with no external I/O.
    ///
    /// This is always granted implicitly — compute capability is the base
    /// state of any sandboxed tool. Listing it is a no-op but is accepted
    /// for explicitness in policy definitions.
    Compute,
    /// Read wall-clock time, monotonic clocks, and system-time APIs.
    Clock,
    /// Draw from the host cryptographic RNG.
    RandomRead,
}

impl SandboxCapability {
    /// Human-readable identifier used in error messages.
    pub fn as_str(&self) -> &'static str {
        match self {
            SandboxCapability::FsRead => "FsRead",
            SandboxCapability::FsWrite => "FsWrite",
            SandboxCapability::Net => "Net",
            SandboxCapability::EnvRead => "EnvRead",
            SandboxCapability::Subprocess => "Subprocess",
            SandboxCapability::Compute => "Compute",
            SandboxCapability::Clock => "Clock",
            SandboxCapability::RandomRead => "RandomRead",
        }
    }

    /// `Compute` is implicitly granted and need not appear in a policy.
    pub fn is_implicit(&self) -> bool {
        matches!(self, SandboxCapability::Compute)
    }

    /// Iterate over every defined capability in sorted order. Useful for
    /// combinatorial policy tests and admin UIs that need to surface the
    /// full capability set.
    pub fn iter_all() -> impl Iterator<Item = SandboxCapability> {
        [
            SandboxCapability::FsRead,
            SandboxCapability::FsWrite,
            SandboxCapability::Net,
            SandboxCapability::EnvRead,
            SandboxCapability::Subprocess,
            SandboxCapability::Compute,
            SandboxCapability::Clock,
            SandboxCapability::RandomRead,
        ]
        .into_iter()
    }
}

impl std::fmt::Display for SandboxCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A filesystem path pattern — either an exact path or a directory prefix
/// that the tool may read/write recursively underneath.
///
/// Patterns are checked by canonicalised-prefix match; globs are not
/// supported here to keep the policy model analyzable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PathPattern {
    /// Exact canonical path; the tool may touch *only* this path.
    Exact(PathBuf),
    /// Directory subtree — tool may touch `prefix/*` recursively.
    Prefix(PathBuf),
}

impl PathPattern {
    /// Returns `true` if `candidate` is covered by this pattern.
    pub fn matches(&self, candidate: &std::path::Path) -> bool {
        match self {
            PathPattern::Exact(p) => p == candidate,
            PathPattern::Prefix(p) => candidate.starts_with(p),
        }
    }
}

/// A permitted outbound network endpoint.
///
/// Either a specific `host:port` pair or a host wildcard (`*`) for any port
/// on a given host. No CIDR/subnet support in the kernel model — backends
/// may extend this internally but the policy surface is kept minimal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum NetEndpoint {
    /// Host + specific port (e.g. `api.openai.com:443`).
    HostPort { host: String, port: u16 },
    /// Host with any port permitted.
    HostAnyPort { host: String },
}

impl NetEndpoint {
    /// Returns `true` if a connection attempt to `(host, port)` is
    /// permitted by this endpoint rule.
    pub fn matches(&self, host: &str, port: u16) -> bool {
        match self {
            NetEndpoint::HostPort { host: h, port: p } => h == host && *p == port,
            NetEndpoint::HostAnyPort { host: h } => h == host,
        }
    }
}

/// Resource limits applied during sandboxed execution.
///
/// Limits are *upper bounds*; a backend may enforce tighter limits (e.g.
/// a wasmtime backend may cap memory more aggressively by default). A
/// limit of `None` means "no explicit cap" — but backends are still free
/// to apply a safety default.
///
/// ```text
///                  ┌──────────────────────┐
///                  │   SandboxResourceLimits     │
///                  │                      │
///                  │  wall_timeout        │── Duration
///                  │  cpu_time_limit      │── Duration
///                  │  memory_limit_bytes  │── u64
///                  │  output_limit_bytes  │── u64
///                  │  max_open_files      │── u32
///                  └──────────────────────┘
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxResourceLimits {
    /// Maximum wall-clock time the sandboxed tool may run.
    pub wall_timeout: Option<Duration>,
    /// Maximum CPU time (user+sys) the process may accumulate.
    pub cpu_time_limit: Option<Duration>,
    /// Maximum resident memory in bytes.
    pub memory_limit_bytes: Option<u64>,
    /// Maximum output size the sandbox will capture and return.
    pub output_limit_bytes: Option<u64>,
    /// Maximum open file descriptors (ignored by backends where it doesn't
    /// apply, e.g. a pure wasmtime in-memory sandbox).
    pub max_open_files: Option<u32>,
}

impl Default for SandboxResourceLimits {
    /// Conservative defaults appropriate for untrusted tool execution:
    /// 30-second wall clock, 10-second CPU, 256 MiB memory, 1 MiB output,
    /// 64 file descriptors.
    fn default() -> Self {
        Self {
            wall_timeout: Some(Duration::from_secs(30)),
            cpu_time_limit: Some(Duration::from_secs(10)),
            memory_limit_bytes: Some(256 * 1024 * 1024),
            output_limit_bytes: Some(1024 * 1024),
            max_open_files: Some(64),
        }
    }
}

impl SandboxResourceLimits {
    /// Unrestricted limits. Use only for trusted tools wrapped in
    /// `NullSandbox` where the sandbox model still requires a value.
    pub fn unlimited() -> Self {
        Self {
            wall_timeout: None,
            cpu_time_limit: None,
            memory_limit_bytes: None,
            output_limit_bytes: None,
            max_open_files: None,
        }
    }

    /// Validate that limits are internally consistent. Called implicitly
    /// by [`SandboxPolicy::validate`].
    pub fn validate(&self) -> SandboxResult<()> {
        if let (Some(wall), Some(cpu)) = (self.wall_timeout, self.cpu_time_limit) {
            if cpu > wall {
                return Err(SandboxError::InvalidPolicy(format!(
                    "cpu_time_limit ({cpu:?}) exceeds wall_timeout ({wall:?})"
                )));
            }
        }
        if let Some(mem) = self.memory_limit_bytes {
            if mem == 0 {
                return Err(SandboxError::InvalidPolicy(
                    "memory_limit_bytes must be > 0".into(),
                ));
            }
        }
        if let Some(out) = self.output_limit_bytes {
            if out == 0 {
                return Err(SandboxError::InvalidPolicy(
                    "output_limit_bytes must be > 0".into(),
                ));
            }
        }
        Ok(())
    }
}

/// A complete declarative sandbox policy.
///
/// Construct with [`SandboxPolicy::builder`] for the common case, or
/// directly for the two canned policies:
/// - [`SandboxPolicy::denied_by_default`] — deny everything except `Compute`
/// - [`SandboxPolicy::trusted`] — grant everything (equivalent to no
///   sandboxing; intended for `NullSandbox`)
///
/// ```
/// use std::path::PathBuf;
/// use std::time::Duration;
/// use mofa_kernel::agent::components::sandbox::{
///     SandboxCapability, NetEndpoint, PathPattern, SandboxResourceLimits, SandboxPolicy,
/// };
///
/// let policy = SandboxPolicy::builder()
///     .allow(SandboxCapability::FsRead)
///     .allow_fs(PathPattern::Prefix(PathBuf::from("/tmp/tool-scratch")))
///     .allow(SandboxCapability::Net)
///     .allow_net(NetEndpoint::HostPort { host: "api.openai.com".into(), port: 443 })
///     .resource_limits(SandboxResourceLimits {
///         wall_timeout: Some(Duration::from_secs(10)),
///         ..Default::default()
///     })
///     .build()
///     .unwrap();
///
/// assert!(policy.grants(SandboxCapability::FsRead));
/// assert!(policy.grants(SandboxCapability::Net));
/// assert!(!policy.grants(SandboxCapability::Subprocess));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxPolicy {
    allowed_capabilities: BTreeSet<SandboxCapability>,
    fs_allow_list: Vec<PathPattern>,
    net_allow_list: Vec<NetEndpoint>,
    env_allow_list: Vec<String>,
    subprocess_allow_list: Vec<String>,
    resource_limits: SandboxResourceLimits,
}

impl SandboxPolicy {
    /// Start building a policy.
    pub fn builder() -> SandboxPolicyBuilder {
        SandboxPolicyBuilder::default()
    }

    /// Deny-everything policy (only implicit `Compute` capability is
    /// available). Use this as a base for untrusted tools.
    pub fn denied_by_default() -> Self {
        Self {
            allowed_capabilities: BTreeSet::new(),
            fs_allow_list: Vec::new(),
            net_allow_list: Vec::new(),
            env_allow_list: Vec::new(),
            subprocess_allow_list: Vec::new(),
            resource_limits: SandboxResourceLimits::default(),
        }
    }

    /// Unrestricted policy. Equivalent to running with no sandbox at all;
    /// use only with [`crate::agent::components::sandbox::SandboxTier::None`]
    /// backends, never in production on untrusted tools.
    pub fn trusted() -> Self {
        Self {
            allowed_capabilities: [
                SandboxCapability::FsRead,
                SandboxCapability::FsWrite,
                SandboxCapability::Net,
                SandboxCapability::EnvRead,
                SandboxCapability::Subprocess,
                SandboxCapability::Compute,
                SandboxCapability::Clock,
                SandboxCapability::RandomRead,
            ]
            .into_iter()
            .collect(),
            fs_allow_list: Vec::new(),
            net_allow_list: Vec::new(),
            env_allow_list: Vec::new(),
            subprocess_allow_list: Vec::new(),
            resource_limits: SandboxResourceLimits::unlimited(),
        }
    }

    pub fn allowed_capabilities(&self) -> &BTreeSet<SandboxCapability> {
        &self.allowed_capabilities
    }

    pub fn fs_allow_list(&self) -> &[PathPattern] {
        &self.fs_allow_list
    }

    pub fn net_allow_list(&self) -> &[NetEndpoint] {
        &self.net_allow_list
    }

    pub fn env_allow_list(&self) -> &[String] {
        &self.env_allow_list
    }

    pub fn subprocess_allow_list(&self) -> &[String] {
        &self.subprocess_allow_list
    }

    pub fn resource_limits(&self) -> &SandboxResourceLimits {
        &self.resource_limits
    }

    /// Returns `true` if the policy grants `cap`. `Compute` is always
    /// granted implicitly regardless of whether it was listed.
    pub fn grants(&self, cap: SandboxCapability) -> bool {
        cap.is_implicit() || self.allowed_capabilities.contains(&cap)
    }

    /// Check whether a filesystem access is permitted.
    ///
    /// Requires the caller-appropriate capability (`FsRead` for reads,
    /// `FsWrite` for writes) and that `path` matches some entry in the
    /// allow-list.
    pub fn check_fs(
        &self,
        tool: &str,
        path: &std::path::Path,
        write: bool,
    ) -> SandboxResult<()> {
        let required = if write {
            SandboxCapability::FsWrite
        } else {
            SandboxCapability::FsRead
        };
        if !self.grants(required) {
            return Err(SandboxError::CapabilityDenied {
                tool: tool.into(),
                capability: required.as_str().into(),
                allowed: self.capability_names(),
            });
        }
        if !self.fs_allow_list.iter().any(|p| p.matches(path)) {
            return Err(SandboxError::PathNotAllowed {
                tool: tool.into(),
                path: path.display().to_string(),
            });
        }
        Ok(())
    }

    /// Check whether a network connection is permitted.
    pub fn check_net(&self, tool: &str, host: &str, port: u16) -> SandboxResult<()> {
        if !self.grants(SandboxCapability::Net) {
            return Err(SandboxError::CapabilityDenied {
                tool: tool.into(),
                capability: SandboxCapability::Net.as_str().into(),
                allowed: self.capability_names(),
            });
        }
        if !self.net_allow_list.iter().any(|e| e.matches(host, port)) {
            return Err(SandboxError::NetworkNotAllowed {
                tool: tool.into(),
                host: host.into(),
                port,
            });
        }
        Ok(())
    }

    /// Check whether reading an environment variable is permitted.
    pub fn check_env(&self, name: &str) -> SandboxResult<()> {
        if !self.grants(SandboxCapability::EnvRead) {
            return Err(SandboxError::EnvVarNotAllowed { name: name.into() });
        }
        if !self.env_allow_list.iter().any(|n| n == name) {
            return Err(SandboxError::EnvVarNotAllowed { name: name.into() });
        }
        Ok(())
    }

    /// Check whether spawning `program` as a subprocess is permitted.
    pub fn check_subprocess(&self, tool: &str, program: &str) -> SandboxResult<()> {
        if !self.grants(SandboxCapability::Subprocess) {
            return Err(SandboxError::SubprocessNotAllowed {
                tool: tool.into(),
                program: program.into(),
            });
        }
        if !self.subprocess_allow_list.iter().any(|p| p == program) {
            return Err(SandboxError::SubprocessNotAllowed {
                tool: tool.into(),
                program: program.into(),
            });
        }
        Ok(())
    }

    /// Consistency check over the whole policy.
    pub fn validate(&self) -> SandboxResult<()> {
        // FS allow list entries are only meaningful if an FS capability is granted.
        if !self.fs_allow_list.is_empty()
            && !self.grants(SandboxCapability::FsRead)
            && !self.grants(SandboxCapability::FsWrite)
        {
            return Err(SandboxError::InvalidPolicy(
                "fs_allow_list provided but no FsRead/FsWrite capability granted".into(),
            ));
        }
        // Net allow list requires Net capability.
        if !self.net_allow_list.is_empty() && !self.grants(SandboxCapability::Net) {
            return Err(SandboxError::InvalidPolicy(
                "net_allow_list provided but Net capability not granted".into(),
            ));
        }
        // Env allow list requires EnvRead.
        if !self.env_allow_list.is_empty() && !self.grants(SandboxCapability::EnvRead) {
            return Err(SandboxError::InvalidPolicy(
                "env_allow_list provided but EnvRead capability not granted".into(),
            ));
        }
        // Subprocess allow list requires Subprocess.
        if !self.subprocess_allow_list.is_empty() && !self.grants(SandboxCapability::Subprocess) {
            return Err(SandboxError::InvalidPolicy(
                "subprocess_allow_list provided but Subprocess capability not granted".into(),
            ));
        }
        self.resource_limits.validate()
    }

    fn capability_names(&self) -> Vec<String> {
        self.allowed_capabilities
            .iter()
            .map(|c| c.as_str().to_string())
            .collect()
    }
}

/// Builder for [`SandboxPolicy`].
#[derive(Debug, Default)]
pub struct SandboxPolicyBuilder {
    allowed_capabilities: BTreeSet<SandboxCapability>,
    fs_allow_list: Vec<PathPattern>,
    net_allow_list: Vec<NetEndpoint>,
    env_allow_list: Vec<String>,
    subprocess_allow_list: Vec<String>,
    resource_limits: Option<SandboxResourceLimits>,
}

impl SandboxPolicyBuilder {
    #[must_use]
    pub fn allow(mut self, cap: SandboxCapability) -> Self {
        self.allowed_capabilities.insert(cap);
        self
    }

    #[must_use]
    pub fn allow_many<I: IntoIterator<Item = SandboxCapability>>(mut self, caps: I) -> Self {
        self.allowed_capabilities.extend(caps);
        self
    }

    #[must_use]
    pub fn allow_fs(mut self, pattern: PathPattern) -> Self {
        self.fs_allow_list.push(pattern);
        self
    }

    #[must_use]
    pub fn allow_net(mut self, endpoint: NetEndpoint) -> Self {
        self.net_allow_list.push(endpoint);
        self
    }

    #[must_use]
    pub fn allow_env(mut self, name: impl Into<String>) -> Self {
        self.env_allow_list.push(name.into());
        self
    }

    #[must_use]
    pub fn allow_subprocess(mut self, program: impl Into<String>) -> Self {
        self.subprocess_allow_list.push(program.into());
        self
    }

    #[must_use]
    pub fn resource_limits(mut self, limits: SandboxResourceLimits) -> Self {
        self.resource_limits = Some(limits);
        self
    }

    pub fn build(self) -> SandboxResult<SandboxPolicy> {
        let policy = SandboxPolicy {
            allowed_capabilities: self.allowed_capabilities,
            fs_allow_list: self.fs_allow_list,
            net_allow_list: self.net_allow_list,
            env_allow_list: self.env_allow_list,
            subprocess_allow_list: self.subprocess_allow_list,
            resource_limits: self.resource_limits.unwrap_or_default(),
        };
        policy.validate()?;
        Ok(policy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn tmp_path() -> PathBuf {
        PathBuf::from("/tmp/mofa-sandbox-test")
    }

    #[test]
    fn denied_by_default_grants_only_compute() {
        let p = SandboxPolicy::denied_by_default();
        assert!(p.grants(SandboxCapability::Compute));
        for c in [
            SandboxCapability::FsRead,
            SandboxCapability::FsWrite,
            SandboxCapability::Net,
            SandboxCapability::EnvRead,
            SandboxCapability::Subprocess,
            SandboxCapability::Clock,
            SandboxCapability::RandomRead,
        ] {
            assert!(!p.grants(c), "{c:?} should be denied");
        }
    }

    #[test]
    fn trusted_grants_everything() {
        let p = SandboxPolicy::trusted();
        for c in [
            SandboxCapability::FsRead,
            SandboxCapability::FsWrite,
            SandboxCapability::Net,
            SandboxCapability::EnvRead,
            SandboxCapability::Subprocess,
            SandboxCapability::Compute,
            SandboxCapability::Clock,
            SandboxCapability::RandomRead,
        ] {
            assert!(p.grants(c));
        }
    }

    #[test]
    fn fs_allow_list_requires_capability() {
        let err = SandboxPolicy::builder()
            .allow_fs(PathPattern::Exact(tmp_path()))
            .build()
            .unwrap_err();
        assert!(matches!(err, SandboxError::InvalidPolicy(_)));
    }

    #[test]
    fn fs_check_denies_unlisted_path() {
        let p = SandboxPolicy::builder()
            .allow(SandboxCapability::FsRead)
            .allow_fs(PathPattern::Exact(tmp_path()))
            .build()
            .unwrap();
        let err = p
            .check_fs("cat", Path::new("/etc/passwd"), false)
            .unwrap_err();
        assert!(matches!(err, SandboxError::PathNotAllowed { .. }));
    }

    #[test]
    fn fs_check_accepts_listed_path() {
        let p = SandboxPolicy::builder()
            .allow(SandboxCapability::FsRead)
            .allow_fs(PathPattern::Exact(tmp_path()))
            .build()
            .unwrap();
        assert!(p.check_fs("cat", &tmp_path(), false).is_ok());
    }

    #[test]
    fn fs_prefix_covers_subtree() {
        let p = SandboxPolicy::builder()
            .allow(SandboxCapability::FsRead)
            .allow_fs(PathPattern::Prefix(PathBuf::from("/tmp/scratch")))
            .build()
            .unwrap();
        assert!(
            p.check_fs("cat", Path::new("/tmp/scratch/nested/file"), false)
                .is_ok()
        );
        assert!(
            p.check_fs("cat", Path::new("/tmp/other"), false)
                .is_err()
        );
    }

    #[test]
    fn fs_write_without_write_cap_is_denied() {
        let p = SandboxPolicy::builder()
            .allow(SandboxCapability::FsRead)
            .allow_fs(PathPattern::Exact(tmp_path()))
            .build()
            .unwrap();
        let err = p.check_fs("writer", &tmp_path(), true).unwrap_err();
        assert!(matches!(err, SandboxError::CapabilityDenied { .. }));
    }

    #[test]
    fn net_allow_host_port() {
        let p = SandboxPolicy::builder()
            .allow(SandboxCapability::Net)
            .allow_net(NetEndpoint::HostPort {
                host: "api.openai.com".into(),
                port: 443,
            })
            .build()
            .unwrap();
        assert!(p.check_net("http", "api.openai.com", 443).is_ok());
        assert!(p.check_net("http", "api.openai.com", 80).is_err());
        assert!(p.check_net("http", "evil.com", 443).is_err());
    }

    #[test]
    fn net_host_any_port() {
        let p = SandboxPolicy::builder()
            .allow(SandboxCapability::Net)
            .allow_net(NetEndpoint::HostAnyPort {
                host: "localhost".into(),
            })
            .build()
            .unwrap();
        assert!(p.check_net("http", "localhost", 8080).is_ok());
        assert!(p.check_net("http", "localhost", 22).is_ok());
        assert!(p.check_net("http", "other", 8080).is_err());
    }

    #[test]
    fn env_check_requires_allow_list_entry() {
        let p = SandboxPolicy::builder()
            .allow(SandboxCapability::EnvRead)
            .allow_env("OPENAI_API_KEY")
            .build()
            .unwrap();
        assert!(p.check_env("OPENAI_API_KEY").is_ok());
        assert!(p.check_env("SECRET").is_err());
    }

    #[test]
    fn subprocess_check_enforces_allow_list() {
        let p = SandboxPolicy::builder()
            .allow(SandboxCapability::Subprocess)
            .allow_subprocess("python3")
            .build()
            .unwrap();
        assert!(p.check_subprocess("runner", "python3").is_ok());
        assert!(p.check_subprocess("runner", "bash").is_err());
    }

    #[test]
    fn resource_limits_reject_cpu_exceeding_wall() {
        let err = SandboxPolicy::builder()
            .resource_limits(SandboxResourceLimits {
                wall_timeout: Some(Duration::from_secs(5)),
                cpu_time_limit: Some(Duration::from_secs(10)),
                ..Default::default()
            })
            .build()
            .unwrap_err();
        assert!(matches!(err, SandboxError::InvalidPolicy(_)));
    }

    #[test]
    fn resource_limits_reject_zero_memory() {
        let err = SandboxPolicy::builder()
            .resource_limits(SandboxResourceLimits {
                memory_limit_bytes: Some(0),
                ..Default::default()
            })
            .build()
            .unwrap_err();
        assert!(matches!(err, SandboxError::InvalidPolicy(_)));
    }

    #[test]
    fn path_pattern_exact_match() {
        let p = PathPattern::Exact(PathBuf::from("/a/b"));
        assert!(p.matches(Path::new("/a/b")));
        assert!(!p.matches(Path::new("/a/b/c")));
    }

    #[test]
    fn path_pattern_prefix_match() {
        let p = PathPattern::Prefix(PathBuf::from("/a"));
        assert!(p.matches(Path::new("/a")));
        assert!(p.matches(Path::new("/a/b/c")));
        assert!(!p.matches(Path::new("/b")));
    }

    #[test]
    fn compute_implicit_even_without_listing() {
        let p = SandboxPolicy::denied_by_default();
        assert!(p.grants(SandboxCapability::Compute));
    }

    #[test]
    fn policy_json_roundtrip() {
        let p = SandboxPolicy::builder()
            .allow(SandboxCapability::FsRead)
            .allow_fs(PathPattern::Exact(tmp_path()))
            .build()
            .unwrap();
        let s = serde_json::to_string(&p).unwrap();
        let parsed: SandboxPolicy = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed, p);
    }
}
