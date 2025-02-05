#!/bin/bash

# Asks the LLM a question with a context

export OLLAMA_NOHISTORY=true

SEP=$'\n---\n'
QUERY="$1$SEP$2"
VERSION=${3:-curt}

echo -n -e "[QUERY (without context)] $2\n[ANSWER] " > /dev/stderr
OUTPUT=$(ollama run --nowordwrap "deepseek-r1:$VERSION" "$QUERY" 2> /dev/null)
if [ "$VERSION" == "curt" ]; then
    OUTPUT=$(echo "$OUTPUT" | sed 's/[.]$//')
fi
echo "$OUTPUT" > /dev/stderr
echo "$OUTPUT" | sed '1,/<\/think>/d' | tr -d '\n'
