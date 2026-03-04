-- ============================================================================
-- 简化的 UUID 生成函数 (兼容多版本)
-- Simplified UUID generation function (compatible with multiple versions)
-- ============================================================================
CREATE OR REPLACE FUNCTION gen_uuid_v7() RETURNS UUID AS $$
BEGIN
    -- 简化实现，避免复杂的异常处理
    -- Simplified implementation to avoid complex exception handling
    -- 检查 PostgreSQL 版本并选择适当和 UUID 生成方式
    -- Check PostgreSQL version and select appropriate UUID generation method
    IF (SELECT current_setting('server_version_num')::int >= 170000) THEN
        -- PostgreSQL 17+ 使用原生 UUID v7
        -- PostgreSQL 17+ uses native UUID v7
        -- 注意：PostgreSQL 17+ 的正确函数名是 uuid_generate_v7()，但需要安装扩展
        -- Note: The correct function in PostgreSQL 17+ is uuid_generate_v7(), but requires an extension
        -- 如果函数不存在，回退到 gen_random_uuid()
        -- If the function does not exist, fall back to gen_random_uuid()
        BEGIN
            RETURN uuid_generate_v7();
        EXCEPTION WHEN undefined_function THEN
            RETURN gen_random_uuid();
        END;
    ELSE
        -- 老版本使用 UUID v4（时间排序稍弱但功能正常）
        -- Older versions use UUID v4 (weaker time ordering but fully functional)
        RETURN gen_random_uuid();
    END IF;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- 创建自定义类型 (修正语法)
-- Create custom types (syntax correction)
-- ============================================================================
-- 注意：为提高兼容性，使用 VARCHAR 代替 ENUM 类型
-- Note: Use VARCHAR instead of ENUM type to improve compatibility
-- 允许值：
-- Allowed values:
--   role: 'system', 'user', 'assistant', 'tool'
--   role: 'system', 'user', 'assistant', 'tool'
--   status: 'success', 'failed', 'timeout', 'rate_limited', 'cancelled'
--   status: 'success', 'failed', 'timeout', 'rate_limited', 'cancelled'

-- ============================================================================
-- 表结构创建
-- Table structure creation
-- ============================================================================

