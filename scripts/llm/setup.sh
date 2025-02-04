#!/bin/bash

# Installs Ollama Modelfiles

export OLLAMA_NOHISTORY=true

cd "$(dirname "$0")"
ollama create -f Llama_curt llama:curt
while [ $? -ne 0 ]; do
    ollama create -f Llama_curt llama:curt
done
ollama create -f Llama_full llama:full
while [ $? -ne 0 ]; do
    ollama create -f Llama_full llama:full
done

# ollama create -f Deepseek_curt deepseek-r1:curt
# ollama create -f Deepseek_full deepseek-r1:full
