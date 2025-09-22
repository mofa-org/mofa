Work In Progress...

# **项目名: Hello_World**

## **团队名称:**

MoFa团队

**组员：**

- 睿类文特 (GitCode用户名: zonghuanwu)
- ChenZi (GitCode用户名: chenzi00103)

## **项目地址:**

[GitCode Repo 地址](https://gitcode.com/moxin-org/mofa/overview)

------

## **安装与运行手册**

### **环境依赖:**

1. **Rust** (用于运行 Dora-RS 框架)
2. **MoFa 框架库**

### **安装步骤:**

### **运行程序:**

1. **运行智能体框架:**
   ```bash
   dora up && dora build hello_world_dataflow.yml && dora start hello_world_dataflow.yml
   ```

2. **启动任务输入端:**
   - 打开另一个终端窗口，运行 `terminal-input`。
   - 在 `terminal-input` 中输入任务指令即可与智能体交互。

------

## **案例介绍**

**Hello_World** 是一个用于测试项目是否运行的工具，展示了如何使用 MoFa 框架构建智能体。它实现了最基本的智能体设计模式：定制的大语言模型提示与大语言模型推理。

### **突破点:**

- **简化智能体开发流程：** 通过模板化配置，用户可以快速生成定制化智能体。
- **分布式计算支持：** 使用 Dora-RS 框架，确保多个智能体之间的高效协作。
- **灵活的提示定制：** 允许用户通过编辑配置文件轻松定制智能体的行为和响应。

------

## **技术开发介绍**

### **使用的框架与工具:**

- **Dora-RS 框架:** 负责智能体间的分布式计算，确保多个智能体在任务执行中的顺畅协作。
- **MoFa 框架 (MoXin智能体组合框架):** 为智能体之间的交互和编排提供了强大的基础设施。

### **技术难点:**

### **技术难点**

- **通过 Dora 连接不同节点进行协作：**
  实现多个智能体在分布式环境下的高效协作，确保各节点之间的数据同步与通信稳定。需要解决网络延迟、节点故障恢复以及任务分配的优化问题，以保证整体系统的高可用性和一致性。

- **创建一个可自由运行的 Agent：**
  设计智能体架构，使其具备独立运行的能力，能够根据不同任务需求动态调整资源分配和执行策略。确保 Agent 的高效性和灵活性，同时支持多种运行环境和扩展接口，以适应不断变化的应用场景。

- **用户自定义配置的灵活性与安全性：**
  提供强大的配置接口，允许用户自由定制提示词和参数设置，以满足多样化的应用需求。同时，必须确保配置过程的安全性，防止潜在的配置错误或安全漏洞，通过权限控制和验证机制保障系统的稳定运行。

------

## **功能说明**

Hello_World 是 MoFa 中最基本的智能体，设计模式为用户输入什么,就返回什么



## **运行 Agent**

1. **启动智能体框架：**
   ```bash
   dora up && dora build hello_world_dataflow.yml && dora start hello_world_dataflow.yml
   ```

2. **启动任务输入端：**
   - 打开另一个终端窗口，运行 `terminal-input`。
   - 在 `terminal-input` 中输入任务指令，与智能体进行交互。


## **案例展示**

### **案例 1: 描写自然的诗词**

**提示:** 你是谁？

**Hello_World 案例输出:**
```
你是谁？
```

# Agent解释
### 输入参数接收
```python
# 从agent配置中获取名为'query'的输入参数
user_query = agent.receive_parameter('query')
参数名称需与dataflow.yml中inputs定义一致
自动完成类型校验（字符串/数字/布尔值）
支持默认值设置（在agent-dataflow.yml配置）
输出参数发送
agent.send_output(
    agent_output_name='hello_world_result',
    agent_result=processed_data
)
输出名称需与agent-dataflow.yml中outputs定义一致
支持多种数据类型：JSON对象、字符串、二进制数据
自动处理结果序列化
逻辑设计规范
初始化阶段

核心逻辑流程

@run_agent
def run(agent: MofaAgent):
    try:
        # 1. 参数接收
        input_data = agent.receive_parameter('query')
        
        # 2. 业务处理
        processed_data = process_business_logic(input_data)
        
        # 3. 结果输出
        agent.send_output('hello_world_result', processed_data)
        
    except Exception as e:
        agent.logger.error(f"处理异常: {str(e)}")
        agent.send_error(error_code=500, error_message="处理失败")
    # 3. 结果输出
    agent.send_output('hello_world_result', processed_data)
    
except Exception as e:
    agent.logger.error(f"处理异常: {str(e)}")
    agent.send_error(error_code=500, error_message="处理失败")
