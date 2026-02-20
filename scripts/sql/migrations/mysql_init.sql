-- MoFA Persistence Layer - MySQL/MariaDB Migration
-- Version: 1.0.0
-- Description: Initialize persistence tables for LLM messages and API call tracking

-- ============================================================================
-- UUID v7 Generator Function
-- ============================================================================
-- UUID v7 format: timestamp (48 bits) + version (4 bits) + variant (2 bits) + random (62 bits)
DELIMITER $$
CREATE FUNCTION IF NOT EXISTS gen_uuid_v7() RETURNS CHAR(36)
DETERMINISTIC
SQL SECURITY INVOKER
BEGIN
    DECLARE unix_ts_ms BIGINT;
    DECLARE uuid_hex CHAR(32);

    -- Get current Unix timestamp in milliseconds
    SET unix_ts_ms = UNIX_TIMESTAMP(NOW(6)) * 1000;

    -- Build UUID v7 hex string:
    -- Format: XXXXXXXX-XXXX-7XXX-YXXX-XXXXXXXXXXXX
    -- where X is random/hex data, 7 is version, Y is variant (8, 9, A, or B)

    -- timestamp: 12 hex chars (48 bits) - most significant first
    -- version+rand_a: 4 hex chars, first is 7
    -- variant+rand_b: 4 hex chars, first is 8/9/A/B
    -- rand_c: 12 hex chars

    SET uuid_hex = CONCAT(
        -- timestamp (12 hex chars)
        LPAD(CONV(unix_ts_ms >> 20, 10, 16), 12, '0'),
        -- version and random bits (4 hex chars)
        CONV((unix_ts_ms & 0xFFFF0) | 0x7000, 10, 16),
        -- variant and random bits (4 hex chars)
        CONV(FLOOR(RAND() * 0x4000) | 0x8000, 10, 16),
        -- random data (12 hex chars)
        LPAD(CONV(FLOOR(RAND() * 281474976710656), 10, 16), 12, '0')
    );

    RETURN CONCAT(
        SUBSTR(uuid_hex, 1, 8), '-',
        SUBSTR(uuid_hex, 9, 4), '-',
        SUBSTR(uuid_hex, 13, 4), '-',
        SUBSTR(uuid_hex, 17, 4), '-',
        SUBSTR(uuid_hex, 21, 12)
    );
END$$
DELIMITER ;

