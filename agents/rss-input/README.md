# RSS Input Agent

RSS 订阅源输入 Agent，负责接收和验证 RSS 订阅 URL。

## 功能

- 支持单个或多个 RSS 订阅 URL 输入
- 支持交互式输入和环境变量配置
- 可选的 Persona 配置传递
- URL 格式验证

## 安装

```bash
cd agents/rss-input
pip install -e .
```

## 配置

### 环境变量

| 变量名 | 说明 | 默认值 |
|--------|------|--------|
| `RSS_URLS` | RSS 订阅 URL（逗号分隔） | - |
| `PERSONA_CONFIG` | JSON 格式的 Persona 配置 | - |

### 示例配置

```bash
# 单个 RSS 源
export RSS_URLS="https://example.com/feed.xml"

# 多个 RSS 源
export RSS_URLS="https://example.com/feed1.xml,https://example.com/feed2.xml"

# 自定义 Persona
export PERSONA_CONFIG='{"tone": "casual", "male_anchor": {"name": "小明"}}'
```

## 输出格式

输出 `RSSInput` JSON 格式：

```json
{
  "urls": ["https://example.com/feed.xml"],
  "config": {
    "tone": "formal"
  }
}
```

## 在 Dataflow 中使用

```yaml
nodes:
  - id: rss-input
    path: dynamic
    inputs:
      tick: dora/timer/millis/1000
    outputs:
      - rss_request
    env:
      RSS_URLS: "https://example.com/feed.xml"
```

## 数据模型

参见 `specs/001-rss-newscaster-script/data-model.md` 中的 `RSSInput` 定义。
