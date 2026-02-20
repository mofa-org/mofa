//! WASM-Dora Integration Module
//!
//! Integrates WASM plugin runtime with Dora dataflow framework.
//! This enables WASM modules to be used as Dora operators and nodes.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, error, info, warn};
use wasmtime::Val;

use crate::dora_adapter::{
    DoraError, DoraNodeConfig,
    DoraResult, MoFAOperator, NodeEventLoop,
    OperatorConfig,
    OperatorInput, OperatorOutput,
};

use super::manager::{PluginEvent, PluginHandle, WasmPluginManager};
use super::plugin::{WasmPlugin, WasmPluginConfig, WasmPluginState};
use super::runtime::{RuntimeConfig, WasmRuntime};
use super::types::{
    ExecutionConfig, PluginCapability, ResourceLimits, WasmError,
    WasmResult, WasmValue,
};

/// WASM Dora Operator configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmOperatorConfig {
    /// Operator ID
    pub operator_id: String,
    /// WASM module source (bytes, path, or WAT)
    pub wasm_source: WasmSource,
    /// Function to call for processing
    pub process_function: String,
    /// Optional init function
    pub init_function: Option<String>,
    /// Optional cleanup function
    pub cleanup_function: Option<String>,
    /// Input port mappings
    pub input_mapping: HashMap<String, String>,
    /// Output port mappings
    pub output_mapping: HashMap<String, String>,
    /// Resource limits
    pub resource_limits: ResourceLimits,
    /// Plugin capabilities
    pub capabilities: Vec<PluginCapability>,
}

impl Default for WasmOperatorConfig {
    fn default() -> Self {
        Self {
            operator_id: uuid::Uuid::now_v7().to_string(),
            wasm_source: WasmSource::Wat(DEFAULT_PASSTHROUGH_WAT.to_string()),
            process_function: "process".to_string(),
            init_function: Some("init".to_string()),
            cleanup_function: Some("cleanup".to_string()),
            input_mapping: HashMap::new(),
            output_mapping: HashMap::new(),
            resource_limits: ResourceLimits::default(),
            capabilities: vec![
                PluginCapability::ReadConfig,
                PluginCapability::SendMessage,
            ],
        }
    }
}

impl WasmOperatorConfig {
    pub fn new(operator_id: &str) -> Self {
        Self {
            operator_id: operator_id.to_string(),
            ..Default::default()
        }
    }

    pub fn with_wat(mut self, wat: &str) -> Self {
        self.wasm_source = WasmSource::Wat(wat.to_string());
        self
    }

    pub fn with_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.wasm_source = WasmSource::Bytes(bytes);
        self
    }

    pub fn with_file(mut self, path: &str) -> Self {
        self.wasm_source = WasmSource::File(path.to_string());
        self
    }

    pub fn with_process_function(mut self, name: &str) -> Self {
        self.process_function = name.to_string();
        self
    }

    pub fn with_input(mut self, port: &str, mapping: &str) -> Self {
        self.input_mapping.insert(port.to_string(), mapping.to_string());
        self
    }

    pub fn with_output(mut self, port: &str, mapping: &str) -> Self {
        self.output_mapping.insert(port.to_string(), mapping.to_string());
        self
    }

    pub fn with_capability(mut self, cap: PluginCapability) -> Self {
        if !self.capabilities.contains(&cap) {
            self.capabilities.push(cap);
        }
        self
    }
}

/// WASM module source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WasmSource {
    /// WAT text format
    Wat(String),
    /// Binary WASM bytes
    Bytes(Vec<u8>),
    /// File path
    File(String),
}

/// Default passthrough WAT module
const DEFAULT_PASSTHROUGH_WAT: &str = r#"
    (module
        (memory (export "memory") 1)

        ;; Simple passthrough - returns input length
        (func (export "process") (param i32 i32) (result i32)
            local.get 1  ;; Return the length
        )

        (func (export "init") (result i32)
            i32.const 0  ;; Success
        )

        (func (export "cleanup") (result i32)
            i32.const 0  ;; Success
        )
    )
"#;

