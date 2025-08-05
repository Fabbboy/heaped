extern crate alloc;

use alloc::alloc::{AllocError, Allocator, Layout};
use core::{
  cell::UnsafeCell,
  mem,
  mem::MaybeUninit,
  ptr::{self, NonNull},
};

struct ChunkInner<A, T, const DROP: bool>
where
  T: Sized,
  A: Allocator,
{
  allocator: A,
  prev: Option<NonNull<Chunk<A, T, DROP>>>,
  next: Option<NonNull<Chunk<A, T, DROP>>>,
  start: *mut u8,
  end: *mut u8,
  storage: NonNull<MaybeUninit<T>>,
  len: usize,
  capacity: usize,
}

pub(crate) struct Chunk<A, T = u8, const DROP: bool = false>
where
  T: Sized,
  A: Allocator,
{
  inner: UnsafeCell<ChunkInner<A, T, DROP>>,
}

impl<A, T, const DROP: bool> Chunk<A, T, DROP>
where
  T: Sized,
  A: Allocator,
{
  fn inner(&self) -> &ChunkInner<A, T, DROP> {
    // SAFETY: inner is only mutably accessed through inner_mut
    unsafe { &*self.inner.get() }
  }

  fn inner_mut(&self) -> &mut ChunkInner<A, T, DROP> {
    // SAFETY: callers ensure exclusive access
    unsafe { &mut *self.inner.get() }
  }

  pub(crate) fn try_new(allocator: A, capacity: usize) -> Result<Self, AllocError> {
    let layout = Layout::array::<MaybeUninit<T>>(capacity).map_err(|_| AllocError)?;
    let raw = allocator.allocate(layout)?;
    let storage = unsafe { NonNull::new_unchecked(raw.as_ptr() as *mut MaybeUninit<T>) };
    let start_ptr = raw.as_ptr() as *mut u8;

    Ok(Self {
      inner: UnsafeCell::new(ChunkInner {
        allocator,
        prev: None,
        next: None,
        start: start_ptr,
        end: unsafe { start_ptr.add(raw.len()) },
        storage,
        len: 0,
        capacity,
      }),
    })
  }

  pub(crate) fn new(allocator: A, capacity: usize) -> Self {
    Self::try_new(allocator, capacity)
      .unwrap_or_else(|_| panic!("Failed to allocate arena chunk of capacity {}", capacity))
  }

  pub(crate) fn has_space(&self, layout: Layout) -> bool {
    let inner = self.inner();
    let remaining = inner.capacity - inner.len;
    let needed = (layout.size() + mem::size_of::<T>() - 1) / mem::size_of::<T>();
    remaining >= needed
  }

  pub(crate) fn contains(&self, ptr: *mut u8) -> bool {
    let inner = self.inner();
    let start = inner.start as usize;
    let end = inner.end as usize;
    let ptr = ptr as usize;
    ptr >= start && ptr < end
  }

  pub(crate) fn next(&self) -> Option<NonNull<Self>> {
    self.inner().next
  }

  pub(crate) fn set_prev(&self, prev: Option<NonNull<Self>>) {
    self.inner_mut().prev = prev;
  }

  pub(crate) fn set_next(&self, next: Option<NonNull<Self>>) {
    self.inner_mut().next = next;
  }
}

