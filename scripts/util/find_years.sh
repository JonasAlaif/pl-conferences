#!/bin/bash

# Finds the years to do, starting from $1 checks if directories exist

FROM=$1
CAT=$2
TO=$(($(date +'%Y')+1))

for i in $(seq $FROM $TO); do
    [ -f "$i/$CAT/cal.ics" ] && continue || true
    [ -d "$i/$CAT" ] && rm -rf "$i/$CAT" || true
    echo $i
done
