//! Prompt 模板引擎
//! Prompt template engine
//!
//! 提供强大的模板变量替换和验证功能
//! Provides powerful template variable replacement and validation functions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Prompt 模板错误
/// Prompt template errors
#[derive(Debug, Error)]
pub enum PromptError {
    /// 模板未找到
    /// Template not found
    #[error("Template not found: {0}")]
    TemplateNotFound(String),
    /// 变量未提供
    /// Variable not provided
    #[error("Required variable not provided: {0}")]
    MissingVariable(String),
    /// 变量类型错误
    /// Variable type mismatch
    #[error("Variable type mismatch for '{name}': expected {expected}, got {actual}")]
    TypeMismatch {
        name: String,
        expected: String,
        actual: String,
    },
    /// 验证失败
    /// Validation failed
    #[error("Validation failed for variable '{name}': {reason}")]
    ValidationFailed { name: String, reason: String },
    /// 解析错误
    /// Parse error
    #[error("Parse error: {0}")]
    ParseError(String),
    /// IO 错误
    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    /// YAML 解析错误
    /// YAML parsing error
    #[error("YAML error: {0}")]
    YamlError(String),
    /// Lock poisoning error
    /// Occurs when a thread panics while holding a lock
    #[error("Lock poisoned: {0}")]
    LockPoisoned(String),
}

/// Prompt 结果类型
/// Prompt result type
pub type PromptResult<T> = Result<T, PromptError>;

/// 变量类型
/// Variable types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VariableType {
    /// 字符串类型
    /// String type
    #[default]
    String,
    /// 整数类型
    /// Integer type
    Integer,
    /// 浮点类型
    /// Float type
    Float,
    /// 布尔类型
    /// Boolean type
    Boolean,
    /// 列表类型
    /// List type
    List,
    /// JSON 对象类型
    /// JSON object type
    Json,
}

impl VariableType {
    /// 验证值是否符合类型
    /// Validate if value matches type
    pub fn validate(&self, value: &str) -> bool {
        match self {
            VariableType::String => true,
            VariableType::Integer => value.parse::<i64>().is_ok(),
            VariableType::Float => value.parse::<f64>().is_ok(),
            VariableType::Boolean => {
                matches!(value.to_lowercase().as_str(), "true" | "false" | "1" | "0")
            }
            VariableType::List => value.starts_with('[') && value.ends_with(']'),
            VariableType::Json => serde_json::from_str::<serde_json::Value>(value).is_ok(),
        }
    }
}

/// Prompt 变量定义
/// Prompt variable definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptVariable {
    /// 变量名称
    /// Variable name
    pub name: String,
    /// 变量描述
    /// Variable description
    #[serde(default)]
    pub description: Option<String>,
    /// 变量类型
    /// Variable type
    #[serde(default)]
    pub var_type: VariableType,
    /// 是否必需
    /// Is required
    #[serde(default = "default_true")]
    pub required: bool,
    /// 默认值
    /// Default value
    #[serde(default)]
    pub default: Option<String>,
    /// 验证正则表达式
    /// Validation regex pattern
    #[serde(default)]
    pub pattern: Option<String>,
    /// 枚举选项
    /// Enum options
    #[serde(default)]
    pub enum_values: Option<Vec<String>>,
}

fn default_true() -> bool {
    true
}

