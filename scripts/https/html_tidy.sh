#!/bin/bash

# Tidies up the html of a webpage

htmlq --remove-nodes 'head > :not(title), script, #program, #event-overview, #Speaker-s-Guide, #Session-Chair-Guide' | (tidy --tidy-mark no -w 0 -q --show-warnings false || true)

# Unfortunately `readable` removes important sidebars
# readable --insecure --style readable --quiet "$1"
