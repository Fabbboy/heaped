use super::{DroplessArena, TypedArena};
use alloc::{string::String, vec, vec::Vec};
use core::{
  cell::Cell,
  sync::atomic::{AtomicUsize, Ordering},
};

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

static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);
static STRING_DROPS: AtomicUsize = AtomicUsize::new(0);
static CONTAINER_DROPS: AtomicUsize = AtomicUsize::new(0);

struct StaticDropCounter;

impl Drop for StaticDropCounter {
  fn drop(&mut self) {
    DROP_COUNT.fetch_add(1, Ordering::SeqCst);
  }
}

struct DroppableString(String);

impl Drop for DroppableString {
  fn drop(&mut self) {
    STRING_DROPS.fetch_add(1, Ordering::SeqCst);
  }
}

struct DroppableContainer {
  s: String,
  v: Vec<StaticDropCounter>,
}

impl Drop for DroppableContainer {
  fn drop(&mut self) {
    CONTAINER_DROPS.fetch_add(1, Ordering::SeqCst);
  }
}

#[test]
fn typed_arena_leak_recover() {
  DROP_COUNT.store(0, Ordering::SeqCst);
  STRING_DROPS.store(0, Ordering::SeqCst);
  CONTAINER_DROPS.store(0, Ordering::SeqCst);

  let string_arena = TypedArena::<DroppableString>::new(1).leak();
  string_arena.alloc(DroppableString(String::from("hello")));
  unsafe { TypedArena::recover(string_arena) };
  assert_eq!(STRING_DROPS.load(Ordering::SeqCst), 1);

  let container_arena = TypedArena::<DroppableContainer>::new(1).leak();
  container_arena.alloc(DroppableContainer {
    s: String::from("world"),
    v: vec![StaticDropCounter],
  });
  unsafe { TypedArena::recover(container_arena) };
  assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1);
  assert_eq!(CONTAINER_DROPS.load(Ordering::SeqCst), 1);
}
