#!/bin/bash

# Tests the LLM on a file

SCRIPT_DIR="$(dirname "$0" | xargs realpath)"
$SCRIPT_DIR/llm/qwen/run.sh "$(cat "$1")" "$2" "$3" "$4" > /dev/null
# $SCRIPT_DIR/llm/deepseek.sh "$(cat "$1")" "$2" "$3" > /dev/null
