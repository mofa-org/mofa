#!/usr/bin/env bash

# Helper script to launch voice-output and script-segmenter together
# so you can generate a podcast with a single command.
# Assumes the dataflow (static nodes) is already running via
# `mofa run-flow dataflow-minimax.yml` or `dora start dataflow-minimax.yml`.

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

default_script="scripts/example_podcast.md"
read -r -p "输入脚本 Markdown 路径 [${default_script}]: " script_path
script_path="${script_path:-$default_script}"

if [[ ! -f "$script_path" ]]; then
  echo "❌ 找不到脚本文件: $script_path"
  exit 1
fi

default_output="out/podcast_output.wav"
read -r -p "输出 WAV 路径 [${default_output}]: " output_path
output_path="${output_path:-$default_output}"

# Resolve absolute paths (兼容绝对/相对路径)
abs_script=$(python3 -c 'import os,sys; print(os.path.abspath(sys.argv[1]))' "$script_path")
abs_output=$(python3 -c 'import os,sys; print(os.path.abspath(sys.argv[1]))' "$output_path")

mkdir -p "$(dirname "$abs_output")"

echo "📄 脚本: $abs_script"
echo "🔊 输出: $abs_output"
echo ""

if command -v voice-output >/dev/null 2>&1 && command -v script-segmenter >/dev/null 2>&1; then
  VOICE_CMD="$(command -v voice-output)"
  SEGMENTER_CMD="$(command -v script-segmenter)"
else
  VENV_DIR=$(ls -dt mofa_run_*/venv 2>/dev/null | head -n 1 || true)
  if [[ -z "$VENV_DIR" ]]; then
    echo "❌ 未找到可用的 voice-output/script-segmenter。请先运行 mofa run-flow 初始化环境。"
    exit 1
  fi
  VOICE_CMD="$(pwd)/$VENV_DIR/bin/voice-output"
  SEGMENTER_CMD="$(pwd)/$VENV_DIR/bin/script-segmenter"
  if [[ ! -x "$VOICE_CMD" || ! -x "$SEGMENTER_CMD" ]]; then
    echo "❌ 无法在 $VENV_DIR 中找到所需命令，请确认 mofa run-flow 已成功安装动态节点。"
    exit 1
  fi
fi

"$VOICE_CMD" --output-file "$abs_output" &
voice_pid=$!

cleanup() {
  if ps -p $voice_pid > /dev/null 2>&1; then
    kill $voice_pid || true
  fi
}
trap cleanup EXIT

echo "▶️  已启动 voice-output (PID $voice_pid)"
sleep 1

"$SEGMENTER_CMD" --input-file "$abs_script"

wait $voice_pid || true

echo "✅ 播客生成完成：$abs_output"
trap - EXIT
