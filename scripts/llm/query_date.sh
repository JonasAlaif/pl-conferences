#!/bin/bash

# Asks the LLM a question about a date formatted in 'YYYY-MM-DD'

SEP=$'\n---\n'
SCRIPT_DIR=$(dirname "$0" | xargs realpath)

ANSWER=$($SCRIPT_DIR/qwen/run.sh "$1" "$2 Answer in one sentence." "$4" sentence)

CONTEXT="$1${SEP}Query: $2${SEP}Response: $ANSWER"
DATE_Q="Given the above response, format the $3 as 'YYYY-MM-DD'. No full sentence, do not write any other text in the answer. If the date was not found, answer 'No' only (one word)."
$SCRIPT_DIR/qwen/run.sh "$CONTEXT" "$DATE_Q" "$4" curt
