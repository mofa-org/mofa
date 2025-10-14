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

## 多个输入参数示例
```bash
 mofa debug ./agent-hub/multi_parameters ./tests/test_multi_param.yml
 ```