/// WASM Dora Operator
///
/// Wraps a WASM plugin as a Dora operator for use in dataflow graphs.
pub struct WasmDoraOperator {
    /// Configuration
    config: WasmOperatorConfig,
    /// WASM runtime
    runtime: Arc<WasmRuntime>,
    /// WASM plugin
    plugin: Option<Arc<WasmPlugin>>,
    /// Operator state
    state: RwLock<WasmOperatorState>,
    /// Execution metrics
    metrics: RwLock<WasmOperatorMetrics>,
    /// Shared memory for input/output
    shared_memory: RwLock<Vec<u8>>,
}

/// WASM operator state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WasmOperatorState {
    Created,
    Initialized,
    Running,
    Paused,
    Error,
    Stopped,
}

impl Default for WasmOperatorState {
    fn default() -> Self {
        Self::Created
    }
}

/// WASM operator metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WasmOperatorMetrics {
    /// Total inputs processed
    pub inputs_processed: u64,
    /// Total outputs produced
    pub outputs_produced: u64,
    /// Total processing time in nanoseconds
    pub total_processing_time_ns: u64,
    /// Average processing time in nanoseconds
    pub avg_processing_time_ns: u64,
    /// Error count
    pub error_count: u64,
    /// Last processing timestamp
    pub last_processed: u64,
}

impl WasmOperatorMetrics {
    fn record(&mut self, duration_ns: u64, outputs: u64, success: bool) {
        self.inputs_processed += 1;
        self.outputs_produced += outputs;
        self.total_processing_time_ns += duration_ns;
        self.avg_processing_time_ns = self.total_processing_time_ns / self.inputs_processed;
        if !success {
            self.error_count += 1;
        }
        self.last_processed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }
}

impl WasmDoraOperator {
    /// Create a new WASM Dora operator
    pub fn new(config: WasmOperatorConfig) -> DoraResult<Self> {
        // Create runtime with appropriate config
        let mut rt_config = RuntimeConfig::new();
        rt_config.execution_config.fuel_metering = false;
        rt_config.execution_config.epoch_interruption = false;
        rt_config.resource_limits = config.resource_limits.clone();

        let runtime = Arc::new(WasmRuntime::new(rt_config)
            .map_err(|e| DoraError::OperatorError(e.to_string()))?);

        Ok(Self {
            config,
            runtime,
            plugin: None,
            state: RwLock::new(WasmOperatorState::Created),
            metrics: RwLock::new(WasmOperatorMetrics::default()),
            shared_memory: RwLock::new(vec![0u8; 64 * 1024]), // 64KB shared memory
        })
    }

    /// Create with existing runtime
    pub fn with_runtime(config: WasmOperatorConfig, runtime: Arc<WasmRuntime>) -> Self {
        Self {
            config,
            runtime,
            plugin: None,
            state: RwLock::new(WasmOperatorState::Created),
            metrics: RwLock::new(WasmOperatorMetrics::default()),
            shared_memory: RwLock::new(vec![0u8; 64 * 1024]),
        }
    }

    /// Get operator ID
    pub fn operator_id(&self) -> &str {
        &self.config.operator_id
    }

    /// Get current state
    pub async fn state(&self) -> WasmOperatorState {
        *self.state.read().await
    }

    /// Get metrics
    pub async fn metrics(&self) -> WasmOperatorMetrics {
        self.metrics.read().await.clone()
    }

    /// Initialize the operator
    pub async fn init(&mut self) -> DoraResult<()> {
        let mut state = self.state.write().await;
        if *state != WasmOperatorState::Created {
            return Err(DoraError::OperatorError(
                format!("Cannot initialize operator in state {:?}", *state)
            ));
        }

        *state = WasmOperatorState::Created;
        drop(state);

        // Load WASM module
        let plugin = self.load_wasm_plugin().await?;

        // Initialize plugin
        plugin.initialize().await
            .map_err(|e| DoraError::OperatorError(e.to_string()))?;

        // Call init function if defined
        if let Some(init_fn) = &self.config.init_function {
            if plugin.has_export(init_fn).await {
                let result = plugin.call_i32(init_fn, &[]).await
                    .map_err(|e| DoraError::OperatorError(e.to_string()))?;
                if result != 0 {
                    return Err(DoraError::OperatorError(
                        format!("Init function returned error code: {}", result)
                    ));
                }
            }
        }

        self.plugin = Some(Arc::new(plugin));
        *self.state.write().await = WasmOperatorState::Initialized;

        info!("WasmDoraOperator {} initialized", self.config.operator_id);
        Ok(())
    }

