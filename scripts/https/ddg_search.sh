#!/bin/bash

# Searches a query on DuckDuckGo and prints the WEBPAGE of the first result

# export USER_AGENT='Mozilla / 5.0 (X11; Linux x86_64; rv: 104.0) Gecko / 20100101 Firefox / 104.0'
JSON=$(ddgr --json --num 1 --expand "$1")
if [ $? -ne 0 ] || [ "$JSON" == "[]" ]; then
    echo "[ERROR] ddgr failed ($JSON)" > /dev/stderr
    exit 1
fi

URL=$(echo "$JSON" | jq -r '.[0].url')
WEBPAGE=$(wget "$URL" --no-check-certificate --quiet -O -)
if [ $? -ne 0 ]; then
    echo "[ERROR] wget failed ($URL -> $WEBPAGE)" > /dev/stderr
    exit 1
fi

echo "[DDG] Search '$1' -> URL '$URL' -> WEBPAGE '${#WEBPAGE} chars'" > /dev/stderr
echo "$WEBPAGE"
