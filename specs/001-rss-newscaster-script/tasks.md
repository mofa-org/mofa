# Tasks: RSS to Multi-Newscaster Script Dataflow

**Input**: Design documents from `/specs/001-rss-newscaster-script/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Tests included per Quality & Testing requirements in constitution (working dataflow example required).

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

Based on plan.md structure:
- **Agents**: `agents/[agent-name]/` - each agent is a separate Python package
- **Flows**: `flows/rss-newscaster/` - dataflow YAML and README
- **Tests**: `agents/[agent-name]/tests/` - per-agent tests

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure for all three agents

- [x] T001 Create rss-input agent directory structure in agents/rss-input/
- [x] T002 [P] Create news-processor agent directory structure in agents/news-processor/
- [x] T003 [P] Create script-generator agent directory structure in agents/script-generator/
- [x] T004 [P] Create flow directory structure in flows/rss-newscaster/
- [x] T005 Create pyproject.toml for rss-input agent in agents/rss-input/pyproject.toml
- [x] T006 [P] Create pyproject.toml for news-processor agent in agents/news-processor/pyproject.toml
- [x] T007 [P] Create pyproject.toml for script-generator agent in agents/script-generator/pyproject.toml
- [x] T008 Create __init__.py files for all agent modules

**Checkpoint**: All agent package structures are ready for implementation

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T009 Create rss_newscaster_dataflow.yml skeleton with node definitions in flows/rss-newscaster/rss_newscaster_dataflow.yml
- [x] T010 Create .env.secret.example with required environment variables in flows/rss-newscaster/.env.secret.example
- [x] T011 Create shared data models module for JSON schema validation (if needed) - SKIPPED: Using JSON directly in agents

**Checkpoint**: Foundation ready - user story implementation can now begin

---

## Phase 3: User Story 1 - Generate News Script from RSS Feed (Priority: P1) 🎯 MVP

**Goal**: Convert a raw RSS feed into a polished broadcast-ready script featuring three distinct voices: male anchor, female anchor, and experienced commentator.

**Independent Test**: Provide a sample RSS feed URL and verify the output contains properly formatted scripts for all three newscaster personas with appropriate role assignments.

### Implementation for User Story 1

#### Node 1: rss-input Agent

- [x] T012 [US1] Create main.py for rss-input agent with MofaAgent base class in agents/rss-input/rss_input/main.py
- [x] T013 [US1] Implement RSS URL input handling (interactive and environment-based) in agents/rss-input/rss_input/main.py
- [x] T014 [US1] Implement RSSInput JSON output format per data-model.md in agents/rss-input/rss_input/main.py
- [x] T015 [US1] Add argument parsing with --help support in agents/rss-input/rss_input/main.py

#### Node 2: news-processor Agent

- [x] T016 [US1] Create main.py for news-processor agent with MofaAgent base class in agents/news-processor/news_processor/main.py
- [x] T017 [US1] Implement RSS feed fetching using feedparser library in agents/news-processor/news_processor/main.py
- [x] T018 [US1] Implement RSS parsing to extract NewsItem entities per data-model.md in agents/news-processor/news_processor/main.py
- [x] T019 [US1] Implement ProcessedFeed JSON output format per data-model.md in agents/news-processor/news_processor/main.py
- [x] T020 [US1] Add error handling for feed fetch failures (feed_fetch_error) in agents/news-processor/news_processor/main.py
- [x] T021 [US1] Add error handling for parse failures (feed_parse_error) in agents/news-processor/news_processor/main.py
- [x] T022 [US1] Add error handling for empty feeds (empty_feed_error) in agents/news-processor/news_processor/main.py

#### Node 3: script-generator Agent

- [x] T023 [US1] Create main.py for script-generator agent with MofaAgent base class in agents/script-generator/script_generator/main.py
- [x] T024 [US1] Implement OpenAI API client setup using environment variables in agents/script-generator/script_generator/main.py
- [x] T025 [US1] Implement default Persona definitions (male_anchor, female_anchor, commentator) in agents/script-generator/script_generator/main.py
- [x] T026 [US1] Implement LLM prompt for generating three-persona news script in agents/script-generator/script_generator/main.py
- [x] T027 [US1] Implement ScriptSegment generation with speaker labels per data-model.md in agents/script-generator/script_generator/main.py
- [x] T028 [US1] Implement BroadcastScript JSON output format per data-model.md in agents/script-generator/script_generator/main.py
- [x] T029 [US1] Add natural transitions between speakers in LLM prompt in agents/script-generator/script_generator/main.py
- [x] T030 [US1] Add error handling for LLM failures (llm_error) in agents/script-generator/script_generator/main.py

#### Dataflow Integration

- [x] T031 [US1] Complete rss_newscaster_dataflow.yml with full node connections in flows/rss-newscaster/rss_newscaster_dataflow.yml
- [x] T032 [US1] Add environment variable configuration to dataflow YAML in flows/rss-newscaster/rss_newscaster_dataflow.yml

#### Working Example (per Constitution)

- [x] T033 [US1] Create working dataflow example test in flows/rss-newscaster/tests/test_dataflow.py
- [x] T034 [US1] Add sample RSS feed URL for testing in flows/rss-newscaster/tests/

**Checkpoint**: User Story 1 complete - can generate three-persona script from single RSS feed

---

## Phase 4: User Story 2 - Customize Persona Characteristics (Priority: P2)

**Goal**: Allow content producers to adjust personality traits, speaking styles, or focus areas of each newscaster.

**Independent Test**: Modify persona configuration via environment variables and verify the generated script reflects the customized characteristics.

### Implementation for User Story 2

- [x] T035 [US2] Add PersonaConfig parsing from environment variables in agents/script-generator/script_generator/main.py
- [x] T036 [US2] Implement PersonaOverride handling for custom names in agents/script-generator/script_generator/main.py
- [x] T037 [US2] Implement PersonaOverride handling for custom styles in agents/script-generator/script_generator/main.py
- [x] T038 [US2] Implement PersonaOverride handling for custom focus areas in agents/script-generator/script_generator/main.py
- [x] T039 [US2] Add tone configuration (formal/casual/neutral) to LLM prompt in agents/script-generator/script_generator/main.py
- [x] T040 [US2] Update dataflow YAML with persona configuration env vars in flows/rss-newscaster/rss_newscaster_dataflow.yml
- [x] T041 [US2] Add config input passthrough from rss-input to script-generator in agents/rss-input/rss_input/main.py
- [x] T042 [US2] Add error handling for invalid configuration (config_error) in agents/script-generator/script_generator/main.py

**Checkpoint**: User Story 2 complete - personas can be customized via configuration

---

## Phase 5: User Story 3 - Handle Multiple RSS Sources (Priority: P3)

**Goal**: Combine news from multiple RSS feeds into a single cohesive broadcast script.

**Independent Test**: Provide two or more RSS feed URLs and verify the output integrates stories from all sources coherently.

### Implementation for User Story 3

- [x] T043 [US3] Update rss-input to accept multiple URLs in agents/rss-input/rss_input/main.py
- [x] T044 [US3] Update news-processor to fetch and parse multiple feeds in agents/news-processor/news_processor/main.py
- [x] T045 [US3] Implement deduplication logic for overlapping stories in agents/news-processor/news_processor/main.py
- [x] T046 [US3] Aggregate NewsItems from multiple feeds into single ProcessedFeed in agents/news-processor/news_processor/main.py
- [x] T047 [US3] Update script-generator to handle combined news from multiple sources in agents/script-generator/script_generator/main.py
- [x] T048 [US3] Ensure source_feeds array correctly lists all input URLs in agents/script-generator/script_generator/main.py

**Checkpoint**: User Story 3 complete - multiple RSS feeds can be combined into single script

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [x] T049 [P] Create README.md for rss-input agent in agents/rss-input/README.md
- [x] T050 [P] Create README.md for news-processor agent in agents/news-processor/README.md
- [x] T051 [P] Create README.md for script-generator agent in agents/script-generator/README.md
- [x] T052 [P] Create README.md for rss-newscaster flow in flows/rss-newscaster/README.md
- [x] T053 Add inline comments to configuration files
- [ ] T054 Run quickstart.md validation to verify all steps work
- [ ] T055 Performance testing with 20+ news items to verify SC-001 (< 2 minutes)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - User Story 1 (P1): MUST be completed first (core functionality)
  - User Story 2 (P2): Can start after US1 complete
  - User Story 3 (P3): Can start after US1 complete (independent of US2)
- **Polish (Phase 6)**: Depends on all user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P2)**: Builds on US1 - adds persona customization features
- **User Story 3 (P3)**: Builds on US1 - adds multi-source handling

### Within Each User Story

- Agent implementation in order: rss-input → news-processor → script-generator
- Dataflow integration after all agents are ready
- Working example validates the complete flow

### Parallel Opportunities

**Phase 1 (Setup)**:
- T002, T003, T004 can run in parallel with T001
- T006, T007 can run in parallel with T005

**Phase 3 (US1)**:
- rss-input, news-processor, script-generator agents can be developed in parallel
- T012-T015 (rss-input) parallel with T016-T022 (news-processor) parallel with T023-T030 (script-generator)

**Phase 6 (Polish)**:
- All README tasks (T049-T052) can run in parallel

---

## Parallel Example: User Story 1

```bash
# Launch all three agents in parallel:
Task: "Create main.py for rss-input agent in agents/rss-input/rss_input/main.py"
Task: "Create main.py for news-processor agent in agents/news-processor/news_processor/main.py"
Task: "Create main.py for script-generator agent in agents/script-generator/script_generator/main.py"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T008)
2. Complete Phase 2: Foundational (T009-T011)
3. Complete Phase 3: User Story 1 (T012-T034)
4. **STOP and VALIDATE**: Test with a real RSS feed
5. Deploy/demo if ready - core functionality works!

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → Deploy/Demo (MVP!)
3. Add User Story 2 → Test with custom personas → Deploy/Demo
4. Add User Story 3 → Test with multiple feeds → Deploy/Demo
5. Each story adds value without breaking previous stories

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Follow existing MoFA patterns (see podcast-generator, openai_chat_agent)
