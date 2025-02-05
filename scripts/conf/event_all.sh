#!/bin/bash

# Downloads data for one conference

CONFERENCE=$1
INFO=$2
SCRIPT_DIR=$(realpath "$(dirname "$0")/..")

# Load the event info
EVENT=$(echo "$INFO" | jq -r '.name')
START_YEAR=$(echo "$INFO" | jq -r '.start')
VOLUNTEER=$(echo "$INFO" | jq -r '.volunteer')
if [ -z "$EVENT" ]; then
    EVENT=$CONFERENCE
    CONFERENCE=""
    CE=$EVENT
else
    CE="$CONFERENCE $EVENT"
fi
[ -d "$EVENT" ] || mkdir "$EVENT"
cd "$EVENT" || exit 1
ROOT=$(pwd)

TODO_YEARS=( $($SCRIPT_DIR/util/find_years.sh "$START_YEAR" "cfp") )
echo -e "======\n======\nFinding CFP data for $CE for years ${TODO_YEARS[@]}\n======\n======"

for i in "${TODO_YEARS[@]}"; do
    cd "$ROOT"
    [ -d "$i/cfp" ] && exit 1 || mkdir -p "$i/cfp"
    cd "$i/cfp" || exit 1

    CEY="$CE $i"
    echo "______ Processing $CEY ______"

    # Download the CFPs
    $SCRIPT_DIR/https/dl_clean.sh "index" "$CEY call for papers cfp"
    if [ $? -ne 0 ]; then
        cd "$ROOT"
        rm -rf "$i/cfp"
        continue
    fi

    # Collect events from CFP page
    $SCRIPT_DIR/conf/dates/collect.sh "$CONFERENCE" "$EVENT" "$i"
    if [ $? -ne 0 ]; then
        echo "[WEBPAGE]"
        echo "$(cat index.txt)"
        cd "$ROOT"
        rm -rf "$i/cfp"
        continue
    fi
done

# Exit if volunteering not possible
[ "$VOLUNTEER" != "true" ] && exit 0 || true

cd "$ROOT"
TODO_YEARS=( $($SCRIPT_DIR/util/find_years.sh "$START_YEAR" "volunteer") )
echo -e "======\n======\nFinding volunteer data for $CE for years ${TODO_YEARS[@]}\n======\n======"

for i in "${TODO_YEARS[@]}"; do
    cd "$ROOT"
    [ -d "$i/volunteer" ] && exit 1 || mkdir -p "$i/volunteer"
    cd "$i/volunteer" || exit 1

    CEY="$CE $i"
    echo "______ Processing $CEY ______"

    # Download the student volunteers page
    $SCRIPT_DIR/https/dl_clean.sh "index" "$CEY student volunteer apply"
    if [ $? -ne 0 ]; then
        cd "$ROOT"
        rm -rf "$i/volunteer"
        continue
    fi

    # Collect events from student volunteer page
    $SCRIPT_DIR/conf/dates/volunteer.sh "$CONFERENCE" "$EVENT" "$i"
    if [ $? -ne 0 ]; then
        echo "[WEBPAGE]"
        echo "$(cat index.txt)"
        cd "$ROOT"
        rm -rf "$i/volunteer"
        continue
    fi
done
