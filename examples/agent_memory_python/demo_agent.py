"""Minimal example showing memory retrieval across two interactions."""

from __future__ import annotations

from memory_module import AgentMemoryWorkflow, MemoryManager


class DemoResponder:
    """Simple responder used for demonstration without external LLM calls."""

    def __call__(self, prompt: str) -> str:
        lowered = prompt.lower()
        if "tea" in lowered:
            return "You mentioned tea preferences earlier, so I recommend jasmine tea."
        return "I can help with that."


def run_demo() -> None:
    memory = MemoryManager(persist_directory=".demo_memory_db", collection_name="demo_agent")
    workflow = AgentMemoryWorkflow(memory=memory, responder=DemoResponder())

    first_query = "My favorite drink is jasmine tea."
    first_turn = workflow.handle_query(first_query)
    print("First response:", first_turn.response)

    second_query = "What drink should I have today?"
    second_turn = workflow.handle_query(second_query)
    print("Second response:", second_turn.response)

    print("\nPrompt used in second turn:\n")
    print(second_turn.prompt)


if __name__ == "__main__":
    run_demo()