    /// Load WASM plugin from source
    async fn load_wasm_plugin(&self) -> DoraResult<WasmPlugin> {
        let mut plugin_config = WasmPluginConfig::new(&self.config.operator_id);
        plugin_config.resource_limits = self.config.resource_limits.clone();
        plugin_config.resource_limits.max_fuel = None; // Disable fuel for operators
        plugin_config.allowed_capabilities = self.config.capabilities.clone();

        let plugin = match &self.config.wasm_source {
            WasmSource::Wat(wat) => {
                self.runtime.create_plugin_from_wat(wat, plugin_config).await
            }
            WasmSource::Bytes(bytes) => {
                self.runtime.create_plugin_from_bytes(bytes, plugin_config).await
            }
            WasmSource::File(path) => {
                let bytes = tokio::fs::read(path).await
                    .map_err(|e| WasmError::IoError(e))?;
                self.runtime.create_plugin_from_bytes(&bytes, plugin_config).await
            }
        }.map_err(|e| DoraError::OperatorError(e.to_string()))?;

        Ok(plugin)
    }

    /// Process input data
    pub async fn process(&self, input: OperatorInput) -> DoraResult<Vec<OperatorOutput>> {
        let state = self.state.read().await;
        if *state != WasmOperatorState::Initialized && *state != WasmOperatorState::Running {
            return Err(DoraError::OperatorError(
                format!("Operator not ready, state: {:?}", *state)
            ));
        }
        drop(state);

        // Update state to running
        *self.state.write().await = WasmOperatorState::Running;

        let start = Instant::now();
        let result = self.process_internal(input).await;
        let duration = start.elapsed();

        // Update metrics
        let success = result.is_ok();
        let output_count = result.as_ref().map(|o| o.len() as u64).unwrap_or(0);
        self.metrics.write().await.record(duration.as_nanos() as u64, output_count, success);

        result
    }

    async fn process_internal(&self, input: OperatorInput) -> DoraResult<Vec<OperatorOutput>> {
        let plugin = self.plugin.as_ref()
            .ok_or_else(|| DoraError::OperatorError("Plugin not loaded".to_string()))?;

        // Write input to shared memory
        let input_len = input.data.len();
        {
            let mut shared_mem = self.shared_memory.write().await;
            if input_len > shared_mem.len() {
                shared_mem.resize(input_len, 0);
            }
            shared_mem[..input_len].copy_from_slice(&input.data);
        }

        // Call process function with (ptr, len) -> result_len
        // For simplicity, we use a convention where:
        // - Input: (i32 ptr, i32 len) pointing to shared memory
        // - Output: i32 result length (output written to shared memory at offset 0)
        let result_len = plugin.call_i32(
            &self.config.process_function,
            &[Val::I32(0), Val::I32(input_len as i32)]
        ).await.map_err(|e| DoraError::OperatorError(e.to_string()))?;

        // Read output from shared memory
        let output_data = if result_len > 0 {
            let shared_mem = self.shared_memory.read().await;
            shared_mem[..result_len as usize].to_vec()
        } else {
            // If no output, pass through input
            input.data.clone()
        };

        // Map to output port
        let output_id = self.config.output_mapping
            .get(&input.input_id)
            .cloned()
            .unwrap_or_else(|| "output".to_string());

        let output = OperatorOutput::new(output_id, output_data);
        Ok(vec![output])
    }

    /// Cleanup the operator
    pub async fn cleanup(&mut self) -> DoraResult<()> {
        if let Some(plugin) = &self.plugin {
            // Call cleanup function if defined
            if let Some(cleanup_fn) = &self.config.cleanup_function {
                if plugin.has_export(cleanup_fn).await {
                    let _ = plugin.call_i32(cleanup_fn, &[]).await;
                }
            }

            // Stop plugin
            plugin.stop().await
                .map_err(|e| DoraError::OperatorError(e.to_string()))?;
        }

        self.plugin = None;
        *self.state.write().await = WasmOperatorState::Stopped;

        info!("WasmDoraOperator {} cleaned up", self.config.operator_id);
        Ok(())
    }
}