-- Create chat session table
CREATE TABLE IF NOT EXISTS entity_chat_session (
    id CHAR(36) PRIMARY KEY DEFAULT (gen_uuid_v7()),
    user_id CHAR(36) NOT NULL,
    agent_id CHAR(36) NOT NULL,
    tenant_id CHAR(36) NOT NULL DEFAULT (gen_uuid_v7()),
    title VARCHAR(255),
    metadata JSON,
    create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    update_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP NOT NULL,
    INDEX idx_chat_session_user (user_id),
    INDEX idx_chat_session_agent (agent_id),
    INDEX idx_chat_session_tenant (tenant_id),
    INDEX idx_chat_session_update_time (update_time DESC)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Create LLM message table
CREATE TABLE IF NOT EXISTS entity_llm_message (
    id CHAR(36) PRIMARY KEY DEFAULT (gen_uuid_v7()),
    parent_message_id CHAR(36),
    chat_session_id CHAR(36) NOT NULL,
    agent_id CHAR(36) NOT NULL,
    user_id CHAR(36) NOT NULL,
    tenant_id CHAR(36) NOT NULL DEFAULT (gen_uuid_v7()),
    role VARCHAR(20) NOT NULL,
    content JSON NOT NULL,
    create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    update_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP NOT NULL,
    INDEX idx_llm_message_session (chat_session_id, create_time),
    INDEX idx_llm_message_user (user_id),
    INDEX idx_llm_message_agent (agent_id),
    INDEX idx_llm_message_create_time (create_time DESC),
    CONSTRAINT fk_message_session FOREIGN KEY (chat_session_id) REFERENCES entity_chat_session(id) ON DELETE CASCADE,
    CONSTRAINT fk_message_parent FOREIGN KEY (parent_message_id) REFERENCES entity_llm_message(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Create LLM API call table
CREATE TABLE IF NOT EXISTS entity_llm_api_call (
    id CHAR(36) PRIMARY KEY DEFAULT (gen_uuid_v7()),
    chat_session_id CHAR(36) NOT NULL,
    agent_id CHAR(36) NOT NULL,
    user_id CHAR(36) NOT NULL,
    tenant_id CHAR(36) NOT NULL DEFAULT (gen_uuid_v7()),
    request_message_id CHAR(36) NOT NULL,
    response_message_id CHAR(36) NOT NULL,
    model_name VARCHAR(100) NOT NULL,
    status ENUM('success', 'failed', 'timeout', 'rate_limited', 'cancelled') DEFAULT 'success' NOT NULL,
    error_message TEXT,
    error_code VARCHAR(50),
    prompt_tokens INT NOT NULL DEFAULT 0,
    completion_tokens INT NOT NULL DEFAULT 0,
    total_tokens INT NOT NULL DEFAULT 0,
    prompt_tokens_details JSON,
    completion_tokens_details JSON,
    total_price DECIMAL(15, 10),
    price_details JSON,
    latency_ms INT,
    time_to_first_token_ms INT,
    tokens_per_second DOUBLE,
    api_response_id VARCHAR(255),
    create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    update_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP NOT NULL,
    INDEX idx_api_call_session (chat_session_id, create_time DESC),
    INDEX idx_api_call_user (user_id, create_time DESC),
    INDEX idx_api_call_agent (agent_id, create_time DESC),
    INDEX idx_api_call_status (status, create_time DESC),
    INDEX idx_api_call_model (model_name, create_time DESC),
    INDEX idx_api_call_time (create_time DESC),
    INDEX idx_api_call_tenant (tenant_id, create_time DESC),
    CONSTRAINT check_tokens_positive CHECK (prompt_tokens >= 0 AND completion_tokens >= 0 AND total_tokens >= 0),
    CONSTRAINT check_time_order CHECK (update_time >= create_time)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Create provider table
CREATE TABLE IF NOT EXISTS entity_provider (
    id CHAR(36) PRIMARY KEY DEFAULT (gen_uuid_v7()),
    tenant_id CHAR(36) NOT NULL DEFAULT (gen_uuid_v7()),
    provider_name VARCHAR(255) NOT NULL,
    provider_type VARCHAR(100) NOT NULL,
    api_base TEXT NOT NULL,
    api_key TEXT NOT NULL,
    enabled BOOLEAN DEFAULT TRUE NOT NULL,
    create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    update_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP NOT NULL,
    CONSTRAINT uk_entity_provider_tenant UNIQUE (provider_name, tenant_id),
    INDEX idx_entity_provider_tenant (tenant_id),
    INDEX idx_entity_provider_enabled (enabled),
    INDEX idx_entity_provider_type (provider_type)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Create agent table
CREATE TABLE IF NOT EXISTS entity_agent (
    id CHAR(36) PRIMARY KEY DEFAULT (gen_uuid_v7()),
    tenant_id CHAR(36) NOT NULL DEFAULT (gen_uuid_v7()),
    agent_code VARCHAR(255) NOT NULL UNIQUE,
    agent_name VARCHAR(255) NOT NULL,
    agent_order INT DEFAULT 0 NOT NULL,
    agent_status BOOLEAN DEFAULT TRUE NOT NULL,
    context_limit INT,
    custom_params JSON,
    max_completion_tokens INT,
    model_name VARCHAR(255) NOT NULL,
    provider_id CHAR(36) NOT NULL,
    response_format VARCHAR(50),
    system_prompt TEXT NOT NULL,
    temperature FLOAT,
    stream BOOLEAN,
    thinking JSON,
    create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    update_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP NOT NULL,
    CONSTRAINT entity_agent_provider_fkey FOREIGN KEY (provider_id) REFERENCES entity_provider(id) ON DELETE CASCADE,
    INDEX idx_entity_agent_tenant (tenant_id),
    INDEX idx_entity_agent_order (agent_order)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Print success message (MySQL doesn't support RAISE NOTICE, use SELECT instead)
SELECT 'MoFA persistence tables initialized successfully!' AS message;

-- ============================================================================
-- Prompt Management Tables
-- ============================================================================

-- Create prompt template table
CREATE TABLE IF NOT EXISTS prompt_template (
    id BINARY(16) PRIMARY KEY DEFAULT (UUID_TO_BIN(gen_uuid_v7())),
    template_id VARCHAR(255) NOT NULL,
    name VARCHAR(255),
    description TEXT,
    content TEXT NOT NULL,
    variables JSON,
    tags JSON,
    version VARCHAR(50),
    metadata JSON,
    enabled BOOLEAN DEFAULT TRUE NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP NOT NULL,
    created_by BINARY(16),
    tenant_id BINARY(16) DEFAULT (UUID_TO_BIN(gen_uuid_v7())),
    UNIQUE KEY uk_prompt_template_tenant (template_id, COALESCE(tenant_id, UUID_TO_BIN(gen_uuid_v7()))),
    INDEX idx_prompt_template_id (template_id),
    INDEX idx_prompt_template_enabled (enabled),
    INDEX idx_prompt_template_tenant (tenant_id),
    INDEX idx_prompt_template_updated (updated_at DESC)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Create prompt composition table
CREATE TABLE IF NOT EXISTS prompt_composition (
    id BINARY(16) PRIMARY KEY DEFAULT (UUID_TO_BIN(gen_uuid_v7())),
    composition_id VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    template_ids JSON,
    separator VARCHAR(50) DEFAULT '\n\n',
    enabled BOOLEAN DEFAULT TRUE NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP NOT NULL,
    tenant_id BINARY(16) DEFAULT (UUID_TO_BIN(gen_uuid_v7())),
    INDEX idx_prompt_composition_id (composition_id),
    INDEX idx_prompt_composition_enabled (enabled)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

SELECT 'MoFA prompt management tables initialized successfully!' AS message;
