use super::{
  dropless::DroplessArena,
  typed::TypedArena,
};
use alloc::alloc::{
  Allocator,
  Layout,
};
use core::cell::Cell;

#[test]
fn dropless_arena_basic() {
  let arena = DroplessArena::new(1024);
  let layout = Layout::array::<u8>(10).unwrap();
  let mut raw = arena.allocate_zeroed(layout).unwrap();
  // SAFETY: raw is valid for 10 bytes
  let slice = unsafe { raw.as_mut() };
  slice.copy_from_slice(b"HelloWorld");
  // SAFETY: slice contains valid utf8
  let str_ref = unsafe { core::str::from_utf8_unchecked(slice) };
  assert_eq!(slice, b"HelloWorld");
  assert_eq!(str_ref, "HelloWorld");
}

#[test]
fn dropless_arena_multiple_chunks() {
  let arena = DroplessArena::new(8);
  let layout = Layout::array::<u8>(8).unwrap();
  let mut first = arena.allocate_zeroed(layout).unwrap();
  // SAFETY: first is valid for 8 bytes
  let first_slice = unsafe { first.as_mut() };
  first_slice.copy_from_slice(b"ABCDEFGH");
  let mut second = arena.allocate_zeroed(layout).unwrap();
  // SAFETY: second is valid for 8 bytes
  let second_slice = unsafe { second.as_mut() };
  second_slice.copy_from_slice(b"IJKLMNOP");
  assert_eq!(first_slice, b"ABCDEFGH");
  assert_eq!(second_slice, b"IJKLMNOP");
}

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
    let _ = arena.try_alloc(DropCounter(&counter)).unwrap();
    let _ = arena.try_alloc(DropCounter(&counter)).unwrap();
    assert_eq!(counter.get(), 0);
  }
  drop(arena);
  assert_eq!(counter.get(), 2);
}

#[test]
fn typed_arena_multiple_chunks() {
  let arena = TypedArena::new(1);
  let a = arena.try_alloc(1u32).unwrap();
  let b = arena.try_alloc(2u32).unwrap();
  assert_eq!((*a, *b), (1, 2));
}

#[test]
fn typed_arena_zero_sized() {
  let arena = TypedArena::new(1);
  let a = arena.try_alloc(()).unwrap();
  let b = arena.try_alloc(()).unwrap();
  assert_eq!((*a, *b), ((), ()));
}
