/**
 * MoFA Kotlin 绑定示例
 *
 * 使用方法：
 * 1. 首先构建 MoFA 库并生成 Kotlin 绑定：
 *
 *    # 构建库（release 模式）
 *    cargo build --features uniffi --release
 *
 *    # 生成 Kotlin 绑定
 *    cargo run --features uniffi --bin uniffi-bindgen generate \
 *        --library target/release/libaimos.dylib \
 *        --language kotlin \
 *        --out-dir bindings/kotlin
 *
 * 2. 编译并运行示例：
 *    cd bindings/kotlin
 *    kotlinc -include-runtime -d Example.jar Example.kt uniffi/mofa/*.kt
 *    java -Djava.library.path=. -jar Example.jar
 *
 * 注意：需要将生成的 .dylib (macOS) / .so (Linux) / .dll (Windows) 文件
 * 复制到 Kotlin 绑定目录或设置 java.library.path。
 */

package example

import uniffi.mofa.*

fun main() {
    println("=" .repeat(60))
    println("MoFA Kotlin 绑定示例")
    println("=".repeat(60))
    println()

    exampleVersion()
    exampleSimpleAgent()
    exampleTaskManager()
    exampleWorkflow()
    exampleMetrics()
    exampleRuntimeConfig()

    println("=".repeat(60))
    println("所有示例运行完成!")
    println("=".repeat(60))
}

/**
 * 获取版本信息示例
 */
fun exampleVersion() {
    println("=".repeat(60))
    println("MoFA 版本示例")
    println("=".repeat(60))

    val version = getVersion()
    println("MoFA 版本: $version")
    println()
}

/**
 * 简单智能体示例
 */
fun exampleSimpleAgent() {
    println("=".repeat(60))
    println("简单智能体示例")
    println("=".repeat(60))

    // 创建智能体
    val agent = SimpleAgent("agent-001", "MyAgent")
    println("创建智能体: ${agent.metadata().name}")
    println("初始状态: ${agent.state()}")

    // 添加能力
    agent.addCapability("text_generation")
    agent.addCapability("code_completion")

    // 添加依赖
    agent.addDependency("openai")

    // 获取元数据
    val metadata = agent.metadata()
    println("智能体ID: ${metadata.agentId}")
    println("能力: ${metadata.capabilities}")
    println("依赖: ${metadata.dependencies}")

    // 初始化智能体
    val config = AgentConfigDict(
        agentId = "agent-001",
        name = "MyAgent",
        nodeConfig = mapOf("model" to "gpt-4")
    )
    agent.init(config)
    println("初始化后状态: ${agent.state()}")

    // 暂停智能体
    agent.pause()
    println("暂停后状态: ${agent.state()}")

    // 销毁智能体
    agent.destroy()
    println("销毁后状态: ${agent.state()}")
    println()
}

/**
 * 任务管理器示例
 */
fun exampleTaskManager() {
    println("=".repeat(60))
    println("任务管理器示例")
    println("=".repeat(60))

    // 创建任务管理器
    val manager = TaskManager()

    // 提交任务
    val task1 = TaskRequestDict(
        taskId = "",  // 空字符串让系统自动生成 ID
        content = "分析代码质量",
        priority = TaskPriorityEnum.HIGH,
        deadlineMs = 5000UL,
        metadata = mapOf("project" to "mofa")
    )
    val task1Id = manager.submitTask(task1)
    println("提交任务1, ID: $task1Id")

    val task2 = TaskRequestDict(
        taskId = "custom-task-001",
        content = "生成单元测试",
        priority = TaskPriorityEnum.MEDIUM,
        deadlineMs = null,
        metadata = mapOf("language" to "rust")
    )
    val task2Id = manager.submitTask(task2)
    println("提交任务2, ID: $task2Id")

    // 查看任务状态
    val status1 = manager.getTaskStatus(task1Id)
    val status2 = manager.getTaskStatus(task2Id)
    println("任务1状态: $status1")
    println("任务2状态: $status2")

    // 统计
    println("等待中任务数: ${manager.pendingCount()}")
    println("运行中任务数: ${manager.runningCount()}")

    // 取消任务
    val cancelled = manager.cancelTask(task1Id)
    println("取消任务1: ${if (cancelled) "成功" else "失败"}")
    println("取消后等待中任务数: ${manager.pendingCount()}")
    println()
}

/**
 * 工作流示例
 */
