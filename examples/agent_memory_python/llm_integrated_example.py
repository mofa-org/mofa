"""
Example showing the memory module integrated with OpenAI LLM.

This example demonstrates how to use the persistent memory module
in a real agent workflow that calls an LLM responder.

Usage:
  OPENAI_API_KEY=sk-... python llm_integrated_example.py
"""

from __future__ import annotations

import os
from memory_module import AgentMemoryWorkflow, MemoryManager


class LLMResponder:
    """Responder that calls OpenAI API to generate responses."""

    def __init__(self, api_key: str = ""):
        self.api_key = api_key or os.getenv("OPENAI_API_KEY", "")
        if not self.api_key:
            raise ValueError(
                "OPENAI_API_KEY environment variable must be set or passed to LLMResponder"
            )

    def __call__(self, prompt: str) -> str:
        try:
            from openai import OpenAI
        except ImportError:
            raise ImportError(
                "openai package is required. Install with: pip install openai"
            )

        client = OpenAI(api_key=self.api_key)
        response = client.chat.completions.create(
            model="gpt-4o-mini",
            messages=[{"role": "user", "content": prompt}],
            temperature=0.7,
            max_tokens=500,
        )
        return response.choices[0].message.content


def main() -> None:
    try:
        responder = LLMResponder()
    except ImportError:
        print("[ERROR] openai not installed. Run: pip install openai")
        return
    except ValueError as exc:
        print(f"[ERROR] {exc}")
        return

    memory = MemoryManager(
        persist_directory=".llm_memory_db",
        collection_name="llm_agent",
    )
    workflow = AgentMemoryWorkflow(memory=memory, responder=responder)

    print("=== Memory-Augmented LLM Agent ===\n")
    print("Type your messages. Use /exit to quit.\n")

    while True:
        user_input = input("you> ").strip()
        if not user_input:
            continue

        if user_input == "/exit":
            print("bye")
            break

        try:
            turn = workflow.handle_query(user_input, top_k=3)
            print(f"agent> {turn.response}\n")
        except Exception as err:
            print(f"[ERROR] {err}\n")


if __name__ == "__main__":
    main()
