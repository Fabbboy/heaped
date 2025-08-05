use std::alloc::AllocError;
use std::cell::RefCell;
use std::{
    alloc::{Allocator, Global, Layout},
    mem::MaybeUninit,
    ptr::NonNull,
};

use core::mem;
use getset::Getters;

#[derive(Debug, Getters)]
pub(crate) struct ArenaChunck<T = u8, const DROP: bool = false, A = Global>
where
    T: Sized,
    A: Allocator,
{
   start: *mut u8,
   stop: *mut u8,
    #[getset(get = "pub(crate)")]
    storage: NonNull<MaybeUninit<T>>,
    #[getset(get = "pub(crate)")]
    entries: RefCell<usize>,
    #[getset(get = "pub(crate)")]
    capacity: usize,
    allocator: A,
}

impl<T, const DROP: bool, A> ArenaChunck<T, DROP, A>
where
    T: Sized,
    A: Allocator,
{
    pub(crate) fn try_new_in(allocator: A, capacity: usize) -> Result<Self, AllocError> {
        let layout = Layout::array::<MaybeUninit<T>>(capacity).unwrap();
        let raw: NonNull<[u8]> = allocator.allocate(layout).map_err(|_| AllocError)?;
        let storage = unsafe { NonNull::new_unchecked(raw.as_ptr() as *mut MaybeUninit<T>) };
        Ok(Self {
            start: raw.as_ptr(),
            stop: unsafe { raw.as_ptr().add(raw.len()) },
            storage,
            entries: RefCell::new(0),
            capacity,
            allocator,
        })
    }

    pub(crate) fn new_in(allocator: A, capacity: usize) -> Self {
        Self::try_new_in(allocator, capacity)
            .unwrap_or_else(|_| panic!("Failed to allocate arena chunk of capacity {}", capacity))
    }
}

impl<T, const DROP: bool> ArenaChunck<T, DROP, Global>
where
    T: Sized,
{
    pub(crate) fn try_new(capacity: usize) -> Result<Self, AllocError> {
        Self::try_new_in(Global, capacity)
    }

    pub(crate) fn new(capacity: usize) -> Self {
        Self::try_new(capacity)
            .unwrap_or_else(|_| panic!("Failed to allocate arena chunk of capacity {}", capacity))
    }
}

impl<T, const DROP: bool, A> ArenaChunck<T, DROP, A>
where
    T: Sized,
    A: Allocator,
{
    pub(crate) fn has_space(&self, layout: Layout) -> bool {
        let remaining = self.capacity - *self.entries.borrow();
        let needed = (layout.size() + mem::size_of::<T>() - 1) / mem::size_of::<T>();
        remaining >= needed
    }

    pub(crate) fn contains
}

unsafe impl<T, const DROP: bool, A> Allocator for ArenaChunck<T, DROP, A>
where
    T: Sized,
    A: Allocator,
{
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if !self.has_space(layout) {
            return Err(AllocError);
        }

        let mut entries = self.entries.borrow_mut();
        let needed = (layout.size() + mem::size_of::<T>() - 1) / mem::size_of::<T>();
        let start = *entries;
        *entries += needed;

        let ptr = unsafe { self.storage.as_ptr().add(start) as *mut u8 };
        let byte_count = needed * mem::size_of::<T>();
        Ok(unsafe { NonNull::new_unchecked(std::slice::from_raw_parts_mut(ptr, byte_count)) })
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let mut entries = self.entries.borrow_mut();
        let needed = (layout.size() + mem::size_of::<T>() - 1) / mem::size_of::<T>();

        let ptr_offset = ptr.as_ptr() as usize - self.storage.as_ptr() as usize;
        let ptr_index = ptr_offset / mem::size_of::<T>();

        if ptr_index + needed == *entries { 
            if DROP {
                for i in ptr_index..*entries {
                    unsafe {
                        let item_ptr = self.storage.as_ptr().add(i);
                        item_ptr.drop_in_place();
                    }
                }
            }
            *entries -= needed;
        }
    }
}

impl<T, const DROP: bool, A> Drop for ArenaChunck<T, DROP, A>
where
    T: Sized,
    A: Allocator,
{
    fn drop(&mut self) {
        if DROP {
            let entries_count = *self.entries.borrow();
            for i in 0..entries_count {
                unsafe {
                    let ptr = self.storage.as_ptr().add(i);
                    ptr.drop_in_place();
                }
            }
        }
        let layout = Layout::array::<MaybeUninit<T>>(self.capacity).unwrap();
        unsafe {
            self.allocator.deallocate(self.storage.cast(), layout);
        }
    }
}
