#!/bin/bash

# Get the list of models
models=$(ollama list | awk 'NR>1 {print $1}')

# Loop through each model name and remove it
for model in $models; do
    ollama rm "$model"
done
