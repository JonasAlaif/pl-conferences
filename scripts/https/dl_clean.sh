#!/bin/bash

# Searches for and downloads webpage and saves it as a cleaned text file

SCRIPT_DIR=$(dirname "$0" | xargs realpath)

OUT="$1"
$SCRIPT_DIR/ddg_search.sh "$2" > "$OUT.html"
[ "$?" -eq 0 ] || exit 1
cat "$OUT.html" | $SCRIPT_DIR/html_text.sh > "$OUT.txt"
