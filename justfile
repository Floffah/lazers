set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

loader_target := "x86_64-unknown-uefi"
kernel_target := "x86_64-unknown-none"
kernel_rustflags := "-C relocation-model=static -C link-arg=-Tkernel/lazers-kernel/linker.ld -C link-arg=-no-pie -C link-arg=--build-id=none -C link-arg=-z -C link-arg=max-page-size=0x1000"
user_rustflags := "-C relocation-model=static -C link-arg=-Tuser/lazers-user-echo/linker.ld -C link-arg=-no-pie -C link-arg=--build-id=none -C link-arg=-z -C link-arg=max-page-size=0x1000"
user_elf_path := "build/lazers-user-echo"

default:
    @just --list

setup-toolchain:
    rustup target add {{loader_target}} {{kernel_target}}

build-loader:
    cargo build --release --package uefi-loader --target {{loader_target}}

build-user:
    mkdir -p build
    RUSTFLAGS='{{user_rustflags}}' cargo build --release --package lazers-user-echo --target {{kernel_target}}
    cp target/{{kernel_target}}/release/lazers-user-echo {{user_elf_path}}

build-kernel: build-user
    LAZERS_USER_ECHO_ELF='{{user_elf_path}}' RUSTFLAGS='{{kernel_rustflags}}' cargo build --release --package lazers-kernel --target {{kernel_target}}

image: build-loader build-kernel
    tools/scripts/build-image.sh

build: image

run: image
    bash tools/scripts/run-qemu.sh

run-headless: image
    bash tools/scripts/run-qemu-headless.sh

check:
    cargo check --package boot-info
    cargo check --package lazers-elf
    cargo check --package uefi-loader --target {{loader_target}}
    cargo check --package lazers-user-echo --target {{kernel_target}}
    cargo check --package lazers-kernel --target {{kernel_target}}

clean:
    cargo clean
    rm -rf build/

workspace-metadata:
    cargo metadata --no-deps --format-version 1 > /dev/null

tree:
    find . -maxdepth 3 -type d | sort