fun exampleWorkflow() {
    println("=".repeat(60))
    println("工作流示例")
    println("=".repeat(60))

    // 创建工作流构建器
    var builder = WorkflowBuilderWrapper("workflow-001", "代码审查工作流")

    // 添加节点
    builder = builder.addStartNode("start")
    builder = builder.addTaskNode("analyze", "code_analysis")
    builder = builder.addTaskNode("review", "code_review")
    builder = builder.addDecisionNode("decision")
    builder = builder.addTaskNode("fix", "auto_fix")
    builder = builder.addEndNode("end")

    // 添加边
    builder = builder.addEdge("start", "analyze", EdgeTypeEnum.SEQUENTIAL)
    builder = builder.addEdge("analyze", "review", EdgeTypeEnum.SEQUENTIAL)
    builder = builder.addEdge("review", "decision", EdgeTypeEnum.SEQUENTIAL)
    builder = builder.addEdge("decision", "fix", EdgeTypeEnum.CONDITIONAL)
    builder = builder.addEdge("decision", "end", EdgeTypeEnum.CONDITIONAL)
    builder = builder.addEdge("fix", "end", EdgeTypeEnum.SEQUENTIAL)

    // 构建工作流
    val workflow = builder.build()

    println("工作流ID: ${workflow.workflowId()}")
    println("工作流名称: ${workflow.name()}")
    println("节点数量: ${workflow.nodeCount()}")
    println("边数量: ${workflow.edgeCount()}")
    println("状态: ${workflow.status()}")

    // 执行工作流
    val inputs = mapOf(
        "code_path" to "/path/to/code",
        "language" to "rust"
    )
    val result = workflow.execute(inputs)

    println()
    println("执行结果:")
    println("  状态: ${result.status}")
    println("  耗时: ${result.durationMs}ms")
    println("  节点结果数: ${result.nodeResults.size}")
    result.nodeResults.forEach { nodeResult ->
        println("    - ${nodeResult.nodeId}: ${nodeResult.status}")
    }
    println()
}

/**
 * 指标收集示例
 */
fun exampleMetrics() {
    println("=".repeat(60))
    println("指标收集示例")
    println("=".repeat(60))

    // 创建指标收集器
    val collector = MetricsCollectorWrapper()

    // 增加计数器
    collector.incrementCounter(
        "requests_total",
        1.0,
        mapOf("method" to "POST", "endpoint" to "/api/agents")
    )
    collector.incrementCounter(
        "requests_total",
        1.0,
        mapOf("method" to "GET", "endpoint" to "/api/tasks")
    )

    // 设置 gauge
    collector.setGauge(
        "active_agents",
        5.0,
        mapOf("type" to "simple")
    )

    // 记录直方图
    collector.recordHistogram(
        "request_duration_ms",
        125.5,
        mapOf("endpoint" to "/api/agents")
    )
    collector.recordHistogram(
        "request_duration_ms",
        89.2,
        mapOf("endpoint" to "/api/tasks")
    )

    // 获取所有指标
    val metrics = collector.getAllMetrics()
    println("收集到 ${metrics.size} 个指标:")
    metrics.forEach { metric ->
        println("  - ${metric.name} (${metric.metricType}): ${metric.value}")
    }

    // 获取系统指标
    val systemMetrics = collector.getSystemMetrics()
    println()
    println("系统指标:")
    println("  CPU 使用率: ${systemMetrics.cpuUsage}%")
    println("  内存使用: ${systemMetrics.memoryUsed}/${systemMetrics.memoryTotal}")
    println("  运行时间: ${systemMetrics.uptimeSecs}秒")
    println()
}

/**
 * 运行时配置示例
 */
fun exampleRuntimeConfig() {
    println("=".repeat(60))
    println("运行时配置示例")
    println("=".repeat(60))

    // 创建嵌入式配置
    val embeddedConfig = EmbeddedConfigDict(
        uv = false,
        writeEventsTo = null,
        logDestination = LogDestinationTypeEnum.TRACING
    )

    // 创建分布式配置
    val distributedConfig = DistributedConfigDict(
        coordinatorAddr = "127.0.0.1:5000",
        machineId = "machine-001",
        localListenPort = 5001U
    )

    // 创建运行时配置
    val runtimeConfig = RuntimeConfigDict(
        mode = RuntimeModeEnum.EMBEDDED,
        dataflowPath = "./dataflow.yml",
        embedded = embeddedConfig,
        distributed = distributedConfig
    )

    println("运行时模式: ${runtimeConfig.mode}")
    println("数据流路径: ${runtimeConfig.dataflowPath}")
    println("日志目标: ${runtimeConfig.embedded.logDestination}")
    println()

    // 使用构建器创建运行时
    println("使用构建器创建运行时...")
    var builder = DoraRuntimeBuilderWrapper("./dataflow.yml")
    builder = builder.setEmbedded()
    builder = builder.setUv(false)
    builder = builder.setLogDestination(LogDestinationTypeEnum.CHANNEL)

    println("构建器配置完成")
    println("注意：实际运行需要有效的 dataflow.yml 文件")
    println()
}
