"""Simple workflow that injects retrieved memories into an agent prompt."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Callable

from .memory_manager import MemoryManager, MemoryRecord


def build_prompt_with_memories(user_query: str, memories: list[MemoryRecord]) -> str:
    """Build a prompt that includes relevant retrieved memories."""
    if not memories:
        return f"User query: {user_query}"

    memory_lines = []
    for idx, memory in enumerate(memories, start=1):
        memory_lines.append(f"[{idx}] {memory.content}")

    memory_block = "\n".join(memory_lines)
    return (
        "Use the memories below when relevant.\n\n"
        f"Relevant memories:\n{memory_block}\n\n"
        f"User query: {user_query}"
    )


@dataclass
class AgentTurnResult:
    prompt: str
    response: str
    memory_id: str


class AgentMemoryWorkflow:
    """Orchestrates retrieval-augmented prompt building and persistence."""

    def __init__(self, memory: MemoryManager, responder: Callable[[str], str]) -> None:
        self.memory = memory
        self.responder = responder

    def handle_query(self, user_query: str, top_k: int = 5, session_id: str = "default") -> AgentTurnResult:
        relevant_memories = self.memory.retrieve(user_query, top_k=top_k)
        prompt = build_prompt_with_memories(user_query, relevant_memories)

        response = self.responder(prompt)

        memory_id = self.memory.store_interaction(
            user_query=user_query,
            agent_response=response,
            session_id=session_id,
            extra_metadata={"retrieved_count": len(relevant_memories)},
        )

        return AgentTurnResult(prompt=prompt, response=response, memory_id=memory_id)