/// Implement MoFAOperator trait for WasmDoraOperator
#[async_trait::async_trait]
impl MoFAOperator for WasmDoraOperator {
    fn operator_id(&self) -> &str {
        &self.config.operator_id
    }

    async fn init_operator(&mut self) -> DoraResult<()> {
        self.init().await
    }

    async fn process(&mut self, input: OperatorInput) -> DoraResult<Vec<OperatorOutput>> {
        WasmDoraOperator::process(self, input).await
    }

    async fn cleanup(&mut self) -> DoraResult<()> {
        WasmDoraOperator::cleanup(self).await
    }
}

/// WASM Dora Node configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmNodeConfig {
    /// Node ID
    pub node_id: String,
    /// Node name
    pub name: String,
    /// Input ports
    pub inputs: Vec<String>,
    /// Output ports
    pub outputs: Vec<String>,
    /// WASM operators in this node
    pub operators: Vec<WasmOperatorConfig>,
    /// Event buffer size
    pub event_buffer_size: usize,
    /// Auto-reload on WASM file change
    pub hot_reload: bool,
}

impl Default for WasmNodeConfig {
    fn default() -> Self {
        Self {
            node_id: uuid::Uuid::now_v7().to_string(),
            name: "wasm_node".to_string(),
            inputs: vec!["input".to_string()],
            outputs: vec!["output".to_string()],
            operators: Vec::new(),
            event_buffer_size: 1024,
            hot_reload: false,
        }
    }
}

impl WasmNodeConfig {
    pub fn new(node_id: &str) -> Self {
        Self {
            node_id: node_id.to_string(),
            ..Default::default()
        }
    }

    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    pub fn with_input(mut self, port: &str) -> Self {
        self.inputs.push(port.to_string());
        self
    }

    pub fn with_output(mut self, port: &str) -> Self {
        self.outputs.push(port.to_string());
        self
    }

    pub fn with_operator(mut self, op_config: WasmOperatorConfig) -> Self {
        self.operators.push(op_config);
        self
    }
}

/// WASM Dora Node
///
/// A Dora node that hosts multiple WASM operators
pub struct WasmDoraNode {
    /// Configuration
    config: WasmNodeConfig,
    /// Shared WASM runtime
    runtime: Arc<WasmRuntime>,
    /// WASM operators
    operators: RwLock<Vec<WasmDoraOperator>>,
    /// Node state
    state: RwLock<WasmNodeState>,
    /// Event sender
    event_tx: broadcast::Sender<WasmNodeEvent>,
    /// Input channel
    input_tx: mpsc::Sender<OperatorInput>,
    input_rx: RwLock<mpsc::Receiver<OperatorInput>>,
    /// Output channel
    output_tx: mpsc::Sender<OperatorOutput>,
    output_rx: RwLock<mpsc::Receiver<OperatorOutput>>,
}

/// WASM node state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WasmNodeState {
    Created,
    Initializing,
    Running,
    Paused,
    Stopping,
    Stopped,
    Error,
}

/// WASM node events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WasmNodeEvent {
    /// Node started
    Started { node_id: String },
    /// Node stopped
    Stopped { node_id: String },
    /// Operator added
    OperatorAdded { node_id: String, operator_id: String },
    /// Operator removed
    OperatorRemoved { node_id: String, operator_id: String },
    /// Input received
    InputReceived { node_id: String, input_id: String, size: usize },
    /// Output produced
    OutputProduced { node_id: String, output_id: String, size: usize },
    /// Error occurred
    Error { node_id: String, error: String },
}

impl WasmDoraNode {
    /// Create a new WASM Dora node
    pub fn new(config: WasmNodeConfig) -> DoraResult<Self> {
        // Create shared runtime
        let mut rt_config = RuntimeConfig::new();
        rt_config.execution_config.fuel_metering = false;
        rt_config.execution_config.epoch_interruption = false;

        let runtime = Arc::new(WasmRuntime::new(rt_config)
            .map_err(|e| DoraError::NodeInitError(e.to_string()))?);

        Self::with_runtime(config, runtime)
    }

