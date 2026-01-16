-- MoFA Persistence Layer - MySQL/MariaDB Migration
-- Version: 1.0.0
-- Description: Initialize persistence tables for LLM messages and API call tracking

-- Create chat session table
CREATE TABLE IF NOT EXISTS entity_chat_session (
    id CHAR(36) PRIMARY KEY,
    user_id CHAR(36) NOT NULL,
    agent_id CHAR(36) NOT NULL,
    title VARCHAR(255),
    metadata JSON,
    create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    update_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP NOT NULL,
    INDEX idx_chat_session_user (user_id),
    INDEX idx_chat_session_agent (agent_id),
    INDEX idx_chat_session_update_time (update_time DESC)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Create LLM message table
CREATE TABLE IF NOT EXISTS entity_llm_message (
    id CHAR(36) PRIMARY KEY,
    parent_message_id CHAR(36),
    chat_session_id CHAR(36) NOT NULL,
    agent_id CHAR(36) NOT NULL,
    user_id CHAR(36) NOT NULL,
    tenant_id CHAR(36) NOT NULL,
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
    id CHAR(36) PRIMARY KEY,
    chat_session_id CHAR(36) NOT NULL,
    agent_id CHAR(36) NOT NULL,
    user_id CHAR(36) NOT NULL,
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
    request_time TIMESTAMP NOT NULL,
    response_time TIMESTAMP NOT NULL,
    create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    INDEX idx_api_call_session (chat_session_id, create_time DESC),
    INDEX idx_api_call_user (user_id, create_time DESC),
    INDEX idx_api_call_agent (agent_id, create_time DESC),
    INDEX idx_api_call_status (status, create_time DESC),
    INDEX idx_api_call_model (model_name, create_time DESC),
    INDEX idx_api_call_time (create_time DESC),
    CONSTRAINT check_tokens_positive CHECK (prompt_tokens >= 0 AND completion_tokens >= 0 AND total_tokens >= 0),
    CONSTRAINT check_time_order CHECK (response_time >= request_time)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Print success message (MySQL doesn't support RAISE NOTICE, use SELECT instead)
SELECT 'MoFA persistence tables initialized successfully!' AS message;

-- ============================================================================
-- Prompt Management Tables
-- ============================================================================

-- Create prompt template table
CREATE TABLE IF NOT EXISTS prompt_template (
    id BINARY(16) PRIMARY KEY,
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
    tenant_id BINARY(16),
    UNIQUE KEY uk_prompt_template_tenant (template_id, tenant_id),
    INDEX idx_prompt_template_id (template_id),
    INDEX idx_prompt_template_enabled (enabled),
    INDEX idx_prompt_template_tenant (tenant_id),
    INDEX idx_prompt_template_updated (updated_at DESC)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Create prompt composition table
CREATE TABLE IF NOT EXISTS prompt_composition (
    id BINARY(16) PRIMARY KEY,
    composition_id VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    template_ids JSON,
    separator VARCHAR(50) DEFAULT '\n\n',
    enabled BOOLEAN DEFAULT TRUE NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP NOT NULL,
    tenant_id BINARY(16),
    INDEX idx_prompt_composition_id (composition_id),
    INDEX idx_prompt_composition_enabled (enabled)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

SELECT 'MoFA prompt management tables initialized successfully!' AS message;
