//! 任务管理器 - 阶段1: 接收想法，记录Todo
//! Task Manager - Phase 1: Receive ideas, record Todos

use super::types::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 任务管理器
/// Task Manager
///
/// 管理 Todo 任务的创建、更新和查询。
/// Manages the creation, updating, and querying of Todo tasks.
pub struct TodoManager {
    /// Todo 列表
    /// Todo list
    todos: Arc<RwLock<HashMap<String, TodoItem>>>,
    /// 计数器（用于生成ID）
    /// Counter (used for ID generation)
    counter: Arc<RwLock<u64>>,
}

impl TodoManager {
    /// 创建新的任务管理器
    /// Create a new task manager
    pub fn new() -> Self {
        Self {
            todos: Arc::new(RwLock::new(HashMap::new())),
            counter: Arc::new(RwLock::new(0)),
        }
    }

    /// 生成新的 Todo ID
    /// Generate a new Todo ID
    async fn generate_id(&self) -> String {
        let mut counter = self.counter.write().await;
        *counter += 1;
        format!("todo_{}", *counter)
    }

    /// 阶段1: 接收想法，创建 Todo
    /// Phase 1: Receive ideas, create Todos
    pub async fn receive_idea(
        &self,
        raw_idea: &str,
        priority: Option<TodoPriority>,
        metadata: Option<HashMap<String, String>>,
    ) -> TodoItem {
        let id = self.generate_id().await;
        let priority = priority.unwrap_or(TodoPriority::Medium);

        let mut todo = TodoItem::new(&id, raw_idea, priority);
        if let Some(meta) = metadata {
            todo.metadata = meta;
        }

        // 保存
        // Save
        {
            let mut todos = self.todos.write().await;
            todos.insert(id.clone(), todo.clone());
        }

        tracing::info!("Created todo: {} - {}", id, raw_idea);
        todo
    }

    /// 获取 Todo
    /// Get Todo
    pub async fn get_todo(&self, todo_id: &str) -> Option<TodoItem> {
        let todos = self.todos.read().await;
        todos.get(todo_id).cloned()
    }

    /// 获取所有 Todo
    /// List all Todos
    pub async fn list_todos(&self) -> Vec<TodoItem> {
        let todos = self.todos.read().await;
        todos.values().cloned().collect()
    }

    /// 按状态筛选 Todo
    /// Filter Todos by status
    pub async fn list_by_status(&self, status: TodoStatus) -> Vec<TodoItem> {
        let todos = self.todos.read().await;
        todos
            .values()
            .filter(|t| t.status == status)
            .cloned()
            .collect()
    }

    /// 更新 Todo 状态
    /// Update Todo status
    pub async fn update_status(&self, todo_id: &str, status: TodoStatus) {
        let mut todos = self.todos.write().await;
        if let Some(todo) = todos.get_mut(todo_id) {
            todo.update_status(status);
        }
    }

    /// 设置澄清后的需求
    /// Set clarified requirements
    pub async fn set_requirement(&self, todo_id: &str, requirement: ProjectRequirement) {
        let mut todos = self.todos.write().await;
        if let Some(todo) = todos.get_mut(todo_id) {
            todo.clarified_requirement = Some(requirement);
            todo.update_status(TodoStatus::Pending);
        }
    }

    /// 分配执行 Agent
    /// Assign execution Agents
    pub async fn assign_agents(&self, todo_id: &str, agent_ids: Vec<String>) {
        let mut todos = self.todos.write().await;
        if let Some(todo) = todos.get_mut(todo_id) {
            todo.assigned_agents = agent_ids;
        }
    }

    /// 设置执行结果
    /// Set execution result
    pub async fn set_result(&self, todo_id: &str, result: ExecutionResult) {
        let mut todos = self.todos.write().await;
        if let Some(todo) = todos.get_mut(todo_id) {
            todo.execution_result = Some(result);
            todo.update_status(TodoStatus::Completed);
        }
    }

    /// 删除 Todo
    /// Delete Todo
    pub async fn delete_todo(&self, todo_id: &str) -> Option<TodoItem> {
        let mut todos = self.todos.write().await;
        todos.remove(todo_id)
    }

    /// 获取统计信息
    /// Get statistics
    pub async fn get_statistics(&self) -> HashMap<String, usize> {
        let todos = self.todos.read().await;
        let mut stats = HashMap::new();

        stats.insert("total".to_string(), todos.len());

        let pending = todos
            .values()
            .filter(|t| t.status == TodoStatus::Pending)
            .count();
        stats.insert("pending".to_string(), pending);

        let in_progress = todos
            .values()
            .filter(|t| t.status == TodoStatus::InProgress)
            .count();
        stats.insert("in_progress".to_string(), in_progress);

        let completed = todos
            .values()
            .filter(|t| t.status == TodoStatus::Completed)
            .count();
        stats.insert("completed".to_string(), completed);

        stats
    }
}

impl Default for TodoManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_receive_idea() {
        let manager = TodoManager::new();
        let todo = manager.receive_idea("Test idea", None, None).await;

        assert_eq!(todo.raw_idea, "Test idea");
        assert_eq!(todo.priority, TodoPriority::Medium);
        assert_eq!(todo.status, TodoStatus::Pending);
    }

    #[tokio::test]
    async fn test_update_status() {
        let manager = TodoManager::new();
        let todo = manager.receive_idea("Test", None, None).await;

        manager
            .update_status(&todo.id, TodoStatus::InProgress)
            .await;

        let updated = manager.get_todo(&todo.id).await.unwrap();
        assert_eq!(updated.status, TodoStatus::InProgress);
    }

    #[tokio::test]
    async fn test_statistics() {
        let manager = TodoManager::new();
        manager.receive_idea("Task 1", None, None).await;
        manager.receive_idea("Task 2", None, None).await;

        let stats = manager.get_statistics().await;
        assert_eq!(stats.get("total"), Some(&2));
        assert_eq!(stats.get("pending"), Some(&2));
    }
}
