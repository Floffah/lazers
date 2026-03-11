---
# https://vitepress.dev/reference/default-theme-home-page
layout: home

hero:
  name: "Lazers"
  text: Hobbyist Operating System
  tagline: Written from scratch in Rust for the love of the game
  actions:
    - theme: brand
      text: Getting Started
      link: /usage
    - theme: alt
      text: Architecture
      link: /architecture

features:
  - title: UEFI Boot
    details: Real UEFI boot path on x86_64 with a custom loader
  - title: Kernel and Userland
    details: Cooperative multitasking kernel with a real user-mode boundary and syscall handling
  - title: Disk and Filesystem
    details: AHCI/SATA disk access with a runtime root filesystem mounted from a GPT partition
---

