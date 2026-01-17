# Quickstart: RSS to Multi-Newscaster Script Dataflow

**Feature**: 001-rss-newscaster-script
**Date**: 2026-01-09

## Prerequisites

- Python 3.10+
- MoFA framework installed
- dora-rs runtime
- LLM API access (OpenAI or compatible provider)

## Installation

```bash
# Navigate to MoFA repository root
cd mofa

# Install the agents
pip install -e agents/rss-input
pip install -e agents/news-processor
pip install -e agents/script-generator
```

## Configuration

### Environment Variables

Create a `.env.secret` file in the flow directory:

```bash
# Required: LLM API configuration
LLM_API_KEY=your-api-key-here
LLM_MODEL=gpt-4o

# Optional: Custom LLM endpoint (for non-OpenAI providers)
# LLM_API_BASE=https://api.example.com/v1

# Optional: Persona customization
# MALE_ANCHOR_NAME=张明
# MALE_ANCHOR_STYLE=清晰、权威的新闻播报
# FEMALE_ANCHOR_NAME=李华
# FEMALE_ANCHOR_STYLE=亲和、引人入胜的新闻播报
# COMMENTATOR_NAME=王教授
# COMMENTATOR_STYLE=分析性、提供背景和专家视角
```

## Running the Dataflow

### Option 1: Interactive Mode

```bash
# Start the dataflow
cd flows/rss-newscaster
dora up
dora start rss_newscaster_dataflow.yml

# In another terminal, run the input node
python -m rss_input --name rss-input

# Enter RSS URL when prompted:
# > https://feeds.bbci.co.uk/news/rss.xml
```

### Option 2: Direct Execution

```bash
# Run with a specific RSS URL
cd flows/rss-newscaster
DATA='{"urls": ["https://feeds.bbci.co.uk/news/rss.xml"]}' dora start rss_newscaster_dataflow.yml
```

## Example Output

```
【张明】各位观众朋友，大家好！欢迎收看今天的新闻播报。我是主持人张明。

【李华】大家好，我是李华。今天我们为您带来最新的国际新闻报道。

【张明】首先来看今天的头条新闻。据报道，科学家们在人工智能领域取得了重大突破...

【李华】这项研究由来自多个国家的科学家团队共同完成...

【王教授】这项突破意味着什么呢？让我来为大家分析一下。从技术角度来看...

【张明】感谢王教授的精彩分析。接下来我们来看第二条新闻...
```

## Dataflow Structure

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

## Testing

```bash
# Run unit tests
pytest agents/rss-input/tests/
pytest agents/news-processor/tests/
pytest agents/script-generator/tests/

# Run integration test
pytest flows/rss-newscaster/tests/
```

## Customization

### Custom Personas

You can customize persona characteristics via environment variables:

```bash
# Formal news style
MALE_ANCHOR_STYLE="正式、严肃的新闻播报风格"
FEMALE_ANCHOR_STYLE="专业、稳重的新闻播报风格"
COMMENTATOR_STYLE="深度分析、专业权威"

# Casual morning show style
MALE_ANCHOR_STYLE="轻松、活泼的早间节目风格"
FEMALE_ANCHOR_STYLE="亲切、有趣的早间节目风格"
COMMENTATOR_STYLE="幽默、接地气的点评风格"
```

### Multiple RSS Sources

```bash
# Process multiple feeds
DATA='{"urls": ["https://feed1.com/rss.xml", "https://feed2.com/rss.xml"]}' dora start rss_newscaster_dataflow.yml
```

## Troubleshooting

### Common Issues

1. **Feed fetch error**: Check if the RSS URL is accessible
   ```bash
   curl -I https://your-rss-feed-url
   ```

2. **LLM error**: Verify API key and model availability
   ```bash
   echo $LLM_API_KEY
   ```

3. **Empty output**: Check if the RSS feed contains items
   ```bash
   curl https://your-rss-feed-url | head -100
   ```

### Debug Mode

```bash
# Enable verbose logging
LOG_LEVEL=DEBUG dora start rss_newscaster_dataflow.yml
```

## Next Steps

- Integrate with TTS agents for audio output (see `podcast-generator` flow)
- Add support for scheduled/automated execution
- Implement script caching for repeated feeds
