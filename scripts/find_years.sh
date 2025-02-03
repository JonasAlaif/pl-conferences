#!/bin/bash

# Finds the years to do, starting from $1 checks if directories exist

FROM="$1"
TO=$(($(date +'%Y')+1))

for i in $(seq $FROM $TO); do
    [ -f "$i/deadlines.ics" ] && continue || true
    [ -d "$i" ] && rm -rf "$i" || true
    echo $i
done
