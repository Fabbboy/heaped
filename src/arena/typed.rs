extern crate alloc;

use alloc::alloc::{
  AllocError,
  Allocator,
  Global,
  Layout,
};
use core::{
  cell::RefCell,
  mem,
  ptr::NonNull,
};

use crate::{
  arena::chunk::Arenachunk,
  once::Once,
};

type TypedChunk<'arena, T, A> = Arenachunk<&'arena A, T, true>;

pub struct TypedArena<'arena, T, A = Global>
where
  T: Sized,
  A: Allocator,
{
  allocator: A,
  csize: usize,
  head: RefCell<Once<NonNull<TypedChunk<'arena, T, A>>>>,
  layout: Layout,
}

impl<'arena, T, A> TypedArena<'arena, T, A>
where
  T: Sized,
  A: Allocator,
{
  pub fn new_in(allocator: A, csize: usize) -> Self {
    let layout = Layout::new::<TypedChunk<'arena, T, A>>();

    Self {
      csize,
      head: RefCell::new(Once::Uninit),
      allocator,
      layout,
    }
  }

  fn new_chunk(
    &self,
    prev: Option<NonNull<TypedChunk<'arena, T, A>>>,
  ) -> Result<NonNull<TypedChunk<'arena, T, A>>, AllocError> {
    let chunk_ptr = self.allocator.allocate(self.layout)?;
    let chunk: *mut TypedChunk<'arena, T, A> = chunk_ptr.as_ptr() as *mut TypedChunk<'arena, T, A>;

    let allocator: &'arena A = unsafe { &*(&self.allocator as *const A) };
    let non_null_ptr = unsafe {
      chunk.write(Arenachunk::new(allocator, self.csize));
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

  fn alloc_raw(&self) -> Result<NonNull<T>, AllocError> {
    let layout = Layout::new::<T>();
    let raw = self.allocate(layout)?;
    let ptr = unsafe { NonNull::new_unchecked(raw.as_ptr() as *mut T) };
    Ok(ptr)
  }

  pub fn alloc(&self, value: T) -> Result<&'arena mut T, AllocError> {
    let ptr = self.alloc_raw()?;
    unsafe {
      ptr.as_ptr().write(value);
      Ok(&mut *ptr.as_ptr())
    }
  }

  pub unsafe fn dealloc(&self, value: &mut T) {
    let layout = Layout::new::<T>();
    let ptr = unsafe { NonNull::new_unchecked(value as *mut T as *mut u8) };
    unsafe { <Self as Allocator>::deallocate(self, ptr, layout) };
  }
}

impl<'arena, T> TypedArena<'arena, T, Global>
where
  T: Sized,
{
  pub fn new(csize: usize) -> Self {
    Self::new_in(Global, csize)
  }
}

impl<'arena, T, A> Drop for TypedArena<'arena, T, A>
where
  T: Sized,
  A: Allocator,
{
  fn drop(&mut self) {
    let head = self.head.borrow_mut();
    if let Some(chunk) = head.get() {
      unsafe {
        let mut current = chunk.as_ptr();
        while !current.is_null() {
          let next = (*current).next().borrow_mut().take();
          // SAFETY: current points to a valid chunk
          core::ptr::drop_in_place(current);
          self
            .allocator
            .deallocate(NonNull::new_unchecked(current as *mut u8), self.layout);
          current = next.map_or(core::ptr::null_mut(), |n| n.as_ptr());
        }
      }
    }
  }
}

unsafe impl<'arena, T, A> Allocator for TypedArena<'arena, T, A>
where
  T: Sized,
  A: Allocator,
{
  fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
    assert_eq!(layout.size(), mem::size_of::<T>());
    assert_eq!(layout.align(), mem::align_of::<T>());

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
        match *unsafe { current.as_ref().next().borrow() } {
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
  use core::cell::Cell;

  struct DropCounter<'a>(&'a Cell<usize>);

  impl<'a> Drop for DropCounter<'a> {
    fn drop(&mut self) {
      let v = self.0.get();
      self.0.set(v + 1);
    }
  }

  #[test]
  fn typed_arena_drop() {
    let arena = TypedArena::new(4);
    let counter = Cell::new(0);
    {
      let _ = arena.alloc(DropCounter(&counter)).unwrap();
      let item2 = arena.alloc(DropCounter(&counter)).unwrap();
      unsafe { arena.dealloc(item2) };
      assert_eq!(counter.get(), 1);
    }
    drop(arena);
    assert_eq!(counter.get(), 2);
  }
}
