extern crate alloc;

use alloc::alloc::{AllocError, Allocator, Global, Layout};
use core::{
  cell::UnsafeCell,
  ptr::{self, NonNull},
};

use crate::{arena::chunk::Chunk as RawChunk, once::Once};

type Chunk<'arena, A> = RawChunk<&'arena A, u8, false>;

struct DroplessArenaInner<'arena, A: Allocator> {
  allocator: A,
  chunk_cap: usize,
  head: Once<NonNull<Chunk<'arena, A>>>,
  layout: Layout,
}

pub struct DroplessArena<'arena, A: Allocator = Global> {
  inner: UnsafeCell<DroplessArenaInner<'arena, A>>,
}

impl<'arena, A> DroplessArena<'arena, A>
where
  A: Allocator,
{
  fn inner_mut(&self) -> &mut DroplessArenaInner<'arena, A> {
    // SAFETY: callers ensure exclusive access
    unsafe { &mut *self.inner.get() }
  }

  pub fn new_in(allocator: A, chunk_cap: usize) -> Self {
    let layout = Layout::new::<Chunk<'arena, A>>();
    Self {
      inner: UnsafeCell::new(DroplessArenaInner {
        allocator,
        chunk_cap,
        head: Once::Uninit,
        layout,
      }),
    }
  }

  fn alloc_chunk(
    &self,
    inner: &mut DroplessArenaInner<'arena, A>,
    prev: Option<NonNull<Chunk<'arena, A>>>,
  ) -> Result<NonNull<Chunk<'arena, A>>, AllocError> {
    let chunk_ptr = inner.allocator.allocate(inner.layout)?;
    let chunk = chunk_ptr.as_ptr() as *mut Chunk<'arena, A>;
    let allocator: &'arena A = unsafe { &*(&inner.allocator as *const A) };
    // SAFETY: chunk points to memory large enough for Chunk
    let non_null = unsafe {
      chunk.write(RawChunk::new(allocator, inner.chunk_cap));
      NonNull::new_unchecked(chunk)
    };

    if let Some(prev_chunk) = prev {
      // SAFETY: prev_chunk and non_null are valid chunks
      unsafe {
        prev_chunk.as_ref().set_next(Some(non_null));
        non_null.as_ref().set_prev(Some(prev_chunk));
      }
    }

    Ok(non_null)
  }
}

impl<'arena> DroplessArena<'arena, Global> {
  pub fn new(chunk_cap: usize) -> Self {
    Self::new_in(Global, chunk_cap)
  }
}

impl<'arena, A> Drop for DroplessArena<'arena, A>
where
  A: Allocator,
{
  fn drop(&mut self) {
    let inner = self.inner_mut();
    if let Some(chunk) = inner.head.get() {
      // SAFETY: chunk is the head of a valid list
      unsafe {
        let mut current = chunk.as_ptr();
        while !current.is_null() {
          let next = (&*current).next();
          ptr::drop_in_place(current);
          inner
            .allocator
            .deallocate(NonNull::new_unchecked(current as *mut u8), inner.layout);
          current = next.map_or(ptr::null_mut(), |n| n.as_ptr());
        }
      }
    }
  }
}

unsafe impl<'arena, A> Allocator for DroplessArena<'arena, A>
where
  A: Allocator,
{
  fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
    let inner = self.inner_mut();
    let mut current = match inner.head.get() {
      Some(h) => *h,
      None => {
        let new_head = self.alloc_chunk(inner, None)?;
        let _ = inner.head.init(new_head);
        new_head
      }
    };

    loop {
      // SAFETY: current points to a valid chunk
      unsafe {
        if current.as_ref().has_space(layout) {
          return current.as_ref().allocate(layout);
        }
        if let Some(next) = current.as_ref().next() {
          current = next;
        } else {
          let new = self.alloc_chunk(inner, Some(current))?;
          return new.as_ref().allocate(layout);
        }
      }
    }
  }

  unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
    let inner = self.inner_mut();
    if let Some(mut current) = inner.head.get().copied() {
      loop {
        if current.as_ref().contains(ptr.as_ptr()) {
          current.as_ref().deallocate(ptr, layout);
          break;
        }
        match current.as_ref().next() {
          Some(next) => current = next,
          None => break,
        }
      }
    }
  }

  unsafe fn grow(
    &self,
    ptr: NonNull<u8>,
    old_layout: Layout,
    new_layout: Layout,
  ) -> Result<NonNull<[u8]>, AllocError> {
    let head = self.inner_mut().head.get().copied();
    if let Some(mut current) = head {
      loop {
        if current.as_ref().contains(ptr.as_ptr()) {
          match current.as_ref().grow(ptr, old_layout, new_layout) {
            Ok(res) => return Ok(res),
            Err(_) => {
              let new_block = self.allocate(new_layout)?;
              ptr::copy_nonoverlapping(
                ptr.as_ptr(),
                new_block.as_ptr() as *mut u8,
                old_layout.size(),
              );
              current.as_ref().deallocate(ptr, old_layout);
              return Ok(new_block);
            }
          }
        }
        match current.as_ref().next() {
          Some(next) => current = next,
          None => break,
        }
      }
    }
    let new_block = self.allocate(new_layout)?;
    ptr::copy_nonoverlapping(
      ptr.as_ptr(),
      new_block.as_ptr() as *mut u8,
      old_layout.size(),
    );
    Ok(new_block)
  }

  unsafe fn shrink(
    &self,
    ptr: NonNull<u8>,
    old_layout: Layout,
    new_layout: Layout,
  ) -> Result<NonNull<[u8]>, AllocError> {
    let head = self.inner_mut().head.get().copied();
    if let Some(mut current) = head {
      loop {
        if current.as_ref().contains(ptr.as_ptr()) {
          match current.as_ref().shrink(ptr, old_layout, new_layout) {
            Ok(res) => return Ok(res),
            Err(_) => {
              let new_block = self.allocate(new_layout)?;
              ptr::copy_nonoverlapping(
                ptr.as_ptr(),
                new_block.as_ptr() as *mut u8,
                new_layout.size(),
              );
              current.as_ref().deallocate(ptr, old_layout);
              return Ok(new_block);
            }
          }
        }
        match current.as_ref().next() {
          Some(next) => current = next,
          None => break,
        }
      }
    }
    let new_block = self.allocate(new_layout)?;
    ptr::copy_nonoverlapping(
      ptr.as_ptr(),
      new_block.as_ptr() as *mut u8,
      new_layout.size(),
    );
    Ok(new_block)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_dropless_arena() {
    let arena = DroplessArena::new(1024);
    let string_layout = Layout::array::<u8>(10).unwrap();
    let mut string_raw = arena.allocate_zeroed(string_layout).unwrap();
    let string_slice = unsafe { string_raw.as_mut() };
    string_slice.copy_from_slice(b"HelloWorld");

    let str_ref = unsafe { core::str::from_utf8_unchecked(string_slice) };

    assert_eq!(string_slice, b"HelloWorld");
    assert_eq!(str_ref, "HelloWorld");
  }
}
