#!/usr/bin/env python3
"""
Example: Python client for MoFA Gateway proxy

This example demonstrates how to use the OpenAI Python SDK with the MoFA Gateway.

Prerequisites:
1. Install OpenAI SDK: pip install openai
2. Start mofa-local-llm server
3. Start gateway: cargo run --example gateway_local_llm_proxy
4. Run this script: python examples/proxy/proxy_python_client.py
"""

from openai import OpenAI
import sys

def main():
    print("🚀 MoFA Gateway Proxy - Python Client Example")
    print("=" * 60)
    
    # Initialize OpenAI client pointing to gateway
    client = OpenAI(
        base_url="http://localhost:8080/v1",
        api_key="not-needed"  # Gateway doesn't require auth yet
    )
    
    # Example 1: List models
    print("\n📋 Example 1: List All Models")
    print("-" * 60)
    try:
        models = client.models.list()
        print(f"✅ Found {len(models.data)} models:")
        for model in models.data:
            print(f"  - {model.id}")
    except Exception as e:
        print(f"❌ Error: {e}")
    
    # Example 2: Get model info
    print("\n📋 Example 2: Get Model Information")
    print("-" * 60)
    try:
        model = client.models.retrieve("qwen2.5-0.5b-instruct")
        print(f"✅ Model Information:")
        print(f"  ID: {model.id}")
        print(f"  Object: {model.object}")
        print(f"  Owner: {model.owned_by}")
    except Exception as e:
        print(f"❌ Error: {e}")
    
    # Example 3: Simple chat completion
    print("\n📋 Example 3: Simple Chat Completion")
    print("-" * 60)
    try:
        response = client.chat.completions.create(
            model="qwen2.5-0.5b-instruct",
            messages=[
                {"role": "user", "content": "What is Rust programming language?"}
            ],
            max_tokens=100
        )
        print("✅ Chat Response:")
        print(f"  {response.choices[0].message.content}")
        print(f"\n  Usage:")
        print(f"    Prompt tokens: {response.usage.prompt_tokens}")
        print(f"    Completion tokens: {response.usage.completion_tokens}")
        print(f"    Total tokens: {response.usage.total_tokens}")
    except Exception as e:
        print(f"❌ Error: {e}")
    
    # Example 4: Chat with system message
    print("\n📋 Example 4: Chat with System Message")
    print("-" * 60)
    try:
        response = client.chat.completions.create(
            model="qwen2.5-0.5b-instruct",
            messages=[
                {"role": "system", "content": "You are a helpful coding assistant."},
                {"role": "user", "content": "Write a hello world in Python"}
            ],
            max_tokens=150,
            temperature=0.7
        )
        print("✅ Chat Response:")
        print(f"  {response.choices[0].message.content}")
    except Exception as e:
        print(f"❌ Error: {e}")
    
    # Example 5: Multi-turn conversation
    print("\n📋 Example 5: Multi-turn Conversation")
    print("-" * 60)
    try:
        messages = [
            {"role": "user", "content": "What is 2+2?"},
            {"role": "assistant", "content": "2+2 equals 4."},
            {"role": "user", "content": "What about 3+3?"}
        ]
        response = client.chat.completions.create(
            model="qwen2.5-0.5b-instruct",
            messages=messages,
            max_tokens=50
        )
        print("✅ Chat Response:")
        print(f"  {response.choices[0].message.content}")
    except Exception as e:
        print(f"❌ Error: {e}")
    
    # Example 6: Streaming (if supported)
    print("\n📋 Example 6: Streaming Response (Future Feature)")
    print("-" * 60)
    print("⏭️  Streaming support coming in Task 14!")
    print("   See NEXT_TASKS.md for details")
    
    print("\n" + "=" * 60)
    print("✨ All examples completed!")
    print("\n💡 Tips:")
    print("  - Use RUST_LOG=debug for detailed gateway logs")
    print("  - Check metrics: curl http://localhost:8080/metrics")
    print("  - See PROXY.md for more examples")

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n\n👋 Goodbye!")
        sys.exit(0)
