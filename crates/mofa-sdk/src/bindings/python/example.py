#!/usr/bin/env python3
"""
MoFA Python 绑定示例

使用方法：
1. 首先构建 MoFA 库并生成 Python 绑定：

   # 构建库（release 模式）
   cargo build --features uniffi --release

   # 生成 Python 绑定
   cargo run --features uniffi --bin uniffi-bindgen generate \
       --library target/release/libaimos.dylib \
       --language python \
       --out-dir bindings/python

2. 运行示例：
   cd bindings/python
   PYTHONPATH=. python example.py

注意：需要将生成的 .dylib (macOS) / .so (Linux) / .dll (Windows) 文件
复制到 Python 绑定目录或设置 LD_LIBRARY_PATH。
"""

import sys
from pathlib import Path

# 尝试导入生成的 mofa 模块
try:
    import mofa
except ImportError:
    print("错误：无法导入 mofa 模块")
    print("请先生成 Python 绑定：")
    print("  cargo run --features uniffi --bin uniffi-bindgen generate \\")
    print("      --library target/release/libaimos.dylib \\")
    print("      --language python \\")
    print("      --out-dir bindings/python")
    sys.exit(1)


def example_version():
    """获取版本信息"""
    print("=" * 60)
    print("MoFA 版本示例")
    print("=" * 60)

    version = mofa.get_version()
    print(f"MoFA 版本: {version}")
    print()


def example_simple_agent():
    """简单智能体示例"""
    print("=" * 60)
    print("简单智能体示例")
    print("=" * 60)

    # 创建智能体
    agent = mofa.SimpleAgent("agent-001", "MyAgent")
    print(f"创建智能体: {agent.metadata().name}")
    print(f"初始状态: {agent.state()}")

    # 添加能力
    agent.add_capability("text_generation")
    agent.add_capability("code_completion")

    # 添加依赖
    agent.add_dependency("openai")

    # 获取元数据
    metadata = agent.metadata()
    print(f"智能体ID: {metadata.agent_id}")
    print(f"能力: {metadata.capabilities}")
    print(f"依赖: {metadata.dependencies}")

    # 初始化智能体
    config = mofa.AgentConfigDict(
        agent_id="agent-001",
        name="MyAgent",
        node_config={"model": "gpt-4"}
    )
    agent.init(config)
    print(f"初始化后状态: {agent.state()}")

    # 暂停智能体
    agent.pause()
    print(f"暂停后状态: {agent.state()}")

    # 销毁智能体
    agent.destroy()
    print(f"销毁后状态: {agent.state()}")
    print()


def example_task_manager():
    """任务管理器示例"""
    print("=" * 60)
    print("任务管理器示例")
    print("=" * 60)

    # 创建任务管理器
    manager = mofa.TaskManager()

    # 提交任务
    task1 = mofa.TaskRequestDict(
        task_id="",  # 空字符串让系统自动生成 ID
        content="分析代码质量",
        priority=mofa.TaskPriorityEnum.HIGH,
        deadline_ms=5000,
        metadata={"project": "mofa"}
    )
    task1_id = manager.submit_task(task1)
    print(f"提交任务1, ID: {task1_id}")

    task2 = mofa.TaskRequestDict(
        task_id="custom-task-001",
        content="生成单元测试",
        priority=mofa.TaskPriorityEnum.MEDIUM,
        deadline_ms=None,
        metadata={"language": "rust"}
    )
    task2_id = manager.submit_task(task2)
    print(f"提交任务2, ID: {task2_id}")

    # 查看任务状态
    status1 = manager.get_task_status(task1_id)
    status2 = manager.get_task_status(task2_id)
    print(f"任务1状态: {status1}")
    print(f"任务2状态: {status2}")

    # 统计
    print(f"等待中任务数: {manager.pending_count()}")
    print(f"运行中任务数: {manager.running_count()}")

    # 取消任务
    cancelled = manager.cancel_task(task1_id)
    print(f"取消任务1: {'成功' if cancelled else '失败'}")
    print(f"取消后等待中任务数: {manager.pending_count()}")
    print()


