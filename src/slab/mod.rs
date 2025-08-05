//! Slab allocator providing constant-time allocation of objects.

use alloc::{
  alloc::{
    AllocError,
    Allocator,
    Global,
    Layout,
  },
  vec::Vec,
};
use core::{
  cell::UnsafeCell,
  mem::ManuallyDrop,
  ptr::{
    self,
    NonNull,
  },
};

const EMPTY: usize = usize::MAX;

union Slot<T> {
  value: ManuallyDrop<T>,
  next: usize,
}

struct SlabInner<T, A: Allocator> {
  slots: Vec<Slot<T>, A>,
  free: usize,
  len: usize,
}

/// Allocator that stores values in a slab for reuse.
pub struct SlabAllocator<T, A: Allocator = Global> {
  /// Interior mutable state of the slab.
  inner: UnsafeCell<SlabInner<T, A>>,
}

impl<T> SlabAllocator<T, Global> {
  /// Create a new slab allocator using the global allocator.
  pub fn new() -> Self {
    Self::new_in(Global)
  }
}

impl<T> Default for SlabAllocator<T, Global> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T, A: Allocator> SlabAllocator<T, A> {
  /// Create a new slab allocator using the provided allocator.
  pub fn new_in(alloc: A) -> Self {
    Self {
      inner: UnsafeCell::new(SlabInner {
        slots: Vec::new_in(alloc),
        free: EMPTY,
        len: 0,
      }),
    }
  }

  /// Try to insert a value, returning its index on success.
  pub fn try_insert(&mut self, value: T) -> Result<usize, AllocError> {
    let inner = self.inner_mut();
    let idx = inner.try_alloc_slot()?;
    inner.slots[idx].value = ManuallyDrop::new(value);
    Ok(idx)
  }

  /// Insert a value, panicking on allocation failure.
  pub fn insert(&mut self, value: T) -> usize {
    self
      .try_insert(value)
      .expect("Failed to insert into SlabAllocator")
  }

  /// Remove a value at the given index, returning it if present.
  pub fn remove(&mut self, index: usize) -> Option<T> {
    let inner = self.inner_mut();
    if index >= inner.slots.len() || inner.is_free(index) {
      return None;
    }
    inner.len -= 1;
    let value = unsafe { ManuallyDrop::into_inner(ptr::read(&inner.slots[index].value)) };
    unsafe { inner.free_slot(index) };
    Some(value)
  }

  /// Get a shared reference to the value at `index` if it exists.
  pub fn get(&self, index: usize) -> Option<&T> {
    let inner = self.inner_ref();
    if index >= inner.slots.len() || inner.is_free(index) {
      None
    } else {
      unsafe { Some(&*(&inner.slots[index].value as *const ManuallyDrop<T> as *const T)) }
    }
  }

  /// Get a mutable reference to the value at `index` if it exists.
  pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
    let inner = self.inner_mut();
    if index >= inner.slots.len() || inner.is_free(index) {
      None
    } else {
      unsafe { Some(&mut *(&mut inner.slots[index].value as *mut ManuallyDrop<T> as *mut T)) }
    }
  }

  /// Number of occupied slots in the slab.
  pub fn len(&self) -> usize {
    self.inner_ref().len
  }

  /// Check whether the slab contains no elements.
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Total capacity of the underlying storage.
  pub fn capacity(&self) -> usize {
    self.inner_ref().slots.len()
  }

  fn inner_ref(&self) -> &SlabInner<T, A> {
    unsafe { &*self.inner.get() }
  }

  fn inner_mut(&mut self) -> &mut SlabInner<T, A> {
    unsafe { &mut *self.inner.get() }
  }
}

impl<T, A: Allocator> SlabInner<T, A> {
  fn try_alloc_slot(&mut self) -> Result<usize, AllocError> {
    match self.free {
      EMPTY => {
        self.slots.try_reserve(1).map_err(|_| AllocError)?;
        self.slots.push(Slot { next: EMPTY });
        self.len += 1;
        Ok(self.slots.len() - 1)
      }
      idx => {
        self.free = unsafe { self.slots[idx].next };
        self.len += 1;
        Ok(idx)
      }
    }
  }

  unsafe fn free_slot(&mut self, index: usize) {
    self.slots[index].next = self.free;
    self.free = index;
  }

  fn is_free(&self, index: usize) -> bool {
    let mut cur = self.free;
    while cur != EMPTY {
      if cur == index {
        return true;
      }
      cur = unsafe { self.slots[cur].next };
    }
    false
  }
}

unsafe impl<T, A: Allocator> Allocator for SlabAllocator<T, A> {
  fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
    if layout.size() != core::mem::size_of::<T>() || layout.align() != core::mem::align_of::<T>() {
      return Err(AllocError);
    }
    let inner = unsafe { &mut *self.inner.get() };
    let idx = inner.try_alloc_slot()?;
    let ptr = inner.slots.as_mut_ptr();
    let ptr = unsafe { ptr.add(idx) as *mut u8 };
    let slice = ptr::slice_from_raw_parts_mut(ptr, layout.size());
    NonNull::new(slice).ok_or(AllocError)
  }

  fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
    let ptr = self.allocate(layout)?;
    let raw = ptr.as_ptr() as *mut u8;
    unsafe {
      ptr::write_bytes(raw, 0, layout.size());
    }
    Ok(ptr)
  }

  unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
    debug_assert!(
      layout.size() == core::mem::size_of::<T>() && layout.align() == core::mem::align_of::<T>()
    );
    let inner = unsafe { &mut *self.inner.get() };
    let base = inner.slots.as_ptr() as usize;
    let idx = (ptr.as_ptr() as usize - base) / core::mem::size_of::<Slot<T>>();
    inner.len -= 1;
    unsafe { inner.free_slot(idx) };
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

impl<T, A: Allocator> Drop for SlabAllocator<T, A> {
  fn drop(&mut self) {
    let inner = self.inner_mut();
    for idx in 0..inner.slots.len() {
      if !inner.is_free(idx) {
        unsafe {
          ManuallyDrop::drop(&mut inner.slots[idx].value);
        }
      }
    }
  }
}

#[cfg(test)]
mod tests;
