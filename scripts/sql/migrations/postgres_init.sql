-- MoFA Persistence Layer - PostgreSQL Migration
-- Version: 1.0.0
-- Description: Initialize persistence tables for LLM messages and API call tracking

-- Create custom types
DO $$ BEGIN
    CREATE TYPE api_call_status AS ENUM ('success', 'failed', 'timeout', 'rate_limited', 'cancelled');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE message_role AS ENUM ('system', 'user', 'assistant', 'tool');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Create chat session table
CREATE TABLE IF NOT EXISTS entity_chat_session (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL,
    agent_id UUID NOT NULL,
    title VARCHAR(255),
    metadata JSONB DEFAULT '{}',
    create_time TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    update_time TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_chat_session_user ON entity_chat_session(user_id);
CREATE INDEX IF NOT EXISTS idx_chat_session_agent ON entity_chat_session(agent_id);
CREATE INDEX IF NOT EXISTS idx_chat_session_update_time ON entity_chat_session(update_time DESC);

-- Create LLM message table
CREATE TABLE IF NOT EXISTS entity_llm_message (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    parent_message_id UUID,
    chat_session_id UUID NOT NULL,
    agent_id UUID NOT NULL,
    user_id UUID NOT NULL,
    tenant_id UUID NOT NULL DEFAULT gen_random_uuid(),
    role VARCHAR(20) NOT NULL,
    content JSONB NOT NULL,
    create_time TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    update_time TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    CONSTRAINT fk_message_session FOREIGN KEY (chat_session_id) REFERENCES entity_chat_session(id) ON DELETE CASCADE,
    CONSTRAINT fk_message_parent FOREIGN KEY (parent_message_id) REFERENCES entity_llm_message(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_llm_message_session ON entity_llm_message(chat_session_id, create_time);
CREATE INDEX IF NOT EXISTS idx_llm_message_user ON entity_llm_message(user_id);
CREATE INDEX IF NOT EXISTS idx_llm_message_agent ON entity_llm_message(agent_id);
CREATE INDEX IF NOT EXISTS idx_llm_message_create_time ON entity_llm_message(create_time DESC);

-- Create LLM API call table
CREATE TABLE IF NOT EXISTS entity_llm_api_call (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chat_session_id UUID NOT NULL,
    agent_id UUID NOT NULL,
    user_id UUID NOT NULL,
    request_message_id UUID NOT NULL,
    response_message_id UUID NOT NULL,
    model_name VARCHAR(100) NOT NULL,
    status VARCHAR(20) DEFAULT 'success' NOT NULL,
    error_message TEXT,
    error_code VARCHAR(50),
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    prompt_tokens_details JSONB,
    completion_tokens_details JSONB,
    total_price NUMERIC(15, 10),
    price_details JSONB,
    latency_ms INTEGER,
    time_to_first_token_ms INTEGER,
    tokens_per_second DOUBLE PRECISION,
    api_response_id VARCHAR(255),
    request_time TIMESTAMP WITH TIME ZONE NOT NULL,
    response_time TIMESTAMP WITH TIME ZONE NOT NULL,
    create_time TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    CONSTRAINT check_tokens_positive CHECK (prompt_tokens >= 0 AND completion_tokens >= 0 AND total_tokens >= 0),
    CONSTRAINT check_time_order CHECK (response_time >= request_time)
);

CREATE INDEX IF NOT EXISTS idx_api_call_session ON entity_llm_api_call(chat_session_id, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_api_call_user ON entity_llm_api_call(user_id, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_api_call_agent ON entity_llm_api_call(agent_id, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_api_call_status ON entity_llm_api_call(status, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_api_call_model ON entity_llm_api_call(model_name, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_api_call_time ON entity_llm_api_call(create_time DESC);

-- Create function for auto-updating update_time (if not exists)
CREATE OR REPLACE FUNCTION update_modified_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.update_time = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Create triggers for auto-updating update_time
DROP TRIGGER IF EXISTS trigger_update_chat_session_time ON entity_chat_session;
CREATE TRIGGER trigger_update_chat_session_time
    BEFORE UPDATE ON entity_chat_session
    FOR EACH ROW EXECUTE FUNCTION update_modified_column();

DROP TRIGGER IF EXISTS trigger_update_llm_message_time ON entity_llm_message;
CREATE TRIGGER trigger_update_llm_message_time
    BEFORE UPDATE ON entity_llm_message
    FOR EACH ROW EXECUTE FUNCTION update_modified_column();

-- Print success message
DO $$
BEGIN
    RAISE NOTICE 'MoFA persistence tables initialized successfully!';
END $$;

-- ============================================================================
-- Prompt Management Tables
-- ============================================================================

-- Create prompt template table
CREATE TABLE IF NOT EXISTS prompt_template (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_id VARCHAR(255) NOT NULL,
    name VARCHAR(255),
    description TEXT,
    content TEXT NOT NULL,
    variables JSONB DEFAULT '[]',
    tags JSONB DEFAULT '[]',
    version VARCHAR(50),
    metadata JSONB DEFAULT '{}',
    enabled BOOLEAN DEFAULT true NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    created_by UUID,
    tenant_id UUID,
    CONSTRAINT uk_prompt_template_id UNIQUE (template_id, COALESCE(tenant_id, '00000000-0000-0000-0000-000000000000'::uuid))
);

CREATE INDEX IF NOT EXISTS idx_prompt_template_id ON prompt_template(template_id);
CREATE INDEX IF NOT EXISTS idx_prompt_template_enabled ON prompt_template(enabled);
CREATE INDEX IF NOT EXISTS idx_prompt_template_tenant ON prompt_template(tenant_id);
CREATE INDEX IF NOT EXISTS idx_prompt_template_tags ON prompt_template USING GIN (tags);
CREATE INDEX IF NOT EXISTS idx_prompt_template_updated ON prompt_template(updated_at DESC);

-- Create prompt composition table
CREATE TABLE IF NOT EXISTS prompt_composition (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    composition_id VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    template_ids JSONB DEFAULT '[]',
    separator VARCHAR(50) DEFAULT E'\n\n',
    enabled BOOLEAN DEFAULT true NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    tenant_id UUID
);

CREATE INDEX IF NOT EXISTS idx_prompt_composition_id ON prompt_composition(composition_id);
CREATE INDEX IF NOT EXISTS idx_prompt_composition_enabled ON prompt_composition(enabled);

-- Create trigger for auto-updating prompt_template update_time
DROP TRIGGER IF EXISTS trigger_update_prompt_template_time ON prompt_template;
CREATE TRIGGER trigger_update_prompt_template_time
    BEFORE UPDATE ON prompt_template
    FOR EACH ROW EXECUTE FUNCTION update_modified_column();

-- Create trigger for auto-updating prompt_composition update_time
DROP TRIGGER IF EXISTS trigger_update_prompt_composition_time ON prompt_composition;
CREATE TRIGGER trigger_update_prompt_composition_time
    BEFORE UPDATE ON prompt_composition
    FOR EACH ROW EXECUTE FUNCTION update_modified_column();

-- Print success message
DO $$
BEGIN
    RAISE NOTICE 'MoFA prompt management tables initialized successfully!';
END $$;
