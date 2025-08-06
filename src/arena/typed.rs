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
pub struct TypedArena<T, A: Allocator = Global>
where
  T: Sized,
{
  base: BaseArena<T, A, true>,
}

impl<T, A> TypedArena<T, A>
where
  T: Sized,
  A: Allocator,
{
  pub fn new_in(allocator: A, chunk_cap: usize) -> Self {
    Self {
      base: BaseArena::new_in(allocator, chunk_cap),
    }
  }

  pub fn try_alloc<'a>(&'a self, value: T) -> Result<&'a mut T, AllocError> {
    let layout = Layout::new::<T>();
    let raw = self.base.allocate(layout)?;
    let ptr = raw.as_ptr() as *mut T;
    unsafe {
      ptr.write(value);
      Ok(&mut *ptr)
    }
  }

  pub fn alloc<'a>(&'a self, value: T) -> &'a mut T {
    self
      .try_alloc(value)
      .expect("Failed to allocate in TypedArena")
  }
}

impl<T> TypedArena<T, Global>
where
  T: Sized,
{
  pub fn new(chunk_cap: usize) -> Self {
    Self::new_in(Global, chunk_cap)
  }
}

unsafe impl<T, A> Allocator for TypedArena<T, A>
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
