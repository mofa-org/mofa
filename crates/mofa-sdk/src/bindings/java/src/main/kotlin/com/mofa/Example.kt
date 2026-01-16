/**
 * MoFA Kotlin 绑定示例
 *
 * 使用方法：
 * 1. 首先生成绑定:
 *    ./bindings/java/generate.sh
 *
 * 2. 编译并运行:
 *    cd bindings/java
 *    mvn compile exec:java -Dexec.mainClass="com.mofa.ExampleKt"
 */

package com.mofa

import uniffi.mofa.*

fun main() {
    println()
    printSeparator("MoFA Kotlin 绑定示例")
    println()

    // 运行各个示例
    exampleVersion()
    exampleSimpleAgent()
    exampleTaskManager()
    exampleWorkflow()
    exampleMetrics()
    exampleRuntimeConfig()

    printSeparator("所有示例运行完成!")
}

fun printSeparator(title: String) {
    println("=".repeat(60))
    println(title)
    println("=".repeat(60))
}

/**
 * 版本示例
 */
fun exampleVersion() {
    printSeparator("MoFA 版本示例")

    val version = getVersion()
    println("MoFA 版本: $version")

    val doraAvailable = isDoraAvailable()
    println("Dora 运行时可用: $doraAvailable")
    println()
}

/**
 * 简单智能体示例
 */
fun exampleSimpleAgent() {
    printSeparator("简单智能体示例")

    // 创建智能体
    val agent = SimpleAgent("agent-001", "MyAgent")
    println("创建智能体: ${agent.metadata().agentId}")
    println("初始状态: ${agent.state()}")

    // 添加能力
    agent.addCapability("text_generation")
    agent.addCapability("code_completion")

    // 添加依赖
    agent.addDependency("openai")

    // 获取元数据
    val metadata = agent.metadata()
    println("智能体ID: ${metadata.agentId}")
    println("智能体名称: ${metadata.name}")
    println("能力: ${metadata.capabilities}")
    println("依赖: ${metadata.dependencies}")

    // 初始化智能体
    val config = AgentConfigDict(
        agentId = "agent-001",
        name = "MyAgent",
        nodeConfig = mapOf("model" to "gpt-4")
    )

    try {
        agent.`init`(config)
        println("初始化后状态: ${agent.state()}")
    } catch (e: AimosException) {
        println("初始化失败: ${e.message}")
    }

    // 暂停智能体
    try {
        agent.pause()
        println("暂停后状态: ${agent.state()}")
    } catch (e: AimosException) {
        println("暂停失败: ${e.message}")
    }

    // 销毁智能体
    try {
        agent.shutdown()
        println("销毁后状态: ${agent.state()}")
    } catch (e: AimosException) {
        println("销毁失败: ${e.message}")
    }

    println()
}

/**
 * 任务管理器示例
 */
fun exampleTaskManager() {
    printSeparator("任务管理器示例")

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

    try {
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
    } catch (e: AimosException) {
        println("任务操作失败: ${e.message}")
    }

    println()
}

/**
 * 工作流示例
 */
fun exampleWorkflow() {
    printSeparator("工作流示例")

    // 创建工作流构建器
    var builder = WorkflowBuilderWrapper("workflow-001", "代码审查工作流")

    // 添加节点
    builder = builder.addStartNode("start")
    builder = builder.addTaskNode("analyze", "code_analysis")
    builder = builder.addTaskNode("review", "code_review")
    builder = builder.addDecisionNode("decision")
    builder = builder.addTaskNode("fix", "auto_fix")
    builder = builder.addEndNode("end")

    try {
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
        val inputs = mapOf("code_path" to "/path/to/code", "language" to "rust")
        val result = workflow.execute(inputs)

        println()
        println("执行结果:")
        println("  状态: ${result.status}")
        println("  耗时: ${result.durationMs}ms")
        println("  节点结果数: ${result.nodeResults.size}")
        for (nodeResult in result.nodeResults) {
            println("    - ${nodeResult.nodeId}: ${nodeResult.status}")
        }
    } catch (e: AimosException) {
        println("工作流操作失败: ${e.message}")
    }

    println()
}

/**
 * 指标收集示例
 */
fun exampleMetrics() {
    printSeparator("指标收集示例")

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
    collector.setGauge("active_agents", 5.0, mapOf("type" to "simple"))

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
    for (metric in metrics) {
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
    printSeparator("运行时配置示例")

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
        localListenPort = 5001u
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

    // 检查 Dora 是否可用
    if (isDoraAvailable()) {
        println("Dora 运行时可用，可以使用 DoraRuntimeBuilderWrapper")
    } else {
        println("Dora 运行时未启用")
        println("要启用 dora，请使用以下命令重新构建：")
        println("  cargo build --features \"uniffi,dora\" --release")
    }
    println()
}
