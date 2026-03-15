set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

loader_target := "x86_64-unknown-uefi"
kernel_target := "x86_64-unknown-none"
kernel_rustflags := "-C relocation-model=static -C link-arg=-Tkernel/kernel/linker.ld -C link-arg=-no-pie -C link-arg=--build-id=none -C link-arg=-z -C link-arg=max-page-size=0x1000"
user_rustflags := "-C relocation-model=static -C link-arg=-Tlibs/liblazer/linker.ld -C link-arg=-no-pie -C link-arg=--build-id=none -C link-arg=-z -C link-arg=max-page-size=0x1000"
default_initial_user_program := "/system/bin/lash"
selftest_initial_user_program := "/system/bin/selftest"

default:
    @just --list

setup-toolchain:
    rustup target add {{loader_target}} {{kernel_target}}

build-loader:
    cargo build --release --package uefi-loader --target {{loader_target}}

build-user:
    source tools/scripts/user-packages.sh ; \
    mkdir -p "$BUILD_DIR" ; \
    load_user_packages ; \
    for package in "${USER_PACKAGES[@]}"; do \
        RUSTFLAGS='{{user_rustflags}}' cargo build --release --package "${package}" --target {{kernel_target}} ; \
        cp "$ROOT_DIR/target/{{kernel_target}}/release/${package}" "$BUILD_DIR/${package}" ; \
    done

build-kernel initial_user_program=default_initial_user_program:
    LAZERS_INITIAL_USER_PROGRAM='{{initial_user_program}}' RUSTFLAGS='{{kernel_rustflags}}' cargo build --release --package kernel --target {{kernel_target}}

image: build-loader build-user build-kernel
    LAZERS_USER_PACKAGES="$(source tools/scripts/user-packages.sh ; list_user_packages)" tools/scripts/build-image.sh

image-selftest: build-loader build-user
    just build-kernel {{selftest_initial_user_program}}
    LAZERS_USER_PACKAGES="$(source tools/scripts/user-packages.sh ; list_user_packages)" LAZERS_IMAGE_NAME='lazers-selftest.img' tools/scripts/build-image.sh

build: image

run: image
    bash tools/scripts/run-qemu.sh

run-headless: image
    bash tools/scripts/run-qemu-headless.sh

run-selftest: image-selftest
    LAZERS_IMAGE_NAME=lazers-selftest.img bash tools/scripts/run-qemu.sh

run-selftest-headless: image-selftest
    LAZERS_IMAGE_NAME=lazers-selftest.img bash tools/scripts/run-qemu-headless.sh

check:
    source tools/scripts/user-packages.sh ; \
    load_user_packages ; \
    cargo check --package boot-info ; \
    cargo check --package elf ; \
    cargo check --package liblazer --target {{kernel_target}} ; \
    cargo check --package uefi-loader --target {{loader_target}} ; \
    for package in "${USER_PACKAGES[@]}"; do \
        cargo check --package "${package}" --target {{kernel_target}} ; \
    done ; \
    cargo check --package kernel --target {{kernel_target}}

test: check
    cargo test --package kernel --lib
    cargo test --package lash

clean:
    cargo clean
    rm -rf build/

workspace-metadata:
    cargo metadata --no-deps --format-version 1 > /dev/null

tree:
    find . -maxdepth 3 -type d | sort

docs-dev:
    bunx vitepress dev docs

docs-build:
    bunx vitepress build docs

docs:
    bunx vitepress preview docs
