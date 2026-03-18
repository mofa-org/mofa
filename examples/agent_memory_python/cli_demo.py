"""Interactive CLI demo for the persistent memory module."""

from __future__ import annotations

import argparse

from memory_module import AgentMemoryWorkflow, MemoryManager


class CLIDemoResponder:
    """Example responder that echoes prompt context usage."""

    def __call__(self, prompt: str) -> str:
        if "Relevant memories:" in prompt:
            return "I used stored memories to answer your question more accurately."
        return "I do not have related memories yet, but I stored this interaction now."


def main() -> None:
    parser = argparse.ArgumentParser(description="MoFA persistent memory CLI demo")
    parser.add_argument("--db-path", default=".cli_memory_db", help="Path for persisted memory DB")
    parser.add_argument("--top-k", type=int, default=5, help="Number of memories to retrieve")
    args = parser.parse_args()

    memory = MemoryManager(persist_directory=args.db_path, collection_name="cli_agent")
    workflow = AgentMemoryWorkflow(memory=memory, responder=CLIDemoResponder())

    print("Type your message. Use /search <text>, /delete <id>, or /exit.")
    while True:
        user_input = input("you> ").strip()
        if not user_input:
            continue

        if user_input == "/exit":
            print("bye")
            break

        if user_input.startswith("/search "):
            query = user_input[len("/search ") :].strip()
            memories = memory.retrieve(query, top_k=args.top_k)
            if not memories:
                print("memory> no relevant memories")
                continue
            for item in memories:
                print(
                    "memory>",
                    f"id={item.memory_id}",
                    f"score={item.score:.3f}",
                    f"content={item.content}",
                )
            continue

        if user_input.startswith("/delete "):
            memory_id = user_input[len("/delete ") :].strip()
            memory.delete(memory_id)
            print(f"memory> deleted {memory_id}")
            continue

        turn = workflow.handle_query(user_input, top_k=args.top_k)
        print("agent>", turn.response)


if __name__ == "__main__":
    main()
