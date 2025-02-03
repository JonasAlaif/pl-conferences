#!/bin/bash

# Print formatted ICS calendar start/end

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 [start|end]"
    exit 1
fi

if [ "$1" == "start" ]; then
    echo "BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//hacksw/handcal//NONSGML v1.0//EN"
elif [ "$1" == "end" ]; then
    echo "END:VCALENDAR"
else
    echo "Usage: $0 [start|end]"
    exit 1
fi
