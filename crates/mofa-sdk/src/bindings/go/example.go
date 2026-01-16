// MoFA Go 绑定示例
//
// 使用方法：
// 1. 安装 uniffi-bindgen-go:
//    cargo install uniffi-bindgen-go --git https://github.com/ArcticOJ/uniffi-bindgen-go
//
// 2. 生成绑定:
//    ./bindings/go/generate.sh
//
// 3. 运行示例:
//    cd bindings/go
//    go run example.go

package main

import (
	"fmt"
	"strings"

	// 导入生成的 mofa 包
	// "github.com/mofa/bindings/go/mofa"
)

func printSeparator(title string) {
	fmt.Println(strings.Repeat("=", 60))
	fmt.Println(title)
	fmt.Println(strings.Repeat("=", 60))
}

// 版本示例
func exampleVersion() {
	printSeparator("MoFA 版本示例")

	// 注意: 以下代码需要在生成绑定后才能使用
	// version := mofa.GetVersion()
	// fmt.Printf("MoFA 版本: %s\n", version)

	fmt.Println("注意: 请先运行 generate.sh 生成绑定")
	fmt.Println()
}

// 简单智能体示例
func exampleSimpleAgent() {
	printSeparator("简单智能体示例")

	/*
	   // 创建智能体
	   agent := mofa.NewSimpleAgent("agent-001", "MyAgent")
	   fmt.Printf("创建智能体: %s\n", agent.Metadata().Name)
	   fmt.Printf("初始状态: %v\n", agent.State())

	   // 添加能力
	   agent.AddCapability("text_generation")
	   agent.AddCapability("code_completion")

	   // 添加依赖
	   agent.AddDependency("openai")

	   // 获取元数据
	   metadata := agent.Metadata()
	   fmt.Printf("智能体ID: %s\n", metadata.AgentId)
	   fmt.Printf("能力: %v\n", metadata.Capabilities)
	   fmt.Printf("依赖: %v\n", metadata.Dependencies)

	   // 初始化智能体
	   config := mofa.AgentConfigDict{
	       AgentId: "agent-001",
	       Name:    "MyAgent",
	       NodeConfig: map[string]string{
	           "model": "gpt-4",
	       },
	   }
	   if err := agent.Init(config); err != nil {
	       fmt.Printf("初始化失败: %v\n", err)
	   } else {
	       fmt.Printf("初始化后状态: %v\n", agent.State())
	   }

	   // 暂停智能体
	   if err := agent.Pause(); err != nil {
	       fmt.Printf("暂停失败: %v\n", err)
	   } else {
	       fmt.Printf("暂停后状态: %v\n", agent.State())
	   }

	   // 销毁智能体
	   if err := agent.Destroy(); err != nil {
	       fmt.Printf("销毁失败: %v\n", err)
	   } else {
	       fmt.Printf("销毁后状态: %v\n", agent.State())
	   }
	*/

	fmt.Println("注意: 请先运行 generate.sh 生成绑定")
	fmt.Println()
}

