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
R1_FOCUS="Find R1, round 1 only. Ignore R2, round 2."
R2_FOCUS="Find R2, round 2 only. Ignore R1, round 1."

# Find the paper submission deadline
PAPER_SUBMISSION_R1_Q="$PREFIX_TABLE_Q the round 1 submission deadline. This is an important date and is the deadline for authors to submit a paper to this conference. The year should be either $im1 or $i. Ignore artifact deadlines. $R1_FOCUS"
PAPER_SUBMISSION_R1=$($SCRIPT_DIR/llm/query_date.sh "$CFP_TABLE" "$PAPER_SUBMISSION_R1_Q" "r1 submission deadline" 2.5)
PAPER_SUBMISSION_R1_YEAR=$($SCRIPT_DIR/util/date.sh -d "$PAPER_SUBMISSION_R1" +%Y)
if [ "$PAPER_SUBMISSION_R1_YEAR" != $i ] && [ "$PAPER_SUBMISSION_R1_YEAR" != $im1 ]; then
    echo "[ERROR] $CEY r1 paper submission not found"
    exit 1
fi

PAPER_SUBMISSION_R2_Q="$PREFIX_TABLE_Q the round 2 submission deadline. This is an important date and is the deadline for authors to submit a paper to this conference. The year should be either $im1 or $i. Ignore artifact deadlines. $R2_FOCUS"
PAPER_SUBMISSION_R2=$($SCRIPT_DIR/llm/query_date.sh "$CFP_TABLE" "$PAPER_SUBMISSION_R2_Q" "r2 submission deadline" 2.5)
PAPER_SUBMISSION_R2_YEAR=$($SCRIPT_DIR/util/date.sh -d "$PAPER_SUBMISSION_R2" +%Y)
if [ "$PAPER_SUBMISSION_R2_YEAR" != $i ] && [ "$PAPER_SUBMISSION_R2_YEAR" != $im1 ]; then
    echo "[ERROR] $CEY r2 paper submission not found"
    exit 1
fi

# Find the rebuttal period dates
REBUTTAL_START_R1_Q="$PREFIX_TABLE_Q when the round 1 author response period starts. This is an important date and is when the authors receive feedback on their papers and start writing a rebuttal, it should be after the round 1 submission deadline on $PAPER_SUBMISSION_R1. $R1_FOCUS"
REBUTTAL_START_R1=$($SCRIPT_DIR/llm/query_date.sh "$CFP_TABLE" "$REBUTTAL_START_R1_Q" "r1 author response period start" 2.5)
REBUTTAL_START_R1_YEAR=$($SCRIPT_DIR/util/date.sh -d "$REBUTTAL_START_R1" +%Y)
if [ "$REBUTTAL_START_R1_YEAR" != $i ] && [ "$REBUTTAL_START_R1_YEAR" != $im1 ]; then
    echo "[ERROR] $CEY r1 rebuttal start not found"
    exit 1
fi
REBUTTAL_END_R1_Q="$PREFIX_TABLE_Q when the round 1 author response period ends. This is an important date and is when the authors submit their rebuttal to the feedback they received, it should be a few days after the start on $REBUTTAL_START_R1. $R1_FOCUS"
REBUTTAL_END_R1=$($SCRIPT_DIR/llm/query_date.sh "$CFP_TABLE" "$REBUTTAL_END_R1_Q" "r1 author response period end" 2.5)
REBUTTAL_END_R1_YEAR=$($SCRIPT_DIR/util/date.sh -d "$REBUTTAL_END_R1" +%Y)
if [ "$REBUTTAL_END_R1_YEAR" != $i ] && [ "$REBUTTAL_END_R1_YEAR" != $im1 ]; then
    echo "[ERROR] $CEY r1 rebuttal end not found"
    exit 1
fi

