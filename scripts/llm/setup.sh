#!/bin/bash

# Installs Ollama Modelfiles

export OLLAMA_NOHISTORY=true

cd "$(dirname "$0")"
# llama/3.1/setup.sh
# llama/3.2/setup.sh
qwen/2.5/setup.sh

# ollama create -f Deepseek_curt deepseek-r1:curt
# ollama create -f Deepseek_full deepseek-r1:full
