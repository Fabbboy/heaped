use alloc::{
  alloc::{
    AllocError,
    Allocator,
    Global,
  },
  vec::Vec,
};
use core::{
  cell::RefCell,
  ptr,
};

use super::{
  chunk::ArenaChunk,
  HUGE_PAGE,
  PAGE_SIZE,
};

pub struct TypedArena<T, A = Global>
where
  A: Allocator + Clone,
{
  chunks: RefCell<Vec<ArenaChunk<T, A>, A>>,
  allocator: A,
}

impl<T, A> TypedArena<T, A>
where
  A: Allocator + Clone,
{
  pub fn new_in(allocator: A) -> Self {
    Self {
      chunks: RefCell::new(Vec::new_in(allocator.clone())),
      allocator,
    }
  }

  fn grow(&self, additional: usize) -> Result<(), AllocError> {
    let mut chunks = self.chunks.borrow_mut();
    
    let new_cap = if chunks.is_empty() {
      let default_cap = PAGE_SIZE / core::mem::size_of::<T>().max(1);
      default_cap.max(additional).max(1)
    } else {
      let last_cap = chunks.last().unwrap().capacity();
      let max_cap = HUGE_PAGE / core::mem::size_of::<T>().max(1);
      (last_cap * 2).min(max_cap).max(additional)
    };

    let new_chunk = ArenaChunk::try_new_in(new_cap, self.allocator.clone())?;
    chunks.push(new_chunk);
    Ok(())
  }

  pub fn alloc(&self, value: T) -> Result<&mut T, AllocError> {
    let mut chunks = self.chunks.borrow_mut();
    
    if chunks.is_empty() {
      drop(chunks);
      self.grow(1)?;
      chunks = self.chunks.borrow_mut();
    }

    loop {
      if let Some(last_chunk) = chunks.last_mut()
        && let Ok(slot) = last_chunk.alloc() {
          unsafe {
            let ptr = slot.as_mut_ptr();
            ptr::write(ptr, value);
            return Ok(&mut *ptr);
          }
        }
      
      drop(chunks);
      self.grow(1)?;
      chunks = self.chunks.borrow_mut();
    }
  }

  pub fn alloc_slice(&self, slice: &[T]) -> Result<&mut [T], AllocError>
  where
    T: Copy,
  {
    if slice.is_empty() {
      return Ok(&mut []);
    }

    let len = slice.len();
    let mut chunks = self.chunks.borrow_mut();
    
    if chunks.is_empty() {
      drop(chunks);
      self.grow(len)?;
      chunks = self.chunks.borrow_mut();
    }

    loop {
      if let Some(last_chunk) = chunks.last_mut()
        && let Ok(slots) = last_chunk.alloc_slice(len) {
          unsafe {
            for (i, &item) in slice.iter().enumerate() {
              let ptr = slots[i].as_mut_ptr();
              ptr::write(ptr, item);
            }
            let ptr = slots[0].as_mut_ptr();
            return Ok(core::slice::from_raw_parts_mut(ptr, len));
          }
        }
      
      drop(chunks);
      self.grow(len)?;
      chunks = self.chunks.borrow_mut();
    }
  }

  pub fn clear(&mut self) {
    for chunk in self.chunks.borrow_mut().iter_mut() {
      for i in 0..chunk.entries() {
        unsafe {
          let ptr = chunk.get_storage_ptr().add(i);
          ptr::drop_in_place((*ptr).as_mut_ptr());
        }
      }
      chunk.clear();
    }
  }
}

impl<T, A> Drop for TypedArena<T, A>
where
  A: Allocator + Clone,
{
  fn drop(&mut self) {
    for chunk in self.chunks.borrow_mut().iter_mut() {
      for i in 0..chunk.entries() {
        unsafe {
          let ptr = chunk.get_storage_ptr().add(i);
          ptr::drop_in_place((*ptr).as_mut_ptr());
        }
      }
    }
  }
}

impl<T> TypedArena<T, Global> {
  pub fn new() -> Self {
    Self::new_in(Global)
  }
}

impl<T, A> Default for TypedArena<T, A>
where
  A: Allocator + Default + Clone,
{
  fn default() -> Self {
    Self::new_in(A::default())
  }
}
