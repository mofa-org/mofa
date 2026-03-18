import tempfile
import unittest
from pathlib import Path
import sys


EXAMPLE_DIR = Path(__file__).resolve().parents[2] / "examples" / "agent_memory_python"
if str(EXAMPLE_DIR) not in sys.path:
    sys.path.insert(0, str(EXAMPLE_DIR))


try:
    from memory_module.memory_manager import MemoryManager
except ImportError:
    MemoryManager = None


class DummyEmbeddingFunction:
    """Deterministic, local embedding for unit tests."""

    @staticmethod
    def name():
        return "dummy-test-embedding"

    @classmethod
    def build_from_config(cls, config):
        return cls()

    def is_legacy(self):
        return False

    def supported_spaces(self):
        return ["cosine"]

    def get_config(self):
        return {"name": self.name(), "type": "test"}

    def embed_documents(self, input):
        return self.__call__(input)

    def embed_query(self, input):
        return self.__call__(input)

    def __call__(self, input):
        vectors = []
        for text in input:
            lowered = text.lower()
            vectors.append(
                [
                    float(lowered.count("tea")),
                    float(lowered.count("coffee")),
                    float(lowered.count("weather")),
                    float(len(lowered) / 100.0),
                ]
            )
        return vectors


@unittest.skipIf(MemoryManager is None, "MemoryManager dependencies are not installed")
class MemoryManagerTests(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.memory = MemoryManager(
            persist_directory=self.temp_dir.name,
            collection_name="test_memories",
            embedding_function=DummyEmbeddingFunction(),
        )

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_store_and_retrieve_returns_semantic_match(self):
        self.memory.store("User likes jasmine tea", {"topic": "preference"})
        self.memory.store("Weather in Berlin is cloudy", {"topic": "weather"})

        results = self.memory.retrieve("Any tea suggestions?", top_k=2)

        self.assertGreaterEqual(len(results), 1)
        self.assertIn("tea", results[0].content.lower())

    def test_delete_removes_memory(self):
        memory_id = self.memory.store("This should be deleted", {"topic": "temp"})
        self.memory.delete(memory_id)

        results = self.memory.retrieve("deleted", top_k=5)
        ids = {item.memory_id for item in results}
        self.assertNotIn(memory_id, ids)

    def test_update_changes_stored_memory(self):
        memory_id = self.memory.store("User likes tea", {"topic": "drink"})

        self.memory.update(memory_id, content="User likes coffee", metadata={"source": "update"})

        results = self.memory.retrieve("coffee", top_k=5)
        matched = [item for item in results if item.memory_id == memory_id]

        self.assertTrue(matched)
        self.assertIn("coffee", matched[0].content.lower())
        self.assertEqual(matched[0].metadata.get("source"), "update")


if __name__ == "__main__":
    unittest.main()
