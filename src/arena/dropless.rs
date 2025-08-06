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
  mem::{
    align_of,
    size_of,
  },
};

use super::{
  chunk::ArenaChunk,
  HUGE_PAGE,
  PAGE_SIZE,
};

#[derive(Debug)]
pub struct DroplessArena<A = Global>
where
  A: Allocator + Clone,
{
  chunks: RefCell<Vec<ArenaChunk<u8, A>, A>>,
  allocator: A,
}

impl<A> DroplessArena<A>
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
      PAGE_SIZE.max(additional)
    } else {
      let last_cap = chunks.last().unwrap().capacity();
      (last_cap * 2).min(HUGE_PAGE).max(additional)
    };

    let new_chunk = ArenaChunk::try_new_in(new_cap, self.allocator.clone())?;
    chunks.push(new_chunk);
    Ok(())
  }

  pub fn alloc<T>(&self, value: T) -> Result<&mut T, AllocError> {
    let size = size_of::<T>();
    let align = align_of::<T>();
    
    let ptr = self.alloc_raw(size, align)?;
    unsafe {
      let typed_ptr = ptr.cast::<T>();
      typed_ptr.write(value);
      Ok(&mut *typed_ptr)
    }
  }

  pub fn alloc_slice<T>(&self, slice: &[T]) -> Result<&mut [T], AllocError>
  where
    T: Copy,
  {
    if slice.is_empty() {
      return Ok(&mut []);
    }

    let size = std::mem::size_of_val(slice);
    let align = align_of::<T>();
    
    let ptr = self.alloc_raw(size, align)?;
    unsafe {
      let typed_ptr = ptr.cast::<T>();
      for (i, &item) in slice.iter().enumerate() {
        typed_ptr.add(i).write(item);
      }
      Ok(core::slice::from_raw_parts_mut(typed_ptr, slice.len()))
    }
  }

  pub fn alloc_str(&self, s: &str) -> Result<&mut str, AllocError> {
    let bytes = self.alloc_slice(s.as_bytes())?;
    unsafe { Ok(core::str::from_utf8_unchecked_mut(bytes)) }
  }

  fn alloc_raw(&self, size: usize, align: usize) -> Result<*mut u8, AllocError> {
    if size == 0 {
      return Ok(align as *mut u8);
    }

    let mut chunks = self.chunks.borrow_mut();
    
    if chunks.is_empty() {
      drop(chunks);
      self.grow(size)?;
      chunks = self.chunks.borrow_mut();
    }

    loop {
      if let Some(last_chunk) = chunks.last_mut() {
        let start = last_chunk.entries();
        let aligned_start = (start + align - 1) & !(align - 1);
        let padding = aligned_start - start;
        
        if aligned_start + size <= last_chunk.capacity() {
          for _ in 0..padding {
            let _ = last_chunk.alloc();
          }
          
          let result = last_chunk.alloc_slice(size)?;
          return Ok(result.as_mut_ptr().cast::<u8>());
        }
      }
      
      drop(chunks);
      self.grow(size + align)?;
      chunks = self.chunks.borrow_mut();
    }
  }
}

impl DroplessArena<Global> {
  pub fn new() -> Self {
    Self::new_in(Global)
  }
}

impl<A> Default for DroplessArena<A>
where
  A: Allocator + Default + Clone,
{
  fn default() -> Self {
    Self::new_in(A::default())
  }
}