// 任务管理器示例
func exampleTaskManager() {
	printSeparator("任务管理器示例")

	/*
	   // 创建任务管理器
	   manager := mofa.NewTaskManager()

	   // 提交任务
	   task1 := mofa.TaskRequestDict{
	       TaskId:     "",  // 空字符串让系统自动生成 ID
	       Content:    "分析代码质量",
	       Priority:   mofa.TaskPriorityEnumHigh,
	       DeadlineMs: mofa.Uint64Ptr(5000),
	       Metadata:   map[string]string{"project": "mofa"},
	   }
	   task1Id, err := manager.SubmitTask(task1)
	   if err != nil {
	       fmt.Printf("提交任务1失败: %v\n", err)
	       return
	   }
	   fmt.Printf("提交任务1, ID: %s\n", task1Id)

	   task2 := mofa.TaskRequestDict{
	       TaskId:     "custom-task-001",
	       Content:    "生成单元测试",
	       Priority:   mofa.TaskPriorityEnumMedium,
	       DeadlineMs: nil,
	       Metadata:   map[string]string{"language": "rust"},
	   }
	   task2Id, err := manager.SubmitTask(task2)
	   if err != nil {
	       fmt.Printf("提交任务2失败: %v\n", err)
	       return
	   }
	   fmt.Printf("提交任务2, ID: %s\n", task2Id)

	   // 查看任务状态
	   status1, _ := manager.GetTaskStatus(task1Id)
	   status2, _ := manager.GetTaskStatus(task2Id)
	   fmt.Printf("任务1状态: %v\n", status1)
	   fmt.Printf("任务2状态: %v\n", status2)

	   // 统计
	   fmt.Printf("等待中任务数: %d\n", manager.PendingCount())
	   fmt.Printf("运行中任务数: %d\n", manager.RunningCount())

	   // 取消任务
	   cancelled, _ := manager.CancelTask(task1Id)
	   if cancelled {
	       fmt.Println("取消任务1: 成功")
	   } else {
	       fmt.Println("取消任务1: 失败")
	   }
	   fmt.Printf("取消后等待中任务数: %d\n", manager.PendingCount())
	*/

	fmt.Println("注意: 请先运行 generate.sh 生成绑定")
	fmt.Println()
}

// 工作流示例
func exampleWorkflow() {
	printSeparator("工作流示例")

	/*
	   // 创建工作流构建器
	   builder := mofa.NewWorkflowBuilderWrapper("workflow-001", "代码审查工作流")

	   // 添加节点
	   builder = builder.AddStartNode("start")
	   builder = builder.AddTaskNode("analyze", "code_analysis")
	   builder = builder.AddTaskNode("review", "code_review")
	   builder = builder.AddDecisionNode("decision")
	   builder = builder.AddTaskNode("fix", "auto_fix")
	   builder = builder.AddEndNode("end")

	   // 添加边
	   builder, _ = builder.AddEdge("start", "analyze", mofa.EdgeTypeEnumSequential)
	   builder, _ = builder.AddEdge("analyze", "review", mofa.EdgeTypeEnumSequential)
	   builder, _ = builder.AddEdge("review", "decision", mofa.EdgeTypeEnumSequential)
	   builder, _ = builder.AddEdge("decision", "fix", mofa.EdgeTypeEnumConditional)
	   builder, _ = builder.AddEdge("decision", "end", mofa.EdgeTypeEnumConditional)
	   builder, _ = builder.AddEdge("fix", "end", mofa.EdgeTypeEnumSequential)

	   // 构建工作流
	   workflow, err := builder.Build()
	   if err != nil {
	       fmt.Printf("构建工作流失败: %v\n", err)
	       return
	   }

	   fmt.Printf("工作流ID: %s\n", workflow.WorkflowId())
	   fmt.Printf("工作流名称: %s\n", workflow.Name())
	   fmt.Printf("节点数量: %d\n", workflow.NodeCount())
	   fmt.Printf("边数量: %d\n", workflow.EdgeCount())
	   fmt.Printf("状态: %v\n", workflow.Status())

	   // 执行工作流
	   inputs := map[string]string{
	       "code_path": "/path/to/code",
	       "language":  "rust",
	   }
	   result, err := workflow.Execute(inputs)
	   if err != nil {
	       fmt.Printf("执行工作流失败: %v\n", err)
	       return
	   }

	   fmt.Println()
	   fmt.Println("执行结果:")
	   fmt.Printf("  状态: %v\n", result.Status)
	   fmt.Printf("  耗时: %dms\n", result.DurationMs)
	   fmt.Printf("  节点结果数: %d\n", len(result.NodeResults))
	   for _, nodeResult := range result.NodeResults {
	       fmt.Printf("    - %s: %v\n", nodeResult.NodeId, nodeResult.Status)
	   }
	*/

	fmt.Println("注意: 请先运行 generate.sh 生成绑定")
	fmt.Println()
}