    /// Create with existing runtime
    pub fn with_runtime(config: WasmNodeConfig, runtime: Arc<WasmRuntime>) -> DoraResult<Self> {
        let (event_tx, _) = broadcast::channel(config.event_buffer_size);
        let (input_tx, input_rx) = mpsc::channel(config.event_buffer_size);
        let (output_tx, output_rx) = mpsc::channel(config.event_buffer_size);

        Ok(Self {
            config,
            runtime,
            operators: RwLock::new(Vec::new()),
            state: RwLock::new(WasmNodeState::Created),
            event_tx,
            input_tx,
            input_rx: RwLock::new(input_rx),
            output_tx,
            output_rx: RwLock::new(output_rx),
        })
    }

    /// Get node ID
    pub fn node_id(&self) -> &str {
        &self.config.node_id
    }

    /// Get current state
    pub async fn state(&self) -> WasmNodeState {
        *self.state.read().await
    }

    /// Subscribe to node events
    pub fn subscribe(&self) -> broadcast::Receiver<WasmNodeEvent> {
        self.event_tx.subscribe()
    }

    /// Get input sender for feeding data to node
    pub fn input_sender(&self) -> mpsc::Sender<OperatorInput> {
        self.input_tx.clone()
    }

    /// Initialize the node
    pub async fn init(&self) -> DoraResult<()> {
        let mut state = self.state.write().await;
        if *state != WasmNodeState::Created {
            return Err(DoraError::NodeInitError(
                format!("Cannot init node in state {:?}", *state)
            ));
        }
        *state = WasmNodeState::Initializing;
        drop(state);

        // Create operators from config
        let mut operators = self.operators.write().await;
        for op_config in &self.config.operators {
            let operator = WasmDoraOperator::with_runtime(
                op_config.clone(),
                self.runtime.clone(),
            );
            operators.push(operator);
        }

        // Initialize all operators
        for op in operators.iter_mut() {
            op.init().await?;
            let _ = self.event_tx.send(WasmNodeEvent::OperatorAdded {
                node_id: self.config.node_id.clone(),
                operator_id: op.operator_id().to_string(),
            });
        }

        *self.state.write().await = WasmNodeState::Running;
        let _ = self.event_tx.send(WasmNodeEvent::Started {
            node_id: self.config.node_id.clone(),
        });

        info!("WasmDoraNode {} initialized with {} operators",
            self.config.node_id, operators.len());
        Ok(())
    }

    /// Add an operator dynamically
    pub async fn add_operator(&self, config: WasmOperatorConfig) -> DoraResult<()> {
        let mut operator = WasmDoraOperator::with_runtime(
            config.clone(),
            self.runtime.clone(),
        );
        operator.init().await?;

        let operator_id = operator.operator_id().to_string();
        self.operators.write().await.push(operator);

        let _ = self.event_tx.send(WasmNodeEvent::OperatorAdded {
            node_id: self.config.node_id.clone(),
            operator_id,
        });

        Ok(())
    }

    /// Remove an operator
    pub async fn remove_operator(&self, operator_id: &str) -> DoraResult<()> {
        let mut operators = self.operators.write().await;
        if let Some(pos) = operators.iter().position(|op| op.operator_id() == operator_id) {
            let mut op = operators.remove(pos);
            op.cleanup().await?;

            let _ = self.event_tx.send(WasmNodeEvent::OperatorRemoved {
                node_id: self.config.node_id.clone(),
                operator_id: operator_id.to_string(),
            });
        }
        Ok(())
    }

    /// Process a single input through all operators (chain)
    pub async fn process(&self, input: OperatorInput) -> DoraResult<Vec<OperatorOutput>> {
        let state = self.state.read().await;
        if *state != WasmNodeState::Running {
            return Err(DoraError::NodeNotRunning);
        }
        drop(state);

        let _ = self.event_tx.send(WasmNodeEvent::InputReceived {
            node_id: self.config.node_id.clone(),
            input_id: input.input_id.clone(),
            size: input.data.len(),
        });

        let operators = self.operators.read().await;
        if operators.is_empty() {
            // Passthrough if no operators
            return Ok(vec![OperatorOutput::new("output".to_string(), input.data)]);
        }

        // Chain execution through operators
        let mut current_input = input;
        let mut final_outputs = Vec::new();

        for (i, op) in operators.iter().enumerate() {
            let outputs = op.process(current_input.clone()).await?;

            if i == operators.len() - 1 {
                // Last operator - collect outputs
                final_outputs = outputs;
            } else if let Some(output) = outputs.into_iter().next() {
                // Intermediate - pass to next operator
                current_input = OperatorInput::new(output.output_id, output.data);
            }
        }

        for output in &final_outputs {
            let _ = self.event_tx.send(WasmNodeEvent::OutputProduced {
                node_id: self.config.node_id.clone(),
                output_id: output.output_id.clone(),
                size: output.data.len(),
            });
        }

        Ok(final_outputs)
    }

