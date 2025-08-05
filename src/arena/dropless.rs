extern crate alloc;

use alloc::alloc::{AllocError, Allocator, Global, Layout};
use core::{cell::RefCell, ptr::NonNull};

use crate::{arena::chunck::ArenaChunck, once::Once};

type DroplessChunk<'arena, A> = ArenaChunck<u8, false, &'arena A>;

pub struct DroplessArena<'arena, A = Global>
where
    A: Allocator,
{
    allocator: A,
    csize: usize,
    head: RefCell<Once<NonNull<DroplessChunk<'arena, A>>>>,
    layout: Layout,
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

        let allocator: &'arena A = unsafe { &*(&self.allocator as *const A) };
        let non_null_ptr = unsafe {
            chunk.write(DroplessChunk::new(allocator, self.csize));
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
        let mut head = self.head.borrow_mut();
        let mut current = match head.get() {
            Some(h) => *h,
            None => {
                let new_head = self.new_chunk(None)?;
                let _ = head.init(new_head);
                new_head
            }
        };
        drop(head);

        loop {
            unsafe {
                if current.as_ref().has_space(layout) {
                    return current.as_ref().allocate(layout);
                }
                if let Some(next) = *current.as_ref().next().borrow() {
                    current = next;
                } else {
                    let new = self.new_chunk(Some(current))?;
                    return new.as_ref().allocate(layout);
                }
            }
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        if let Some(mut current) = self.head.borrow().get().copied() {
            loop {
                if unsafe { current.as_ref().contains(ptr.as_ptr()) } {
                    unsafe { current.as_ref().deallocate(ptr, layout) };
                    break;
                }
                match unsafe { *current.as_ref().next().borrow() } {
                    Some(next) => current = next,
                    None => break,
                }
            }
        }
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

        let str_ref = unsafe { core::str::from_utf8_unchecked(string_slice) };

        assert_eq!(string_slice, b"HelloWorld");
        assert_eq!(str_ref, "HelloWorld");
        println!("String from arena: {}", str_ref);
    }
}
