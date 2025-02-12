#!/bin/bash

# Installs Ollama Modelfiles

export OLLAMA_NOHISTORY=true

cd "$(dirname "$0")"
ollama create -f Modelfile_curt qwen2.5:curt
while [ $? -ne 0 ]; do
    ollama create -f Modelfile_curt qwen2.5:curt
done
ollama create -f Modelfile_sentence qwen2.5:sentence
while [ $? -ne 0 ]; do
    ollama create -f Modelfile_sentence qwen2.5:sentence
done
ollama create -f Modelfile_paragraph qwen2.5:paragraph
while [ $? -ne 0 ]; do
    ollama create -f Modelfile_paragraph qwen2.5:paragraph
done
ollama create -f Modelfile_full qwen2.5:full
while [ $? -ne 0 ]; do
    ollama create -f Modelfile_full qwen2.5:full
done

ollama run --nowordwrap "qwen2.5:sentence" "Quick test. Say 'LLM here, I was setup successfully.'. Say only that sentence."
