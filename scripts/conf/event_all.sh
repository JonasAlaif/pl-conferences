#!/bin/bash

# Downloads data for one conference

CONFERENCE=$1
INFO=$2
SCRIPT_DIR=$(realpath "$(dirname "$0")/..")

# Load the event info
EVENT=$(echo "$INFO" | jq -r '.name')
START_YEAR=$(echo "$INFO" | jq -r '.start')
if [ -z "$EVENT" ]; then
    EVENT=$CONFERENCE
    CONFERENCE=""
    CE=$EVENT
else
    CE="$CONFERENCE $EVENT"
fi
[ -d "$EVENT" ] || mkdir "$EVENT"
cd "$EVENT" || exit 1

TODO_YEARS=( $($SCRIPT_DIR/util/find_years.sh "$START_YEAR") )
echo "Downloading data for $CE for years ${TODO_YEARS[@]}"

for i in "${TODO_YEARS[@]}"; do
    [ -d "$i" ] && exit 1 || mkdir "$i"
    echo -e "======\nProcessing $CE $i\n======"
    CEY="$CE $i"

    # Download the CFPs
    $SCRIPT_DIR/https/dl_cfp.sh "$i/cfp" "$CONFERENCE" "$EVENT" "$i"
    if [ $? -ne 0 ]; then
        rm -rf "$i"
        continue
    fi
    CFP_WEBPAGE=$(cat "$i/cfp.txt")

    # Check page is not an error page
    CFP_ERROR_Q="Does the above webpage look like an error page (e.g. 404) or a normal page with information?"
    CFP_ERROR=$($SCRIPT_DIR/llm/query_yes_no.sh "$CFP_WEBPAGE" "$CFP_ERROR_Q" "if error" "otherwise" 2.5)
    [ -z "$CFP_ERROR" ] && echo "[ERROR] AI error, empty response" && exit 1
    if [ "$CFP_ERROR" == "Yes" ]; then
        echo "[ERROR] $CEY CFP error page"
        echo "[WEBPAGE]"
        echo "$CFP_WEBPAGE"
        rm -rf "$i"
        continue
    fi

    # Check we have the right thing
    CFP_CORRECT_Q="Does the above webpage look like a page about '$CEY'?"
    CFP_CORRECT=$($SCRIPT_DIR/llm/query_yes_no.sh "$CFP_WEBPAGE" "$CFP_CORRECT_Q" "if both the name and year are mentioned" "otherwise" 2.5)
    if [ "$CFP_CORRECT" == "No" ]; then
        echo "[ERROR] $CEY CFP bad page"
        echo "[WEBPAGE]"
        echo "$CFP_WEBPAGE"
        rm -rf "$i"
        continue
    fi

    $SCRIPT_DIR/conf/event_one.sh "$CONFERENCE" "$EVENT" "$i" "$CFP_WEBPAGE"
    if [ $? -ne 0 ]; then
        echo "[WEBPAGE]"
        echo "$CFP_WEBPAGE"
        rm -rf "$i"
        continue
    fi
done
