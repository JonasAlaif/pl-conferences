#!/bin/bash

# Converts a html webpage to text only (keeping only innerHTML text)

SCRIPT_DIR=$(dirname "$0")
$SCRIPT_DIR/html_tidy.sh | pandoc -f html -t plain
# $SCRIPT_DIR/html_tidy.sh | htmlq --text | perl -00 -pE 's/\n{3,}/\n\n/g'
