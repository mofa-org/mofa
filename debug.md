# mofa debug 手册

- mofa run-node
- mofa unit-test

## 1. mofa run-node
**用户输入参数直接执行代码**,
直接向用户展示agent代码执行结果

### 1.1 使用方式
```bash
mofa run-node --help                               
Usage: mofa run-node [OPTIONS] NODE_FOLDER_PATH

  With mofa run-node, user just need to provide values for input parameters,
  no need to provide output parameters as in the "mofa unit-test" required.

Options:
  --help  Show this message and exit.
```

### 1.2 示例说明
```bash
(.venv)  mofa run-node ./agents/hello-world
===== 运行节点，输入参数 =====
请输入input参数: hi        # 用户输入
Hello, World! Received query: hi
Hello, World! agent_result: hi
节点运行结果, Output: hi

(.venv)  mofa run-node ./agents/multi_parameters
===== 运行节点，输入参数 =====
请输入input参数: ['a', 'b', 'c']       # 用户输入
Received data: ['a', 'b', 'c']
Sending data back: ['a', 'b', 'c']
当前工作目录: /Users/eva/workspace/mofa
Using GPT-3.5 Turbo model
LLM Response: Simulated response from gpt-3.5-turbo for prompt: Process the following data: ['a', 'b', 'c']
节点运行结果, Output: ['a', 'b', 'c']
```

## 2. mofa unit-test
**单agent单元测试**,
无需新启动终端， 也无需构建复杂数据流，仅关注agent自身的输入和输出进行单元测试。

### 2.1 使用方式
支持两种：
- 【方式一】**yml文件** （配置输入输出，执行单/多个测试用例）；
- 【方式二】**交互式** （输入输出，执行单/多个测试用例）；

推荐方式一，可以将agent功能测试固化到yml文件里，当多个agent协同工作时，方便快速进行各个节点的功能验证。

```bash
(.venv) ➜  mofa unit-test --help                            
Usage: mofa unit-test [OPTIONS] NODE_FOLDER_PATH [TEST_CASE_YML]

  Run unit tests for a single agent

Options:
  --interactive  Enable interactive input mode
  --help         Show this message and exit.                                            

```
### 2.2 示例说明
#### 2.2.1 yml 文件
##### 2.2.1.1 单个输入参数示例
```bash
mofa unit-test ./agents/hello-world ./agents/hello-world/tests/test_hello_world.yml
```

运行结果
```bash
(.venv) ➜  mofa git:(enhance/mofa-debug)  mofa unit-test ./agents/hello-world ./agents/hello-world/tests/test_hello_world.yml
Hello, World! Received query: hello
Hello, World! agent_result: hello
Hello, World! Received query: 
Hello, World! agent_result: 
Test case 1/2: test_hello
Status: [PASS] Passed
----------------------------------
Test case 2/2: test_empty
Status: [PASS] Passed
----------------------------------

========================================
Test Summary:
Total test cases: 2
Passed: 2
Failed: 0
Pass rate: 100.00%
========================================
```

##### 2.2.1.2 多个输入参数示例
```bash
 mofa unit-test ./agents/multi_parameters ./agents/multi_parameters/tests/test_multi_param.yml 
 ```
 
 运行结果
```bash
(.venv) ➜  mofa git:(enhance/mofa-debug) mofa unit-test ./agents/multi_parameters ./agents/multi_parameters/tests/test_multi_param.yml 
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
Status: [PASS] Passed
----------------------------------
Test case 2/2: test_params_2
Status: [PASS] Passed
----------------------------------

========================================
Test Summary:
Total test cases: 2
Passed: 2
Failed: 0
Pass rate: 100.00%
========================================
```

##### 2.2.1.3 支持全局libray和客制化库导入
```bash
mofa unit-test ./agents/multi_parameters ./agents/multi_parameters/tests/test_multi_param.yml 
 ```
 
 运行结果

```
(.venv) ➜  mofa git:(enhance/mofa-debug) mofa unit-test ./agents/multi_parameters ./agents/multi_parameters/tests/test_multi_param.yml 
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
Status: [PASS] Passed
----------------------------------
Test case 2/2: test_params_2
Status: [PASS] Passed
----------------------------------

========================================
Test Summary:
Total test cases: 2
Passed: 2
Failed: 0
Pass rate: 100.00%
========================================
```

#### 2.2.2 交互式输入（无需YAML文件）
##### 2.2.2.1 单个输入参数示例

```bash
(.venv) ➜  mofa git:(enhance/mofa-debug) mofa unit-test ./agents/hello-world --interactive
===== 交互式测试用例输入 =====
请输入用例名称（默认: test_case_1）:       # 用户输入，可直接回车使用默认名称

请输入input参数
  普通值如： hello
  列表如： ["a", "b", "c"]
input参数: hi      # 用户输入

请输入预期输出
  普通值如：hello 
  列表如： ["a", "b", "c"]
expected_output参数: hi      # 用户输入

是否继续添加下一个测试用例？ [y/N]: n      # 用户输入

已收集 1 个测试用例，开始执行...
Hello, World! Received query: hi
Hello, World! agent_result: hi
Test case 1/1: test_case_1
Status: [PASS] Passed
----------------------------------

========================================
Test Summary:
Total test cases: 1
Passed: 1
Failed: 0
Pass rate: 100.00%
========================================
```

##### 2.2.2.2 多个输入参数示例
```bash
(.venv) ➜  mofa git:(enhance/mofa-debug) mofa unit-test ./agents/multi_parameters --interactive                                       
===== 交互式测试用例输入 =====
请输入用例名称（默认: test_case_1）:       # 用户输入，可直接回车使用默认名称

请输入input参数
  普通值如： hello
  列表如： ["a", "b", "c"]
input参数: ["a", "b", "c"]        # 用户输入

请输入预期输出
  普通值如：hello 
  列表如： ["a", "b", "c"]
expected_output参数: ["a", "b", "c"]      # 用户输入

是否继续添加下一个测试用例？ [y/N]: n      # 用户输入

已收集 1 个测试用例，开始执行...
Received data: ["a", "b", "c"]
Sending data back: ["a", "b", "c"]
当前工作目录: /Users/eva/workspace/mofa
Using GPT-3.5 Turbo model
LLM Response: Simulated response from gpt-3.5-turbo for prompt: Process the following data: ["a", "b", "c"]
Test case 1/1: test_case_1
Status: [PASS] Passed
----------------------------------

========================================
Test Summary:
Total test cases: 1
Passed: 1
Failed: 0
Pass rate: 100.00%
========================================
```
