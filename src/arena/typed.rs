//! Arena allocator for a single value type `T`.
//!
//! `TypedArena` provides fast allocation for homogenous data and
//! ensures that destructors are run when the arena is dropped.

extern crate alloc;

use alloc::alloc::{
  AllocError,
  Allocator,
  Global,
  Layout,
};
use core::{
  mem,
  ptr::NonNull,
};

use crate::arena::base::Arena as BaseArena;

#[derive(Debug)]
/// Arena allocator that stores values of type `T`.
pub struct TypedArena<'arena, T, A: Allocator = Global>
where
  T: Sized,
{
  /// Underlying arena implementation.
  base: BaseArena<'arena, T, A, true>,
}

impl<'arena, T, A> TypedArena<'arena, T, A>
where
  T: Sized,
  A: Allocator,
{
  /// Create a new arena using the given allocator and chunk capacity.
  pub fn new_in(allocator: A, chunk_cap: usize) -> Self {
    Self {
      base: BaseArena::new_in(allocator, chunk_cap),
    }
  }

  /// Try to allocate a value within the arena.
  pub fn try_alloc(&self, value: T) -> Result<&'arena mut T, AllocError> {
    let layout = Layout::new::<T>();
    let raw = self.base.allocate(layout)?;
    let ptr = raw.as_ptr() as *mut T;
    unsafe {
      ptr.write(value);
      Ok(&mut *ptr)
    }
  }

  /// Allocate a value, panicking if allocation fails.
  pub fn alloc(&self, value: T) -> &'arena mut T {
    self
      .try_alloc(value)
      .expect("Failed to allocate in TypedArena")
  }
}

impl<'arena, T> TypedArena<'arena, T, Global>
where
  T: Sized,
{
  /// Create a new arena backed by the global allocator.
  pub fn new(chunk_cap: usize) -> Self {
    Self::new_in(Global, chunk_cap)
  }
}

unsafe impl<'arena, T, A> Allocator for TypedArena<'arena, T, A>
where
  T: Sized,
  A: Allocator,
{
  fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
    assert_eq!(layout.size(), mem::size_of::<T>());
    assert_eq!(layout.align(), mem::align_of::<T>());
    self.base.allocate(layout)
  }

  unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
    unsafe { self.base.deallocate(ptr, layout) }
  }

  unsafe fn grow(
    &self,
    _ptr: NonNull<u8>,
    _old_layout: Layout,
    _new_layout: Layout,
  ) -> Result<NonNull<[u8]>, AllocError> {
    Err(AllocError)
  }

  unsafe fn shrink(
    &self,
    _ptr: NonNull<u8>,
    _old_layout: Layout,
    _new_layout: Layout,
  ) -> Result<NonNull<[u8]>, AllocError> {
    Err(AllocError)
  }
}
