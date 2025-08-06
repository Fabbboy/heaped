# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Heaped is a `no_std` compatible Rust library providing memory handling and allocation types. The library operates solely on `core` and `alloc` crates, making it suitable for constrained environments and embedded systems.

## Development Commands

### Building and Testing
- `cargo build` - Build the library
- `cargo test` - Run all tests
- `cargo check` - Quick compile check
- `cargo fmt` - Format code according to project style
- `cargo clippy` - Run lints

### Toolchain
- Uses nightly Rust toolchain (see `rust-toolchain.toml`)
- Requires `#![feature(allocator_api)]` for allocator trait implementations

## Architecture

The library is organized into several key modules:

### Core Allocators
- **`fixed/`** - Fixed-size bump allocator (`FixedAllocator`) operating on user-provided buffers. Implements interior mutability via `UnsafeCell` for allocation state tracking.
- **`slab/`** - Constant-time object allocator (`SlabAllocator`) using slot-based storage with free-list management via unions.
- **`bitmap`** - Dynamically resizable bitmap for tracking boolean flags, with allocator-generic support.

### Utility Types  
- **`once`** - Single-assignment container (`Once<T>`) for values that can only be written once.

### Module Structure
- Each allocator module contains implementation in `mod.rs` with separate `tests.rs` for unit tests
- All allocators implement the standard `Allocator` trait from `alloc::alloc`
- Generic over allocator types with `Global` as default

## Development Guidelines (from AGENTS.md)

### Core Constraints
- Only use `core` and `alloc` - adjust imports accordingly
- External crates must work on `no_std`
- Avoid types relying on global allocator (`Vec`, `Box` without explicit allocator)
- Never clone allocators - they cannot be copied or cloned

### Code Style
- No comments except `SAFETY` comments for unsafe code
- Keep non-user-facing APIs private
- Use `expect` for internal failures, provide `try_*` variants for user error handling
- Use `assert` in debug builds for user guidance

### Memory Safety
- Unsafe code is allowed but final user API should be safe
- Ensure all manually allocated memory is eventually freed
- Error handling should be possible but `expect` is acceptable for internal logic

## Testing
- Individual module tests in `tests.rs` files
- Tests cover allocation, deallocation, growth, and edge cases
- Use `cargo test` to run all tests

## Formatting Configuration
- 2-space indentation
- 100 character line width
- Crate-level import granularity with vertical layout (see `rustfmt.toml`)