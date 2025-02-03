#!/bin/bash

# Fetches and tidies up the website

# wget "$1" -q -O - | tidy -w 0 -q --show-warnings false
readable --insecure --style readable --quiet "$1" | (tidy -w 0 -q --show-warnings false || true)