unsafe impl<A, T, const DROP: bool> Allocator for Chunk<A, T, DROP>
where
  T: Sized,
  A: Allocator,
{
  fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
    if !self.has_space(layout) {
      return Err(AllocError);
    }

    let inner = self.inner_mut();
    let needed = (layout.size() + mem::size_of::<T>() - 1) / mem::size_of::<T>();
    let start = inner.len;
    inner.len += needed;

    let ptr = unsafe { inner.storage.as_ptr().add(start) as *mut u8 };
    let byte_count = needed * mem::size_of::<T>();
    // SAFETY: ptr is within storage and byte_count computed from capacity
    Ok(unsafe { NonNull::new_unchecked(ptr::slice_from_raw_parts_mut(ptr, byte_count)) })
  }

  unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
    let inner = self.inner_mut();
    let needed = (layout.size() + mem::size_of::<T>() - 1) / mem::size_of::<T>();

    let ptr_offset = ptr.as_ptr() as usize - inner.storage.as_ptr() as usize;
    let ptr_index = ptr_offset / mem::size_of::<T>();

    if ptr_index + needed == inner.len {
      if DROP {
        for i in ptr_index..inner.len {
          // SAFETY: item_ptr points to an initialized entry
          unsafe {
            let item_ptr = inner.storage.as_ptr().add(i);
            (*item_ptr).assume_init_drop();
          }
        }
      }
      inner.len -= needed;
    }
  }

  unsafe fn grow(
    &self,
    ptr: NonNull<u8>,
    old_layout: Layout,
    new_layout: Layout,
  ) -> Result<NonNull<[u8]>, AllocError> {
    let inner = self.inner_mut();
    let old_needed = (old_layout.size() + mem::size_of::<T>() - 1) / mem::size_of::<T>();
    let new_needed = (new_layout.size() + mem::size_of::<T>() - 1) / mem::size_of::<T>();
    let ptr_offset = ptr.as_ptr() as usize - inner.storage.as_ptr() as usize;
    let ptr_index = ptr_offset / mem::size_of::<T>();

    if ptr_index + old_needed == inner.len {
      let additional = new_needed.saturating_sub(old_needed);
      if inner.capacity - inner.len >= additional {
        inner.len = ptr_index + new_needed;
        let raw = ptr.as_ptr();
        // SAFETY: raw is valid for new_layout.size() bytes
        let slice = ptr::slice_from_raw_parts_mut(raw, new_layout.size());
        return Ok(NonNull::new_unchecked(slice));
      }
    }

    let new_ptr = self.allocate(new_layout)?;
    ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_ptr() as *mut u8, old_layout.size());
    Ok(new_ptr)
  }

  unsafe fn shrink(
    &self,
    ptr: NonNull<u8>,
    old_layout: Layout,
    new_layout: Layout,
  ) -> Result<NonNull<[u8]>, AllocError> {
    let inner = self.inner_mut();
    let old_needed = (old_layout.size() + mem::size_of::<T>() - 1) / mem::size_of::<T>();
    let new_needed = (new_layout.size() + mem::size_of::<T>() - 1) / mem::size_of::<T>();
    let ptr_offset = ptr.as_ptr() as usize - inner.storage.as_ptr() as usize;
    let ptr_index = ptr_offset / mem::size_of::<T>();

    if ptr_index + old_needed == inner.len {
      if DROP {
        for i in ptr_index + new_needed..inner.len {
          // SAFETY: item_ptr points to an initialized entry
          unsafe {
            let item_ptr = inner.storage.as_ptr().add(i);
            (*item_ptr).assume_init_drop();
          }
        }
      }
      inner.len = ptr_index + new_needed;
      let raw = ptr.as_ptr();
      // SAFETY: raw is valid for new_layout.size() bytes
      let slice = ptr::slice_from_raw_parts_mut(raw, new_layout.size());
      return Ok(NonNull::new_unchecked(slice));
    }

    let new_ptr = self.allocate(new_layout)?;
    ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_ptr() as *mut u8, new_layout.size());
    Ok(new_ptr)
  }
}

impl<A, T, const DROP: bool> Drop for Chunk<A, T, DROP>
where
  T: Sized,
  A: Allocator,
{
  fn drop(&mut self) {
    let inner = self.inner_mut();
    if DROP {
      for i in 0..inner.len {
        // SAFETY: ptr points to an initialized entry
        unsafe {
          let ptr = inner.storage.as_ptr().add(i);
          (*ptr).assume_init_drop();
        }
      }
    }
    let layout = Layout::array::<MaybeUninit<T>>(inner.capacity).unwrap();
    // SAFETY: storage was allocated with this allocator and layout
    unsafe { inner.allocator.deallocate(inner.storage.cast(), layout) };
  }
}
