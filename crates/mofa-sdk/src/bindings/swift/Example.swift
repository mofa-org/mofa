/**
 * MoFA Swift 绑定示例
 *
 * 使用方法：
 * 1. 首先构建 MoFA 库并生成 Swift 绑定：
 *    ./bindings/swift/generate.sh
 *
 * 2. 将生成的文件添加到 Xcode 项目或 Swift Package
 *
 * 3. 编译运行：
 *    swiftc -L. -laimos -import-objc-header aimosFFI.h Example.swift mofa.swift -o example
 *    ./example
 */

import Foundation

// MARK: - 版本示例

func exampleVersion() {
    print(String(repeating: "=", count: 60))
    print("MoFA 版本示例")
    print(String(repeating: "=", count: 60))

    let version = getVersion()
    print("MoFA 版本: \(version)")
    print()
}

// MARK: - 简单智能体示例

func exampleSimpleAgent() {
    print(String(repeating: "=", count: 60))
    print("简单智能体示例")
    print(String(repeating: "=", count: 60))

    // 创建智能体
    let agent = SimpleAgent(agentId: "agent-001", name: "MyAgent")
    print("创建智能体: \(agent.metadata().name)")
    print("初始状态: \(agent.state())")

    // 添加能力
    agent.addCapability(capability: "text_generation")
    agent.addCapability(capability: "code_completion")

    // 添加依赖
    agent.addDependency(dependency: "openai")

    // 获取元数据
    let metadata = agent.metadata()
    print("智能体ID: \(metadata.agentId)")
    print("能力: \(metadata.capabilities)")
    print("依赖: \(metadata.dependencies)")

    // 初始化智能体
    let config = AgentConfigDict(
        agentId: "agent-001",
        name: "MyAgent",
        nodeConfig: ["model": "gpt-4"]
    )
    do {
        try agent.init_(config: config)
        print("初始化后状态: \(agent.state())")
    } catch {
        print("初始化失败: \(error)")
    }

    // 暂停智能体
    do {
        try agent.pause()
        print("暂停后状态: \(agent.state())")
    } catch {
        print("暂停失败: \(error)")
    }

    // 销毁智能体
    do {
        try agent.destroy()
        print("销毁后状态: \(agent.state())")
    } catch {
        print("销毁失败: \(error)")
    }
    print()
}

// MARK: - 任务管理器示例

func exampleTaskManager() {
    print(String(repeating: "=", count: 60))
    print("任务管理器示例")
    print(String(repeating: "=", count: 60))

    // 创建任务管理器
    let manager = TaskManager()

    // 提交任务
    let task1 = TaskRequestDict(
        taskId: "",  // 空字符串让系统自动生成 ID
        content: "分析代码质量",
        priority: .high,
        deadlineMs: 5000,
        metadata: ["project": "mofa"]
    )

    do {
        let task1Id = try manager.submitTask(request: task1)
        print("提交任务1, ID: \(task1Id)")

        let task2 = TaskRequestDict(
            taskId: "custom-task-001",
            content: "生成单元测试",
            priority: .medium,
            deadlineMs: nil,
            metadata: ["language": "rust"]
        )
        let task2Id = try manager.submitTask(request: task2)
        print("提交任务2, ID: \(task2Id)")

        // 查看任务状态
        let status1 = try manager.getTaskStatus(taskId: task1Id)
        let status2 = try manager.getTaskStatus(taskId: task2Id)
        print("任务1状态: \(status1)")
        print("任务2状态: \(status2)")

        // 统计
        print("等待中任务数: \(manager.pendingCount())")
        print("运行中任务数: \(manager.runningCount())")

        // 取消任务
        let cancelled = try manager.cancelTask(taskId: task1Id)
        print("取消任务1: \(cancelled ? "成功" : "失败")")
        print("取消后等待中任务数: \(manager.pendingCount())")
    } catch {
        print("任务操作失败: \(error)")
    }
    print()
}

// MARK: - 工作流示例

