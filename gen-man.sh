#!/usr/bin/env bash

help2man -N \
        --name="Redup is a command-line tool for finding duplicate files by hashing their contents" \
        --section=1 \
        --manual="User Commands" \
        --source="redup" \
        ./redup > man/redup.1