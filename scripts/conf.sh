#!/bin/bash

# Downloads data for one conference

DATE_Q_SUFFIX="It is imperative to answer in the format 'YYYY-MM-DD' or 'No' if not found, no full sentence. Use the format 'YYYY-MM-DD', do not write any other text in the answer."
MODEL="llama" # deepseek or llama

SCRIPT_DIR=$(dirname "$0" | xargs realpath)
cd "$SCRIPT_DIR/../conferences/$1" || exit 1

INFO=$(cat "info.json")
NAME=$(echo "$INFO" | jq -r '.name')
START_YEAR=$(echo "$INFO" | jq -r '.start')

TODO_YEARS=( $($SCRIPT_DIR/find_years.sh "$START_YEAR") )
echo "Downloading data for $NAME for years ${TODO_YEARS[@]}"

# Download the CFPs
# pids=()
for i in "${TODO_YEARS[@]}"; do
    [ -d "$i" ] && exit 1 || true
    $SCRIPT_DIR/https/get_cfp.sh "$NAME" "$i"
    #  &
    # pids+=($!)
    sleep 1
done

# for pid in "${pids[@]}"; do
#     wait $pid
# done

for i in "${TODO_YEARS[@]}"; do
    # Skip if the CFP was not found
    [ ! -f "$i/cfp.html" ] && continue || true

    echo -e "======\nExtracting data for $NAME $i\n======"
    im1=$((i-1))
    CFP_WEBPAGE=$(cat "$i/cfp.html")

    # Check we have the right thing
    CFP_Q="Is the webpage above an error page (e.g. 404) or normal page with information? Answer with 'Yes' if error or 'No' if normal only, no full sentence. One word answer."
    LLM_OUTPUT=$($SCRIPT_DIR/llm/$MODEL.sh "$CFP_WEBPAGE" "$CFP_Q" curt)
    if [ "$LLM_OUTPUT" == "Yes" ]; then
        echo "[ERROR] $NAME $i CFP error page"
        echo "$CFP_WEBPAGE"
        continue
    fi

    # Check we have the right thing
    CFP_Q="Is the webpage above a page about \"$NAME $i\"? Answer with 'Yes' if both the name and year are mentioned, 'No' otherwise. No full sentence, one word answer."
    LLM_OUTPUT=$($SCRIPT_DIR/llm/$MODEL.sh "$CFP_WEBPAGE" "$CFP_Q" curt)
    if [ "$LLM_OUTPUT" == "No" ]; then
        echo "[ERROR] $NAME $i CFP bad page"
        echo "$CFP_WEBPAGE"
        continue
    fi

    # Find the paper submission deadline
    PAPER_SUBMISSION_Q="The page above is the call for papers of $NAME $i. In this page, find the date of the paper submission deadline (year either $im1 or $i). $DATE_Q_SUFFIX"
    PAPER_SUBMISSION=$($SCRIPT_DIR/llm/$MODEL.sh "$CFP_WEBPAGE" "$PAPER_SUBMISSION_Q" curt)
    PAPER_SUBMISSION_YEAR=$($SCRIPT_DIR/date.sh -d "$PAPER_SUBMISSION" +%Y)
    # Either the same year or a year earlier
    if [ "$PAPER_SUBMISSION_YEAR" != $i ] && [ "$PAPER_SUBMISSION_YEAR" != $im1 ]; then
        echo "[ERROR] $NAME $i paper submission not found"
        echo "$CFP_WEBPAGE"
        continue
    fi

    # Find the rebuttal period dates
    REBUTTAL_START_Q="The page above is the call for papers of $NAME $i. Find the date when the author feedback (rebuttal) period starts (a few months after $PAPER_SUBMISSION). $DATE_Q_SUFFIX"
    REBUTTAL_START=$($SCRIPT_DIR/llm/$MODEL.sh "$CFP_WEBPAGE" "$REBUTTAL_START_Q" curt)
    REBUTTAL_START_YEAR=$($SCRIPT_DIR/date.sh -d "$REBUTTAL_START" +%Y)
    if [ "$REBUTTAL_START_YEAR" != $i ] && [ "$REBUTTAL_START_YEAR" != $im1 ]; then
        echo "[ERROR] $NAME $i rebuttal start not found"
        echo "$CFP_WEBPAGE"
        continue
    fi
    REBUTTAL_END_Q="The page above is the call for papers of $NAME $i. Find the date when the author feedback (rebuttal) period ends (it starts $REBUTTAL_START). $DATE_Q_SUFFIX"
    REBUTTAL_END=$($SCRIPT_DIR/llm/$MODEL.sh "$CFP_WEBPAGE" "$REBUTTAL_END_Q" curt)
    REBUTTAL_END_YEAR=$($SCRIPT_DIR/date.sh -d "$REBUTTAL_END" +%Y)
    if [ "$REBUTTAL_END_YEAR" != $i ] && [ "$REBUTTAL_END_YEAR" !- $im1 ]; then
        echo "[ERROR] $NAME $i rebuttal end not found"
        echo "$CFP_WEBPAGE"
        continue
    fi

    # Find the notification (accept/reject) date
    NOTIFICATION_Q="The page above is the call for papers of $NAME $i. Find the date when the paper notification (of acceptance/rejection) is (after $REBUTTAL_END). $DATE_Q_SUFFIX"
    NOTIFICATION=$($SCRIPT_DIR/llm/$MODEL.sh "$CFP_WEBPAGE" "$NOTIFICATION_Q" curt)
    NOTIFICATION_YEAR=$($SCRIPT_DIR/date.sh -d "$NOTIFICATION" +%Y)
    if [ "$NOTIFICATION_YEAR" != $i ] && [ "$NOTIFICATION_YEAR" != $im1 ]; then
        echo "[ERROR] $NAME $i notification not found"
        echo "$CFP_WEBPAGE"
        continue
    fi

    # Find the conference period dates
    CONFERENCE_START_Q="The page above is the call for papers of $NAME $i. Find the date when the conference starts (after $NOTIFICATION). $DATE_Q_SUFFIX"
    CONFERENCE_START=$($SCRIPT_DIR/llm/$MODEL.sh "$CFP_WEBPAGE" "$CONFERENCE_START_Q" curt)
    CONFERENCE_START_YEAR=$($SCRIPT_DIR/date.sh -d "$CONFERENCE_START" +%Y)
    if [ "$CONFERENCE_START_YEAR" != $i ]; then
        echo "[ERROR] $NAME $i conference start not found"
        echo "$CFP_WEBPAGE"
        continue
    fi
    CONFERENCE_END_Q="The page above is the call for papers of $NAME $i. Find the month, day and year when the conference ends (it starts $CONFERENCE_START). $DATE_Q_SUFFIX"
    CONFERENCE_END=$($SCRIPT_DIR/llm/$MODEL.sh "$CFP_WEBPAGE" "$CONFERENCE_END_Q" curt)
    CONFERENCE_END_YEAR=$($SCRIPT_DIR/date.sh -d "$CONFERENCE_END" +%Y)
    if [ "$CONFERENCE_END_YEAR" != $i ]; then
        echo "[ERROR] $NAME $i conference end not found"
        echo "$CFP_WEBPAGE"
        continue
    fi

    # Check that the dates are all in order
    if [ "$PAPER_SUBMISSION" \> "$REBUTTAL_START" ] || [ "$REBUTTAL_START" \> "$REBUTTAL_END" ] || [ "$REBUTTAL_END" \> "$NOTIFICATION" ] || [ "$NOTIFICATION" \> "$CONFERENCE_START" ] || [ "$CONFERENCE_START" \> "$CONFERENCE_END" ]; then
        echo "[ERROR] $NAME $i dates are not in order"
        echo "$CFP_WEBPAGE"
        continue
    fi

    # Find the conference city and country
    CITY_COUNTRY_Q="The page above is the call for papers of $NAME $i. What city and world country is the $NAME $i conference? Answer with 'city, world country' or 'No' if not found, no full sentence. Two word answer."
    CITY_COUNTRY=$($SCRIPT_DIR/llm/$MODEL.sh "$CFP_WEBPAGE" "$CITY_COUNTRY_Q" curt)
    if [ "$CITY_COUNTRY" == "No" ]; then
        CITY_COUNTRY=""
    fi

    echo "{
    \"paper_submission\": \"$PAPER_SUBMISSION\",
    \"rebuttal_start\": \"$REBUTTAL_START\",
    \"rebuttal_end\": \"$REBUTTAL_END\",
    \"notification\": \"$NOTIFICATION\",
    \"conference_start\": \"$CONFERENCE_START\",
    \"conference_end\": \"$CONFERENCE_END\",
    \"city_country\": \"$CITY_COUNTRY\"
}" > "$i/deadlines.json"
done

