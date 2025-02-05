#!/bin/bash

# Searches for and downloads cfp from the web

SCRIPT_DIR=$(dirname "$0" | xargs realpath)

OUT="$1"
if [ -z "$2" ]; then
    CEY="$3 $4"
else
    CEY="$2 $3 $4"
fi

# Search for the call for papers webpage
QUERY="$CEY call for papers cfp"
$SCRIPT_DIR/ddg_search.sh "$QUERY" > "$OUT.html"
[ "$?" -eq 0 ] || exit 1
cat "$OUT.html" | $SCRIPT_DIR/html_text.sh > "$OUT.txt"
