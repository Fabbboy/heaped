//! Arena allocator variants.
//!
//! This module provides arena implementations used throughout the
//! crate. The main public types are [`dropless::DroplessArena`] and
//! [`typed::TypedArena`].

pub(crate) mod base;
pub(crate) mod chunk;
pub mod dropless;
pub mod typed;

#[cfg(test)]
mod tests;
