#!/usr/bin/env python3
"""
MoFA Session Management Example

Demonstrates how to create, manage, and persist conversation sessions
using the MoFA SDK from Python.
"""

import sys
import os

# Add the generated bindings to the path
bindings_path = os.path.join(
    os.path.dirname(__file__), "..", "..", "crates", "mofa-ffi", "bindings", "python"
)
sys.path.insert(0, bindings_path)

from mofa import SessionManager, Session


def main():
    print("=== MoFA Session Management ===\n")

    # --- In-memory session manager ---
    print("1. Creating in-memory session manager...")
    manager = SessionManager.new_in_memory()

    # Create a new session
    session = manager.get_or_create("conversation-1")
    print(f"   Session key: {session.get_key()}")
    print(f"   Is empty: {session.is_empty()}")

    # Add messages to the session
    session.add_message("user", "Hello, can you help me with Python?")
    session.add_message("assistant", "Of course! What would you like to know?")
    session.add_message("user", "How do I read a file?")
    session.add_message("assistant", "You can use open() with a with statement.")

    print(f"   Messages added: {session.message_count()}")

    # Retrieve history (most recent 3 messages)
    print("\n2. Getting recent history (last 3 messages):")
    history = session.get_history(3)
    for msg in history:
        print(f"   [{msg.role}] {msg.content}")

    # Save and reload
    print("\n3. Saving and reloading session...")
    manager.save_session(session)

    loaded = manager.get_session("conversation-1")
    if loaded:
        print(f"   Loaded session has {loaded.message_count()} messages")
    else:
        print("   Session not found!")

    # Session metadata
    print("\n4. Session metadata:")
    session.set_metadata("topic", '"Python basics"')
    session.set_metadata("rating", "5")
    topic = session.get_metadata("topic")
    print(f"   Topic: {topic}")
    print(f"   Missing key: {session.get_metadata('nonexistent')}")

    # List sessions
    print("\n5. Listing all sessions:")
    keys = manager.list_sessions()
    for key in keys:
        print(f"   - {key}")

    # Clear session
    print("\n6. Clearing session...")
    session.clear()
    print(f"   Messages after clear: {session.message_count()}")

    # Delete session
    deleted = manager.delete_session("conversation-1")
    print(f"   Deleted: {deleted}")

    # --- Standalone session (without manager) ---
    print("\n7. Creating standalone session:")
    standalone = Session("quick-chat")
    standalone.add_message("user", "Quick question!")
    standalone.add_message("assistant", "Sure, go ahead!")
    print(f"   Key: {standalone.get_key()}, Messages: {standalone.message_count()}")

    print("\nDone!")


if __name__ == "__main__":
    main()