REBUTTAL_START_R2_Q="$PREFIX_TABLE_Q when the round 2 author response period starts. This is an important date and is when the authors receive feedback on their papers and start writing a rebuttal, it should be after the round 2 submission deadline on $PAPER_SUBMISSION_R2. $R2_FOCUS"
REBUTTAL_START_R2=$($SCRIPT_DIR/llm/query_date.sh "$CFP_TABLE" "$REBUTTAL_START_R2_Q" "r2 author response period start" 2.5)
REBUTTAL_START_R2_YEAR=$($SCRIPT_DIR/util/date.sh -d "$REBUTTAL_START_R2" +%Y)
if [ "$REBUTTAL_START_R2_YEAR" != $i ] && [ "$REBUTTAL_START_R2_YEAR" != $im1 ]; then
    echo "[ERROR] $CEY r2 rebuttal start not found"
    exit 1
fi
REBUTTAL_END_R2_Q="$PREFIX_TABLE_Q when the round 2 author response period ends. This is an important date and is when the authors submit their rebuttal to the feedback they received, it should be a few days after the start on $REBUTTAL_START_R2. $R2_FOCUS"
REBUTTAL_END_R2=$($SCRIPT_DIR/llm/query_date.sh "$CFP_TABLE" "$REBUTTAL_END_R2_Q" "r2 author response period end" 2.5)
REBUTTAL_END_R2_YEAR=$($SCRIPT_DIR/util/date.sh -d "$REBUTTAL_END_R2" +%Y)
if [ "$REBUTTAL_END_R2_YEAR" != $i ] && [ "$REBUTTAL_END_R2_YEAR" != $im1 ]; then
    echo "[ERROR] $CEY r2 rebuttal end not found"
    exit 1
fi

# Find the notification (accept/reject) date
NOTIFICATION_R1_Q="$PREFIX_TABLE_Q and answer with all notification dates (round 1 only). There may only be one. First, list all dates. Then, talk about the earliest one (should be preliminary or conditional). It should be soon after the end of the response period on $REBUTTAL_END_R1. Ignore artifact deadlines. $R1_FOCUS"
NOTIFICATION_R1=$($SCRIPT_DIR/llm/query_date.sh "$CFP_TABLE" "$NOTIFICATION_R1_Q" "r1 preliminary notification date" 2.5)
NOTIFICATION_R1_YEAR=$($SCRIPT_DIR/util/date.sh -d "$NOTIFICATION_R1" +%Y)
if [ "$NOTIFICATION_R1_YEAR" != $i ] && [ "$NOTIFICATION_R1_YEAR" != $im1 ]; then
    echo "[ERROR] $CEY r1 notification not found"
    exit 1
fi

NOTIFICATION_R2_Q="$PREFIX_TABLE_Q and answer with all notification dates (round 2 only). There may only be one. First, list all dates. Then, talk about the earliest one (should be preliminary or conditional). It should be soon after the end of the response period on $REBUTTAL_END_R2. Ignore artifact deadlines. $R2_FOCUS"
NOTIFICATION_R2=$($SCRIPT_DIR/llm/query_date.sh "$CFP_TABLE" "$NOTIFICATION_R2_Q" "r2 preliminary notification date" 2.5)
NOTIFICATION_R2_YEAR=$($SCRIPT_DIR/util/date.sh -d "$NOTIFICATION_R2" +%Y)
if [ "$NOTIFICATION_R2_YEAR" != $i ] && [ "$NOTIFICATION_R2_YEAR" != $im1 ]; then
    echo "[ERROR] $CEY r2 notification not found"
    exit 1
fi

# Find the conference period dates
CONFERENCE_START_Q="$PREFIX_Q the start and end dates of the conference. Provide the full dates for both, this should be some time after $NOTIFICATION_R2."
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

# Check that the dates are all in order (and that "$REBUTTAL_END_R1" < "$PAPER_SUBMISSION_R2" and "$NOTIFICATION_R1" <= "$NOTIFICATION_R2")
if [ ! "$PAPER_SUBMISSION_R1" \< "$REBUTTAL_START_R1" ] || [ ! "$REBUTTAL_START_R1" \< "$REBUTTAL_END_R1" ] || [ ! "$REBUTTAL_END_R1" \< "$NOTIFICATION_R1" ] || [ ! "$NOTIFICATION_R1" \< "$CONFERENCE_START" ]; then
    echo "[ERROR] $CEY r1 dates are not in order"
    exit 1
