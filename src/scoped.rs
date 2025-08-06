//! Scoped arena management for complex lifetime scenarios.
//!
//! This module provides a simple scoped arena pattern that helps
//! manage arena lifetimes in complex scenarios like compilers.

use crate::arena::{DroplessArena, TypedArena};
use alloc::alloc::Global;

/// Execute a closure with properly scoped arenas.
/// 
/// This function creates all the arenas needed for compilation,
/// passes them to the closure, and ensures they're all dropped
/// in the correct order after the closure completes.
/// 
/// This pattern avoids the need for ManuallyDrop by using function
/// scope to control arena lifetimes naturally.
pub fn with_arenas<R, F>(f: F) -> R
where
    F: for<'arena> FnOnce(
        &'arena DroplessArena<Global>,
        &'arena TypedArena<(), Global>, // Placeholder for now, will be specialized
    ) -> R,
{
    let dropless = DroplessArena::new(4096);
    let typed = TypedArena::new(1024);
    
    f(&dropless, &typed)
}

/// A more flexible scoped arena executor that allows creating
/// multiple typed arenas as needed.
/// 
/// Usage:
/// ```
/// let result = scoped_arenas(|builder| {
///     let strings = builder.dropless();
///     let arena1 = builder.typed_arena::<MyType>();
///     let arena2 = builder.typed_arena::<OtherType>();
///     // Use the arenas...
///     42
/// });
/// ```
pub fn scoped_arenas<R, F>(f: F) -> R
where
    F: FnOnce(&mut ArenaBuilder) -> R,
{
    let mut builder = ArenaBuilder::new();
    f(&mut builder)
}

/// Builder for creating arenas within a scoped context.
pub struct ArenaBuilder {
    dropless: DroplessArena<Global>,
}

impl ArenaBuilder {
    fn new() -> Self {
        Self {
            dropless: DroplessArena::new(4096),
        }
    }

    /// Get access to the dropless arena
    pub fn dropless(&self) -> &DroplessArena<Global> {
        &self.dropless
    }

    /// Create a new typed arena.
    /// 
    /// Note: Due to Rust's lifetime system, you need to use this
    /// pattern carefully to ensure the returned arena reference
    /// doesn't outlive the builder.
    pub fn with_typed_arena<T, R>(
        &self, 
        chunk_size: usize,
        f: impl FnOnce(&TypedArena<T, Global>) -> R
    ) -> R {
        let arena = TypedArena::new(chunk_size);
        f(&arena)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scoped_arenas() {
        let result = scoped_arenas(|builder| {
            let s = builder.dropless().alloc_str("hello");
            
            builder.with_typed_arena(1024, |arena: &TypedArena<String, Global>| {
                let val = arena.alloc("world".to_string());
                format!("{} {}", s, val)
            })
        });
        
        assert_eq!(result, "hello world");
    }
}