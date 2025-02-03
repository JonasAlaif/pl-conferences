#!/bin/bash

# Tests the LLM on a file

SCRIPT_DIR="$(dirname "$0" | xargs realpath)"
$SCRIPT_DIR/llm/llama.sh "$(cat "$1")" "$2" "$3" > /dev/null
# $SCRIPT_DIR/llm/deepseek.sh "$(cat "$1")" "$2" "$3" > /dev/null
