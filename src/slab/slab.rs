extern crate alloc;

use alloc::{
    alloc::{AllocError, Allocator, Global, Layout},
    vec::Vec,
};
use core::{
    cell::UnsafeCell,
    mem::ManuallyDrop,
    ptr::{self, NonNull},
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

pub struct Slab<T, A: Allocator = Global> {
    inner: UnsafeCell<SlabInner<T, A>>,
}

impl<T> Slab<T, Global> {
    pub fn new() -> Self {
        Self::new_in(Global)
    }
}

impl<T, A: Allocator> Slab<T, A> {
    pub fn new_in(alloc: A) -> Self {
        Self {
            inner: UnsafeCell::new(SlabInner {
                slots: Vec::new_in(alloc),
                free: EMPTY,
                len: 0,
            }),
        }
    }

    pub fn insert(&mut self, value: T) -> usize {
        let inner = self.inner_mut();
        let idx = inner.alloc_slot();
        inner.slots[idx].value = ManuallyDrop::new(value);
        idx
    }

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

    pub fn get(&self, index: usize) -> Option<&T> {
        let inner = self.inner_ref();
        if index >= inner.slots.len() || inner.is_free(index) {
            None
        } else {
            unsafe { Some(&*(&inner.slots[index].value as *const ManuallyDrop<T> as *const T)) }
        }
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        let inner = self.inner_mut();
        if index >= inner.slots.len() || inner.is_free(index) {
            None
        } else {
            unsafe { Some(&mut *(&mut inner.slots[index].value as *mut ManuallyDrop<T> as *mut T)) }
        }
    }

    pub fn len(&self) -> usize {
        self.inner_ref().len
    }

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
    fn alloc_slot(&mut self) -> usize {
        match self.free {
            EMPTY => {
                self.slots.push(Slot { next: EMPTY });
                self.len += 1;
                self.slots.len() - 1
            }
            idx => {
                self.free = unsafe { self.slots[idx].next };
                self.len += 1;
                idx
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

unsafe impl<T, A: Allocator> Allocator for Slab<T, A> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if layout.size() != core::mem::size_of::<T>() || layout.align() != core::mem::align_of::<T>() {
            return Err(AllocError);
        }
        let inner = unsafe { &mut *self.inner.get() };
        let idx = inner.alloc_slot();
        let ptr = inner.slots.as_mut_ptr();
        let ptr = unsafe { ptr.add(idx) as *mut u8 };
        let slice = ptr::slice_from_raw_parts_mut(ptr, layout.size());
        NonNull::new(slice).ok_or(AllocError)
    }

    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let ptr = self.allocate(layout)?;
        let raw = ptr.as_ptr() as *mut u8;
        unsafe { ptr::write_bytes(raw, 0, layout.size()); }
        Ok(ptr)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        debug_assert!(layout.size() == core::mem::size_of::<T>() && layout.align() == core::mem::align_of::<T>());
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

impl<T, A: Allocator> Drop for Slab<T, A> {
    fn drop(&mut self) {
        let inner = self.inner_mut();
        for idx in 0..inner.slots.len() {
            if !inner.is_free(idx) {
                unsafe { ManuallyDrop::drop(&mut inner.slots[idx].value); }
            }
        }
    }
}