// 指标收集示例
func exampleMetrics() {
	printSeparator("指标收集示例")

	/*
	   // 创建指标收集器
	   collector := mofa.NewMetricsCollectorWrapper()

	   // 增加计数器
	   collector.IncrementCounter(
	       "requests_total",
	       1.0,
	       map[string]string{"method": "POST", "endpoint": "/api/agents"},
	   )
	   collector.IncrementCounter(
	       "requests_total",
	       1.0,
	       map[string]string{"method": "GET", "endpoint": "/api/tasks"},
	   )

	   // 设置 gauge
	   collector.SetGauge(
	       "active_agents",
	       5.0,
	       map[string]string{"type": "simple"},
	   )

	   // 记录直方图
	   collector.RecordHistogram(
	       "request_duration_ms",
	       125.5,
	       map[string]string{"endpoint": "/api/agents"},
	   )
	   collector.RecordHistogram(
	       "request_duration_ms",
	       89.2,
	       map[string]string{"endpoint": "/api/tasks"},
	   )

	   // 获取所有指标
	   metrics := collector.GetAllMetrics()
	   fmt.Printf("收集到 %d 个指标:\n", len(metrics))
	   for _, metric := range metrics {
	       fmt.Printf("  - %s (%v): %f\n", metric.Name, metric.MetricType, metric.Value)
	   }

	   // 获取系统指标
	   systemMetrics := collector.GetSystemMetrics()
	   fmt.Println()
	   fmt.Println("系统指标:")
	   fmt.Printf("  CPU 使用率: %.2f%%\n", systemMetrics.CpuUsage)
	   fmt.Printf("  内存使用: %d/%d\n", systemMetrics.MemoryUsed, systemMetrics.MemoryTotal)
	   fmt.Printf("  运行时间: %d秒\n", systemMetrics.UptimeSecs)
	*/

	fmt.Println("注意: 请先运行 generate.sh 生成绑定")
	fmt.Println()
}

// 运行时配置示例
func exampleRuntimeConfig() {
	printSeparator("运行时配置示例")

	/*
	   // 创建嵌入式配置
	   embeddedConfig := mofa.EmbeddedConfigDict{
	       Uv:             false,
	       WriteEventsTo:  nil,
	       LogDestination: mofa.LogDestinationTypeEnumTracing,
	   }

	   // 创建分布式配置
	   distributedConfig := mofa.DistributedConfigDict{
	       CoordinatorAddr: "127.0.0.1:5000",
	       MachineId:       mofa.StringPtr("machine-001"),
	       LocalListenPort: 5001,
	   }

	   // 创建运行时配置
	   runtimeConfig := mofa.RuntimeConfigDict{
	       Mode:         mofa.RuntimeModeEnumEmbedded,
	       DataflowPath: "./dataflow.yml",
	       Embedded:     embeddedConfig,
	       Distributed:  distributedConfig,
	   }

	   fmt.Printf("运行时模式: %v\n", runtimeConfig.Mode)
	   fmt.Printf("数据流路径: %s\n", runtimeConfig.DataflowPath)
	   fmt.Printf("日志目标: %v\n", runtimeConfig.Embedded.LogDestination)
	   fmt.Println()

	   // 使用构建器创建运行时
	   fmt.Println("使用构建器创建运行时...")
	   builder := mofa.NewDoraRuntimeBuilderWrapper("./dataflow.yml")
	   builder = builder.SetEmbedded()
	   builder = builder.SetUv(false)
	   builder = builder.SetLogDestination(mofa.LogDestinationTypeEnumChannel)

	   fmt.Println("构建器配置完成")
	   fmt.Println("注意：实际运行需要有效的 dataflow.yml 文件")
	*/

	fmt.Println("注意: 请先运行 generate.sh 生成绑定")
	fmt.Println()
}

func main() {
	fmt.Println()
	printSeparator("MoFA Go 绑定示例")
	fmt.Println()

	// 运行各个示例
	exampleVersion()
	exampleSimpleAgent()
	exampleTaskManager()
	exampleWorkflow()
	exampleMetrics()
	exampleRuntimeConfig()

	printSeparator("所有示例运行完成!")
}
