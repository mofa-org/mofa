#!/usr/bin/env python3
"""Test script for YOLO mode with agent reuse"""
import os
import sys
from pathlib import Path

# Add mofa to path
sys.path.insert(0, str(Path(__file__).parent))

from mofa.vibe.yolo_engine import YoloEngine
from dotenv import load_dotenv

# Load environment
load_dotenv()

# Get API key
api_key = os.getenv('OPENAI_API_KEY')
if not api_key:
    print("ERROR: OPENAI_API_KEY not found in environment")
    sys.exit(1)

# Test requirement that could reuse feet-to-meters
requirement = "Create a flow that converts feet to meters and then calculates the square of the result"

print(f"\nTesting YOLO mode with requirement:")
print(f"  {requirement}\n")

# Initialize YOLO engine
engine = YoloEngine(
    requirement=requirement,
    llm_model="gpt-4o-mini",
    api_key=api_key,
    agents_output="./agents",
    flows_output="./flows",
)

# Run YOLO generation
try:
    result = engine.run()

    if result:
        print(f"\n✓ SUCCESS!")
        print(f"  Flow path: {result['flow_path']}")
        print(f"  Agents: {', '.join(result['agents'])}")
    else:
        print("\n✗ FAILED")
        sys.exit(1)

except KeyboardInterrupt:
    print("\n\nTest interrupted")
    sys.exit(0)
except Exception as e:
    print(f"\n✗ ERROR: {e}")
    import traceback
    traceback.print_exc()
    sys.exit(1)
