import unittest
from unittest.mock import patch

from mofa.utils.ai import conn


class FakeOpenAI:
    def __init__(self, api_key=None, base_url=None, **kwargs):
        self.api_key = api_key
        self.base_url = base_url
        self.kwargs = kwargs


class AIConnTests(unittest.TestCase):
    def test_create_openai_client_reads_env_at_call_time(self):
        with patch.object(conn, "OpenAI", FakeOpenAI), \
             patch.dict("os.environ", {"OPENAI_API_KEY": "call-time-key"}, clear=True):
            client = conn.create_openai_client(env_file=".env.missing")

        self.assertEqual(client.api_key, "call-time-key")

    def test_create_openai_client_prefers_explicit_api_key(self):
        with patch.object(conn, "OpenAI", FakeOpenAI), \
             patch.dict("os.environ", {"OPENAI_API_KEY": "env-key", "LLM_API_KEY": "llm-key"}, clear=True):
            client = conn.create_openai_client(api_key="explicit-key", env_file=".env.missing")

        self.assertEqual(client.api_key, "explicit-key")

    def test_create_openai_client_raises_if_no_key(self):
        with patch.object(conn, "OpenAI", FakeOpenAI), \
             patch.dict("os.environ", {}, clear=True):
            with self.assertRaises(RuntimeError):
                conn.create_openai_client(env_file=".env.missing")


if __name__ == "__main__":
    unittest.main()
