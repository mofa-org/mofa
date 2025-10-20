# mofa debug 

## 单个agent调试
无需新启动终端， 也无需构建复杂数据流，仅关注agent自身的输入和输出进行单元测试。

## 使用方式
```bash
(.venv) ➜  mofa debug --help        
Usage: mofa debug [OPTIONS] NODE_FOLDER_PATH TEST_CASE_YML

  Run unit tests for a single node/agent

Options:
  --help  Show this message and exit.                                                  

```

## 单个输入参数示例
```bash
mofa debug ./agent-hub/hello-world ./tests/test_hello_world.yml
```

运行结果
```bash
(.venv) ➜  mofa git:(feature/debug-cli) ✗ mofa debug ./agent-hub/hello-world ./tests/test_hello_world.yml     
Hello, World! Received query: hello
Hello, World! agent_result: hello
Hello, World! Received query: 
Hello, World! agent_result: 
Test case 1/2: test_hello
Status: ✅ Passed
----------------------------------
Test case 2/2: test_empty
Status: ✅ Passed
----------------------------------

========================================
Test Summary:
Total test cases: 2
Passed: 2
Failed: 0
Pass rate: 100.00%
========================================
```

## 多个输入参数示例
```bash
 mofa debug ./agent-hub/multi_parameters ./tests/test_multi_param.yml
 ```
 
 运行结果
```bash
(.venv) ➜  mofa git:(feature/debug-cli) ✗ mofa debug ./agent-hub/multi_parameters ./tests/test_multi_param.yml
Received data: ['a', 'b', 'c']
Sending data back: ['a', 'b', 'c']
Received data: ['D', 'E', 'F']
Sending data back: ['D', 'E', 'F']
Test case 1/2: test_parms_1
Status: ✅ Passed
----------------------------------
Test case 2/2: test_params_2
Status: ✅ Passed
----------------------------------

========================================
Test Summary:
Total test cases: 2
Passed: 2
Failed: 0
Pass rate: 100.00%
```

## 支持全局libray和客制化库导入
```bash
 mofa debug ./agent-hub/multi_parameters ./tests/test_multi_param.yml
 ```
 
 运行结果

```
(.venv) ➜  mofa git:(feature/debug-cli) ✗ mofa debug ./agent-hub/multi_parameters ./tests/test_multi_param.yml
Received data: ['a', 'b', 'c']
Sending data back: ['a', 'b', 'c']
当前工作目录: /Users/eva/workspace/mofa
Using GPT-3.5 Turbo model
LLM Response: Simulated response from gpt-3.5-turbo for prompt: Process the following data: ['a', 'b', 'c']
Received data: ['D', 'E', 'F']
Sending data back: ['D', 'E', 'F']
当前工作目录: /Users/eva/workspace/mofa
Using GPT-3.5 Turbo model
LLM Response: Simulated response from gpt-3.5-turbo for prompt: Process the following data: ['D', 'E', 'F']
Test case 1/2: test_parms_1
Status: ✅ Passed
----------------------------------
Test case 2/2: test_params_2
Status: ✅ Passed
----------------------------------

========================================
Test Summary:
Total test cases: 2
Passed: 2
Failed: 0
Pass rate: 100.00%
========================================
```