impl PromptVariable {
    /// 创建新的变量定义
    /// Create new variable definition
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            var_type: VariableType::String,
            required: true,
            default: None,
            pattern: None,
            enum_values: None,
        }
    }

    /// 设置描述
    /// Set description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// 设置类型
    /// Set type
    pub fn with_type(mut self, var_type: VariableType) -> Self {
        self.var_type = var_type;
        self
    }

    /// 设置是否必需
    /// Set if required
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// 设置默认值
    /// Set default value
    pub fn with_default(mut self, default: impl Into<String>) -> Self {
        self.default = Some(default.into());
        self.required = false;
        self
    }

    /// 设置验证正则
    /// Set validation regex
    pub fn with_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.pattern = Some(pattern.into());
        self
    }

    /// 设置枚举值
    /// Set enum values
    pub fn with_enum(mut self, values: Vec<String>) -> Self {
        self.enum_values = Some(values);
        self
    }

    /// 验证值
    /// Validate value
    pub fn validate(&self, value: &str) -> PromptResult<()> {
        // 类型验证
        // Type validation
        if !self.var_type.validate(value) {
            return Err(PromptError::TypeMismatch {
                name: self.name.clone(),
                expected: format!("{:?}", self.var_type),
                actual: "invalid".to_string(),
            });
        }

        // 正则验证
        // Regex validation
        if let Some(ref pattern) = self.pattern {
            let re =
                regex::Regex::new(pattern).map_err(|e| PromptError::ParseError(e.to_string()))?;
            if !re.is_match(value) {
                return Err(PromptError::ValidationFailed {
                    name: self.name.clone(),
                    reason: format!("Value does not match pattern: {}", pattern),
                });
            }
        }

        // 枚举验证
        // Enum validation
        if let Some(ref enum_values) = self.enum_values
            && !enum_values.contains(&value.to_string())
        {
            return Err(PromptError::ValidationFailed {
                name: self.name.clone(),
                reason: format!("Value must be one of: {:?}", enum_values),
            });
        }

        Ok(())
    }
}

