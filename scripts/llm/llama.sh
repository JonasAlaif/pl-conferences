#!/bin/bash

# Asks the LLM a question with a context

export OLLAMA_NOHISTORY=true

SEP=$'\n---\n'
QUERY="$1$SEP$2"
VERSION=${3:-curt}

echo -n -e "[QUERY (without context)] $2\n[ANSWER] " > /dev/stderr
OUTPUT=$(ollama run --nowordwrap "llama:$VERSION" "$QUERY" 2> /dev/null)
if [ "$VERSION" == "curt" ]; then
    OUTPUT=$(echo "$OUTPUT" | sed 's/[.]$//')
fi
echo -e "$OUTPUT\n" > /dev/stderr
echo "$OUTPUT" | tr -d '\n'
