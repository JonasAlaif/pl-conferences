#!/bin/bash

# Downloads data for one conference

CONFERENCE=$1
SCRIPT_DIR=$(realpath "$(dirname "$0")/..")
cd "$SCRIPT_DIR/../conferences/$CONFERENCE" || exit 1

INFO=$(cat "info.json")
# If array run `event_all` on each element
if [ "$(echo "$INFO" | jq -r 'type')" == "array" ]; then
    LENGTH=$(echo "$INFO" | jq 'length')
    for i in $(seq 1 $LENGTH); do
        $SCRIPT_DIR/conf/event_all.sh "$CONFERENCE" "$(echo "$INFO" | jq -r ".[$i-1]")"
    done
else
    $SCRIPT_DIR/conf/event_all.sh "$CONFERENCE" "$INFO"
fi
