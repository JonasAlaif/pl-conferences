#!/bin/bash

# Searches a query on DuckDuckGo and prints the URL of the first result

JSON="$(ddgr --json --num 1 --expand "$1")"
URL="$(echo "$JSON" | jq -r '.[0].url')"
echo "$URL"
