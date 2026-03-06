#!/usr/bin/env node
/**
 * Example: JavaScript/Node.js client for MoFA Gateway proxy
 * 
 * This example demonstrates how to use the OpenAI JavaScript SDK with the MoFA Gateway.
 * 
 * Prerequisites:
 * 1. Install OpenAI SDK: npm install openai
 * 2. Start mofa-local-llm server
 * 3. Start gateway: cargo run --example gateway_local_llm_proxy
 * 4. Run this script: node examples/proxy/proxy_javascript_client.js
 */

const OpenAI = require('openai');

async function main() {
    console.log('🚀 MoFA Gateway Proxy - JavaScript Client Example');
    console.log('='.repeat(60));

    // Initialize OpenAI client pointing to gateway
    const client = new OpenAI({
        baseURL: 'http://localhost:8080/v1',
        apiKey: 'not-needed' // Gateway doesn't require auth yet
    });

    // Example 1: List models
    console.log('\n📋 Example 1: List All Models');
    console.log('-'.repeat(60));
    try {
        const models = await client.models.list();
        console.log(`✅ Found ${models.data.length} models:`);
        models.data.forEach(model => {
            console.log(`  - ${model.id}`);
        });
    } catch (error) {
        console.log(`❌ Error: ${error.message}`);
    }

    // Example 2: Get model info
    console.log('\n📋 Example 2: Get Model Information');
    console.log('-'.repeat(60));
    try {
        const model = await client.models.retrieve('qwen2.5-0.5b-instruct');
        console.log('✅ Model Information:');
        console.log(`  ID: ${model.id}`);
        console.log(`  Object: ${model.object}`);
        console.log(`  Owner: ${model.owned_by}`);
    } catch (error) {
        console.log(`❌ Error: ${error.message}`);
    }

    // Example 3: Simple chat completion
    console.log('\n📋 Example 3: Simple Chat Completion');
    console.log('-'.repeat(60));
    try {
        const response = await client.chat.completions.create({
            model: 'qwen2.5-0.5b-instruct',
            messages: [
                { role: 'user', content: 'What is Rust programming language?' }
            ],
            max_tokens: 100
        });
        console.log('✅ Chat Response:');
        console.log(`  ${response.choices[0].message.content}`);
        console.log('\n  Usage:');
        console.log(`    Prompt tokens: ${response.usage.prompt_tokens}`);
        console.log(`    Completion tokens: ${response.usage.completion_tokens}`);
        console.log(`    Total tokens: ${response.usage.total_tokens}`);
    } catch (error) {
        console.log(`❌ Error: ${error.message}`);
    }

    // Example 4: Chat with system message
    console.log('\n📋 Example 4: Chat with System Message');
    console.log('-'.repeat(60));
    try {
        const response = await client.chat.completions.create({
            model: 'qwen2.5-0.5b-instruct',
            messages: [
                { role: 'system', content: 'You are a helpful coding assistant.' },
                { role: 'user', content: 'Write a hello world in JavaScript' }
            ],
            max_tokens: 150,
            temperature: 0.7
        });
        console.log('✅ Chat Response:');
        console.log(`  ${response.choices[0].message.content}`);
    } catch (error) {
        console.log(`❌ Error: ${error.message}`);
    }

    // Example 5: Multi-turn conversation
    console.log('\n📋 Example 5: Multi-turn Conversation');
    console.log('-'.repeat(60));
    try {
        const messages = [
            { role: 'user', content: 'What is 2+2?' },
            { role: 'assistant', content: '2+2 equals 4.' },
            { role: 'user', content: 'What about 3+3?' }
        ];
        const response = await client.chat.completions.create({
            model: 'qwen2.5-0.5b-instruct',
            messages: messages,
            max_tokens: 50
        });
        console.log('✅ Chat Response:');
        console.log(`  ${response.choices[0].message.content}`);
    } catch (error) {
        console.log(`❌ Error: ${error.message}`);
    }

    // Example 6: Error handling
    console.log('\n📋 Example 6: Error Handling - Invalid Model');
    console.log('-'.repeat(60));
    try {
        await client.models.retrieve('non-existent-model');
        console.log('✅ Model found (unexpected)');
    } catch (error) {
        console.log(`✅ Expected error: ${error.message}`);
    }

    console.log('\n' + '='.repeat(60));
    console.log('✨ All examples completed!');
    console.log('\n💡 Tips:');
    console.log('  - Use RUST_LOG=debug for detailed gateway logs');
    console.log('  - Check metrics: curl http://localhost:8080/metrics');
    console.log('  - See PROXY.md for more examples');
}

main().catch(error => {
    console.error('Fatal error:', error);
    process.exit(1);
});
