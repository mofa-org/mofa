"""Memory manager with semantic retrieval backed by Chroma DB."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable
from uuid import uuid4


EmbeddingFunction = Callable[[list[str]], list[list[float]]]


@dataclass
class MemoryRecord:
    """In-memory representation of a stored memory item."""

    memory_id: str
    content: str
    metadata: dict[str, Any]
    score: float | None = None


class MemoryManager:
    """Persistent memory manager for long-term agent context."""

    def __init__(
        self,
        persist_directory: str = ".mofa_memory_db",
        collection_name: str = "agent_memories",
        embedding_function: EmbeddingFunction | None = None,
    ) -> None:
        self.persist_directory = Path(persist_directory)
        self.persist_directory.mkdir(parents=True, exist_ok=True)

        try:
            import chromadb
        except ImportError as exc:
            raise ImportError(
                "chromadb is required for MemoryManager. Install dependencies from "
                "examples/agent_memory_python/requirements.txt"
            ) from exc

        if embedding_function is None:
            from chromadb.utils.embedding_functions import (
                SentenceTransformerEmbeddingFunction,
            )

            # Small default model keeps setup lightweight while enabling semantic search.
            embedding_function = SentenceTransformerEmbeddingFunction(
                model_name="all-MiniLM-L6-v2"
            )

        self._client = chromadb.PersistentClient(path=str(self.persist_directory))
        self._collection = self._client.get_or_create_collection(
            name=collection_name,
            embedding_function=embedding_function,
            metadata={"hnsw:space": "cosine"},
        )

    def store(self, content: str, metadata: dict[str, Any]) -> str:
        """Store text content with metadata and return memory ID."""
        if not content.strip():
            raise ValueError("content must not be empty")

        memory_id = str(uuid4())
        doc_metadata = {
            **metadata,
            "created_at": datetime.now(timezone.utc).isoformat(),
        }

        self._collection.add(documents=[content], metadatas=[doc_metadata], ids=[memory_id])
        return memory_id

    def retrieve(self, query: str, top_k: int = 5) -> list[MemoryRecord]:
        """Retrieve semantically similar memories for a query."""
        if not query.strip():
            return []

        result = self._collection.query(query_texts=[query], n_results=max(top_k, 1))

        ids = result.get("ids", [[]])[0]
        docs = result.get("documents", [[]])[0]
        metadatas = result.get("metadatas", [[]])[0]
        distances = result.get("distances", [[]])[0]

        memories: list[MemoryRecord] = []
        for memory_id, content, metadata, distance in zip(ids, docs, metadatas, distances):
            similarity = max(0.0, 1.0 - float(distance))
            memories.append(
                MemoryRecord(
                    memory_id=memory_id,
                    content=content,
                    metadata=metadata or {},
                    score=similarity,
                )
            )

        return memories

    def delete(self, memory_id: str) -> None:
        """Delete a memory by ID."""
        self._collection.delete(ids=[memory_id])

    def update(
        self,
        memory_id: str,
        *,
        content: str | None = None,
        metadata: dict[str, Any] | None = None,
    ) -> None:
        """Update a memory's content and/or metadata."""
        if content is None and metadata is None:
            raise ValueError("content or metadata must be provided")

        existing = self._collection.get(ids=[memory_id], include=["documents", "metadatas"])
        if not existing.get("ids"):
            raise KeyError(f"memory id not found: {memory_id}")

        existing_doc = existing["documents"][0] if existing.get("documents") else ""
        existing_meta = existing["metadatas"][0] if existing.get("metadatas") else {}

        new_doc = content if content is not None else existing_doc
        new_meta = {**existing_meta, **(metadata or {})}
        new_meta["updated_at"] = datetime.now(timezone.utc).isoformat()

        self._collection.update(ids=[memory_id], documents=[new_doc], metadatas=[new_meta])

    def store_interaction(
        self,
        user_query: str,
        agent_response: str,
        session_id: str = "default",
        extra_metadata: dict[str, Any] | None = None,
    ) -> str:
        """Store a user/assistant interaction as a single memory item."""
        memory_text = f"User: {user_query}\nAssistant: {agent_response}"
        metadata = {
            "type": "conversation",
            "session_id": session_id,
            **(extra_metadata or {}),
        }
        return self.store(memory_text, metadata)
