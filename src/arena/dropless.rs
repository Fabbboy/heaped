extern crate alloc;

use alloc::alloc::{AllocError, Allocator, Global, Layout};
use core::ptr::NonNull;

use crate::arena::base::Arena as BaseArena;

#[derive(Debug)]
pub struct DroplessArena<'arena, A: Allocator = Global> {
  base: BaseArena<'arena, u8, A, false>,
}

impl<'arena, A> DroplessArena<'arena, A>
where
  A: Allocator,
{
  pub fn new_in(allocator: A, chunk_cap: usize) -> Self {
    Self {
      base: BaseArena::new_in(allocator, chunk_cap),
    }
  }

  pub fn try_alloc_str(&self, value: &str) -> Result<&'arena mut str, AllocError> {
    let layout = Layout::array::<u8>(value.len()).map_err(|_| AllocError)?;
    let mem = self.allocate_zeroed(layout)?;
    let slice = unsafe { core::slice::from_raw_parts_mut(mem.as_ptr() as *mut u8, value.len()) };
    slice.copy_from_slice(value.as_bytes());
    Ok(unsafe { core::str::from_utf8_unchecked_mut(slice) })
  }

  pub fn alloc_str(&self, value: &str) -> &'arena mut str {
    self
      .try_alloc_str(value)
      .expect("Failed to allocate string")
  }
}

impl<'arena> DroplessArena<'arena, Global> {
  pub fn new(chunk_cap: usize) -> Self {
    Self::new_in(Global, chunk_cap)
  }
}

unsafe impl<'arena, A> Allocator for DroplessArena<'arena, A>
where
  A: Allocator,
{
  fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
    self.base.allocate(layout)
  }

  unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
    unsafe { self.base.deallocate(ptr, layout) }
  }

  unsafe fn grow(
    &self,
    ptr: NonNull<u8>,
    old_layout: Layout,
    new_layout: Layout,
  ) -> Result<NonNull<[u8]>, AllocError> {
    unsafe { self.base.grow(ptr, old_layout, new_layout) }
  }

  unsafe fn shrink(
    &self,
    ptr: NonNull<u8>,
    old_layout: Layout,
    new_layout: Layout,
  ) -> Result<NonNull<[u8]>, AllocError> {
    unsafe { self.base.shrink(ptr, old_layout, new_layout) }
  }
}
