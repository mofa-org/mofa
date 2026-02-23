//! Hot-reload support for Prompt templates
//!
//! 基于 mofa-plugins 提供的热重载机制，实现 Prompt 模板的自动更新
//! Implements automatic updates for Prompt templates based on the hot-reload mechanism provided by mofa-plugins

use super::plugin::{PromptTemplatePlugin, RhaiScriptPromptPlugin};
// 只导入明确需要的类型，避免冲突
// Only import explicitly required types to avoid conflicts
use mofa_plugins::hot_reload::{
    // 不导入 HotReloadConfig，而是在代码中直接指定类型
    // Do not import HotReloadConfig, but specify the type directly in the code
    HotReloadManager,
    ReloadEvent,
};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// 支持热重载的 Rhai 脚本 Prompt 模板插件
/// Rhai script Prompt template plugin supporting hot-reload
pub struct HotReloadableRhaiPromptPlugin {
    /// 内部 Prompt 插件
    /// Internal Prompt plugin
    inner: Arc<RwLock<RhaiScriptPromptPlugin>>,
    /// 热重载管理器
    /// Hot-reload manager
    hot_reload_manager: Arc<RwLock<HotReloadManager>>,
}

impl HotReloadableRhaiPromptPlugin {
    /// 创建新的支持热重载的 Prompt 模板插件
    /// Create a new Prompt template plugin that supports hot-reload
    pub async fn new(script_path: impl AsRef<Path>) -> Self {
        let script_path = script_path.as_ref().to_path_buf();
        let inner = Arc::new(RwLock::new(RhaiScriptPromptPlugin::new(&script_path)));

        // 初始化热重载配置 - 使用 mofa_plugins 的配置格式
        // Initialize hot-reload config - using the mofa_plugins config format
        let plugin_reload_config = mofa_plugins::hot_reload::HotReloadConfig::default();

        // 创建热重载管理器
        // Create hot-reload manager
        let hot_reload_manager = HotReloadManager::new(plugin_reload_config);

        // 加载初始模板 - 暂时不实现
        // Load initial templates - not implemented yet
        {
            // 这里可以添加未来的模板加载逻辑
            // Future template loading logic can be added here
        }

        // 创建并返回实例
        // Create and return instance
        Self {
            inner,
            hot_reload_manager: Arc::new(RwLock::new(hot_reload_manager)),
        }
    }

    /// 启动热重载监听
    /// Start hot-reload watcher
    pub async fn start_reload_watcher(&self) {
        let script_path = {
            let inner_guard = self.inner.read().await;
            inner_guard.script_path().to_path_buf()
        };

        let inner_clone = self.inner.clone();

        // 添加目录监听
        // Add directory watch
        {
            let manager_guard = self.hot_reload_manager.write().await;
            if let Err(e) = manager_guard.add_watch_path(&script_path).await {
                warn!("Failed to add watch path: {}", e);
                return;
            }
        }

        // 启动热重载管理器
        // Start hot-reload manager
        {
            let mut manager_guard = self.hot_reload_manager.write().await;
            if let Err(e) = manager_guard.start().await {
                warn!("Failed to start hot-reload manager: {}", e);
                return;
            }
        }

        info!(
            "Hot-reload prompt template watcher started for path: {:?}",
            script_path
        );

        // 订阅热重载事件
        // Subscribe to hot-reload events
        let mut event_subscriber = self.hot_reload_manager.read().await.subscribe();

        // 处理热重载事件
        // Handle hot-reload events
        tokio::spawn(async move {
            while let Ok(event) = event_subscriber.recv().await {
                match event {
                    ReloadEvent::ReloadCompleted {
                        plugin_id,
                        path,
                        duration,
                        .. // 忽略 success 字段，因为我们当前不使用它
                           // Ignore success field as we are not currently using it
                    } => {
                        info!(
                            "Plugin {} reloaded in {:?} from path {:?}",
                            plugin_id, duration, path
                        );
                        // 刷新模板
                        // Refresh templates
                        let inner_guard = inner_clone.write().await;
                        if let Err(e) = inner_guard.refresh_templates().await {
                            warn!("Failed to refresh templates: {}", e);
                        }
                    }

                    ReloadEvent::ReloadFailed {
                        plugin_id,
                        path,
                        error,
                        attempt,
                    } => {
                        warn!(
                            "Plugin {} reload failed (attempt {}): {} at path {:?}",
                            plugin_id, attempt, error, path
                        );
                    }

                    ReloadEvent::PluginDiscovered { path } => {
                        info!("New plugin discovered at path {:?}", path);
                        // 刷新模板
                        // Refresh templates
                        let inner_guard = inner_clone.write().await;
                        if let Err(e) = inner_guard.refresh_templates().await {
                            warn!("Failed to refresh templates: {}", e);
                        }
                    }

                    ReloadEvent::PluginRemoved {
                        plugin_id,
                        path,
                    } => {
                        info!(
                            "Plugin {} removed from path {:?}",
                            plugin_id, path
                        );
                        // 刷新模板
                        // Refresh templates
                        let inner_guard = inner_clone.write().await;
                        if let Err(e) = inner_guard.refresh_templates().await {
                            warn!("Failed to refresh templates: {}", e);
                        }
                    }

                    _ => {} // Ignore other events
                }
            }
        });
    }

    /// 停止热重载监听
    /// Stop hot-reload watcher
    pub async fn stop_reload_watcher(&self) {
        let mut manager_guard = self.hot_reload_manager.write().await;
        if let Err(e) = manager_guard.stop().await {
            warn!("Failed to stop hot-reload manager: {}", e);
        }
    }

    /// 获取内部插件实例
    /// Get internal plugin instance
    pub async fn inner(&self) -> Arc<RwLock<RhaiScriptPromptPlugin>> {
        self.inner.clone()
    }

    /// 设置当前活动的场景
    /// Set the currently active scenario
    pub async fn set_active_scenario(&self, scenario: impl Into<String>) {
        let scenario = scenario.into();
        info!("Switching to scenario: {}", scenario);

        let inner_guard = self.inner.write().await;
        inner_guard.set_active_scenario(scenario).await;
    }
}

#[async_trait::async_trait]
impl super::plugin::PromptTemplatePlugin for HotReloadableRhaiPromptPlugin {
    async fn get_prompt_template(
        &self,
        scenario: &str,
    ) -> Option<Arc<super::template::PromptTemplate>> {
        let inner_guard = self.inner.read().await;
        inner_guard.get_prompt_template(scenario).await
    }

    async fn get_active_scenario(&self) -> String {
        let inner_guard = self.inner.read().await;
        inner_guard.get_active_scenario().await
    }

    async fn set_active_scenario(&self, scenario: &str) {
        let inner_guard = self.inner.write().await;
        inner_guard.set_active_scenario(scenario).await;
    }

    async fn get_available_scenarios(&self) -> Vec<String> {
        let inner_guard = self.inner.read().await;
        inner_guard.get_available_scenarios().await
    }

    async fn refresh_templates(&self) -> mofa_kernel::plugin::PluginResult<()> {
        let inner_guard = self.inner.write().await;
        inner_guard.refresh_templates().await
    }
}
