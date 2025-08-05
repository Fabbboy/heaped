extern crate alloc;

use alloc::alloc::{
  AllocError,
  Allocator,
  Global,
  Layout,
};
use core::{
  cell::UnsafeCell,
  mem,
  ptr::{
    self,
    NonNull,
  },
};

use crate::{
  arena::chunk::Chunk as RawChunk,
  once::Once,
};

type Chunk<'arena, T, A> = RawChunk<&'arena A, T, true>;

struct TypedArenaInner<'arena, T, A: Allocator>
where
  T: Sized,
{
  allocator: A,
  chunk_cap: usize,
  head: Once<NonNull<Chunk<'arena, T, A>>>,
  layout: Layout,
}

pub struct TypedArena<'arena, T, A: Allocator = Global>
where
  T: Sized,
{
  inner: UnsafeCell<TypedArenaInner<'arena, T, A>>,
}

impl<'arena, T, A> TypedArena<'arena, T, A>
where
  T: Sized,
  A: Allocator,
{
  unsafe fn inner_mut(&self) -> &mut TypedArenaInner<'arena, T, A> {
    // SAFETY: callers ensure exclusive access
    unsafe { &mut *self.inner.get() }
  }

  pub fn new_in(allocator: A, chunk_cap: usize) -> Self {
    let layout = Layout::new::<Chunk<'arena, T, A>>();
    Self {
      inner: UnsafeCell::new(TypedArenaInner {
        allocator,
        chunk_cap,
        head: Once::Uninit,
        layout,
      }),
    }
  }

  fn alloc_chunk(
    &self,
    inner: &mut TypedArenaInner<'arena, T, A>,
    prev: Option<NonNull<Chunk<'arena, T, A>>>,
  ) -> Result<NonNull<Chunk<'arena, T, A>>, AllocError> {
    let chunk_ptr = inner.allocator.allocate(inner.layout)?;
    let chunk = chunk_ptr.as_ptr() as *mut Chunk<'arena, T, A>;
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

  pub fn try_alloc(&self, value: T) -> Result<&'arena mut T, AllocError> {
    let layout = Layout::new::<T>();
    let raw = self.allocate(layout)?;
    let ptr = raw.as_ptr() as *mut T;
    // SAFETY: ptr is valid for writes of T
    unsafe {
      ptr.write(value);
      Ok(&mut *ptr)
    }
  }

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
  pub fn new(chunk_cap: usize) -> Self {
    Self::new_in(Global, chunk_cap)
  }
}

impl<'arena, T, A> Drop for TypedArena<'arena, T, A>
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

unsafe impl<'arena, T, A> Allocator for TypedArena<'arena, T, A>
where
  T: Sized,
  A: Allocator,
{
  fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
    assert_eq!(layout.size(), mem::size_of::<T>());
    assert_eq!(layout.align(), mem::align_of::<T>());

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

  unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
    let inner = unsafe { self.inner_mut() };
    if let Some(mut current) = inner.head.get().copied() {
      loop {
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
