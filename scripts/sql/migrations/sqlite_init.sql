-- MoFA Persistence Layer - SQLite Migration
-- Version: 1.0.0
-- Description: Initialize persistence tables for LLM messages and API call tracking

-- ============================================================================
-- UUID v7 Note
-- ============================================================================
-- SQLite does not support custom SQL functions. UUID v7 values must be
-- generated in the application layer using a UUID v7 library.
--
-- For Rust applications using this schema, use the `uuid` crate with feature
-- "v7" to generate UUID v7 values:
--
--   use uuid::Uuid;
--   let id = Uuid::now_v7();  // Generate UUID v7
--
-- When inserting records, always provide a UUID v7 value for id columns:
--
--   INSERT INTO entity_chat_session (id, user_id, agent_id, tenant_id, ...)
--   VALUES (?, ?, ?, ?, ...);
--
-- Where the UUID values are generated application-side.

-- Create chat session table
CREATE TABLE IF NOT EXISTS entity_chat_session (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    tenant_id TEXT,
    title TEXT,
    metadata TEXT,
    create_time TEXT NOT NULL,
    update_time TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_chat_session_user ON entity_chat_session(user_id);
CREATE INDEX IF NOT EXISTS idx_chat_session_agent ON entity_chat_session(agent_id);
CREATE INDEX IF NOT EXISTS idx_chat_session_tenant ON entity_chat_session(tenant_id);
CREATE INDEX IF NOT EXISTS idx_chat_session_update_time ON entity_chat_session(update_time DESC);

-- Create LLM message table
CREATE TABLE IF NOT EXISTS entity_llm_message (
    id TEXT PRIMARY KEY,
    parent_message_id TEXT,
    chat_session_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    tenant_id TEXT,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    create_time TEXT NOT NULL,
    update_time TEXT NOT NULL,
    FOREIGN KEY (chat_session_id) REFERENCES entity_chat_session(id) ON DELETE CASCADE,
    FOREIGN KEY (parent_message_id) REFERENCES entity_llm_message(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_llm_message_session ON entity_llm_message(chat_session_id, create_time);
CREATE INDEX IF NOT EXISTS idx_llm_message_user ON entity_llm_message(user_id);
CREATE INDEX IF NOT EXISTS idx_llm_message_agent ON entity_llm_message(agent_id);
CREATE INDEX IF NOT EXISTS idx_llm_message_create_time ON entity_llm_message(create_time DESC);

-- Create LLM API call table
CREATE TABLE IF NOT EXISTS entity_llm_api_call (
    id TEXT PRIMARY KEY,
    chat_session_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    tenant_id TEXT NOT NULL,
    request_message_id TEXT NOT NULL,
    response_message_id TEXT NOT NULL,
    model_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('success', 'failed', 'timeout', 'rate_limited', 'cancelled')),
    error_message TEXT,
    error_code TEXT,
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    prompt_tokens_details TEXT,
    completion_tokens_details TEXT,
    total_price REAL,
    price_details TEXT,
    latency_ms INTEGER,
    time_to_first_token_ms INTEGER,
    tokens_per_second REAL,
    api_response_id TEXT,
    create_time TEXT NOT NULL,
    update_time TEXT NOT NULL,
    CHECK (prompt_tokens >= 0 AND completion_tokens >= 0 AND total_tokens >= 0)
);

CREATE INDEX IF NOT EXISTS idx_api_call_session ON entity_llm_api_call(chat_session_id, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_api_call_user ON entity_llm_api_call(user_id, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_api_call_agent ON entity_llm_api_call(agent_id, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_api_call_status ON entity_llm_api_call(status, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_api_call_model ON entity_llm_api_call(model_name, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_api_call_time ON entity_llm_api_call(create_time DESC);
CREATE INDEX IF NOT EXISTS idx_api_call_tenant ON entity_llm_api_call(tenant_id, create_time DESC);

-- SQLite doesn't support procedures, print is just a comment
-- MoFA persistence tables initialized successfully!

-- ============================================================================
-- Prompt Management Tables
-- ============================================================================

-- Create prompt template table
CREATE TABLE IF NOT EXISTS prompt_template (
    id TEXT PRIMARY KEY,
    template_id TEXT NOT NULL UNIQUE,
    name TEXT,
    description TEXT,
    content TEXT NOT NULL,
    variables TEXT DEFAULT '[]',
    tags TEXT DEFAULT '[]',
    version TEXT,
    metadata TEXT DEFAULT '{}',
    enabled INTEGER DEFAULT 1 NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    created_by TEXT,
    tenant_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_prompt_template_id ON prompt_template(template_id);
CREATE INDEX IF NOT EXISTS idx_prompt_template_enabled ON prompt_template(enabled);
CREATE INDEX IF NOT EXISTS idx_prompt_template_tenant ON prompt_template(tenant_id);
CREATE INDEX IF NOT EXISTS idx_prompt_template_updated ON prompt_template(updated_at DESC);

-- Create prompt composition table
CREATE TABLE IF NOT EXISTS prompt_composition (
    id TEXT PRIMARY KEY,
    composition_id TEXT NOT NULL UNIQUE,
    description TEXT,
    template_ids TEXT DEFAULT '[]',
    separator TEXT DEFAULT '\n\n',
    enabled INTEGER DEFAULT 1 NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    tenant_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_prompt_composition_id ON prompt_composition(composition_id);
CREATE INDEX IF NOT EXISTS idx_prompt_composition_enabled ON prompt_composition(enabled);

-- Create provider table
CREATE TABLE IF NOT EXISTS entity_provider (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    provider_name TEXT NOT NULL,
    provider_type TEXT NOT NULL,
    api_base TEXT NOT NULL,
    api_key TEXT NOT NULL,
    enabled INTEGER DEFAULT 1 NOT NULL,
    create_time TEXT NOT NULL,
    update_time TEXT NOT NULL,
    CONSTRAINT uk_entity_provider_tenant UNIQUE (tenant_id, provider_name)
);

CREATE INDEX IF NOT EXISTS idx_entity_provider_tenant ON entity_provider(tenant_id);
CREATE INDEX IF NOT EXISTS idx_entity_provider_enabled ON entity_provider(enabled);
CREATE INDEX IF NOT EXISTS idx_entity_provider_type ON entity_provider(provider_type);

-- Create agent table
CREATE TABLE IF NOT EXISTS entity_agent (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    agent_code TEXT NOT NULL UNIQUE,
    agent_name TEXT NOT NULL,
    agent_order INTEGER NOT NULL DEFAULT 0,
    agent_status INTEGER NOT NULL DEFAULT 1,
    context_limit INTEGER,
    custom_params TEXT,
    max_completion_tokens INTEGER,
    model_name TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    response_format TEXT,
    system_prompt TEXT NOT NULL,
    temperature REAL,
    stream INTEGER,
    thinking TEXT,
    create_time TEXT NOT NULL,
    update_time TEXT NOT NULL,
    FOREIGN KEY (provider_id) REFERENCES entity_provider(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_entity_agent_tenant ON entity_agent(tenant_id);
CREATE INDEX IF NOT EXISTS idx_entity_agent_order ON entity_agent(agent_order);

-- MoFA prompt management tables initialized successfully!
