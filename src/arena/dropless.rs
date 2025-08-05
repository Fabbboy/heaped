use std::{
    alloc::{Allocator, Global, GlobalAlloc},
    cell::RefCell,
};

use crate::arena::chunck::ArenaChunck;

pub struct DroplessArena<A = Global>
where
    A: Allocator,
{
    csize: usize,
    chunks: RefCell<Vec<ArenaChunck<u8, false, A>>>,
    allocator: A,
}

impl<A> DroplessArena<A>
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

impl DroplessArena<Global> {
    pub fn new(csize: usize) -> Self {
        Self::new_in(Global, csize)
    }
}