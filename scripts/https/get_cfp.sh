#!/bin/bash

# Searches for and downloads cfp from the web

SCRIPT_DIR=$(dirname "$0" | xargs realpath)

# Search for the call for papers webpage
QUERY="$1 $2 call for papers cfp"
CFP_URL=$($SCRIPT_DIR/ddg_search.sh "$QUERY")
echo "CFP $1 $2 URL: $CFP_URL (searched '$QUERY')"
if [ $? -ne 0 ] || [ "$CFP_URL" == "null" ]; then
    echo "[ERROR] $1 $2 CFP not found"
    exit 0
fi

CFP_WEBPAGE=$($SCRIPT_DIR/fetch.sh "$CFP_URL")
echo "CFP $1 $2 WEBPAGE: ${#CFP_WEBPAGE} chars"
if [ $? -ne 0 ]; then
    echo "[ERROR] $1 $2 download failed"
    exit 0
fi

mkdir "$2"
echo "$CFP_WEBPAGE" > "$2/cfp.html"
