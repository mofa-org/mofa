// 简单插件注册中心实现

use super::{AgentResult, Plugin, PluginRegistry, PluginStage};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// 简单插件注册中心实现
pub struct SimplePluginRegistry {
    plugins: RwLock<HashMap<String, Arc<dyn Plugin>>>,
}

impl SimplePluginRegistry {
    /// 创建新的插件注册中心
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for SimplePluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginRegistry for SimplePluginRegistry {
    fn register(&self, plugin: Arc<dyn Plugin>) -> AgentResult<()> {
        let mut plugins = self.plugins.write().map_err(|_| super::AgentError::ExecutionFailed("Failed to acquire write lock".to_string()))?;
        plugins.insert(plugin.name().to_string(), plugin);
        Ok(())
    }

    fn unregister(&self, name: &str) -> AgentResult<bool> {
        let mut plugins = self.plugins.write().map_err(|_| super::AgentError::ExecutionFailed("Failed to acquire write lock".to_string()))?;
        Ok(plugins.remove(name).is_some())
    }

    fn get(&self, name: &str) -> Option<Arc<dyn Plugin>> {
        let plugins = self.plugins.read().ok()?;
        plugins.get(name).cloned()
    }

    fn list(&self) -> Vec<Arc<dyn Plugin>> {
        self.plugins
            .read()
            .ok()
            .map(|plugins| plugins.values().cloned().collect())
            .unwrap_or_default()
    }

    fn list_by_stage(&self, stage: PluginStage) -> Vec<Arc<dyn Plugin>> {
        self.plugins
            .read()
            .ok()
            .map(|plugins| plugins.values()
                .filter(|plugin| plugin.metadata().stages.contains(&stage))
                .cloned()
                .collect())
            .unwrap_or_default()
    }

    fn contains(&self, name: &str) -> bool {
        self.plugins
            .read()
            .ok()
            .map(|plugins| plugins.contains_key(name))
            .unwrap_or(false)
    }

    fn count(&self) -> usize {
        self.plugins.read().ok().map(|plugins| plugins.len()).unwrap_or(0)
    }
}
