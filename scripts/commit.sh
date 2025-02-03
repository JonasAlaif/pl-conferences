#!/bin/bash

# Commits all changes in `conferences/$1` to the repository

git pull
git add "conferences/$1"
# If there are no changes, then exit early
if git diff --staged --quiet; then
    echo "No changes to commit"
    exit 0
fi
git commit -m "Add new $1 years"
git push
