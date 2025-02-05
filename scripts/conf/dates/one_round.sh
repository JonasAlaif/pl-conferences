#!/bin/bash

# Downloads data for one conference year

CONFERENCE=$1
EVENT=$2
i=$3
CFP_WEBPAGE=$4
CFP_TABLE=$5

im1=$((i-1))
SCRIPT_DIR=$(realpath "$(dirname "$0")/../..")

if [ -z "$CONFERENCE" ]; then
    CEY="$EVENT $i"
else
    CEY="$CONFERENCE $EVENT $i"
fi
PREFIX_Q="The page above is the call for papers of $CEY. In this page, find"
PREFIX_TABLE_Q="The table above contains all important dates for $CEY. In this table, find"

# Find the paper submission deadline
PAPER_SUBMISSION_Q="$PREFIX_TABLE_Q the submission deadline. This is an important date and is the deadline for authors to submit a paper to this conference. The year should be either $im1 or $i. Ignore artifact deadlines."
PAPER_SUBMISSION=$($SCRIPT_DIR/llm/query_date.sh "$CFP_TABLE" "$PAPER_SUBMISSION_Q" "submission deadline" 2.5)
PAPER_SUBMISSION_YEAR=$($SCRIPT_DIR/util/date.sh -d "$PAPER_SUBMISSION" +%Y)
if [ "$PAPER_SUBMISSION_YEAR" != $i ] && [ "$PAPER_SUBMISSION_YEAR" != $im1 ]; then
    echo "[ERROR] $CEY paper submission not found"
    exit 1
fi

# Find the rebuttal period dates
REBUTTAL_START_Q="$PREFIX_TABLE_Q when the author response period starts. This is an important date and is when the authors receive feedback on their papers and start writing a rebuttal, it should be after the submission deadline on $PAPER_SUBMISSION."
REBUTTAL_START=$($SCRIPT_DIR/llm/query_date.sh "$CFP_TABLE" "$REBUTTAL_START_Q" "author response period start" 2.5)
REBUTTAL_START_YEAR=$($SCRIPT_DIR/util/date.sh -d "$REBUTTAL_START" +%Y)
if [ "$REBUTTAL_START_YEAR" != $i ] && [ "$REBUTTAL_START_YEAR" != $im1 ]; then
    echo "[ERROR] $CEY rebuttal start not found"
    exit 1
fi
REBUTTAL_END_Q="$PREFIX_TABLE_Q when the author response period ends. This is an important date and is when the authors submit their rebuttal to the feedback they received, it should be a few days after the start on $REBUTTAL_START."
REBUTTAL_END=$($SCRIPT_DIR/llm/query_date.sh "$CFP_TABLE" "$REBUTTAL_END_Q" "author response period end" 2.5)
REBUTTAL_END_YEAR=$($SCRIPT_DIR/util/date.sh -d "$REBUTTAL_END" +%Y)
if [ "$REBUTTAL_END_YEAR" != $i ] && [ "$REBUTTAL_END_YEAR" != $im1 ]; then
    echo "[ERROR] $CEY rebuttal end not found"
    exit 1
fi

# Find the notification (accept/reject) date
NOTIFICATION_Q="$PREFIX_TABLE_Q and answer with all notification dates. There may only be one. First, list all dates. Then, talk about the earliest one (should be preliminary or conditional). It should be soon after the end of the response period on $REBUTTAL_END. Ignore artifact deadlines."
NOTIFICATION=$($SCRIPT_DIR/llm/query_date.sh "$CFP_TABLE" "$NOTIFICATION_Q" "preliminary notification date" 2.5)
NOTIFICATION_YEAR=$($SCRIPT_DIR/util/date.sh -d "$NOTIFICATION" +%Y)
if [ "$NOTIFICATION_YEAR" != $i ] && [ "$NOTIFICATION_YEAR" != $im1 ]; then
    echo "[ERROR] $CEY notification not found"
    exit 1
fi

# Find the conference period dates
CONFERENCE_START_Q="$PREFIX_Q the start and end dates of the conference. Provide the full dates for both, this should be some time after $NOTIFICATION."
CONFERENCE_START=$($SCRIPT_DIR/llm/query_date.sh "$CFP_WEBPAGE" "$CONFERENCE_START_Q" "conference start date" 2.5)
CONFERENCE_START_YEAR=$($SCRIPT_DIR/util/date.sh -d "$CONFERENCE_START" +%Y)
if [ "$CONFERENCE_START_YEAR" != $i ]; then
    echo "[ERROR] $CEY conference start not found"
    exit 1
fi
CONFERENCE_END_Q="$PREFIX_Q the start and end dates of the conference. Provide the full dates for both (the conference starts on $CONFERENCE_START)."
CONFERENCE_END=$($SCRIPT_DIR/llm/query_date.sh "$CFP_WEBPAGE" "$CONFERENCE_END_Q" "conference end date" 2.5)
CONFERENCE_END_YEAR=$($SCRIPT_DIR/util/date.sh -d "$CONFERENCE_END" +%Y)
if [ "$CONFERENCE_END_YEAR" != $i ]; then
    echo "[ERROR] $CEY conference end not found"
    exit 1
fi

# Check that the dates are all in order
if [ ! "$PAPER_SUBMISSION" \< "$REBUTTAL_START" ] || [ ! "$REBUTTAL_START" \< "$REBUTTAL_END" ] || [ ! "$REBUTTAL_END" \< "$NOTIFICATION" ] || [ ! "$NOTIFICATION" \< "$CONFERENCE_START" ] || [ ! "$CONFERENCE_START" \< "$CONFERENCE_END" ]; then
    echo "[ERROR] $CEY dates are not in order"
    exit 1
fi

# Find the conference city and country
CITY_COUNTRY_Q="$PREFIX_Q what city and world country is the $CEY conference? Answer with 'city, world country' or 'No' if not found, no full sentence. Two word answer."
CITY_COUNTRY=$($SCRIPT_DIR/llm/qwen/run.sh "$CFP_WEBPAGE" "$CITY_COUNTRY_Q" 2.5 sentence)
if [ "$CITY_COUNTRY" == "No" ]; then
    CITY_COUNTRY=""
fi

EVENT_DESCRIPTION_Q="The page above is the call for papers of $CEY. Write a paragraph with all information about submitting a paper to $CEY. It should include all important facts and links. Do not leave any blanks to fill."
EVENT_DESCRIPTION=$($SCRIPT_DIR/llm/qwen/run.sh "$CFP_WEBPAGE" "$EVENT_DESCRIPTION_Q" 2.5 full)

$SCRIPT_DIR/util/ics_calendar.sh start > "cal.ics"
$SCRIPT_DIR/util/ics_event.sh "[$EVENT $i] Paper Submission Deadline" "$EVENT_DESCRIPTION" "" "$PAPER_SUBMISSION" "$PAPER_SUBMISSION" >> "cal.ics"
$SCRIPT_DIR/util/ics_event.sh "[$EVENT $i] Rebuttal" "" "" "$REBUTTAL_START" "$REBUTTAL_END" >> "cal.ics"
$SCRIPT_DIR/util/ics_event.sh "[$EVENT $i] Notification" "" "" "$NOTIFICATION" "$NOTIFICATION" >> "cal.ics"
$SCRIPT_DIR/util/ics_event.sh "[$EVENT $i] Conference" "" "$CITY_COUNTRY" "$CONFERENCE_START" "$CONFERENCE_END" >> "cal.ics"
$SCRIPT_DIR/util/ics_calendar.sh end >> "cal.ics"