/// Prompt 模板
/// Prompt template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    /// 模板 ID
    /// Template ID
    pub id: String,
    /// 模板名称
    /// Template name
    #[serde(default)]
    pub name: Option<String>,
    /// 模板描述
    /// Template description
    #[serde(default)]
    pub description: Option<String>,
    /// 模板内容
    /// Template content
    #[serde(default)]
    pub content: String,
    /// 变量定义
    /// Variable definitions
    #[serde(default)]
    pub variables: Vec<PromptVariable>,
    /// 标签
    /// Tags
    #[serde(default)]
    pub tags: Vec<String>,
    /// 版本
    /// Version
    #[serde(default)]
    pub version: Option<String>,
    /// 元数据
    /// Metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl PromptTemplate {
    /// 创建新模板
    /// Create new template
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            description: None,
            content: String::new(),
            variables: Vec::new(),
            tags: Vec::new(),
            version: None,
            metadata: HashMap::new(),
        }
    }

    /// 设置名称
    /// Set name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// 设置描述
    /// Set description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// 设置内容
    /// Set content
    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        // 自动解析变量
        // Auto-parse variables
        self.parse_variables();
        self
    }

    /// 添加变量定义
    /// Add variable definition
    pub fn with_variable(mut self, variable: PromptVariable) -> Self {
        self.variables.push(variable);
        self
    }

    /// 添加标签
    /// Add tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// 设置版本
    /// Set version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// 添加元数据
    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// 解析模板中的变量（不覆盖已有定义）
    /// Parse template variables (don't overwrite existing)
    fn parse_variables(&mut self) {
        // 不自动解析，让用户手动定义变量
        // No auto-parse, let user define variables manually
        // 这样可以保留用户设置的默认值和验证规则
        // This preserves user default values and validation rules
    }

    /// 获取所有预定义变量名
    /// Get all predefined variable names
    pub fn variable_names(&self) -> Vec<&str> {
        self.variables.iter().map(|v| v.name.as_str()).collect()
    }

    /// 获取模板中所有变量名（从内容中解析）
    /// Get all template variable names (parse from content)
    pub fn extract_variables(&self) -> Vec<String> {
        let re = regex::Regex::new(r"\{(\w+)\}").unwrap();
        let mut vars = std::collections::HashSet::new();

        for cap in re.captures_iter(&self.content) {
            vars.insert(cap[1].to_string());
        }

        vars.into_iter().collect()
    }

    /// 获取必需变量
    /// Get required variables
    pub fn required_variables(&self) -> Vec<&PromptVariable> {
        self.variables.iter().filter(|v| v.required).collect()
    }

    /// 渲染模板
    /// Render template
    ///
    /// # 参数
    /// # Parameters
    /// - `vars`: 变量名和值的列表
    /// - `vars`: List of variable names and values
    ///
    /// # 示例
    /// # Example
    /// ```rust,ignore
    /// let template = PromptTemplate::new("greeting")
    ///     .with_content("Hello, {name}! Welcome to {place}.");
    ///
    /// let result = template.render(&[
    ///     ("name", "Alice"),
    ///     ("place", "Wonderland"),
    /// ])?;
    /// assert_eq!(result, "Hello, Alice! Welcome to Wonderland.");
    /// ```
    pub fn render(&self, vars: &[(&str, &str)]) -> PromptResult<String> {
        let var_map: HashMap<&str, &str> = vars.iter().copied().collect();
        self.render_with_map(&var_map)
    }

    /// 使用 HashMap 渲染模板
    /// Render template using HashMap
    pub fn render_with_map(&self, vars: &HashMap<&str, &str>) -> PromptResult<String> {
        let mut result = self.content.clone();

        // 首先处理预定义的变量（带验证和默认值）
        // First handle predefined variables (with validation and defaults)
        for var_def in &self.variables {
            let placeholder = format!("{{{}}}", var_def.name);

            if let Some(&value) = vars.get(var_def.name.as_str()) {
                // 验证值
                // Validate value
                var_def.validate(value)?;
                result = result.replace(&placeholder, value);
            } else if let Some(ref default) = var_def.default {
                // 使用默认值
                // Use default value
                result = result.replace(&placeholder, default);
            } else if var_def.required {
                // 缺少必需变量
                // Missing required variable
                return Err(PromptError::MissingVariable(var_def.name.clone()));
            }
        }

        // 然后处理模板中存在但未在 variables 中预定义的变量
        // Then handle variables in template not predefined in variables
        let re = regex::Regex::new(r"\{(\w+)\}").unwrap();
        let defined_vars: std::collections::HashSet<_> =
            self.variables.iter().map(|v| v.name.as_str()).collect();

        // 收集所有未定义但在模板中出现的变量
        // Collect all undefined variables appearing in template
        let mut missing = Vec::new();
        for cap in re.captures_iter(&result.clone()) {
            let var_name = &cap[1];
            if !defined_vars.contains(var_name) {
                if let Some(&value) = vars.get(var_name) {
                    let placeholder = format!("{{{}}}", var_name);
                    result = result.replace(&placeholder, value);
                } else {
                    missing.push(var_name.to_string());
                }
            }
        }

        // 如果还有未替换的变量，报错
        // If unreplaced variables remain, return error
        if !missing.is_empty() {
            return Err(PromptError::MissingVariable(missing.join(", ")));
        }

        Ok(result)
    }

    /// 使用 owned HashMap 渲染模板
    /// Render template using owned HashMap
    pub fn render_with_owned_map(&self, vars: &HashMap<String, String>) -> PromptResult<String> {
        let borrowed: HashMap<&str, &str> =
            vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        self.render_with_map(&borrowed)
    }

    /// 部分渲染（只替换提供的变量）
    /// Partial render (only replace provided variables)
    pub fn partial_render(&self, vars: &[(&str, &str)]) -> String {
        let var_map: HashMap<&str, &str> = vars.iter().copied().collect();
        let mut result = self.content.clone();

        for (name, value) in var_map {
            let placeholder = format!("{{{}}}", name);
            result = result.replace(&placeholder, value);
        }

        result
    }

    /// 检查模板是否有效（所有必需变量都有默认值或在提供的变量中）
    /// Check if template is valid (all required vars have defaults or values)
    pub fn is_valid_with(&self, vars: &[&str]) -> bool {
        let var_set: std::collections::HashSet<_> = vars.iter().copied().collect();

        // 检查预定义的必需变量
        // Check predefined required variables
        for var_def in &self.variables {
            if var_def.required
                && var_def.default.is_none()
                && !var_set.contains(var_def.name.as_str())
            {
                return false;
            }
        }

        // 检查模板中的未定义变量
        // Check undefined variables in template
        let re = regex::Regex::new(r"\{(\w+)\}").unwrap();
        let defined_vars: std::collections::HashSet<_> =
            self.variables.iter().map(|v| v.name.as_str()).collect();

        for cap in re.captures_iter(&self.content) {
            let var_name = &cap[1];
            // 如果变量未在预定义列表中，且未在提供的变量中
            // If variable not in predefined list and not in provided variables
            if !defined_vars.contains(var_name) && !var_set.contains(var_name) {
                return false;
            }
        }

        true
    }
}

