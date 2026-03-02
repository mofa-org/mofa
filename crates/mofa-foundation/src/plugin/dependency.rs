use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    pub dependencies: Vec<PluginDependency>,
    pub optional_dependencies: Vec<PluginDependency>,
    pub conflicts: Vec<PluginConflict>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependency {
    pub plugin_id: String,
    pub version_constraint: VersionConstraint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionConstraint {
    pub min_version: Option<String>,
    pub max_version: Option<String>,
    pub exact_version: Option<String>,
}

impl VersionConstraint {
    pub fn exact(version: impl Into<String>) -> Self {
        Self {
            min_version: None,
            max_version: None,
            exact_version: Some(version.into()),
        }
    }

    pub fn range(min: impl Into<String>, max: impl Into<String>) -> Self {
        Self {
            min_version: Some(min.into()),
            max_version: Some(max.into()),
            exact_version: None,
        }
    }

    pub fn satisfies(&self, version: &str) -> bool {
        if let Some(ref exact) = self.exact_version {
            return exact == version;
        }

        if let Some(ref min) = self.min_version {
            if version < min {
                return false;
            }
        }

        if let Some(ref max) = self.max_version {
            if version > max {
                return false;
            }
        }

        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConflict {
    pub plugin_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PluginNode {
    pub manifest: PluginManifest,
    pub state: PluginLoadState,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PluginLoadState {
    NotLoaded,
    Loading,
    Loaded,
    Failed(String),
}

pub struct DependencyGraph {
    nodes: HashMap<String, PluginNode>,
    edges: HashMap<String, Vec<String>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
        }
    }

    pub fn add_plugin(&mut self, manifest: PluginManifest) -> Result<(), DependencyError> {
        let plugin_id = manifest.plugin_id.clone();

        if self.nodes.contains_key(&plugin_id) {
            return Err(DependencyError::DuplicatePlugin(plugin_id));
        }

        self.nodes.insert(
            plugin_id.clone(),
            PluginNode {
                manifest: manifest.clone(),
                state: PluginLoadState::NotLoaded,
            },
        );

        let dependencies: Vec<String> = manifest
            .dependencies
            .iter()
            .map(|d| d.plugin_id.clone())
            .collect();

        self.edges.insert(plugin_id, dependencies);

        Ok(())
    }

    pub fn resolve_load_order(&self) -> Result<Vec<String>, DependencyError> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut queue: VecDeque<String> = VecDeque::new();
        let mut result: Vec<String> = Vec::new();

        for (plugin_id, deps) in &self.edges {
            let degree = deps.len();
            in_degree.insert(plugin_id.clone(), degree);
        }

        for (plugin_id, degree) in &in_degree {
            if *degree == 0 {
                queue.push_back(plugin_id.clone());
            }
        }

        while let Some(plugin_id) = queue.pop_front() {
            result.push(plugin_id.clone());

            for (other_id, deps) in &self.edges {
                if deps.contains(&plugin_id) {
                    if let Some(degree) = in_degree.get_mut(other_id) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(other_id.clone());
                        }
                    }
                }
            }
        }

        if result.len() != self.nodes.len() {
            return Err(DependencyError::CircularDependency);
        }

        Ok(result)
    }

    pub fn detect_cycles(&self) -> Result<(), DependencyError> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut recursion_stack: HashSet<String> = HashSet::new();

        for plugin_id in self.nodes.keys() {
            if !visited.contains(plugin_id) {
                self.detect_cycles_helper(plugin_id, &mut visited, &mut recursion_stack)?;
            }
        }

        Ok(())
    }

    fn detect_cycles_helper(
        &self,
        plugin_id: &str,
        visited: &mut HashSet<String>,
        recursion_stack: &mut HashSet<String>,
    ) -> Result<(), DependencyError> {
        visited.insert(plugin_id.to_string());
        recursion_stack.insert(plugin_id.to_string());

        if let Some(deps) = self.edges.get(plugin_id) {
            for dep in deps {
                if !visited.contains(dep) {
                    self.detect_cycles_helper(dep, visited, recursion_stack)?;
                } else if recursion_stack.contains(dep) {
                    return Err(DependencyError::CircularDependency);
                }
            }
        }

        recursion_stack.remove(plugin_id);
        Ok(())
    }

    pub fn get_dependencies(&self, plugin_id: &str) -> Option<Vec<String>> {
        self.edges.get(plugin_id).cloned()
    }

    pub fn get_dependents(&self, plugin_id: &str) -> Vec<String> {
        self.edges
            .iter()
            .filter(|(_, deps)| deps.contains(&plugin_id.to_string()))
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn validate(&self) -> Result<(), DependencyError> {
        self.detect_cycles()?;

        for (plugin_id, node) in &self.nodes {
            for dep in &node.manifest.dependencies {
                if !self.nodes.contains_key(&dep.plugin_id) {
                    return Err(DependencyError::MissingDependency {
                        plugin: plugin_id.clone(),
                        dependency: dep.plugin_id.clone(),
                    });
                }

                let dep_version = &self.nodes[&dep.plugin_id].manifest.version;
                if !dep.version_constraint.satisfies(dep_version) {
                    return Err(DependencyError::VersionMismatch {
                        plugin: plugin_id.clone(),
                        dependency: dep.plugin_id.clone(),
                        required: format!(\"{:?}\", dep.version_constraint),
                        found: dep_version.clone(),
                    });
                }
            }

            for conflict in &node.manifest.conflicts {
                if self.nodes.contains_key(&conflict.plugin_id) {
                    return Err(DependencyError::Conflict {
                        plugin: plugin_id.clone(),
                        conflicting: conflict.plugin_id.clone(),
                        reason: conflict.reason.clone(),
                    });
                }
            }
        }

        Ok(())
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DependencyError {
    #[error(\"Circular dependency detected\")]
    CircularDependency,

    #[error(\"Duplicate plugin: {0}\")]
    DuplicatePlugin(String),

    #[error(\"Missing dependency: {0} requires {1}\")]
    MissingDependency { plugin: String, dependency: String },

    #[error(\"Version mismatch: {0} requires {1} but found {2}\")]
    VersionMismatch {
        plugin: String,
        dependency: String,
        required: String,
        found: String,
    },

    #[error(\"Conflict: {0} conflicts with {1}\")]
    Conflict {
        plugin: String,
        conflicting: String,
        reason: Option<String>,
    },
}

pub type DependencyResult<T> = Result<T, DependencyError>;

pub struct PluginRegistry<G: PluginRegistryStorage> {
    graph: DependencyGraph,
    storage: G,
}

impl<G: PluginRegistryStorage> PluginRegistry<G> {
    pub fn new(storage: G) -> Self {
        Self {
            graph: DependencyGraph::new(),
            storage,
        }
    }

    pub fn register(&mut self, manifest: PluginManifest) -> DependencyResult<()> {
        self.graph.add_plugin(manifest)?;
        self.storage.save_manifest(manifest)?;
        Ok(())
    }

    pub fn load_plugin(&mut self, plugin_id: &str) -> DependencyResult<()> {
        let load_order = self.graph.resolve_load_order()?;

        for id in load_order {
            if id == plugin_id {
                break;
            }

            self.load_single_plugin(&id)?;
        }

        self.load_single_plugin(plugin_id)
    }

    fn load_single_plugin(&mut self, plugin_id: &str) -> DependencyResult<()> {
        if let Some(node) = self.graph.nodes.get_mut(plugin_id) {
            node.state = PluginLoadState::Loading;
            node.state = PluginLoadState::Loaded;
            Ok(())
        } else {
            Err(DependencyError::MissingDependency {
                plugin: plugin_id.to_string(),
                dependency: String::new(),
            })
        }
    }

    pub fn get_load_order(&self) -> DependencyResult<Vec<String>> {
        self.graph.resolve_load_order()
    }

    pub fn validate_all(&self) -> DependencyResult<()> {
        self.graph.validate()
    }
}

pub trait PluginRegistryStorage: Send + Sync {
    fn save_manifest(&self, manifest: PluginManifest) -> Result<(), DependencyError>;
    fn load_manifest(&self, plugin_id: &str) -> Result<Option<PluginManifest>, DependencyError>;
    fn list_plugins(&self) -> Result<Vec<String>, DependencyError>;
}

pub struct InMemoryPluginStorage {
    manifests: HashMap<String, PluginManifest>,
}

impl InMemoryPluginStorage {
    pub fn new() -> Self {
        Self {
            manifests: HashMap::new(),
        }
    }
}

impl Default for InMemoryPluginStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginRegistryStorage for InMemoryPluginStorage {
    fn save_manifest(&self, manifest: PluginManifest) -> Result<(), DependencyError> {
        Ok(())
    }

    fn load_manifest(&self, plugin_id: &str) -> Result<Option<PluginManifest>, DependencyError> {
        Ok(self.manifests.get(plugin_id).cloned())
    }

    fn list_plugins(&self) -> Result<Vec<String>, DependencyError> {
        Ok(self.manifests.keys().cloned().collect())
    }
}