    /// Run the node event loop
    pub async fn run(&self) -> DoraResult<()> {
        let mut input_rx = self.input_rx.write().await;

        while *self.state.read().await == WasmNodeState::Running {
            tokio::select! {
                Some(input) = input_rx.recv() => {
                    match self.process(input).await {
                        Ok(outputs) => {
                            for output in outputs {
                                let _ = self.output_tx.send(output).await;
                            }
                        }
                        Err(e) => {
                            error!("Error processing input: {}", e);
                            let _ = self.event_tx.send(WasmNodeEvent::Error {
                                node_id: self.config.node_id.clone(),
                                error: e.to_string(),
                            });
                        }
                    }
                }
                else => break,
            }
        }

        Ok(())
    }

    /// Stop the node
    pub async fn stop(&self) -> DoraResult<()> {
        *self.state.write().await = WasmNodeState::Stopping;

        // Cleanup all operators
        let mut operators = self.operators.write().await;
        for op in operators.iter_mut() {
            op.cleanup().await?;
        }
        operators.clear();

        *self.state.write().await = WasmNodeState::Stopped;
        let _ = self.event_tx.send(WasmNodeEvent::Stopped {
            node_id: self.config.node_id.clone(),
        });

        info!("WasmDoraNode {} stopped", self.config.node_id);
        Ok(())
    }

    /// Get output receiver for consuming outputs
    pub async fn take_output_receiver(&self) -> mpsc::Receiver<OperatorOutput> {
        let (new_tx, new_rx) = mpsc::channel(self.config.event_buffer_size);
        let mut rx_guard = self.output_rx.write().await;
        std::mem::replace(&mut *rx_guard, new_rx)
    }
}

/// WASM Dora Pipeline
///
/// A pipeline of WASM nodes connected in a dataflow
pub struct WasmDoraPipeline {
    /// Pipeline ID
    id: String,
    /// Nodes in the pipeline
    nodes: RwLock<HashMap<String, Arc<WasmDoraNode>>>,
    /// Connections between nodes (source_node:output -> target_node:input)
    connections: RwLock<Vec<PipelineConnection>>,
    /// Pipeline state
    state: RwLock<PipelineState>,
    /// Shared runtime
    runtime: Arc<WasmRuntime>,
}

/// Connection between nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConnection {
    pub source_node: String,
    pub source_output: String,
    pub target_node: String,
    pub target_input: String,
}

/// Pipeline state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineState {
    Created,
    Running,
    Paused,
    Stopped,
}

impl WasmDoraPipeline {
    /// Create a new pipeline
    pub fn new(id: &str) -> DoraResult<Self> {
        let mut rt_config = RuntimeConfig::new();
        rt_config.execution_config.fuel_metering = false;
        rt_config.execution_config.epoch_interruption = false;

        let runtime = Arc::new(WasmRuntime::new(rt_config)
            .map_err(|e| DoraError::Internal(e.to_string()))?);

        Ok(Self {
            id: id.to_string(),
            nodes: RwLock::new(HashMap::new()),
            connections: RwLock::new(Vec::new()),
            state: RwLock::new(PipelineState::Created),
            runtime,
        })
    }

    /// Add a node to the pipeline
    pub async fn add_node(&self, config: WasmNodeConfig) -> DoraResult<()> {
        let node = WasmDoraNode::with_runtime(config.clone(), self.runtime.clone())?;
        self.nodes.write().await.insert(config.node_id, Arc::new(node));
        Ok(())
    }

