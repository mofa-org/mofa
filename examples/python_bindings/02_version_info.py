#!/usr/bin/env python3
"""
MoFA SDK Python Example - Version Information

This example demonstrates how to get SDK version information.
"""

import os
import sys

# Add the bindings directory to the path to import mofa module
# The bindings are generated at: crates/mofa-sdk/bindings/python/
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', '..', 'crates', 'mofa-sdk', 'bindings', 'python'))

from mofa import get_version, is_dora_available


def main():
    print("=" * 50)
    print("MoFA SDK Python Example - Version Information")
    print("=" * 50)
    print()

    # Get SDK version
    version = get_version()
    print(f"SDK Version: {version}")
    print()

    # Check Dora availability
    dora_available = is_dora_available()
    print(f"Dora Runtime Available: {dora_available}")
    print()

    if dora_available:
        print("Distributed dataflow features are enabled.")
    else:
        print("Distributed dataflow features are not enabled.")
        print("Rebuild with --features dora to enable Dora support.")


if __name__ == "__main__":
    main()
