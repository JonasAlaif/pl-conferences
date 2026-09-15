#!/bin/bash

# Downloads a webpage and asks a series of questions to the LLM

NEWLINE=$'\n'
SCRIPT_DIR="$(dirname "$0" | xargs realpath)"

WEBSITE_TEXT="$($SCRIPT_DIR/fetch.sh "$1")$NEWLINE---$NEWLINE"

for i in "${@:2}"; do
  LLM_OUTPUT="$($SCRIPT_DIR/llm.sh "$WEBSITE_TEXT$i")"
  echo "$LLM_OUTPUT"
done
