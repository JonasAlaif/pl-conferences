#!/bin/bash
export OLLAMA_NOHISTORY=true

cd "$(dirname "$0")"

WEBSITE=$1 # e.g. https://conferences.i-cav.org/2025/
QUESTION=${@:2} # e.g. What is the paper sumbission deadline of the above conference? Answer with 'DD/MM/YYYY' only.
# wget "$WEBSITE" -q -O - | tidy -w 0 -q --show-warnings false > website.html
# readable -s readable -q "$WEBSITE" | tidy -w 0 -q --show-warnings false  > website.html
cat <(cat website.html) <(echo -e "\n---\n$QUESTION") | (tee /dev/stderr && echo -e "\n" > /dev/stderr) | ollama run --nowordwrap llama3.2:1b 2> /dev/null | sed 's/[.]$//' | tee /dev/stderr | tr -d '\n'

# What city is the 2026 conference? Answer with city name only, no full sentence. One word answer.
# What city and world country is the 2026 conference? Answer with 'city, world country' only, no full sentence. Two word answer.

# What is the URL for the 2025 conference? Answer with URL only, no full sentence.

# The above html is the list of all years of the conference, ignore the readability links. What is the homepage URL (href) of the 2024 POPL conference? Answer with https URL only, no full sentence.
