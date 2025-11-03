#!/bin/bash
# Podcast Generator - Quick Launch Script
# This script helps launch all components in the correct order

set -e

echo "=========================================="
echo "Podcast Generator Launch Helper"
echo "=========================================="
echo ""

# Check if script file exists
SCRIPT_FILE="${1:-scripts/example_podcast.md}"
OUTPUT_FILE="${2:-out/podcast_output.wav}"
DATAFLOW_FILE="${3:-dataflow-minimax.yml}"

if [ ! -f "$SCRIPT_FILE" ]; then
    echo "❌ Error: Script file not found: $SCRIPT_FILE"
    echo ""
    echo "Usage: $0 [script.md] [output.wav] [dataflow.yml]"
    echo "Example: $0 scripts/example_podcast.md out/my_podcast.wav dataflow-minimax.yml"
    exit 1
fi

if [ ! -f "$DATAFLOW_FILE" ]; then
    echo "❌ Error: Dataflow file not found: $DATAFLOW_FILE"
    exit 1
fi

if [ -z "$MINIMAX_API_KEY" ]; then
    echo "⚠️  Warning: MINIMAX_API_KEY 未设置，MiniMax TTS 将无法工作。"
    echo "    请先执行: export MINIMAX_API_KEY=\"你的API Key\""
    echo ""
fi

echo "📄 Script: $SCRIPT_FILE"
echo "🔊 Output: $OUTPUT_FILE"
echo ""
echo "📜 Dataflow: $DATAFLOW_FILE"

# Check if dataflow is already running
if dora list 2>/dev/null | grep -q "podcast-generator\|dataflow"; then
    echo "⚠️  Warning: A dataflow might already be running."
    echo "   Stopping it first..."
    dora stop 2>/dev/null || true
    sleep 2
fi

echo "=========================================="
echo "Launch Instructions:"
echo "=========================================="
echo ""
echo "You need to run these commands in SEPARATE terminals:"
echo ""
echo "Terminal 1 (Dataflow):"
echo "  cd $(pwd)"
echo "  dora start $DATAFLOW_FILE"
echo ""
echo "Terminal 2 (Voice Output) - Launch IMMEDIATELY after Terminal 1:"
echo "  cd $(pwd)"
echo "  voice-output --output-file $OUTPUT_FILE"
echo ""
echo "Terminal 3 (Script Segmenter) - Launch IMMEDIATELY after Terminal 2:"
echo "  cd $(pwd)"
echo "  script-segmenter --input-file $SCRIPT_FILE"
echo ""
echo "Terminal 4 (Viewer - Optional):"
echo "  cd $(pwd)"
echo "  viewer"
echo ""
echo "=========================================="
echo "⏰ TIMING IS CRITICAL!"
echo "=========================================="
echo "The dynamic nodes (Terminal 2 & 3) must connect within a few seconds"
echo "of the dataflow starting, otherwise the dataflow will quit."
echo ""
echo "Press Ctrl+C in any terminal to stop."
echo ""
