#!/usr/bin/env python3
"""
MoFA SDK Python Example - Basic LLM Agent

This example demonstrates basic LLM agent usage including:
- Creating an agent with the builder pattern
- Simple Q&A (ask)
- Multi-turn chat
- Getting conversation history
"""

import os
import sys

# Add the bindings directory to the path to import mofa module
# The bindings are generated at: crates/mofa-sdk/bindings/python/
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', '..', 'crates', 'mofa-sdk', 'bindings', 'python'))

from mofa import (
    LLMAgentBuilder,
    MoFaError
)


def main():
    print("=" * 50)
    print("MoFA SDK Python Example - Basic LLM Agent")
    print("=" * 50)
    print()

    # Check for API key
    api_key = os.environ.get("OPENAI_API_KEY")
    if not api_key:
        print("Error: OPENAI_API_KEY environment variable not set")
        print("Set it with: export OPENAI_API_KEY=your-key-here")
        sys.exit(1)

    try:
        # Create an LLM agent using the builder pattern
        print("1. Creating LLM Agent...")
        builder = LLMAgentBuilder.create()
        builder = builder.set_id("my-agent")
        builder = builder.set_name("Python Agent")
        builder = builder.set_system_prompt("You are a helpful assistant.")
        builder = builder.set_temperature(0.7)
        builder = builder.set_max_tokens(1000)
        builder = builder.set_openai_provider(
            api_key,
            base_url=os.environ.get("OPENAI_BASE_URL"),
            model=os.environ.get("OPENAI_MODEL", "gpt-3.5-turbo")
        )

        agent = builder.build()
        print(f"   Agent created: ID={agent.agent_id()}, Name={agent.name()}")
        print()

        # Simple Q&A (no context retention)
        print("2. Simple Q&A (ask)...")
        question = "What is Rust?"
        answer = agent.ask(question)
        print(f"   Q: {question}")
        print(f"   A: {answer}")
        print()

        # Multi-turn chat (with context retention)
        print("3. Multi-turn chat...")
        messages = [
            "My favorite color is blue.",
            "What did I just tell you?",
        ]
        for msg in messages:
            response = agent.chat(msg)
            print(f"   User: {msg}")
            print(f"   Agent: {response}")
            print()

        # Get conversation history
        print("4. Conversation history...")
        history = agent.get_history()
        print(f"   Total messages: {len(history)}")
        for i, msg in enumerate(history):
            print(f"   [{i+1}] {msg.role.name}: {msg.content[:50]}...")
        print()

        # Clear history
        print("5. Clearing history...")
        agent.clear_history()
        history = agent.get_history()
        print(f"   History after clear: {len(history)} messages")
        print()

    except MoFaError as e:
        print(f"Error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