echo "Collected all dates for $NAME, creating ICS files"

for i in "${TODO_YEARS[@]}"; do
    if [ ! -f "$i/deadlines.json" ]; then
        [ -d "$i" ] && rm -rf "$i" || true
        continue
    fi

    CFP_WEBPAGE=$(cat "$i/cfp.html")
    PAPER_SUBMISSION=$(jq -r '.paper_submission' "$i/deadlines.json")
    REBUTTAL_START=$(jq -r '.rebuttal_start' "$i/deadlines.json")
    REBUTTAL_END=$(jq -r '.rebuttal_end' "$i/deadlines.json")
    NOTIFICATION=$(jq -r '.notification' "$i/deadlines.json")
    CONFERENCE_START=$(jq -r '.conference_start' "$i/deadlines.json")
    CONFERENCE_END=$(jq -r '.conference_end' "$i/deadlines.json")
    CITY_COUNTRY=$(jq -r '.city_country' "$i/deadlines.json")
    rm "$i/deadlines.json"

    EVENT_DESCRIPTION_Q="The page above is the call for papers of $NAME $i. Write a short paragraph with all information about submitting a paper to this conference. It should include all important facts and links. Do not leave any blanks to fill."
    EVENT_DESCRIPTION=$($SCRIPT_DIR/llm/$MODEL.sh "$CFP_WEBPAGE" "$EVENT_DESCRIPTION_Q" full)

    $SCRIPT_DIR/ics_calendar.sh start > "$i/deadlines.ics"
    $SCRIPT_DIR/ics_event.sh "[$NAME $i] Paper Submission Deadline" "$EVENT_DESCRIPTION" "" "$PAPER_SUBMISSION" "$PAPER_SUBMISSION" >> "$i/deadlines.ics"
    $SCRIPT_DIR/ics_event.sh "[$NAME $i] Rebuttal Period" "" "" "$REBUTTAL_START" "$REBUTTAL_END" >> "$i/deadlines.ics"
    $SCRIPT_DIR/ics_event.sh "[$NAME $i] Notification" "" "" "$NOTIFICATION" "$NOTIFICATION" >> "$i/deadlines.ics"
    $SCRIPT_DIR/ics_event.sh "[$NAME $i] Conference" "" "$CITY_COUNTRY" "$CONFERENCE_START" "$CONFERENCE_END" >> "$i/deadlines.ics"
    $SCRIPT_DIR/ics_calendar.sh end >> "$i/deadlines.ics"
done
