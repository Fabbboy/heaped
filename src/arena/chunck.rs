use std::alloc::AllocError;
use std::{
    alloc::{Allocator, Global, Layout},
    mem::MaybeUninit,
    ptr::NonNull,
};

use getset::Getters;

#[derive(Debug, Getters)]
pub(crate) struct ArenaChunck<T = u8, const DROP: bool = false, A = Global>
where
    T: Sized,
    A: Allocator,
{
    #[getset(get = "pub(crate)")]
    storage: NonNull<MaybeUninit<T>>,
    #[getset(get = "pub(crate)")]
    entries: usize,
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
            storage,
            entries: 0,
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
        let remaining = self.capacity - self.entries;
        let needed = (layout.size() + std::mem::size_of::<T>() - 1) / std::mem::size_of::<T>();
        remaining >= needed
    }
}

impl<T, const DROP: bool, A> Drop for ArenaChunck<T, DROP, A>
where
    T: Sized,
    A: Allocator,
{
    fn drop(&mut self) {
        if DROP {
            for i in 0..self.entries {
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
