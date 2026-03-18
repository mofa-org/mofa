"""
Multi-agent coordination example using the memory module.

Shows how to use persistent memory in a two-agent collaboration scenario:
- Agent A: information coordinator (retrieves and summarizes memories)
- Agent B: decision maker (uses memory to make informed decisions)

Usage:
  OPENAI_API_KEY=sk-... python multi_agent_example.py
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from memory_module import AgentMemoryWorkflow, MemoryManager


@dataclass
class AgentRole:
    name: str
    system_prompt: str


class OpenAIResponder:
    """Responder that calls OpenAI with system context."""

    def __init__(self, role: AgentRole, api_key: str = ""):
        self.role = role
        self.api_key = api_key or os.getenv("OPENAI_API_KEY", "")

    def __call__(self, prompt: str) -> str:
        try:
            from openai import OpenAI
        except ImportError:
            raise ImportError("openai package is required. Install with: pip install openai")

        client = OpenAI(api_key=self.api_key)
        combined_prompt = f"{self.role.system_prompt}\n\n{prompt}"
        response = client.chat.completions.create(
            model="gpt-4o-mini",
            messages=[{"role": "user", "content": combined_prompt}],
            temperature=0.7,
            max_tokens=300,
        )
        return response.choices[0].message.content


def main() -> None:
    api_key = os.getenv("OPENAI_API_KEY", "")
    if not api_key:
        print("[ERROR] OPENAI_API_KEY environment variable must be set")
        return

    try:
        from openai import OpenAI
        OpenAI(api_key=api_key)
    except ImportError:
        print("[ERROR] openai not installed. Run: pip install openai")
        return

    coordinator_role = AgentRole(
        name="Coordinator",
        system_prompt=(
            "You are a helpful information coordinator. "
            "Your job is to retrieve relevant memories and summarize them for the team."
        ),
    )

    decision_role = AgentRole(
        name="DecisionMaker",
        system_prompt=(
            "You are a strategic decision maker. "
            "Based on memories and context, provide actionable recommendations."
        ),
    )

    coordinator_memory = MemoryManager(
        persist_directory=".coordinator_memory",
        collection_name="coordinator_agent",
    )
    decision_memory = MemoryManager(
        persist_directory=".decision_memory",
        collection_name="decision_agent",
    )

    coordinator = AgentMemoryWorkflow(
        memory=coordinator_memory,
        responder=OpenAIResponder(coordinator_role, api_key),
    )
    decision_maker = AgentMemoryWorkflow(
        memory=decision_memory,
        responder=OpenAIResponder(decision_role, api_key),
    )

    print("=== Multi-Agent Collaboration with Memory ===\n")
    print("Coordinator Agent and Decision Maker Agent collaborate using persistent memory.\n")

    scenarios = [
        "We need to improve customer satisfaction this quarter.",
        "What insights can we gather from past interactions?",
        "Based on that, what actions should we prioritize?",
    ]

    for scenario in scenarios:
        print(f"\n>>> Scenario: {scenario}\n")

        coord_turn = coordinator.handle_query(scenario, top_k=3)
        print(f"Coordinator: {coord_turn.response}\n")

        decision_turn = decision_maker.handle_query(
            f"Based on this context: {coord_turn.response}\n\nWhat should we do?",
            top_k=3,
        )
        print(f"Decision Maker: {decision_turn.response}\n")


if __name__ == "__main__":
    main()