def example_workflow():
    """工作流示例"""
    print("=" * 60)
    print("工作流示例")
    print("=" * 60)

    # 创建工作流构建器
    builder = mofa.WorkflowBuilderWrapper("workflow-001", "代码审查工作流")

    # 添加节点
    builder = builder.add_start_node("start")
    builder = builder.add_task_node("analyze", "code_analysis")
    builder = builder.add_task_node("review", "code_review")
    builder = builder.add_decision_node("decision")
    builder = builder.add_task_node("fix", "auto_fix")
    builder = builder.add_end_node("end")

    # 添加边
    builder = builder.add_edge("start", "analyze", mofa.EdgeTypeEnum.SEQUENTIAL)
    builder = builder.add_edge("analyze", "review", mofa.EdgeTypeEnum.SEQUENTIAL)
    builder = builder.add_edge("review", "decision", mofa.EdgeTypeEnum.SEQUENTIAL)
    builder = builder.add_edge("decision", "fix", mofa.EdgeTypeEnum.CONDITIONAL)
    builder = builder.add_edge("decision", "end", mofa.EdgeTypeEnum.CONDITIONAL)
    builder = builder.add_edge("fix", "end", mofa.EdgeTypeEnum.SEQUENTIAL)

    # 构建工作流
    workflow = builder.build()

    print(f"工作流ID: {workflow.workflow_id()}")
    print(f"工作流名称: {workflow.name()}")
    print(f"节点数量: {workflow.node_count()}")
    print(f"边数量: {workflow.edge_count()}")
    print(f"状态: {workflow.status()}")

    # 执行工作流
    inputs = {
        "code_path": "/path/to/code",
        "language": "rust"
    }
    result = workflow.execute(inputs)

    print(f"\n执行结果:")
    print(f"  状态: {result.status}")
    print(f"  耗时: {result.duration_ms}ms")
    print(f"  节点结果数: {len(result.node_results)}")
    for node_result in result.node_results:
        print(f"    - {node_result.node_id}: {node_result.status}")
    print()


def example_metrics():
    """指标收集示例"""
    print("=" * 60)
    print("指标收集示例")
    print("=" * 60)

    # 创建指标收集器
    collector = mofa.MetricsCollectorWrapper()

    # 增加计数器
    collector.increment_counter(
        "requests_total",
        1.0,
        {"method": "POST", "endpoint": "/api/agents"}
    )
    collector.increment_counter(
        "requests_total",
        1.0,
        {"method": "GET", "endpoint": "/api/tasks"}
    )

    # 设置 gauge
    collector.set_gauge(
        "active_agents",
        5.0,
        {"type": "simple"}
    )

    # 记录直方图
    collector.record_histogram(
        "request_duration_ms",
        125.5,
        {"endpoint": "/api/agents"}
    )
    collector.record_histogram(
        "request_duration_ms",
        89.2,
        {"endpoint": "/api/tasks"}
    )

    # 获取所有指标
    metrics = collector.get_all_metrics()
    print(f"收集到 {len(metrics)} 个指标:")
    for metric in metrics:
        print(f"  - {metric.name} ({metric.metric_type}): {metric.value}")

    # 获取系统指标
    system_metrics = collector.get_system_metrics()
    print(f"\n系统指标:")
    print(f"  CPU 使用率: {system_metrics.cpu_usage}%")
    print(f"  内存使用: {system_metrics.memory_used}/{system_metrics.memory_total}")
    print(f"  运行时间: {system_metrics.uptime_secs}秒")
    print()


def example_runtime_config():
    """运行时配置示例"""
    print("=" * 60)
    print("运行时配置示例")
    print("=" * 60)

    # 创建嵌入式配置
    embedded_config = mofa.EmbeddedConfigDict(
        uv=False,
        write_events_to=None,
        log_destination=mofa.LogDestinationTypeEnum.TRACING
    )

    # 创建分布式配置
    distributed_config = mofa.DistributedConfigDict(
        coordinator_addr="127.0.0.1:5000",
        machine_id="machine-001",
        local_listen_port=5001
    )

    # 创建运行时配置
    runtime_config = mofa.RuntimeConfigDict(
        mode=mofa.RuntimeModeEnum.EMBEDDED,
        dataflow_path="./dataflow.yml",
        embedded=embedded_config,
        distributed=distributed_config
    )

    print(f"运行时模式: {runtime_config.mode}")
    print(f"数据流路径: {runtime_config.dataflow_path}")
    print(f"日志目标: {runtime_config.embedded.log_destination}")
    print()


def example_runtime_builder():
    """运行时构建器示例"""
    print("=" * 60)
    print("运行时构建器示例")
    print("=" * 60)

    # 检查 dora 是否可用
    if not mofa.is_dora_available():
        print("Dora 运行时未启用，跳过此示例")
        print("要启用 dora，请使用以下命令重新构建：")
        print("  cargo build --features \"uniffi,dora\" --release")
        print()
        return

    # 使用构建器创建运行时
    builder = mofa.DoraRuntimeBuilderWrapper("./dataflow.yml")

    # 配置构建器
    builder = builder.set_embedded()
    builder = builder.set_uv(False)
    builder = builder.set_log_destination(mofa.LogDestinationTypeEnum.CHANNEL)

    print("构建器配置完成")
    print("注意：实际运行需要有效的 dataflow.yml 文件")
    print()


def main():
    """主函数"""
    print("\n" + "=" * 60)
    print("MoFA Python 绑定示例")
    print("=" * 60 + "\n")

    # 运行各个示例
    example_version()
    example_simple_agent()
    example_task_manager()
    example_workflow()
    example_metrics()
    example_runtime_config()
    example_runtime_builder()

    print("=" * 60)
    print("所有示例运行完成!")
    print("=" * 60)


if __name__ == "__main__":
    main()
