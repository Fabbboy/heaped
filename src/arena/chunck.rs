extern crate alloc;

use alloc::alloc::{AllocError, Allocator, Layout};
use core::mem;
use core::{cell::RefCell, mem::MaybeUninit, ptr::NonNull};
use getset::Getters;

#[derive(Debug, Getters)]
pub(crate) struct ArenaChunck<A, T = u8, const DROP: bool = false>
where
    T: Sized,
    A: Allocator,
{
    allocator: A,
    #[getset(get = "pub(crate)")]
    prev: RefCell<Option<NonNull<ArenaChunck<A, T, DROP>>>>,
    #[getset(get = "pub(crate)")]
    next: RefCell<Option<NonNull<ArenaChunck<A, T, DROP>>>>,
    start: *mut u8,
    stop: *mut u8,
    #[getset(get = "pub(crate)")]
    storage: NonNull<MaybeUninit<T>>,
    #[getset(get = "pub(crate)")]
    entries: RefCell<usize>,
    #[getset(get = "pub(crate)")]
    capacity: usize,
}

impl<A, T, const DROP: bool> ArenaChunck<A, T, DROP>
where
    T: Sized,
    A: Allocator,
{
    pub(crate) fn try_new(allocator: A, capacity: usize) -> Result<Self, AllocError> {
        let layout = Layout::array::<MaybeUninit<T>>(capacity).unwrap();
        let raw: NonNull<[u8]> = allocator.allocate(layout).map_err(|_| AllocError)?;
        let storage = unsafe { NonNull::new_unchecked(raw.as_ptr() as *mut MaybeUninit<T>) };
        let start_ptr = raw.as_ptr() as *mut u8;

        Ok(Self {
            start: start_ptr,
            stop: unsafe { start_ptr.add(raw.len()) },
            prev: RefCell::new(None),
            next: RefCell::new(None),
            storage,
            entries: RefCell::new(0),
            capacity,
            allocator,
        })
    }

    pub(crate) fn new(allocator: A, capacity: usize) -> Self {
        Self::try_new(allocator, capacity)
            .unwrap_or_else(|_| panic!("Failed to allocate arena chunk of capacity {}", capacity))
    }
}

impl<A, T, const DROP: bool> ArenaChunck<A, T, DROP>
where
    T: Sized,
    A: Allocator,
{
    pub(crate) fn has_space(&self, layout: Layout) -> bool {
        let remaining = self.capacity - *self.entries.borrow();
        let needed = (layout.size() + mem::size_of::<T>() - 1) / mem::size_of::<T>();
        remaining >= needed
    }

    pub(crate) fn contains(&self, ptr: *mut u8) -> bool {
        let start = self.start as usize;
        let stop = self.stop as usize;
        let ptr = ptr as usize;
        ptr >= start && ptr < stop
    }
}

unsafe impl<A, T, const DROP: bool> Allocator for ArenaChunck<A, T, DROP>
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
        Ok(unsafe { NonNull::new_unchecked(core::slice::from_raw_parts_mut(ptr, byte_count)) })
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
                        // SAFETY: item_ptr points to an initialized entry
                        (*item_ptr).assume_init_drop();
                    }
                }
            }
            *entries -= needed;
        }
    }
}

impl<A, T, const DROP: bool> Drop for ArenaChunck<A, T, DROP>
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
                    // SAFETY: ptr points to an initialized entry
                    (*ptr).assume_init_drop();
                }
            }
        }
        let layout = Layout::array::<MaybeUninit<T>>(self.capacity).unwrap();
        unsafe {
            self.allocator.deallocate(self.storage.cast(), layout);
        }
    }
}
