# Hello World Agent 开发指南

本文档详细说明如何创建和配置MOFA平台的Agent，重点介绍参数传递和业务逻辑设计规范。

## 参数处理机制
### 输入参数接收
```python
# 从agent配置中获取名为'query'的输入参数
user_query = agent.receive_parameter('query')
```
- 参数名称需与agent-config.yml中`inputs`定义一致
- 自动完成类型校验（字符串/数字/布尔值）
- 支持默认值设置（在agent-config.yml配置）

### 输出参数发送
```python
agent.send_output(
    agent_output_name='hello_world_result',
    agent_result=processed_data
)
```
- 输出名称需与agent-config.yml中`outputs`定义一致
- 支持多种数据类型：JSON对象、字符串、二进制数据

## 逻辑设计规范
1. 初始化阶段
```python
def agent_factory():
    """Agent初始化工厂方法"""
    agent = MofaAgent(agent_name='hello-world')
    agent.configure(
        log_level='DEBUG',
        timeout=3000
    )
    return agent
```

2. 核心逻辑流程
```python
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
```

