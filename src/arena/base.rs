//! Internal arena allocator backing [`TypedArena`] and [`DroplessArena`].

extern crate alloc;

use alloc::alloc::{
  AllocError,
  Allocator,
  Layout,
};
use core::{
  cell::UnsafeCell,
  ptr::{
    self,
    NonNull,
  },
};

use crate::{
  arena::chunk::Chunk as RawChunk,
  once::Once,
};

type Chunk<T, A, const DROP: bool> = RawChunk<A, T, DROP>;

#[derive(Debug)]
/// Core arena structure used by public arena types.
pub(crate) struct Arena<T, A: Allocator, const DROP: bool>
where
  T: Sized,
{
  /// Interior mutable state of the arena.
  inner: UnsafeCell<ArenaInner<T, A, DROP>>,
}

#[derive(Debug)]
struct ArenaInner<T, A: Allocator, const DROP: bool>
where
  T: Sized,
{
  allocator: A,
  chunk_cap: usize,
  head: Once<NonNull<Chunk<T, A, DROP>>>,
  layout: Layout,
}

impl<T, A, const DROP: bool> Arena<T, A, DROP>
where
  T: Sized,
  A: Allocator,
{
  unsafe fn inner_mut(&self) -> &mut ArenaInner<T, A, DROP> {
    // SAFETY: callers ensure exclusive access
    unsafe { &mut *self.inner.get() }
  }

  pub(crate) fn new_in(allocator: A, chunk_cap: usize) -> Self {
    let layout = Layout::new::<Chunk<T, A, DROP>>();
    Self {
      inner: UnsafeCell::new(ArenaInner {
        allocator,
        chunk_cap,
        head: Once::Uninit,
        layout,
      }),
    }
  }

  fn alloc_chunk(
    &self,
    inner: &mut ArenaInner<T, A, DROP>,
    prev: Option<NonNull<Chunk<T, A, DROP>>>,
  ) -> Result<NonNull<Chunk<T, A, DROP>>, AllocError> {
    let chunk_ptr = inner.allocator.allocate(inner.layout)?;
    let chunk = chunk_ptr.as_ptr() as *mut Chunk<T, A, DROP>;
    let allocator = &inner.allocator as *const A;
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

  fn alloc_impl(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
    let inner = unsafe { self.inner_mut() };
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

  unsafe fn dealloc_impl(&self, ptr: NonNull<u8>, layout: Layout) {
    let inner = unsafe { self.inner_mut() };
    if let Some(mut current) = inner.head.get().copied() {
      loop {
        // SAFETY: current points to a valid chunk
        unsafe {
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
  }

  unsafe fn grow_impl(
    &self,
    ptr: NonNull<u8>,
    old_layout: Layout,
    new_layout: Layout,
  ) -> Result<NonNull<[u8]>, AllocError> {
    let head = unsafe { self.inner_mut() }.head.get().copied();
    if let Some(mut current) = head {
      loop {
        // SAFETY: current points to a valid chunk
        unsafe {
          if current.as_ref().contains(ptr.as_ptr()) {
            match current.as_ref().grow(ptr, old_layout, new_layout) {
              Ok(res) => return Ok(res),
              Err(_) => {
                let new_block = self.alloc_impl(new_layout)?;
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
    }
    let new_block = self.alloc_impl(new_layout)?;
    // SAFETY: new_block is valid for new_layout bytes, ptr is valid for old_layout bytes
    unsafe {
      ptr::copy_nonoverlapping(
        ptr.as_ptr(),
        new_block.as_ptr() as *mut u8,
        old_layout.size(),
      );
    }
    Ok(new_block)
  }

  unsafe fn shrink_impl(
    &self,
    ptr: NonNull<u8>,
    old_layout: Layout,
    new_layout: Layout,
  ) -> Result<NonNull<[u8]>, AllocError> {
    let head = unsafe { self.inner_mut() }.head.get().copied();
    if let Some(mut current) = head {
      loop {
        // SAFETY: current points to a valid chunk
        unsafe {
          if current.as_ref().contains(ptr.as_ptr()) {
            match current.as_ref().shrink(ptr, old_layout, new_layout) {
              Ok(res) => return Ok(res),
              Err(_) => {
                let new_block = self.alloc_impl(new_layout)?;
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
    }
    let new_block = self.alloc_impl(new_layout)?;
    // SAFETY: new_block is valid for new_layout bytes, ptr is valid for old_layout bytes
    unsafe {
      ptr::copy_nonoverlapping(
        ptr.as_ptr(),
        new_block.as_ptr() as *mut u8,
        new_layout.size(),
      );
    }
    Ok(new_block)
  }
}

impl<T, A, const DROP: bool> Drop for Arena<T, A, DROP>
where
  T: Sized,
  A: Allocator,
{
  fn drop(&mut self) {
    let inner = unsafe { self.inner_mut() };
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

unsafe impl<T, A, const DROP: bool> Allocator for Arena<T, A, DROP>
where
  T: Sized,
  A: Allocator,
{
  fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
    self.alloc_impl(layout)
  }

  unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
    unsafe { self.dealloc_impl(ptr, layout) }
  }

  unsafe fn grow(
    &self,
    ptr: NonNull<u8>,
    old_layout: Layout,
    new_layout: Layout,
  ) -> Result<NonNull<[u8]>, AllocError> {
    unsafe { self.grow_impl(ptr, old_layout, new_layout) }
  }

  unsafe fn shrink(
    &self,
    ptr: NonNull<u8>,
    old_layout: Layout,
    new_layout: Layout,
  ) -> Result<NonNull<[u8]>, AllocError> {
    unsafe { self.shrink_impl(ptr, old_layout, new_layout) }
  }
}
