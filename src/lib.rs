//! A collection of `no_std` friendly allocators.
//!
//! The crate provides arena allocators, a fixed-size bump allocator,
//! a slab allocator, and a simple bitmap implementation. All
//! structures are designed to operate with the [`core`] and [`alloc`]
//! crates only, making them suitable for constrained environments.

#![feature(allocator_api)]
#![feature(dropck_eyepatch)]
#![allow(clippy::module_inception, clippy::mut_from_ref)]

extern crate alloc;

pub mod arena;
pub mod bitmap;
pub mod fixed;
pub mod once;
