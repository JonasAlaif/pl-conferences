#!/bin/bash

# Asks the LLM a question answered in 'Yes' or 'No', the initial question should be more open ended.

SEP=$'\n---\n'
SCRIPT_DIR=$(dirname "$0" | xargs realpath)

ANSWER=$($SCRIPT_DIR/qwen/run.sh "$1" "$2 Answer in one sentence." "$5" sentence)

CONTEXT="$1${SEP}Query: $2${SEP}Response: $ANSWER"
YES_NO_Q="Given the query and response above, answer with 'Yes' $3, or 'No' $4. No full sentence, one word answer."
$SCRIPT_DIR/qwen/run.sh "$CONTEXT" "$YES_NO_Q" "$5" curt