-- Create chat session table
CREATE TABLE IF NOT EXISTS entity_chat_session (
    id UUID PRIMARY KEY DEFAULT gen_uuid_v7(),
    user_id UUID NOT NULL,
    agent_id UUID NOT NULL,
    tenant_id UUID NOT NULL DEFAULT gen_uuid_v7(),
    title VARCHAR(255),
    metadata JSONB DEFAULT '{}',
    create_time TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    update_time TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- 创建索引
-- Create indexes
CREATE INDEX IF NOT EXISTS idx_chat_session_user ON entity_chat_session(user_id);
CREATE INDEX IF NOT EXISTS idx_chat_session_agent ON entity_chat_session(agent_id);
CREATE INDEX IF NOT EXISTS idx_chat_session_tenant ON entity_chat_session(tenant_id);
CREATE INDEX IF NOT EXISTS idx_chat_session_update_time ON entity_chat_session(update_time DESC);

-- Create LLM message table
CREATE TABLE IF NOT EXISTS entity_llm_message (
    id UUID PRIMARY KEY DEFAULT gen_uuid_v7(),
    parent_message_id UUID,
    chat_session_id UUID NOT NULL,
    agent_id UUID NOT NULL,
    user_id UUID NOT NULL,
    tenant_id UUID NOT NULL DEFAULT gen_uuid_v7(),
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
    id UUID PRIMARY KEY DEFAULT gen_uuid_v7(),
    chat_session_id UUID NOT NULL,
    agent_id UUID NOT NULL,
    user_id UUID NOT NULL,
    tenant_id UUID NOT NULL DEFAULT gen_uuid_v7(),
    request_message_id UUID NOT NULL,
    response_message_id UUID NOT NULL,
    model_name VARCHAR(100) NOT NULL,
    status VARCHAR(20) DEFAULT 'success' NOT null,
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
    create_time TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    update_time TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    CONSTRAINT check_tokens_positive CHECK (prompt_tokens >= 0 AND completion_tokens >= 0 AND total_tokens >= 0),
    CONSTRAINT check_time_order CHECK (update_time >= create_time)
);

CREATE INDEX IF NOT EXISTS idx_api_call_session ON entity_llm_api_call(chat_session_id, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_api_call_user ON entity_llm_api_call(user_id, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_api_call_agent ON entity_llm_api_call(agent_id, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_api_call_status ON entity_llm_api_call(status, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_api_call_model ON entity_llm_api_call(model_name, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_api_call_time ON entity_llm_api_call(create_time DESC);

-- ============================================================================
-- 自动更新函数和触发器
-- Automatic update functions and triggers
-- ============================================================================

-- 创建自动更新时间的函数
-- Create function to automatically update time
CREATE OR REPLACE FUNCTION update_modified_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.update_time = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- 创建触发器
-- Create triggers
DROP TRIGGER IF EXISTS trigger_update_chat_session_time ON entity_chat_session;
CREATE TRIGGER trigger_update_chat_session_time
    BEFORE UPDATE ON entity_chat_session
    FOR EACH ROW EXECUTE FUNCTION update_modified_column();

DROP TRIGGER IF EXISTS trigger_update_llm_message_time ON entity_llm_message;
CREATE TRIGGER trigger_update_llm_message_time
    BEFORE UPDATE ON entity_llm_message
    FOR EACH ROW EXECUTE FUNCTION update_modified_column();

-- crate provider table
CREATE TABLE public.entity_provider (
  id uuid DEFAULT uuidv7() NOT NULL,
  api_base text NOT NULL,
  api_key text NOT NULL,
  enabled bool DEFAULT true NOT NULL,
  provider_name varchar(255) DEFAULT 'default'::character varying NOT NULL,
  provider_type varchar(100) NOT NULL,
  tenant_id uuid NOT NULL,
  create_time TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
  update_time TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
  CONSTRAINT entity_provider_pkey PRIMARY KEY (id),
  CONSTRAINT uk_entity_provider_tenant UNIQUE (provider_name, tenant_id)
);
CREATE INDEX idx_entity_provider_create_time ON public.entity_provider USING btree (create_time);
CREATE INDEX idx_entity_provider_enabled ON public.entity_provider USING btree (enabled);
CREATE INDEX idx_entity_provider_tenant ON public.entity_provider USING btree (tenant_id);
CREATE INDEX idx_entity_provider_type ON public.entity_provider USING btree (provider_type);
CREATE INDEX idx_entity_provider_update_time ON public.entity_provider USING btree (update_time);

-- Table Triggers

create trigger trigger_update_entity_provider_update_time before
update
    on
    public.entity_provider for each row execute function moddatetime('update_time');

-- crate agent table
CREATE TABLE public.entity_agent (
  id uuid DEFAULT uuidv7() NOT NULL,
  tenant_id uuid NOT NULL,
  agent_code varchar(255) NOT NULL,
  agent_name varchar(255) NOT NULL,
  agent_order int4 DEFAULT 0 NOT NULL,
  agent_status bool DEFAULT true NOT NULL,
  context_limit int4 DEFAULT 1 NULL,
  custom_params jsonb NULL,
  max_completion_tokens int4 DEFAULT 16384 NULL,
  model_name varchar(255) NOT NULL,
  provider_id uuid NOT NULL,
  response_format varchar(50) DEFAULT 'text'::character varying NULL,
  system_prompt text NOT NULL,
  temperature float4 DEFAULT 0.7 NULL,
  stream bool DEFAULT true NULL,
  thinking jsonb NULL,
  create_time TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
  update_time TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
  CONSTRAINT entity_agent_pkey PRIMARY KEY (id),
  CONSTRAINT uk_entity_agent_code UNIQUE (agent_code),
  CONSTRAINT entity_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.entity_provider(id) ON DELETE CASCADE
);
CREATE INDEX idx_entity_agent_create_time ON public.entity_agent USING btree (create_time);
CREATE INDEX idx_entity_agent_order ON public.entity_agent USING btree (agent_order);
CREATE INDEX idx_entity_agent_update_time ON public.entity_agent USING btree (update_time);

-- Table Triggers

create trigger trigger_update_entity_agent_update_time before
update
    on
    public.entity_agent for each row execute function moddatetime('update_time');

$$ LANGUAGE plpgsql;

-- ============================================================================
-- 成功消息
-- Success messages
-- ============================================================================

DO $$
BEGIN
    RAISE NOTICE 'MoFA persistence tables initialized successfully!';
END
$$;

DO $$
BEGIN
    RAISE NOTICE 'MoFA prompt management tables initialized successfully!';
END
$$;