/// Prompt 组合（多个模板的组合）
/// Prompt composition (combination of multiple templates)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptComposition {
    /// 组合 ID
    /// Composition ID
    pub id: String,
    /// 组合描述
    /// Composition description
    #[serde(default)]
    pub description: Option<String>,
    /// 模板 ID 列表（按顺序组合）
    /// List of template IDs (combined in order)
    pub template_ids: Vec<String>,
    /// 分隔符
    /// Separator
    #[serde(default = "default_separator")]
    pub separator: String,
}

fn default_separator() -> String {
    "\n\n".to_string()
}

impl PromptComposition {
    /// 创建新的组合
    /// Create new composition
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: None,
            template_ids: Vec::new(),
            separator: "\n\n".to_string(),
        }
    }

    /// 添加模板
    /// Add template
    pub fn add_template(mut self, template_id: impl Into<String>) -> Self {
        self.template_ids.push(template_id.into());
        self
    }

    /// 设置分隔符
    /// Set separator
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    /// 设置描述
    /// Set description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_basic() {
        let template = PromptTemplate::new("test")
            .with_content("Hello, {name}!")
            .with_description("A greeting template");

        assert_eq!(template.id, "test");
        assert_eq!(template.extract_variables(), vec!["name"]);

        let result = template.render(&[("name", "World")]).unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_template_multiple_vars() {
        let template = PromptTemplate::new("test")
            .with_content("Hello, {name}! Welcome to {place}. Your role is {role}.");

        let result = template
            .render(&[
                ("name", "Alice"),
                ("place", "Wonderland"),
                ("role", "explorer"),
            ])
            .unwrap();

        assert_eq!(
            result,
            "Hello, Alice! Welcome to Wonderland. Your role is explorer."
        );
    }

    #[test]
    fn test_template_with_default() {
        let template = PromptTemplate::new("test")
            .with_content("Hello, {name}!")
            .with_variable(PromptVariable::new("name").with_default("World"));

        // 不提供变量时使用默认值
        // Use default value when variable is not provided
        let result = template.render(&[]).unwrap();
        assert_eq!(result, "Hello, World!");

        // 提供变量时使用提供的值
        // Use provided value when variable is provided
        let result = template.render(&[("name", "Alice")]).unwrap();
        assert_eq!(result, "Hello, Alice!");
    }

    #[test]
    fn test_template_missing_required() {
        let template = PromptTemplate::new("test").with_content("Hello, {name}!");

        let result = template.render(&[]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PromptError::MissingVariable(_)
        ));
    }

    #[test]
    fn test_variable_type_validation() {
        assert!(VariableType::String.validate("anything"));
        assert!(VariableType::Integer.validate("123"));
        assert!(!VariableType::Integer.validate("abc"));
        assert!(VariableType::Float.validate("3.14"));
        assert!(VariableType::Boolean.validate("true"));
        assert!(VariableType::Boolean.validate("false"));
        assert!(VariableType::Json.validate(r#"{"key": "value"}"#));
    }

    #[test]
    fn test_variable_enum() {
        let var = PromptVariable::new("language")
            .with_enum(vec!["rust".to_string(), "python".to_string()]);

        assert!(var.validate("rust").is_ok());
        assert!(var.validate("python").is_ok());
        assert!(var.validate("java").is_err());
    }

    #[test]
    fn test_partial_render() {
        let template =
            PromptTemplate::new("test").with_content("Hello, {name}! Your {item} is ready.");

        let result = template.partial_render(&[("name", "Alice")]);
        assert_eq!(result, "Hello, Alice! Your {item} is ready.");
    }

    #[test]
    fn test_is_valid_with() {
        let template = PromptTemplate::new("test")
            .with_content("{required_var} and {optional_var}")
            .with_variable(PromptVariable::new("required_var"))
            .with_variable(PromptVariable::new("optional_var").with_default("default"));

        assert!(template.is_valid_with(&["required_var"]));
        assert!(!template.is_valid_with(&[]));
        assert!(!template.is_valid_with(&["optional_var"]));
    }
}
