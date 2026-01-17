# Research: RSS to Multi-Newscaster Script Dataflow

**Feature**: 001-rss-newscaster-script
**Date**: 2026-01-09
**Status**: Complete

## Research Tasks

### 1. MoFA Dataflow Patterns

**Decision**: Follow existing MoFA dataflow patterns as demonstrated in `flows/podcast-generator/` and `flows/openai_chat_agent/`.

**Rationale**: 
- Existing patterns are proven and aligned with the MoFA constitution
- YAML-based declarative dataflow definitions
- Agents communicate via pyarrow arrays through dora-rs
- Each agent is a Python package with `main.py` entry point using `MofaAgent` base class

**Alternatives Considered**:
- Custom workflow engine: Rejected - violates Constitution Principle II (Data Flow First)
- Imperative Python script: Rejected - violates Constitution Principle II (declarative YAML preferred)

### 2. RSS Parsing Library

**Decision**: Use `feedparser` library for RSS/Atom feed parsing.

**Rationale**:
- Most widely used Python RSS parser
- Handles RSS 2.0 and Atom formats (per spec assumptions)
- Robust error handling for malformed feeds
- Active maintenance and community support
- Simple API: `feedparser.parse(url)` returns structured data

**Alternatives Considered**:
- `atoma`: Lighter weight but less mature, smaller community
- Custom XML parsing: Rejected - unnecessary complexity, reinventing the wheel

### 3. LLM Integration for Script Generation

**Decision**: Use OpenAI-compatible API via existing MoFA patterns (as seen in `openai_chat_agent`).

**Rationale**:
- Proven pattern in the codebase
- Supports multiple LLM providers via `LLM_API_BASE` environment variable
- Handles streaming and non-streaming responses
- Environment-based configuration (`LLM_API_KEY`, `LLM_MODEL`)

**Alternatives Considered**:
- Direct Anthropic/Claude API: Would work but adds dependency, OpenAI-compatible is more flexible
- Local LLM: Possible future enhancement but adds complexity for MVP

### 4. Script Output Format

**Decision**: Structured JSON output with speaker labels, then formatted as readable text.

**Rationale**:
- JSON structure allows programmatic processing downstream
- Text format suitable for human review or TTS conversion (per spec assumptions)
- Clear speaker labels with `【男主播】`, `【女主播】`, `【评论员】` format
- Compatible with existing `script-segmenter` agent patterns for future TTS integration

**Output Format**:
```json
{
  "broadcast_script": {
    "title": "News Broadcast - {date}",
    "segments": [
      {
        "speaker": "male_anchor",
        "speaker_label": "【男主播】",
        "content": "...",
        "position": 1
      },
      ...
    ]
  }
}
```

**Alternatives Considered**:
- Markdown format only: Less structured for programmatic use
- XML format: Overly complex for this use case

### 5. Node Architecture

**Decision**: Three-node dataflow architecture:

1. **rss-input**: Dynamic node accepting RSS URL from user/environment
2. **news-processor**: Fetches and parses RSS content, extracts structured news items
3. **script-generator**: LLM-based agent that generates multi-persona script

**Rationale**:
- Follows Unix philosophy (each node does one thing well)
- Aligns with user's specification: "first one take the RSS URL as input, the second node handles news data fetch and process, the third one will turn the data into scripts"
- Each node independently testable
- Clear data contracts between nodes

**Alternatives Considered**:
- Two-node design (combine processing + generation): Reduces composability, harder to test independently
- Four-node design (separate nodes per persona): Over-engineered, LLM can handle all three personas in one call

### 6. Persona Configuration

**Decision**: Environment variables and optional config file for persona customization.

**Rationale**:
- Aligns with MoFA patterns (see TTS agents using env vars for voice configuration)
- Supports FR-010: "System MUST support configuration of persona characteristics through dataflow parameters"
- Default personas work out of the box; customization optional

**Configuration Approach**:
```yaml
# In dataflow YAML
env:
  MALE_ANCHOR_STYLE: "authoritative, professional"
  FEMALE_ANCHOR_STYLE: "warm, engaging"
  COMMENTATOR_STYLE: "analytical, insightful"
```

**Alternatives Considered**:
- Separate config YAML file: Adds complexity for MVP
- Hardcoded personas: Doesn't meet FR-010 requirement

### 7. Error Handling

**Decision**: Graceful degradation with clear error messages.

**Rationale**:
- Edge cases defined in spec (empty feed, invalid URL, missing descriptions)
- Return meaningful JSON error structure
- Log errors for debugging

**Error Response Format**:
```json
{
  "error": true,
  "error_type": "feed_fetch_error",
  "message": "Unable to fetch RSS feed: connection timeout",
  "partial_result": null
}
```

## Resolved Clarifications

All technical context items are now resolved. No NEEDS CLARIFICATION markers remain.

## Dependencies Summary

| Dependency | Version | Purpose |
|------------|---------|---------|
| mofa | latest | Framework base classes and utilities |
| dora-rs | latest | Dataflow runtime |
| pyarrow | latest | Inter-node data passing |
| feedparser | >=6.0 | RSS/Atom parsing |
| openai | >=1.0 | LLM API client |
| python-dotenv | latest | Environment configuration |

## Next Steps

Proceed to Phase 1: Design & Contracts
- Generate data-model.md with entity definitions
- Generate contracts for inter-node communication
- Create quickstart.md with working example
