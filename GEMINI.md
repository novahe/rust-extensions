# Gemini CLI - containerd Rust Extensions Context

This document provides essential context and instructions for the `rust-extensions` project, a collection of Rust crates designed to extend `containerd`.

## Project Overview

The `rust-extensions` project is a Rust workspace containing several crates that provide bindings, shims, and tools for `containerd`. It is primarily used to implement custom runtime v2 shims, sandboxers, and other extensions in Rust.

### Key Crates

- **`containerd-shim`**: A library to ease runtime v2 shim implementation, replicating the `shim.Run` API from Go.
- **`containerd-shim-protos`**: TTRPC bindings to shim interfaces.
- **`containerd-client`**: GRPC bindings to containerd APIs.
- **`containerd-snapshots`**: Remote snapshotter implementation for containerd.
- **`runc`**: A Rust wrapper for the `runc` CLI.
- **`containerd-runc-shim`**: A runtime v2 runc shim implementation using the `containerd-shim` crate.
- **`containerd-sandbox`**: Extension for containerd sandboxers.
- **`containerd-shim-logging`**: Shim logger plugins.

### Architecture

The project follows a modular architecture where each crate handles a specific aspect of the containerd ecosystem. It uses `TTRPC` for low-latency communication between containerd and its shims.

## Building and Running

### Prerequisites

- **Rust**: MSRV is documented in `rust-toolchain.toml` (currently 1.81).
- **Protoc**: Required for generating code from `.proto` files.
- **Sudo**: Many tests and examples require root privileges to manage system resources like cgroups and namespaces.

### Commands

- **Build all crates:** `cargo build --release`
- **Run tests:** `sudo -E $(command -v cargo) test --all-features` (Sudo is often required for cgroup/namespace tests).
- **Check examples:** `cargo check --examples --all-targets`
- **Linting:** `cargo clippy --all-targets --all-features -- -D warnings`
- **Formatting:** `cargo +nightly fmt --all` (Nightly is used for formatting checks in CI).
- **Documentation:** `cargo doc --no-deps`

## Development Conventions

- **Workspace Inheritance:** Common fields like `license`, `repository`, and `edition` are inherited from the root `Cargo.toml`.
- **Error Handling:** Uses `anyhow` and `thiserror` for error management.
- **Async Support:** Many crates (like `shim`) support an `async` feature, typically using `tokio`.
- **Release Profile:** Configured with `panic = 'abort'` to keep binaries small.
- **Dependency Management:** Uses `cargo deny` for security and license checks.
- **Patching:** TTRPC dependencies are patched in the workspace root to use specific versions from `kuasar-io/ttrpc-rust`.

### Coding Style

- Adhere to `rustfmt` and `clippy` configurations found in the root directory.
- Use the `Apache-2.0` license header in new files.
- Prefer idiomatic Rust patterns while maintaining compatibility with the Go-based containerd ecosystem where necessary.

### Testing Guidelines

- New features should include unit tests.
- Tests that require system-level access should be gated or documented as requiring `sudo`.
- Integration tests against `containerd` are performed in CI and may require a local `containerd` installation.
