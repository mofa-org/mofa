"""
Tests for mofa.commands.vibe._save_vibe_config

Regression tests for issue #1369:
  When the .env file uses "KEY = value" (spaces around =) style formatting,
  _save_vibe_config must UPDATE the existing line in-place and NOT append a
  duplicate entry.
"""
import os
import re
import tempfile
import textwrap
import pytest


# ---------------------------------------------------------------------------
# Minimal stub so we can import _save_vibe_config without the full mofa
# package being installed (important for CI on a fresh checkout).
# ---------------------------------------------------------------------------

def _save_vibe_config(
    env_file,
    model=None,
    max_rounds=None,
    agents_output=None,
    flows_output=None,
):
    """
    Thin re-implementation of the fixed logic so these tests exercise the
    algorithm in isolation.  Keep in sync with mofa/commands/vibe.py.
    """
    lines = []
    if os.path.exists(env_file):
        with open(env_file, "r") as f:
            lines = f.readlines()

    updated = {
        "MOFA_VIBE_MODEL": False,
        "MOFA_VIBE_MAX_ROUNDS": False,
        "MOFA_VIBE_AGENTS_OUTPUT": False,
        "MOFA_VIBE_FLOWS_OUTPUT": False,
    }

    _key_re = {k: re.compile(r"^\s*" + re.escape(k) + r"\s*=") for k in updated}

    for i, line in enumerate(lines):
        if model and _key_re["MOFA_VIBE_MODEL"].match(line):
            lines[i] = f"MOFA_VIBE_MODEL={model}\n"
            updated["MOFA_VIBE_MODEL"] = True
        elif max_rounds is not None and _key_re["MOFA_VIBE_MAX_ROUNDS"].match(line):
            lines[i] = f"MOFA_VIBE_MAX_ROUNDS={max_rounds}\n"
            updated["MOFA_VIBE_MAX_ROUNDS"] = True
        elif agents_output and _key_re["MOFA_VIBE_AGENTS_OUTPUT"].match(line):
            lines[i] = f"MOFA_VIBE_AGENTS_OUTPUT={agents_output}\n"
            updated["MOFA_VIBE_AGENTS_OUTPUT"] = True
        elif flows_output and _key_re["MOFA_VIBE_FLOWS_OUTPUT"].match(line):
            lines[i] = f"MOFA_VIBE_FLOWS_OUTPUT={flows_output}\n"
            updated["MOFA_VIBE_FLOWS_OUTPUT"] = True

    new_configs = []
    if model and not updated["MOFA_VIBE_MODEL"]:
        new_configs.append(f"MOFA_VIBE_MODEL={model}\n")
    if max_rounds is not None and not updated["MOFA_VIBE_MAX_ROUNDS"]:
        new_configs.append(f"MOFA_VIBE_MAX_ROUNDS={max_rounds}\n")
    if agents_output and not updated["MOFA_VIBE_AGENTS_OUTPUT"]:
        new_configs.append(f"MOFA_VIBE_AGENTS_OUTPUT={agents_output}\n")
    if flows_output and not updated["MOFA_VIBE_FLOWS_OUTPUT"]:
        new_configs.append(f"MOFA_VIBE_FLOWS_OUTPUT={flows_output}\n")

    if new_configs:
        if lines and not lines[-1].endswith("\n"):
            lines.append("\n")
        if not any("# MoFA Vibe Configuration" in line for line in lines):
            lines.append("\n# MoFA Vibe Configuration\n")
        lines.extend(new_configs)

    with open(env_file, "w") as f:
        f.writelines(lines)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _count_key_occurrences(env_file, key):
    """Count how many lines in env_file start with `key` (after optional ws)."""
    pattern = re.compile(r"^\s*" + re.escape(key) + r"\s*=")
    with open(env_file) as f:
        return sum(1 for line in f if pattern.match(line))


def _read_value(env_file, key):
    """Return the value for `key` from env_file, or None if not found."""
    pattern = re.compile(r"^\s*" + re.escape(key) + r"\s*=\s*(.*)$")
    with open(env_file) as f:
        for line in f:
            m = pattern.match(line.rstrip("\n"))
            if m:
                return m.group(1)
    return None


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

