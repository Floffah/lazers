set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

loader_target := "x86_64-unknown-uefi"
kernel_target := "x86_64-unknown-none"
kernel_rustflags := "-C relocation-model=static -C link-arg=-Tkernel/kernel/linker.ld -C link-arg=-no-pie -C link-arg=--build-id=none -C link-arg=-z -C link-arg=max-page-size=0x1000"
user_rustflags := "-C relocation-model=static -C link-arg=-Tlibs/liblazer/linker.ld -C link-arg=-no-pie -C link-arg=--build-id=none -C link-arg=-z -C link-arg=max-page-size=0x1000"
echo_elf_path := "build/echo"
cat_elf_path := "build/cat"
lash_elf_path := "build/lash"
ls_elf_path := "build/ls"

default:
    @just --list

setup-toolchain:
    rustup target add {{loader_target}} {{kernel_target}}

build-loader:
    cargo build --release --package uefi-loader --target {{loader_target}}

build-user:
    mkdir -p build
    RUSTFLAGS='{{user_rustflags}}' cargo build --release --package cat --target {{kernel_target}}
    cp target/{{kernel_target}}/release/cat {{cat_elf_path}}
    RUSTFLAGS='{{user_rustflags}}' cargo build --release --package echo --target {{kernel_target}}
    cp target/{{kernel_target}}/release/echo {{echo_elf_path}}
    RUSTFLAGS='{{user_rustflags}}' cargo build --release --package lash --target {{kernel_target}}
    cp target/{{kernel_target}}/release/lash {{lash_elf_path}}
    RUSTFLAGS='{{user_rustflags}}' cargo build --release --package ls --target {{kernel_target}}
    cp target/{{kernel_target}}/release/ls {{ls_elf_path}}

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
    cargo check --package cat --target {{kernel_target}}
    cargo check --package echo --target {{kernel_target}}
    cargo check --package lash --target {{kernel_target}}
    cargo check --package ls --target {{kernel_target}}
    cargo check --package kernel --target {{kernel_target}}

clean:
    cargo clean
    rm -rf build/

workspace-metadata:
    cargo metadata --no-deps --format-version 1 > /dev/null

tree:
    find . -maxdepth 3 -type d | sort
