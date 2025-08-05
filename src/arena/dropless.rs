use std::{
    alloc::{AllocError, Allocator, Global, GlobalAlloc, Layout},
    cell::RefCell,
    ptr::NonNull,
};

use crate::arena::chunck::ArenaChunck;

pub struct DroplessArena<'arena, A = Global>
where
    A: Allocator,
{
    csize: usize,
    chunks: RefCell<Vec<ArenaChunck<u8, false, &'arena A>>>,
    allocator: A,
}

impl<'arena, A> DroplessArena<'arena, A>
where
    A: Allocator,
{
    pub fn new_in(allocator: A, csize: usize) -> Self {
        Self {
            csize,
            chunks: RefCell::new(Vec::new()),
            allocator,
        }
    }
}

impl<'arena> DroplessArena<'arena, Global> {
    pub fn new(csize: usize) -> Self {
        Self::new_in(Global, csize)
    }
}

unsafe impl<'arena, A> Allocator for DroplessArena<'arena, A>
where
    A: Allocator,
{
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let chunks = self.chunks.borrow_mut();
        let mut suiting = None;
        for chunk in chunks.iter() {
            if chunk.has_space(layout) {
                suiting = Some(chunk);
                break;
            }
        }

        if let Some(chunk) = suiting {
            return chunk.allocate(layout);
        }

        let new_chunk: ArenaChunck<u8, false, &'arena A> =
            ArenaChunck::try_new_in(&self.allocator, self.csize)?;
        self.chunks.borrow_mut().push(new_chunk);
        let last_chunk = chunks.last().unwrap();
        last_chunk.allocate(layout)
    }

    fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
      
    }
}