    /// Connect two nodes
    pub async fn connect(
        &self,
        source_node: &str,
        source_output: &str,
        target_node: &str,
        target_input: &str,
    ) -> DoraResult<()> {
        let connection = PipelineConnection {
            source_node: source_node.to_string(),
            source_output: source_output.to_string(),
            target_node: target_node.to_string(),
            target_input: target_input.to_string(),
        };
        self.connections.write().await.push(connection);
        Ok(())
    }

    /// Initialize all nodes
    pub async fn init(&self) -> DoraResult<()> {
        let nodes = self.nodes.read().await;
        for node in nodes.values() {
            node.init().await?;
        }
        *self.state.write().await = PipelineState::Running;
        info!("WasmDoraPipeline {} initialized with {} nodes", self.id, nodes.len());
        Ok(())
    }

    /// Stop the pipeline
    pub async fn stop(&self) -> DoraResult<()> {
        let nodes = self.nodes.read().await;
        for node in nodes.values() {
            node.stop().await?;
        }
        *self.state.write().await = PipelineState::Stopped;
        info!("WasmDoraPipeline {} stopped", self.id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    const TEST_DOUBLER_WAT: &str = r#"
        (module
            (func (export "process") (param i32 i32) (result i32)
                ;; Return double the length (simplified test)
                local.get 1
                i32.const 2
                i32.mul
            )
            (func (export "init") (result i32)
                i32.const 0
            )
        )
    "#;

    #[tokio::test]
    async fn test_wasm_operator_config() {
        let config = WasmOperatorConfig::new("test-op")
            .with_wat(TEST_DOUBLER_WAT)
            .with_process_function("process")
            .with_input("in", "input")
            .with_output("out", "output");

        assert_eq!(config.operator_id, "test-op");
        assert_eq!(config.process_function, "process");
    }

    #[tokio::test]
    async fn test_wasm_dora_operator_init() {
        let config = WasmOperatorConfig::new("test-op")
            .with_wat(DEFAULT_PASSTHROUGH_WAT);

        let mut operator = WasmDoraOperator::new(config).unwrap();
        operator.init().await.unwrap();

        assert_eq!(operator.state().await, WasmOperatorState::Initialized);
    }

    #[tokio::test]
    async fn test_wasm_dora_operator_process() {
        let config = WasmOperatorConfig::new("test-op")
            .with_wat(DEFAULT_PASSTHROUGH_WAT);

        let mut operator = WasmDoraOperator::new(config).unwrap();
        operator.init().await.unwrap();

        let input = OperatorInput::new("input".to_string(), b"Hello, WASM!".to_vec());
        let outputs = operator.process(input).await.unwrap();

        assert_eq!(outputs.len(), 1);

        let metrics = operator.metrics().await;
        assert_eq!(metrics.inputs_processed, 1);
    }

    #[tokio::test]
    async fn test_wasm_dora_node_creation() {
        let config = WasmNodeConfig::new("test-node")
            .with_name("Test Node")
            .with_input("input")
            .with_output("output");

        let node = WasmDoraNode::new(config).unwrap();
        assert_eq!(node.state().await, WasmNodeState::Created);
    }

    #[tokio::test]
    async fn test_wasm_dora_node_with_operator() {
        let op_config = WasmOperatorConfig::new("passthrough")
            .with_wat(DEFAULT_PASSTHROUGH_WAT);

        let config = WasmNodeConfig::new("test-node")
            .with_operator(op_config);

        let node = WasmDoraNode::new(config).unwrap();
        node.init().await.unwrap();

        assert_eq!(node.state().await, WasmNodeState::Running);

        let input = OperatorInput::new("input".to_string(), b"Test data".to_vec());
        let outputs = node.process(input).await.unwrap();

        assert!(!outputs.is_empty());

        node.stop().await.unwrap();
        assert_eq!(node.state().await, WasmNodeState::Stopped);
    }

    #[tokio::test]
    async fn test_wasm_pipeline_creation() {
        let pipeline = WasmDoraPipeline::new("test-pipeline").unwrap();

        let node1_config = WasmNodeConfig::new("node1")
            .with_operator(WasmOperatorConfig::new("op1").with_wat(DEFAULT_PASSTHROUGH_WAT));

        pipeline.add_node(node1_config).await.unwrap();
        pipeline.init().await.unwrap();
        pipeline.stop().await.unwrap();
    }
}
