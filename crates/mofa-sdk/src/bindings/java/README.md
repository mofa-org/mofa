# MoFA Java/Kotlin 绑定

基于 UniFFI 生成的 MoFA Kotlin 绑定，可在 Java/Kotlin 项目中使用。

## 要求

- Java 9+ (推荐 Java 11+)
- Maven 3.x

## 快速开始

### 1. 生成绑定

```bash
cd bindings/java
./generate.sh
```

启用 Dora Runtime:
```bash
./generate.sh --dora
```

### 2. 编译项目

```bash
JAVA_HOME=/path/to/java11+ mvn compile
```

### 3. 运行测试

```bash
JAVA_HOME=/path/to/java11+ mvn test
```

### 4. 运行示例

```bash
JAVA_HOME=/path/to/java11+ mvn exec:java
```

## 项目结构

```
bindings/java/
├── src/
│   ├── main/
│   │   ├── kotlin/
│   │   │   ├── uniffi/mofa/    # 生成的 UniFFI 绑定
│   │   │   └── com/mofa/       # 示例代码
│   │   └── resources/           # 原生库 (JNA 自动加载)
│   └── test/
│       └── kotlin/com/mofa/    # 测试代码
├── libs/                        # 原生库 (备用)
├── pom.xml                      # Maven 配置
└── generate.sh                  # 绑定生成脚本
```

## 使用示例

### Kotlin

```kotlin
import uniffi.mofa.*

fun main() {
    // 获取版本
    println("MoFA 版本: ${getVersion()}")

    // 检查 Dora 是否可用
    println("Dora 可用: ${isDoraAvailable()}")

    // 创建智能体
    val agent = SimpleAgent("agent-001", "MyAgent")
    agent.addCapability("text_generation")

    val config = AgentConfigDict(
        agentId = "agent-001",
        name = "MyAgent",
        nodeConfig = mapOf("model" to "gpt-4")
    )
    agent.`init`(config)

    println("智能体状态: ${agent.state()}")

    // 创建任务
    val manager = TaskManager()
    val task = TaskRequestDict(
        taskId = "",
        content = "分析代码",
        priority = TaskPriorityEnum.HIGH,
        deadlineMs = 5000UL,
        metadata = mapOf("project" to "mofa")
    )
    val taskId = manager.submitTask(task)
    println("任务 ID: $taskId")

    // 创建工作流
    var builder = WorkflowBuilderWrapper("wf-001", "示例工作流")
    builder = builder.addStartNode("start")
    builder = builder.addTaskNode("process", "data_process")
    builder = builder.addEndNode("end")
    builder = builder.addEdge("start", "process", EdgeTypeEnum.SEQUENTIAL)
    builder = builder.addEdge("process", "end", EdgeTypeEnum.SEQUENTIAL)

    val workflow = builder.build()
    val result = workflow.execute(mapOf("input" to "test"))
    println("工作流状态: ${result.status}")
}
```

### Java

UniFFI 生成的是 Kotlin 代码，但可以在 Java 项目中直接使用。需要添加 Kotlin 运行时依赖。

```java
import uniffi.mofa.*;

public class Example {
    public static void main(String[] args) {
        // 获取版本
        String version = AimosKt.getVersion();
        System.out.println("MoFA 版本: " + version);

        // 检查 Dora 是否可用
        boolean doraAvailable = AimosKt.isDoraAvailable();
        System.out.println("Dora 可用: " + doraAvailable);
    }
}
```

## API 概览

### 核心类型

- `SimpleAgent` - 智能体实现
- `TaskManager` - 任务管理器
- `WorkflowBuilderWrapper` - 工作流构建器
- `WorkflowWrapper` - 工作流
- `MetricsCollectorWrapper` - 指标收集器

### 枚举类型

- `AgentStateEnum` - 智能体状态
- `TaskPriorityEnum` - 任务优先级
- `SchedulingStatusEnum` - 调度状态
- `WorkflowStatusEnum` - 工作流状态
- `EdgeTypeEnum` - 边类型

### 数据类型

- `AgentMetadataDict` - 智能体元数据
- `AgentConfigDict` - 智能体配置
- `TaskRequestDict` - 任务请求
- `RuntimeConfigDict` - 运行时配置
- `MetricValueDict` - 指标值
- `SystemMetricsDict` - 系统指标

### 顶层函数

- `getVersion()` - 获取版本号
- `isDoraAvailable()` - 检查 Dora Runtime 是否可用
- `runDataflowSync()` - 同步运行数据流 (需要 Dora)

## 注意事项

1. **Java 版本**: 需要 Java 9+ (使用 `java.lang.ref.Cleaner`)
2. **Kotlin 运行时**: Java 项目需要添加 kotlin-stdlib 依赖
3. **原生库加载**: 库会自动从 resources 目录加载
4. **Dora Runtime**: 需要在编译时启用 `--dora` 选项
