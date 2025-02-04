#!/bin/bash

# Searches for and downloads cfp from the web

SCRIPT_DIR=$(dirname "$0" | xargs realpath)

# Search for the call for papers webpage
QUERY="$1 $2 call for papers cfp"
CFP_URL=$($SCRIPT_DIR/ddg_search.sh "$QUERY")
echo "CFP $1 $2 URL: $CFP_URL (searched '$QUERY')" > /dev/stderr
if [ $? -ne 0 ] || [ "$CFP_URL" == "null" ]; then
    echo "[ERROR] $1 $2 CFP not found" > /dev/stderr
    exit 1
fi

CFP_WEBPAGE=$($SCRIPT_DIR/fetch.sh "$CFP_URL")
echo "CFP $1 $2 WEBPAGE: ${#CFP_WEBPAGE} chars" > /dev/stderr
if [ $? -ne 0 ]; then
    echo "[ERROR] $1 $2 download failed" > /dev/stderr
    exit 1
fi

echo "$CFP_WEBPAGE"