fi
if [ ! "$PAPER_SUBMISSION_R2" \< "$REBUTTAL_START_R2" ] || [ ! "$REBUTTAL_START_R2" \< "$REBUTTAL_END_R2" ] || [ ! "$REBUTTAL_END_R2" \< "$NOTIFICATION_R2" ] || [ ! "$NOTIFICATION_R2" \< "$CONFERENCE_START" ]; then
    echo "[ERROR] $CEY r2 dates are not in order"
    exit 1
fi
if [ ! "$REBUTTAL_END_R1" \< "$PAPER_SUBMISSION_R2" ] || [ "$NOTIFICATION_R1" \> "$NOTIFICATION_R2" ] || [ ! "$CONFERENCE_START" \< "$CONFERENCE_END" ]; then
    echo "[ERROR] $CEY dates are not in order"
    exit 1
fi

# Find the conference city and country
CITY_COUNTRY_Q="$PREFIX_Q what city and world country is the $CEY conference? Answer with 'city, world country' or 'No' if not found, no full sentence. Two word answer."
CITY_COUNTRY=$($SCRIPT_DIR/llm/qwen/run.sh "$CFP_WEBPAGE" "$CITY_COUNTRY_Q" 2.5 curt)
if [ "$CITY_COUNTRY" == "No" ]; then
    CITY_COUNTRY=""
fi

EVENT_DESCRIPTION_Q="The page above is the call for papers of $CEY. Write a paragraph with all information about submitting a paper to this conference. It should include all important facts and links. Do not leave any blanks to fill."
EVENT_DESCRIPTION=$($SCRIPT_DIR/llm/qwen/run.sh "$CFP_WEBPAGE" "$EVENT_DESCRIPTION_Q" 2.5 full)

$SCRIPT_DIR/util/ics_calendar.sh start > "cal.ics"
$SCRIPT_DIR/util/ics_event.sh "[$EVENT $i] R1 Paper Submission Deadline" "$EVENT_DESCRIPTION" "" "$PAPER_SUBMISSION_R1" "$PAPER_SUBMISSION_R1" >> "cal.ics"
$SCRIPT_DIR/util/ics_event.sh "[$EVENT $i] R1 Rebuttal" "" "" "$REBUTTAL_START_R1" "$REBUTTAL_END_R1" >> "cal.ics"
if [ "$NOTIFICATION_R1" != "$NOTIFICATION_R2" ]; then
    $SCRIPT_DIR/util/ics_event.sh "[$EVENT $i] R1 Notification" "" "" "$NOTIFICATION_R1" "$NOTIFICATION_R1" >> "cal.ics"
fi
$SCRIPT_DIR/util/ics_event.sh "[$EVENT $i] R2 Paper Submission Deadline" "$EVENT_DESCRIPTION" "" "$PAPER_SUBMISSION_R2" "$PAPER_SUBMISSION_R2" >> "cal.ics"
$SCRIPT_DIR/util/ics_event.sh "[$EVENT $i] R2 Rebuttal" "" "" "$REBUTTAL_START_R2" "$REBUTTAL_END_R2" >> "cal.ics"
if [ "$NOTIFICATION_R1" != "$NOTIFICATION_R2" ]; then
    $SCRIPT_DIR/util/ics_event.sh "[$EVENT $i] R2 Notification" "" "" "$NOTIFICATION_R2" "$NOTIFICATION_R2" >> "cal.ics"
else
    $SCRIPT_DIR/util/ics_event.sh "[$EVENT $i] Notification" "" "" "$NOTIFICATION_R2" "$NOTIFICATION_R2" >> "cal.ics"
fi
$SCRIPT_DIR/util/ics_event.sh "[$EVENT $i] Conference" "" "$CITY_COUNTRY" "$CONFERENCE_START" "$CONFERENCE_END" >> "cal.ics"
$SCRIPT_DIR/util/ics_calendar.sh end >> "cal.ics"
