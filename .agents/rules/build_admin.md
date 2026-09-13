---
name: build-admin
description: How to build the admin app
trigger: always_on
---

# Building the Admin App

When building or checking the admin app (e.g. to verify it compiles), you MUST use `dx build -p admin` instead of `cargo check` or `cargo build`. 

This is because the admin app targets WebAssembly (Wasm) and uses web features that may fail to compile using standard native targets.
