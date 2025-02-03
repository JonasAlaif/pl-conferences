#!/bin/bash

# Installs Ollama Modelfiles

export OLLAMA_NOHISTORY=true

cd "$(dirname "$0")"
ollama create -f Llama_curt llama:curt
ollama create -f Llama_full llama:full
# ollama create -f Deepseek_curt deepseek-r1:curt
# ollama create -f Deepseek_full deepseek-r1:full
