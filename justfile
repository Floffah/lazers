set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

loader_target := "x86_64-unknown-uefi"
kernel_target := "x86_64-unknown-none"

default:
    @just --list

setup-toolchain:
    rustup target add {{loader_target}} {{kernel_target}}

build-loader:
    cargo build --release --package uefi-loader --target {{loader_target}}

build-kernel:
    cargo build --release --package lazers-kernel --target {{kernel_target}}

image: build-loader build-kernel
    tools/scripts/build-image.sh

build: image

run: image
    bash tools/scripts/run-qemu.sh

run-headless: image
    bash tools/scripts/run-qemu-headless.sh

check:
    cargo check --package boot-info
    cargo check --package uefi-loader --target {{loader_target}}
    cargo check --package lazers-kernel --target {{kernel_target}}

clean:
    cargo clean
    rm -rf build/

workspace-metadata:
    cargo metadata --no-deps --format-version 1 > /dev/null

tree:
    find . -maxdepth 3 -type d | sort
