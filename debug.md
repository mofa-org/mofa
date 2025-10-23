# mofa debug 使用手册

## 一、用途
**单个agent调试**
无需新启动终端， 也无需构建复杂数据流，仅关注agent自身的输入和输出进行单元测试。

## 二、使用方式
支持两种：
- 【方式一】**yml文件** （配置输入输出，执行单/多个测试用例）；
- 【方式二】**交互式** （输入输出，执行单/多个测试用例）；

推荐方式一，可以将agent功能测试固化到yml文件里，当多个agent协同工作时，快速进行验证。

```bash
(.venv) ➜  mofa debug --help                                    
Usage: mofa debug [OPTIONS] NODE_FOLDER_PATH [TEST_CASE_YML]

  Run unit tests for a single node/agent

Options:
  --interactive  启用交互式输入（无需YAML文件）
  --help         Show this message and exit.                                                 

```
## 三、示例说明
### 3.1 yml 文件
#### 3.1.1 单个输入参数示例
```bash
mofa debug ./agent-hub/hello-world ./agent-hub/hello-world/tests/test_hello_world.yml
```

运行结果
```bash
(.venv) ➜  mofa git:(enhance/mofa-debug) ✗ mofa debug ./agent-hub/hello-world ./agent-hub/hello-world/tests/test_hello_world.yml
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

#### 3.1.2 多个输入参数示例
```bash
 mofa debug ./agent-hub/multi_parameters ./agent-hub/multi_parameters/tests/test_multi_param.yml
 ```
 
 运行结果
```bash
(.venv) ➜  mofa git:(enhance/mofa-debug) ✗ mofa debug ./agent-hub/multi_parameters ./agent-hub/multi_parameters/tests/test_multi_param.yml
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

#### 3.1.3 支持全局libray和客制化库导入
```bash
 mofa debug ./agent-hub/multi_parameters ./agent-hub/multi_parameters/tests/test_multi_param.yml
 ```
 
 运行结果

```
(.venv) ➜  mofa git:(enhance/mofa-debug) ✗ mofa debug ./agent-hub/multi_parameters ./agent-hub/multi_parameters/tests/test_multi_param.yml
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

### 3.2 交互式输入（无需YAML文件）
#### 3.2.1 单个输入参数示例

```bash
(.venv) ➜  mofa git:(enhance/mofa-debug) mofa debug ./agent-hub/hello-world --interactive     
===== 交互式测试用例输入 =====
请按提示输入用例信息（格式与YAML保持一致）
支持普通键值对（如 query:hello）和列表（如 parameter_names: ["a", "b", "c"]）

请输入用例名称（默认: test_case_1）:    # 用户输入

请输入input参数（格式：key:value，例如：
  普通值：query:hello
  列表：parameter_names: ["a", "b", "c"]
input参数: query:hello     # 用户输入

请输入预期输出（格式：key:value，例如：
  普通值：hello_world_result:hello 
  列表：receive_data: ["a", "b", "c"]
expected_output参数: hello_world_result:hello     # 用户输入

是否继续添加下一个测试用例？ [y/N]: n     # 用户输入

已收集 1 个测试用例，开始执行...
Hello, World! Received query: hello
Hello, World! agent_result: hello
Test case 1/1: test_case_1
Status: ✅ Passed
----------------------------------

========================================
Test Summary:
Total test cases: 1
Passed: 1
Failed: 0
Pass rate: 100.00%
========================================
```

#### 3.1.2 多个输入参数示例
```bash
(.venv) ➜  mofa git:(enhance/mofa-debug) mofa debug ./agent-hub/multi_parameters --interactive
===== 交互式测试用例输入 =====
请按提示输入用例信息（格式与YAML保持一致）
支持普通键值对（如 query:hello）和列表（如 parameter_names: ["a", "b", "c"]）

请输入用例名称（默认: test_case_1）: my-case    # 用户输入

请输入input参数（格式：key:value，例如：
  普通值：query:hello
  列表：parameter_names: ["a", "b", "c"]
input参数: parameter_names: ["a", "b", "c"]    # 用户输入

请输入预期输出（格式：key:value，例如：
  普通值：hello_world_result:hello 
  列表：receive_data: ["a", "b", "c"]
expected_output参数: receive_data: ["a", "b", "c"].   # 用户输入

是否继续添加下一个测试用例？ [y/N]: n    # 用户输入

已收集 1 个测试用例，开始执行...
Received data: ['a', 'b', 'c']
Sending data back: ['a', 'b', 'c']
当前工作目录: /Users/eva/workspace/mofa
Using GPT-3.5 Turbo model
LLM Response: Simulated response from gpt-3.5-turbo for prompt: Process the following data: ['a', 'b', 'c']
Test case 1/1: my-case
Status: ✅ Passed
----------------------------------

========================================
Test Summary:
Total test cases: 1
Passed: 1
Failed: 0
Pass rate: 100.00%
========================================
```

## 四、Q&A

未给出正确的agent 文件夹目录，或yml文件不存在, 将给出提示🔔：
```bash
(.venv) ➜  mofa git:(enhance/mofa-debug) ✗ mofa debug ./agent-hub/multi_parameters ./test_no_yml.yml
Usage: mofa debug [OPTIONS] NODE_FOLDER_PATH TEST_CASE_YML
Try 'mofa debug --help' for help.

Error: Invalid value for 'TEST_CASE_YML': Path './test_no_yml.yml' does not exist.
(.venv) ➜  mofa git:(enhance/mofa-debug) ✗ mofa debug ./agent-hub/multi_parameters_no ./test_no_yml.yml
Usage: mofa debug [OPTIONS] NODE_FOLDER_PATH TEST_CASE_YML
Try 'mofa debug --help' for help.

Error: Invalid value for 'NODE_FOLDER_PATH': Path './agent-hub/multi_parameters_no' does not exist.
```
