# RSS Newscaster Flow

将 RSS Feed 转换为带主播分配的新闻播报稿的完整数据流。

## 流程图

```
┌─────────────────┐
│  terminal-input │  用户输入 RSS URL
└────────┬────────┘
         │ data (RSS URL string)
         ▼
┌─────────────────┐
│   rss-fetcher   │  获取并解析 RSS Feed
└────────┬────────┘
         │ rss_feed (JSON with entries)
         ▼
┌─────────────────────────┐
│  link-content-scripter  │  访问链接获取内容，生成播报稿
└────────────┬────────────┘
             │ news_scripts (JSON with scripts)
             ▼
┌─────────────────┐
│ anchor-assigner │  分配男女主播
└────────┬────────┘
         │ anchor_pairs (script, anchor pairs)
```

## 节点说明

| 节点 | 功能 | 输入 | 输出 |
|------|------|------|------|
| terminal-input | 接收用户输入的 RSS URL | anchor-assigner/anchor_pairs | data |
| rss-fetcher | 获取并解析 RSS Feed | rss_url | rss_feed |
| link-content-scripter | 访问链接获取内容，生成播报稿 | rss_feed | news_scripts |
| anchor-assigner | 为播报稿分配男女主播 | news_scripts | anchor_pairs |

## 配置

### 环境变量

在运行前设置以下环境变量：

```bash
# LLM API 配置（必需）
export LLM_API_KEY="your-api-key-here"

# 可选配置
export LLM_MODEL="gpt-4o-mini"      # LLM 模型
export MAX_ENTRIES="5"              # 处理的最大条目数
export MALE_ANCHOR_NAME="张明"       # 男主播名称
export FEMALE_ANCHOR_NAME="李华"     # 女主播名称
```

或者在项目根目录创建 `.env.secret` 文件：

```bash
LLM_API_KEY=your-api-key-here
LLM_MODEL=gpt-4o-mini
```

## 运行方式

### 方式一：使用 mofa run-flow

```bash
cd flows/rss-newscaster
mofa run-flow dataflow.yml
```

然后输入 RSS URL，例如：
```
https://news.ycombinator.com/rss
```

### 方式二：后台运行

```bash
mofa run-flow dataflow.yml --detach
```

停止：
```bash
mofa stop-flow rss-newscaster
```

## 输出示例

```json
{
  "error": false,
  "feed_title": "Hacker News",
  "pairs": [
    {
      "script": "今日要闻：Windows 8 桌面环境现已登陆 Linux 系统...",
      "title": "Windows 8 Desktop Environment for Linux",
      "anchor": {
        "name": "李华",
        "gender": "female",
        "role": "女主播"
      },
      "position": 1
    },
    {
      "script": "接下来这条有趣的新闻：软盘竟成为最佳儿童电视遥控器...",
      "title": "Floppy disks turn out to be the greatest TV remote for kids",
      "anchor": {
        "name": "张明",
        "gender": "male",
        "role": "男主播"
      },
      "position": 2
    }
  ],
  "pair_count": 2
}
```

## 自定义

### 修改主播名称

在 `dataflow.yml` 中修改 `anchor-assigner` 节点的环境变量：

```yaml
env:
  MALE_ANCHOR_NAME: 王刚
  FEMALE_ANCHOR_NAME: 张丽
```

### 修改处理条目数

修改 `link-content-scripter` 节点的 `MAX_ENTRIES`：

```yaml
env:
  MAX_ENTRIES: "10"
```

### 使用不同的脚本生成器

如果只需要基于 RSS 描述生成脚本（更快，无需访问链接），
将 `link-content-scripter` 替换为 `feed-to-scripts`：

```yaml
- id: feed-to-scripts
  build: pip install -e ../../agents/feed-to-scripts
  path: feed-to-scripts
  inputs:
    rss_feed: rss-fetcher/rss_feed
  outputs:
    - news_scripts
```
