#!/bin/bash

# Searches a query on DuckDuckGo and prints the URL of the first result

# export USER_AGENT='Mozilla / 5.0 (X11; Linux x86_64; rv: 104.0) Gecko / 20100101 Firefox / 104.0'
JSON=$(ddgr --json --num 1 --expand "$1")
URL=$(echo "$JSON" | jq -r '.[0].url')
echo "$URL"
