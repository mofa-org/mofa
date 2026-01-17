# 快速入门：RSS 转多主播新闻稿数据流

**功能**: 001-rss-newscaster-script
**日期**: 2026-01-09

## 前提条件

- Python 3.10+
- 已安装 MoFA 框架
- dora-rs 运行时
- LLM API 访问权限（OpenAI 或兼容提供商）

## 安装

```bash
# 导航到 MoFA 仓库根目录
cd mofa

# 安装 Agent
pip install -e agents/rss-input
pip install -e agents/news-processor
pip install -e agents/script-generator
```

## 配置

### 环境变量

在流目录中创建 `.env.secret` 文件：

```bash
# 必需：LLM API 配置
LLM_API_KEY=your-api-key-here
LLM_MODEL=gpt-4o

# 可选：自定义 LLM 端点（用于非 OpenAI 提供商）
# LLM_API_BASE=https://api.example.com/v1

# 可选：主播自定义
# MALE_ANCHOR_NAME=张明
# MALE_ANCHOR_STYLE=清晰、权威的新闻播报
# FEMALE_ANCHOR_NAME=李华
# FEMALE_ANCHOR_STYLE=亲和、引人入胜的新闻播报
# COMMENTATOR_NAME=王教授
# COMMENTATOR_STYLE=分析性、提供背景和专家视角
```

## 运行数据流

### 方式 1：交互模式

```bash
# 启动数据流
cd flows/rss-newscaster
dora up
dora start rss_newscaster_dataflow.yml

# 在另一个终端中运行输入节点
python -m rss_input --name rss-input

# 在提示时输入 RSS URL：
# > https://feeds.bbci.co.uk/news/rss.xml
```

### 方式 2：直接执行

```bash
# 使用特定 RSS URL 运行
cd flows/rss-newscaster
DATA='{"urls": ["https://feeds.bbci.co.uk/news/rss.xml"]}' dora start rss_newscaster_dataflow.yml
```

## 示例输出

```
【张明】各位观众朋友，大家好！欢迎收看今天的新闻播报。我是主持人张明。

【李华】大家好，我是李华。今天我们为您带来最新的国际新闻报道。

【张明】首先来看今天的头条新闻。据报道，科学家们在人工智能领域取得了重大突破...

【李华】这项研究由来自多个国家的科学家团队共同完成...

【王教授】这项突破意味着什么呢？让我来为大家分析一下。从技术角度来看...

【张明】感谢王教授的精彩分析。接下来我们来看第二条新闻...
```

## 数据流结构

```yaml
# rss_newscaster_dataflow.yml
nodes:
  - id: rss-input
    build: pip install -e ../../agents/rss-input
    path: dynamic
    outputs:
      - rss_request
    inputs:
      script_output: script-generator/broadcast_script

  - id: news-processor
    build: pip install -e ../../agents/news-processor
    path: news-processor
    inputs:
      rss_request: rss-input/rss_request
    outputs:
      - processed_feed
      - error

  - id: script-generator
    build: pip install -e ../../agents/script-generator
    path: script-generator
    inputs:
      processed_feed: news-processor/processed_feed
    outputs:
      - broadcast_script
      - error
    env:
      IS_DATAFLOW_END: true
      WRITE_LOG: true
```

## 测试

```bash
# 运行单元测试
pytest agents/rss-input/tests/
pytest agents/news-processor/tests/
pytest agents/script-generator/tests/

# 运行集成测试
pytest flows/rss-newscaster/tests/
```

## 自定义

### 自定义主播

您可以通过环境变量自定义主播特征：

```bash
# 正式新闻风格
MALE_ANCHOR_STYLE="正式、严肃的新闻播报风格"
FEMALE_ANCHOR_STYLE="专业、稳重的新闻播报风格"
COMMENTATOR_STYLE="深度分析、专业权威"

# 轻松早间节目风格
MALE_ANCHOR_STYLE="轻松、活泼的早间节目风格"
FEMALE_ANCHOR_STYLE="亲切、有趣的早间节目风格"
COMMENTATOR_STYLE="幽默、接地气的点评风格"
```

### 多个 RSS 来源

```bash
# 处理多个订阅
DATA='{"urls": ["https://feed1.com/rss.xml", "https://feed2.com/rss.xml"]}' dora start rss_newscaster_dataflow.yml
```

## 故障排除

### 常见问题

1. **订阅获取错误**：检查 RSS URL 是否可访问
   ```bash
   curl -I https://your-rss-feed-url
   ```

2. **LLM 错误**：验证 API 密钥和模型可用性
   ```bash
   echo $LLM_API_KEY
   ```

3. **空输出**：检查 RSS 订阅是否包含项目
   ```bash
   curl https://your-rss-feed-url | head -100
   ```

### 调试模式

```bash
# 启用详细日志
LOG_LEVEL=DEBUG dora start rss_newscaster_dataflow.yml
```

## 下一步

- 与 TTS Agent 集成以生成音频输出（参见 `podcast-generator` 流）
- 添加定时/自动执行支持
- 为重复订阅实现稿件缓存
