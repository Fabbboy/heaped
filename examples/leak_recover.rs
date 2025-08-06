#![feature(allocator_api)]

extern crate alloc;

use alloc::{
  string::String,
  vec,
  vec::Vec,
};
use core::sync::atomic::{
  AtomicUsize,
  Ordering,
};
use heaped::arena::TypedArena;

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
    _ = self.0;
    STRING_DROPS.fetch_add(1, Ordering::SeqCst);
  }
}

struct DroppableContainer {
  _s: String,
  _v: Vec<StaticDropCounter>,
}
impl Drop for DroppableContainer {
  fn drop(&mut self) {
    CONTAINER_DROPS.fetch_add(1, Ordering::SeqCst);
  }
}

fn main() {
  DROP_COUNT.store(0, Ordering::SeqCst);
  STRING_DROPS.store(0, Ordering::SeqCst);
  CONTAINER_DROPS.store(0, Ordering::SeqCst);

  let string_arena = TypedArena::<DroppableString>::new(1).leak();
  string_arena.alloc(DroppableString(String::from("hello")));
  unsafe { TypedArena::recover(string_arena) };
  assert_eq!(STRING_DROPS.load(Ordering::SeqCst), 1);

  let container_arena = TypedArena::<DroppableContainer>::new(1).leak();
  container_arena.alloc(DroppableContainer {
    _s: String::from("world"),
    _v: vec![StaticDropCounter],
  });
  unsafe { TypedArena::recover(container_arena) };
  assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1);
  assert_eq!(CONTAINER_DROPS.load(Ordering::SeqCst), 1);
}
