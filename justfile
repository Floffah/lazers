set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

default:
    @just --list

build:
    echo "Building the project..."