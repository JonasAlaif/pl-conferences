#!/bin/bash

# Asks the LLM a question with a context

export OLLAMA_NOHISTORY=true

SEP=$'\n---\n'
QUERY="$1$SEP$2"

VERSION=${3:-3.1}
LENGTH=${4:-curt}

echo -n -e "[QUERY, no context] $2\n[RESPONSE] " > /dev/stderr
# Space before $QUERY, otherwise if query starts with a '-' it is parsed as a flag...
OUTPUT=$(ollama run --nowordwrap "qwen$VERSION:$LENGTH" " $QUERY" 2> /dev/null)
if [ "$LENGTH" == "curt" ]; then
    OUTPUT=$(echo "$OUTPUT" | sed 's/[.]$//')
fi
echo "$OUTPUT" > /dev/stderr
echo "$OUTPUT"
