#!/bin/bash

# Installs Ollama Modelfiles

export OLLAMA_NOHISTORY=true

cd "$(dirname "$0")"
ollama create -f Modelfile_curt llama3.1:curt
while [ $? -ne 0 ]; do
    ollama create -f Modelfile_curt llama3.1:curt
done
ollama create -f Modelfile_sentence llama3.1:sentence
while [ $? -ne 0 ]; do
    ollama create -f Modelfile_sentence llama3.1:sentence
done
ollama create -f Modelfile_paragraph llama3.1:paragraph
while [ $? -ne 0 ]; do
    ollama create -f Modelfile_paragraph llama3.1:paragraph
done
ollama create -f Modelfile_full llama3.1:full
while [ $? -ne 0 ]; do
    ollama create -f Modelfile_full llama3.1:full
done
