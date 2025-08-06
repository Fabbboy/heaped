/*
arena chunk the heart of both typed and dropless arenas
it should provide efficient allocation and also provide abilty to deallocate tail allocations to provide memory reuse
*/

use alloc::{
  alloc::{
    AllocError,
    Allocator,
    Global,
  },
  boxed::Box,
};
use core::{
  mem::MaybeUninit,
  ptr::NonNull,
};

pub(crate) struct ArenaChunk<T, A = Global>
where
  A: Allocator,
{
  storage: NonNull<[MaybeUninit<T>]>,
  entries: usize,
  allocator: A,
}

impl<T, A> ArenaChunk<T, A>
where
  A: Allocator,
{
  pub fn try_new_in(capacity: usize, allocator: A) -> Result<Self, AllocError> {
    let slice = Box::try_new_uninit_slice_in(capacity, &allocator)?;
    let storage = NonNull::from(Box::leak(slice));
    Ok(Self {
      storage,
      entries: 0,
      allocator,
    })
  }

  pub fn capacity(&self) -> usize {
    self.storage.len()
  }

  pub fn entries(&self) -> usize {
    self.entries
  }

  pub fn alloc(&mut self) -> Result<&mut MaybeUninit<T>, AllocError> {
    if self.entries < self.capacity() {
      unsafe {
        let ptr = self.storage.as_ptr().cast::<MaybeUninit<T>>().add(self.entries);
        self.entries += 1;
        Ok(&mut *ptr)
      }
    } else {
      Err(AllocError)
    }
  }

  pub fn alloc_slice(&mut self, len: usize) -> Result<&mut [MaybeUninit<T>], AllocError> {
    if self.entries + len <= self.capacity() {
      unsafe {
        let ptr = self.storage.as_ptr().cast::<MaybeUninit<T>>().add(self.entries);
        self.entries += len;
        Ok(core::slice::from_raw_parts_mut(ptr, len))
      }
    } else {
      Err(AllocError)
    }
  }


  pub fn clear(&mut self) {
    self.entries = 0;
  }

  pub fn get_storage_ptr(&self) -> *mut MaybeUninit<T> {
    self.storage.as_ptr().cast::<MaybeUninit<T>>()
  }

}



impl<T, A> Drop for ArenaChunk<T, A>
where
  A: Allocator,
{
  // SAFETY: Caller is responsible for optionally dropping the contents inside the chunk.
  fn drop(&mut self) {
    unsafe { drop(Box::from_raw_in(self.storage.as_mut(), &self.allocator)) }
  }
}
