//! Generic arena allocator.

extern crate alloc;

use alloc::alloc::{AllocError, Allocator, Global, Layout};
use core::{
  cell::UnsafeCell,
  ptr::{self, NonNull},
};

use crate::{arena::chunk::Chunk as RawChunk, once::Once};

type Chunk<T, A, const DROP: bool> = RawChunk<A, T, DROP>;

#[derive(Debug)]
/// Core arena structure.
pub struct Arena<T, A: Allocator, const DROP: bool>
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
  /// Run a closure with a reference to this arena.
  /// This can help with lifetime management by ensuring the arena
  /// outlives any references created within the closure.
  pub fn with<R>(&self, f: impl FnOnce(&Self) -> R) -> R {
    f(self)
  }
  unsafe fn inner_mut(&self) -> &mut ArenaInner<T, A, DROP> {
    // SAFETY: callers ensure exclusive access
    unsafe { &mut *self.inner.get() }
  }

  pub fn new_in(allocator: A, chunk_cap: usize) -> Self {
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

impl<T, const DROP: bool> Arena<T, Global, DROP>
where
  T: Sized,
{
  pub fn new(chunk_cap: usize) -> Self {
    Self::new_in(Global, chunk_cap)
  }
}

impl<T, A> Arena<T, A, true>
where
  T: Sized,
  A: Allocator,
{
  pub fn try_alloc(&self, value: T) -> Result<&mut T, AllocError> {
    let layout = Layout::new::<T>();
    let raw = Allocator::allocate(self, layout)?;
    let ptr = raw.as_ptr() as *mut T;
    // SAFETY: ptr is valid for writes of T
    unsafe {
      ptr.write(value);
      Ok(&mut *ptr)
    }
  }

  pub fn alloc(&self, value: T) -> &mut T {
    self
      .try_alloc(value)
      .expect("typed arena allocation failed")
  }
}

impl<T, A> Arena<T, A, true>
where
  T: Sized + Clone,
  A: Allocator,
{
  pub fn try_alloc_slice(&self, values: &[T]) -> Result<&mut [T], AllocError> {
    let layout = Layout::array::<T>(values.len()).map_err(|_| AllocError)?;
    let raw = Allocator::allocate(self, layout)?;
    let ptr = raw.as_ptr() as *mut T;
    // SAFETY: ptr is valid for values.len() items
    unsafe {
      for (i, v) in values.iter().enumerate() {
        ptr.add(i).write(v.clone());
      }
      Ok(core::slice::from_raw_parts_mut(ptr, values.len()))
    }
  }

  pub fn alloc_slice(&self, values: &[T]) -> &mut [T] {
    self
      .try_alloc_slice(values)
      .expect("typed arena slice allocation failed")
  }
}

impl<A> Arena<u8, A, false>
where
  A: Allocator,
{
  pub fn try_alloc_bytes(&self, data: &[u8]) -> Result<&mut [u8], AllocError> {
    let layout = Layout::array::<u8>(data.len()).map_err(|_| AllocError)?;
    let mut raw = Allocator::allocate(self, layout)?;
    let slice = unsafe { raw.as_mut() };
    slice.copy_from_slice(data);
    Ok(slice)
  }

  pub fn alloc_bytes(&self, data: &[u8]) -> &mut [u8] {
    self
      .try_alloc_bytes(data)
      .expect("dropless arena byte allocation failed")
  }

  pub fn try_alloc_str(&self, value: &str) -> Result<&mut str, AllocError> {
    let slice = self.try_alloc_bytes(value.as_bytes())?;
    // SAFETY: slice originates from valid utf8
    unsafe { Ok(core::str::from_utf8_unchecked_mut(slice)) }
  }

  pub fn alloc_str(&self, value: &str) -> &mut str {
    self
      .try_alloc_str(value)
      .expect("dropless arena str allocation failed")
  }

  pub fn try_alloc_slice<T>(&self, values: &[T]) -> Result<&mut [T], AllocError>
  where
    T: Copy,
  {
    let layout = Layout::array::<T>(values.len()).map_err(|_| AllocError)?;
    let raw = Allocator::allocate(self, layout)?;
    let ptr = raw.as_ptr() as *mut T;
    // SAFETY: ptr is valid for values.len() items and T: Copy
    unsafe {
      ptr.copy_from_nonoverlapping(values.as_ptr(), values.len());
      Ok(core::slice::from_raw_parts_mut(ptr, values.len()))
    }
  }

  pub fn alloc_slice<T>(&self, values: &[T]) -> &mut [T]
  where
    T: Copy,
  {
    self
      .try_alloc_slice(values)
      .expect("dropless arena slice allocation failed")
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
          
          // Only call drop_in_place if DROP is true
          // This gives the borrow checker more flexibility with dropless arenas
          if DROP {
            ptr::drop_in_place(current);
          }
          
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
