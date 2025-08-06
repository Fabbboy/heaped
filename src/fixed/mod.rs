//! Simple fixed-size bump allocator operating on a user-provided buffer.

use alloc::alloc::{
  AllocError,
  Allocator,
  Layout,
};
use core::{
  cell::UnsafeCell,
  ptr,
  ptr::NonNull,
};

struct FixedInner<'fixed> {
  mem: &'fixed mut [u8],
  used: usize,
  capacity: usize,
}

/// Allocator that hands out memory from a fixed slice.
pub struct FixedAllocator<'fixed> {
  /// Interior mutable state tracking the buffer.
  inner: UnsafeCell<FixedInner<'fixed>>,
}

impl<'fixed> FixedAllocator<'fixed> {
  /// Create a new allocator from the given memory slice.
  pub fn new(mem: &'fixed mut [u8]) -> Self {
    let capacity = mem.len();
    let inner = FixedInner {
      mem,
      used: 0,
      capacity,
    };

    Self {
      inner: UnsafeCell::new(inner),
    }
  }

  fn get(&self) -> &FixedInner<'fixed> {
    unsafe { &*self.inner.get() }
  }

  fn get_mut(&self) -> &mut FixedInner<'fixed> {
    unsafe { &mut *self.inner.get() }
  }

  /// Total capacity of the underlying buffer.
  pub fn capacity(&self) -> usize {
    self.get().capacity
  }

  /// Amount of memory already allocated.
  pub fn used(&self) -> usize {
    self.get().used
  }

  /// Remaining capacity in bytes.
  pub fn available(&self) -> usize {
    let inner = self.get();
    inner.capacity - inner.used
  }

  /// Reset the allocator, forgetting all previous allocations.
  ///
  /// # Safety
  /// Caller must ensure no allocated memory is still in use.
  pub unsafe fn reset(&self) {
    let inner = self.get_mut();
    inner.used = 0;
  }
}

unsafe impl<'fixed> Allocator for FixedAllocator<'fixed> {
  fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
    let inner = self.get_mut();

    let start = inner.used;
    let align = layout.align();
    let size = layout.size();

    let aligned_start = (start + align - 1) & !(align - 1);

    if aligned_start + size > inner.capacity {
      return Err(AllocError);
    }

    inner.used = aligned_start + size;

    let ptr = unsafe { NonNull::new_unchecked(inner.mem.as_mut_ptr().add(aligned_start)) };

    Ok(NonNull::slice_from_raw_parts(ptr, size))
  }

  unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
    let inner = self.get_mut();

    let ptr_addr = ptr.as_ptr() as usize;
    let mem_start = inner.mem.as_ptr() as usize;

    if ptr_addr < mem_start || ptr_addr >= mem_start + inner.capacity {
      return;
    }

    let offset = ptr_addr - mem_start;
    let size = layout.size();

    if offset + size == inner.used {
      inner.used = offset;
    }
  }

  unsafe fn grow(
    &self,
    ptr: NonNull<u8>,
    old_layout: Layout,
    new_layout: Layout,
  ) -> Result<NonNull<[u8]>, AllocError> {
    let inner = self.get_mut();

    let ptr_addr = ptr.as_ptr() as usize;
    let mem_start = inner.mem.as_ptr() as usize;

    if ptr_addr < mem_start || ptr_addr >= mem_start + inner.capacity {
      return Err(AllocError);
    }

    let offset = ptr_addr - mem_start;
    let old_size = old_layout.size();
    let new_size = new_layout.size();

    if new_size <= old_size {
      return Ok(NonNull::slice_from_raw_parts(ptr, new_size));
    }

    if offset + old_size == inner.used {
      let additional_size = new_size - old_size;
      if inner.used + additional_size <= inner.capacity {
        inner.used += additional_size;
        return Ok(NonNull::slice_from_raw_parts(ptr, new_size));
      }
    }

    let new_ptr = self.allocate(new_layout)?;
    unsafe {
      ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_ptr().cast::<u8>(), old_size);
      self.deallocate(ptr, old_layout);
    }

    Ok(new_ptr)
  }

  unsafe fn shrink(
    &self,
    ptr: NonNull<u8>,
    old_layout: Layout,
    new_layout: Layout,
  ) -> Result<NonNull<[u8]>, AllocError> {
    let inner = self.get_mut();

    let ptr_addr = ptr.as_ptr() as usize;
    let mem_start = inner.mem.as_ptr() as usize;

    if ptr_addr < mem_start || ptr_addr >= mem_start + inner.capacity {
      return Err(AllocError);
    }

    let offset = ptr_addr - mem_start;
    let old_size = old_layout.size();
    let new_size = new_layout.size();

    debug_assert!(new_size <= old_size);

    if offset + old_size == inner.used {
      inner.used = offset + new_size;
    }

    Ok(NonNull::slice_from_raw_parts(ptr, new_size))
  }
}

#[cfg(test)]
mod tests;