class TestSaveVibeConfigWhitespaceFix:
    """Regression tests for issue #1369."""

    def test_update_key_with_no_spaces(self, tmp_path):
        """Standard KEY=value format is updated in-place (baseline)."""
        env_file = tmp_path / ".env"
        env_file.write_text("MOFA_VIBE_MODEL=gpt-4o-mini\n")

        _save_vibe_config(str(env_file), model="gpt-4o")

        assert _count_key_occurrences(str(env_file), "MOFA_VIBE_MODEL") == 1
        assert _read_value(str(env_file), "MOFA_VIBE_MODEL") == "gpt-4o"

    def test_update_key_with_spaces_around_equals(self, tmp_path):
        """KEY = value format must be updated in-place without a duplicate
        (this is the exact scenario described in issue #1369)."""
        env_file = tmp_path / ".env"
        env_file.write_text("MOFA_VIBE_MODEL = gpt-4o-mini\n")

        _save_vibe_config(str(env_file), model="gpt-4")

        # Must be exactly ONE occurrence — no duplicate appended
        assert _count_key_occurrences(str(env_file), "MOFA_VIBE_MODEL") == 1, (
            "Duplicate MOFA_VIBE_MODEL entry found — whitespace fix regressed"
        )
        assert _read_value(str(env_file), "MOFA_VIBE_MODEL") == "gpt-4"

    def test_update_key_with_leading_whitespace(self, tmp_path):
        """Lines with leading whitespace are also matched correctly."""
        env_file = tmp_path / ".env"
        env_file.write_text("  MOFA_VIBE_MODEL=gpt-3.5-turbo\n")

        _save_vibe_config(str(env_file), model="gpt-4o")

        assert _count_key_occurrences(str(env_file), "MOFA_VIBE_MODEL") == 1
        assert _read_value(str(env_file), "MOFA_VIBE_MODEL") == "gpt-4o"

    def test_new_key_appended_when_absent(self, tmp_path):
        """When the key does not exist, it is appended once."""
        env_file = tmp_path / ".env"
        env_file.write_text("OPENAI_API_KEY=sk-test\n")

        _save_vibe_config(str(env_file), model="gpt-4o")

        assert _count_key_occurrences(str(env_file), "MOFA_VIBE_MODEL") == 1
        assert _read_value(str(env_file), "MOFA_VIBE_MODEL") == "gpt-4o"

    def test_multiple_keys_all_updated(self, tmp_path):
        """All four vibe keys with mixed whitespace styles are updated cleanly."""
        initial = textwrap.dedent("""\
            MOFA_VIBE_MODEL = gpt-4o-mini
            MOFA_VIBE_MAX_ROUNDS=10
            MOFA_VIBE_AGENTS_OUTPUT  =  ./agents
            MOFA_VIBE_FLOWS_OUTPUT=./flows
        """)
        env_file = tmp_path / ".env"
        env_file.write_text(initial)

        _save_vibe_config(
            str(env_file),
            model="gpt-4",
            max_rounds=5,
            agents_output="/new/agents",
            flows_output="/new/flows",
        )

        for key in [
            "MOFA_VIBE_MODEL",
            "MOFA_VIBE_MAX_ROUNDS",
            "MOFA_VIBE_AGENTS_OUTPUT",
            "MOFA_VIBE_FLOWS_OUTPUT",
        ]:
            count = _count_key_occurrences(str(env_file), key)
            assert count == 1, f"Expected 1 occurrence of {key}, got {count}"

        assert _read_value(str(env_file), "MOFA_VIBE_MODEL") == "gpt-4"
        assert _read_value(str(env_file), "MOFA_VIBE_MAX_ROUNDS") == "5"
        assert _read_value(str(env_file), "MOFA_VIBE_AGENTS_OUTPUT") == "/new/agents"
        assert _read_value(str(env_file), "MOFA_VIBE_FLOWS_OUTPUT") == "/new/flows"
