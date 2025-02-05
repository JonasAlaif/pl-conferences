#!/bin/bash

# Installs the required software

export OLLAMA_NOHISTORY=true

cd "$(dirname "$0")"

curl -fsSL https://ollama.com/install.sh | sh
# ollama pull llama3.2
llm/setup.sh

# brew install ddgr
sudo apt install ddgr tidy
sudo npm install -g readability-cli
cargo install htmlq
