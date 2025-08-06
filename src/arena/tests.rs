use super::{DroplessArena, TypedArena};
use core::cell::Cell;

#[test]
fn dropless_arena_basic() {
  let arena = DroplessArena::new(1024);
  let slice = arena.alloc_bytes(b"HelloWorld");
  let str_ref = arena.alloc_str("HelloWorld");
  assert_eq!(slice, b"HelloWorld");
  assert_eq!(str_ref, "HelloWorld");
}

#[test]
fn dropless_arena_multiple_chunks() {
  let arena = DroplessArena::new(8);
  let first = arena.alloc_bytes(b"ABCDEFGH");
  let second = arena.alloc_bytes(b"IJKLMNOP");
  assert_eq!(first, b"ABCDEFGH");
  assert_eq!(second, b"IJKLMNOP");
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

#[test]
fn typed_arena_alloc_slice() {
  let arena = TypedArena::new(4);
  let slice = arena.alloc_slice(&[1u32, 2u32]);
  assert_eq!(slice, &[1, 2]);
}
