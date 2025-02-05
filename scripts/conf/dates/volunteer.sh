#!/bin/bash

# Downloads volunteering data for one conference year

CONFERENCE=$1
EVENT=$2
i=$3
VOLUNTEER_WEBPAGE=$(cat index.txt)

im1=$((i-1))
SCRIPT_DIR=$(realpath "$(dirname "$0")/../..")

if [ -z "$CONFERENCE" ]; then
    CEY="$EVENT $i"
else
    CEY="$CONFERENCE $EVENT $i"
fi

# Check we have the right thing
VOLUNTEER_CORRECT_Q="Does the above webpage look like a page about volunteering to help with '$CEY'? If yes, are there details and a deadline for applying to volunteer?"
VOLUNTEER_CORRECT=$($SCRIPT_DIR/llm/query_yes_no.sh "$VOLUNTEER_WEBPAGE" "$VOLUNTEER_CORRECT_Q" "if page with details and a deadline for volunteering for '$CEY'" "otherwise" 2.5)
if [ "$VOLUNTEER_CORRECT" == "No" ]; then
    echo "[ERROR] $CEY volunteer bad page"
    exit 1
fi

PREFIX_Q="The page above is about volunteering to help with $CEY."

# Find deadline for applying as a volunteer
DEADLINE_Q="$PREFIX_Q In this page, find the deadline for applying as a volunteer. The year should be either $im1 or $i."
DEADLINE=$($SCRIPT_DIR/llm/query_date.sh "$VOLUNTEER_WEBPAGE" "$DEADLINE_Q" "application deadline" 2.5)
DEADLINE_YEAR=$($SCRIPT_DIR/util/date.sh -d "$DEADLINE" +%Y)
if [ "$DEADLINE_YEAR" != $i ] && [ "$DEADLINE_YEAR" != $im1 ]; then
    echo "[ERROR] $CEY volunteer deadline not found"
    exit 1
fi

REGISTER_DESCRIPTION_Q="$PREFIX_Q Write a paragraph in markdown formatting with all information about applying to volunteer. It should include all important facts and links or emails (if present). Do not leave any blanks to fill."
REGISTER_DESCRIPTION=$($SCRIPT_DIR/llm/qwen/run.sh "$VOLUNTEER_WEBPAGE" "$REGISTER_DESCRIPTION_Q" 2.5 full)

$SCRIPT_DIR/util/ics_calendar.sh start > "cal.ics"
$SCRIPT_DIR/util/ics_event.sh "[$EVENT $i] Volunteer Application Deadline" "$REGISTER_DESCRIPTION" "" "$DEADLINE" "$DEADLINE" >> "cal.ics"
$SCRIPT_DIR/util/ics_calendar.sh end >> "cal.ics"