func exampleWorkflow() {
    print(String(repeating: "=", count: 60))
    print("工作流示例")
    print(String(repeating: "=", count: 60))

    // 创建工作流构建器
    var builder = WorkflowBuilderWrapper(workflowId: "workflow-001", name: "代码审查工作流")

    // 添加节点
    builder = builder.addStartNode(nodeId: "start")
    builder = builder.addTaskNode(nodeId: "analyze", taskType: "code_analysis")
    builder = builder.addTaskNode(nodeId: "review", taskType: "code_review")
    builder = builder.addDecisionNode(nodeId: "decision")
    builder = builder.addTaskNode(nodeId: "fix", taskType: "auto_fix")
    builder = builder.addEndNode(nodeId: "end")

    do {
        // 添加边
        builder = try builder.addEdge(fromNode: "start", toNode: "analyze", edgeType: .sequential)
        builder = try builder.addEdge(fromNode: "analyze", toNode: "review", edgeType: .sequential)
        builder = try builder.addEdge(fromNode: "review", toNode: "decision", edgeType: .sequential)
        builder = try builder.addEdge(fromNode: "decision", toNode: "fix", edgeType: .conditional)
        builder = try builder.addEdge(fromNode: "decision", toNode: "end", edgeType: .conditional)
        builder = try builder.addEdge(fromNode: "fix", toNode: "end", edgeType: .sequential)

        // 构建工作流
        let workflow = try builder.build()

        print("工作流ID: \(workflow.workflowId())")
        print("工作流名称: \(workflow.name())")
        print("节点数量: \(workflow.nodeCount())")
        print("边数量: \(workflow.edgeCount())")
        print("状态: \(workflow.status())")

        // 执行工作流
        let inputs: [String: String] = [
            "code_path": "/path/to/code",
            "language": "rust"
        ]
        let result = try workflow.execute(inputs: inputs)

        print()
        print("执行结果:")
        print("  状态: \(result.status)")
        print("  耗时: \(result.durationMs)ms")
        print("  节点结果数: \(result.nodeResults.count)")
        for nodeResult in result.nodeResults {
            print("    - \(nodeResult.nodeId): \(nodeResult.status)")
        }
    } catch {
        print("工作流操作失败: \(error)")
    }
    print()
}

// MARK: - 指标收集示例

func exampleMetrics() {
    print(String(repeating: "=", count: 60))
    print("指标收集示例")
    print(String(repeating: "=", count: 60))

    // 创建指标收集器
    let collector = MetricsCollectorWrapper()

    // 增加计数器
    collector.incrementCounter(
        name: "requests_total",
        value: 1.0,
        labels: ["method": "POST", "endpoint": "/api/agents"]
    )
    collector.incrementCounter(
        name: "requests_total",
        value: 1.0,
        labels: ["method": "GET", "endpoint": "/api/tasks"]
    )

    // 设置 gauge
    collector.setGauge(
        name: "active_agents",
        value: 5.0,
        labels: ["type": "simple"]
    )

    // 记录直方图
    collector.recordHistogram(
        name: "request_duration_ms",
        value: 125.5,
        labels: ["endpoint": "/api/agents"]
    )
    collector.recordHistogram(
        name: "request_duration_ms",
        value: 89.2,
        labels: ["endpoint": "/api/tasks"]
    )

    // 获取所有指标
    let metrics = collector.getAllMetrics()
    print("收集到 \(metrics.count) 个指标:")
    for metric in metrics {
        print("  - \(metric.name) (\(metric.metricType)): \(metric.value)")
    }

    // 获取系统指标
    let systemMetrics = collector.getSystemMetrics()
    print()
    print("系统指标:")
    print("  CPU 使用率: \(systemMetrics.cpuUsage)%")
    print("  内存使用: \(systemMetrics.memoryUsed)/\(systemMetrics.memoryTotal)")
    print("  运行时间: \(systemMetrics.uptimeSecs)秒")
    print()
}

// MARK: - 运行时配置示例

func exampleRuntimeConfig() {
    print(String(repeating: "=", count: 60))
    print("运行时配置示例")
    print(String(repeating: "=", count: 60))

    // 创建嵌入式配置
    let embeddedConfig = EmbeddedConfigDict(
        uv: false,
        writeEventsTo: nil,
        logDestination: .tracing
    )

    // 创建分布式配置
    let distributedConfig = DistributedConfigDict(
        coordinatorAddr: "127.0.0.1:5000",
        machineId: "machine-001",
        localListenPort: 5001
    )

    // 创建运行时配置
    let runtimeConfig = RuntimeConfigDict(
        mode: .embedded,
        dataflowPath: "./dataflow.yml",
        embedded: embeddedConfig,
        distributed: distributedConfig
    )

    print("运行时模式: \(runtimeConfig.mode)")
    print("数据流路径: \(runtimeConfig.dataflowPath)")
    print("日志目标: \(runtimeConfig.embedded.logDestination)")
    print()

    // 使用构建器创建运行时
    print("使用构建器创建运行时...")
    var builder = DoraRuntimeBuilderWrapper(dataflowPath: "./dataflow.yml")
    builder = builder.setEmbedded()
    builder = builder.setUv(uv: false)
    builder = builder.setLogDestination(dest: .channel)

    print("构建器配置完成")
    print("注意：实际运行需要有效的 dataflow.yml 文件")
    print()
}

// MARK: - 主函数

func main() {
    print()
    print(String(repeating: "=", count: 60))
    print("MoFA Swift 绑定示例")
    print(String(repeating: "=", count: 60))
    print()

    // 运行各个示例
    exampleVersion()
    exampleSimpleAgent()
    exampleTaskManager()
    exampleWorkflow()
    exampleMetrics()
    exampleRuntimeConfig()

    print(String(repeating: "=", count: 60))
    print("所有示例运行完成!")
    print(String(repeating: "=", count: 60))
}

// 入口点
main()
