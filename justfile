set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

loader_target := "x86_64-unknown-uefi"
kernel_target := "x86_64-unknown-none"
kernel_rustflags := "-C relocation-model=static -C link-arg=-Tkernel/kernel/linker.ld -C link-arg=-no-pie -C link-arg=--build-id=none -C link-arg=-z -C link-arg=max-page-size=0x1000"
user_rustflags := "-C relocation-model=static -C link-arg=-Tlibs/liblazer/linker.ld -C link-arg=-no-pie -C link-arg=--build-id=none -C link-arg=-z -C link-arg=max-page-size=0x1000"

default:
    @just --list

setup-toolchain:
    rustup target add {{loader_target}} {{kernel_target}}

build-loader:
    cargo build --release --package uefi-loader --target {{loader_target}}

build-user:
    mkdir -p build
    USER_PACKAGES="$(for dir in user/*; do if [[ -d "${dir}" ]]; then basename "${dir}"; fi; done | sort)" ; \
    for package in $USER_PACKAGES; do \
        RUSTFLAGS='{{user_rustflags}}' cargo build --release --package "${package}" --target {{kernel_target}} ; \
        cp "target/{{kernel_target}}/release/${package}" "build/${package}" ; \
    done

build-kernel:
    RUSTFLAGS='{{kernel_rustflags}}' cargo build --release --package kernel --target {{kernel_target}}

image: build-loader build-user build-kernel
    tools/scripts/build-image.sh

build: image

run: image
    bash tools/scripts/run-qemu.sh

run-headless: image
    bash tools/scripts/run-qemu-headless.sh

check:
    cargo check --package boot-info
    cargo check --package elf
    cargo test --package lash
    cargo check --package liblazer --target {{kernel_target}}
    cargo check --package uefi-loader --target {{loader_target}}
    USER_PACKAGES="$(for dir in user/*; do if [[ -d "${dir}" ]]; then basename "${dir}"; fi; done | sort)" ; \
    for package in $USER_PACKAGES; do \
        cargo check --package "${package}" --target {{kernel_target}} ; \
    done
    cargo check --package kernel --target {{kernel_target}}

clean:
    cargo clean
    rm -rf build/

workspace-metadata:
    cargo metadata --no-deps --format-version 1 > /dev/null

tree:
    find . -maxdepth 3 -type d | sort
