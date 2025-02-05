#!/bin/bash

# Downloads CFP data for one conference year

CONFERENCE=$1
EVENT=$2
i=$3
CFP_WEBPAGE=$(cat index.txt)

SEP=$'\n---\n'
SCRIPT_DIR=$(realpath "$(dirname "$0")/../..")

if [ -z "$CONFERENCE" ]; then
    CEY="$EVENT $i"
else
    CEY="$CONFERENCE $EVENT $i"
fi

# Check we have the right thing
CFP_CORRECT_Q="Does the above webpage look like a page about '$CEY'?"
CFP_CORRECT=$($SCRIPT_DIR/llm/query_yes_no.sh "$CFP_WEBPAGE" "$CFP_CORRECT_Q" "if both the name and year are mentioned" "otherwise" 2.5)
[ -z "$CFP_CORRECT" ] && echo "[ERROR] AI error, empty response" && exit 1
if [ "$CFP_CORRECT" == "No" ]; then
    echo "[ERROR] $CEY CFP bad page"
    exit 1
fi

# Find all important dates as a table
CFP_TABLE_Q="The page above is the call for papers of $CEY. In this page, find all dates and date ranges and list them in a table. The table has two columns: one for the event description and one for the date. If there are two rounds include dates for both. Answer with table only, no extra text."
CFP_TABLE=$($SCRIPT_DIR/llm/qwen/run.sh "$CFP_WEBPAGE" "$CFP_TABLE_Q" 2.5 paragraph)
echo "$CFP_TABLE" > "dates.txt"

# Check if there are two rounds
TWO_ROUNDS_Q="The page above is the call for papers of $CEY. Below the page the table above contains all important dates for $CEY. In this page and table, identify how many opportunities there are for submitting a new paper to $CEY. A **new paper submission deadline** is a date when entirely new papers are submitted for review. Do **not** count deadlines for revised submissions, camera-ready versions, rebuttals, or artifacts. Explain your reasoning clearly.  Explain your reasoning."
TWO_ROUNDS=$($SCRIPT_DIR/llm/query_yes_no.sh "$CFP_WEBPAGE$SEP$CFP_TABLE" "$TWO_ROUNDS_Q" "if there are two new paper submission deadlines" "otherwise" 2.5)

echo "."

if [ "$TWO_ROUNDS" == "Yes" ]; then
    $SCRIPT_DIR/conf/dates/two_rounds.sh "$CONFERENCE" "$EVENT" "$i" "$CFP_WEBPAGE" "$CFP_TABLE"
else
    $SCRIPT_DIR/conf/dates/one_round.sh "$CONFERENCE" "$EVENT" "$i" "$CFP_WEBPAGE" "$CFP_TABLE"
fi
