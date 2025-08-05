use std::{
    alloc::{AllocError, Allocator, Global, Layout},
    cell::RefCell,
    ptr::NonNull,
};

use crate::{arena::chunck::ArenaChunck, once::Once};

type DroplessChunk<'arena, A> = ArenaChunck<u8, false, &'arena A>;

pub struct DroplessArena<'arena, A = Global>
where
    A: Allocator,
{
    csize: usize,
    head: RefCell<Once<NonNull<DroplessChunk<'arena, A>>>>,
    layout: Layout,
    allocator: A,
}

impl<'arena, A> DroplessArena<'arena, A>
where
    A: Allocator,
{
    pub fn new_in(allocator: A, csize: usize) -> Self {
        let layout = Layout::new::<DroplessChunk<'arena, A>>();

        Self {
            csize,
            head: RefCell::new(Once::Uninit),
            allocator,
            layout,
        }
    }

    // does NOT modify the head
    fn new_chunk(
        &self,
        prev: Option<NonNull<DroplessChunk<'arena, A>>>,
    ) -> Result<NonNull<DroplessChunk<'arena, A>>, AllocError> {
        let chunk_ptr = self.allocator.allocate(self.layout)?;
        let chunk: *mut DroplessChunk<'arena, A> =
            chunk_ptr.as_ptr() as *mut DroplessChunk<'arena, A>;

        let non_null_ptr = unsafe {
            chunk.write(DroplessChunk::new_in(&self.allocator, self.csize));
            NonNull::new_unchecked(chunk)
        };

        if let Some(prev_chunk) = prev {
            unsafe {
                prev_chunk
                    .as_ref()
                    .next()
                    .borrow_mut()
                    .replace(non_null_ptr);
                non_null_ptr
                    .as_ref()
                    .prev()
                    .borrow_mut()
                    .replace(prev_chunk);
            }
        }

        Ok(non_null_ptr)
    }
}

impl<'arena> DroplessArena<'arena, Global> {
    pub fn new(csize: usize) -> Self {
        Self::new_in(Global, csize)
    }
}

impl<'arena, A> Drop for DroplessArena<'arena, A>
where
    A: Allocator,
{
    fn drop(&mut self) {
        let head = self.head.borrow_mut();
        if let Some(chunk) = head.get() {
            unsafe {
                let mut current = chunk.as_ptr();
                while !current.is_null() {
                    let next = (*current).next().borrow_mut().take();
                    self.allocator
                        .deallocate(NonNull::new_unchecked(current as *mut u8), self.layout);
                    current = next.map_or(std::ptr::null_mut(), |n| n.as_ptr());
                }
            }
        }
    }
}

unsafe impl<'arena, A> Allocator for DroplessArena<'arena, A>
where
    A: Allocator,
{
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // 1. Get the head if not initialized do it
        // 2. recursively find a chunk that has space
        // 3.1 if no chunk found, create a new one
        // 3.2 if chunk found, allocate from it
        todo!()
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // Dropless arena has the word "dropless" in its name which means data inside the arena is not freed
        // but the underlying memory can be reused thats why we need to deallocate in the chunck which might free up new memory for the future
        // but T it self is not freed
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dropless_arena() {
        let arena = DroplessArena::new(1024);
        let string_layout = Layout::array::<u8>(10).unwrap();
        let mut string_raw = arena.allocate_zeroed(string_layout).unwrap();
        let string_slice = unsafe { string_raw.as_mut() };
        string_slice.copy_from_slice(b"HelloWorld");

        assert_eq!(string_slice, b"HelloWorld");
        println!("Allocated string: {:?}", string_slice);
        // if this segfaults we know where to work on
    }
}
