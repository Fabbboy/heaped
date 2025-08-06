use alloc::alloc::Global;
use crate::arena::{DroplessArena, TypedArena};

#[test]
fn test_dropless_arena_basic_allocation() {
  let arena = DroplessArena::new();
  
  let num = arena.alloc(42i32).expect("should allocate");
  assert_eq!(*num, 42);
  
  let string = arena.alloc_str("hello").expect("should allocate string");
  assert_eq!(string, "hello");
}

#[test]
fn test_dropless_arena_slice_allocation() {
  let arena = DroplessArena::new();
  
  let slice = arena.alloc_slice(&[1, 2, 3, 4, 5]).expect("should allocate slice");
  assert_eq!(slice, &[1, 2, 3, 4, 5]);
}

#[test]
fn test_dropless_arena_empty_slice() {
  let arena = DroplessArena::new();
  
  let empty_slice: &mut [i32] = arena.alloc_slice(&[]).expect("should handle empty slice");
  assert!(empty_slice.is_empty());
}

#[test]
fn test_typed_arena_basic_allocation() {
  let arena = TypedArena::<i32>::new();
  
  let num = arena.alloc(42).expect("should allocate");
  assert_eq!(*num, 42);
  
  let another = arena.alloc(100).expect("should allocate another");
  assert_eq!(*another, 100);
}

#[test]
fn test_typed_arena_slice_allocation() {
  let arena = TypedArena::<i32>::new();
  
  let slice = arena.alloc_slice(&[1, 2, 3, 4, 5]).expect("should allocate slice");
  assert_eq!(slice, &[1, 2, 3, 4, 5]);
}

#[test]
fn test_typed_arena_empty_slice() {
  let arena = TypedArena::<i32>::new();
  
  let empty_slice: &mut [i32] = arena.alloc_slice(&[]).expect("should handle empty slice");
  assert!(empty_slice.is_empty());
}

#[test]
fn test_typed_arena_with_drop() {
  use alloc::string::String;
  
  let arena = TypedArena::<String>::new();
  
  let s1 = arena.alloc(String::from("hello")).expect("should allocate string");
  let s2 = arena.alloc(String::from("world")).expect("should allocate string");
  
  assert_eq!(s1, "hello");
  assert_eq!(s2, "world");
}

#[test]
fn test_typed_arena_clear() {
  let arena = TypedArena::<i32>::new();
  
  let _num1 = arena.alloc(42).expect("should allocate");
  let _num2 = arena.alloc(100).expect("should allocate");
  
  let mut arena = arena;
  arena.clear();
}

#[test]
fn test_alignment() {
  let arena = DroplessArena::new();
  
  let byte = arena.alloc(42u8).expect("should allocate u8");
  let int = arena.alloc(42u32).expect("should allocate u32");
  let long = arena.alloc(42u64).expect("should allocate u64");
  
  assert_eq!(*byte, 42);
  assert_eq!(*int, 42);
  assert_eq!(*long, 42);
  
  assert_eq!(int as *const u32 as usize % core::mem::align_of::<u32>(), 0);
  assert_eq!(long as *const u64 as usize % core::mem::align_of::<u64>(), 0);
}

#[test]
fn test_custom_allocator() {
  let arena = DroplessArena::new_in(Global);
  
  let num = arena.alloc(42i32).expect("should allocate with custom allocator");
  assert_eq!(*num, 42);
}