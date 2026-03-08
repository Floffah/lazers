set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

loader_target := "x86_64-unknown-uefi"
kernel_target := "x86_64-unknown-none"
kernel_rustflags := "-C relocation-model=static -C link-arg=-Tkernel/kernel/linker.ld -C link-arg=-no-pie -C link-arg=--build-id=none -C link-arg=-z -C link-arg=max-page-size=0x1000"
user_rustflags := "-C relocation-model=static -C link-arg=-Tlibs/liblazer/linker.ld -C link-arg=-no-pie -C link-arg=--build-id=none -C link-arg=-z -C link-arg=max-page-size=0x1000"
user_elf_path := "build/echo"

default:
    @just --list

setup-toolchain:
    rustup target add {{loader_target}} {{kernel_target}}

build-loader:
    cargo build --release --package uefi-loader --target {{loader_target}}

build-user:
    mkdir -p build
    RUSTFLAGS='{{user_rustflags}}' cargo build --release --package echo --target {{kernel_target}}
    cp target/{{kernel_target}}/release/echo {{user_elf_path}}

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
    cargo check --package liblazer --target {{kernel_target}}
    cargo check --package uefi-loader --target {{loader_target}}
    cargo check --package echo --target {{kernel_target}}
    cargo check --package kernel --target {{kernel_target}}

clean:
    cargo clean
    rm -rf build/

workspace-metadata:
    cargo metadata --no-deps --format-version 1 > /dev/null

tree:
    find . -maxdepth 3 -type d | sort
