#!/bin/bash

# Installs Ollama Modelfiles

export OLLAMA_NOHISTORY=true

cd "$(dirname "$0")"
ollama create -f Modelfile_curt llama3.2:curt
while [ $? -ne 0 ]; do
    ollama create -f Modelfile_curt llama3.2:curt
done
ollama create -f Modelfile_sentence llama3.2:sentence
while [ $? -ne 0 ]; do
    ollama create -f Modelfile_sentence llama3.2:sentence
done
ollama create -f Modelfile_paragraph llama3.2:paragraph
while [ $? -ne 0 ]; do
    ollama create -f Modelfile_paragraph llama3.2:paragraph
done
ollama create -f Modelfile_full llama3.2:full
while [ $? -ne 0 ]; do
    ollama create -f Modelfile_full llama3.2:full
done
