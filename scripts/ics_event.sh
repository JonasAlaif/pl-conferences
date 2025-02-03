#!/bin/bash

# Print formatted ICS event

if [ "$#" -ne 5 ]; then
    echo "Usage: $0 'Event Name' 'Description' 'Location' 'Start Date Inclusive (MM/DD/YYYY)' 'End Date Inclusive (MM/DD/YYYY)'"
    exit 1
fi

SCRIPT_DIR="$(dirname "$0" | xargs realpath)"

NAME="$1"
DESC="$2"
LOCATION="$3"
START_DATE=$($SCRIPT_DIR/date.sh -d "$4" +%Y%m%d)
END_DATE=$($SCRIPT_DIR/date.sh -d "$5 + 1 day" +%Y%m%d)
UUID=$(uuidgen)
TIMESTAMP=$($SCRIPT_DIR/date.sh -u +%Y%m%dT%H%M%SZ)

echo "BEGIN:VEVENT
UID:${UUID}
DTSTAMP:${TIMESTAMP}
DTSTART;VALUE=DATE:${START_DATE}
DTEND;VALUE=DATE:${END_DATE}
SUMMARY:${NAME}
DESCRIPTION:${DESC}
LOCATION:${LOCATION}
END:VEVENT"
