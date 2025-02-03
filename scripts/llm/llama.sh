#!/bin/bash

# Asks the LLM a question with a context

export OLLAMA_NOHISTORY=true

SEP=$'\n---\n'
QUERY="$1$SEP$2"
MODEL=${3:-curt}

echo -n -e "Query (without context): $2\nAnswer: " > /dev/stderr
OUTPUT=$(ollama run --nowordwrap "llama3.2:$MODEL" "$QUERY" 2> /dev/null)
if [ "$MODEL" == "curt" ]; then
    OUTPUT=$(echo "$OUTPUT" | sed 's/[.]$//')
fi
echo -e "$OUTPUT\n" > /dev/stderr
echo "$OUTPUT" | tr -d '\n'
