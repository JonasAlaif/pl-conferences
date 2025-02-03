#!/bin/bash

# Runs the `date` command with all the arguments

if command -v gdate &> /dev/null; then
    gdate "$@"
else
    date "$@"
fi
