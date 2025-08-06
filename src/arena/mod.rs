//! Arena allocator variants.

extern crate alloc;

use alloc::alloc::Global;

mod base;
pub(crate) mod chunk;

pub use base::Arena;
pub type TypedArena<T, A = Global> = Arena<T, A, true>;
pub type DroplessArena<A = Global> = Arena<u8, A, false>;

#[cfg(test)]
mod tests;
