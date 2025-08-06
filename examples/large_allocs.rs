extern crate alloc;

use heaped::arena::{DroplessArena, TypedArena};
use alloc::vec::Vec;

fn test_dropless_arena_large_allocation() {
  let arena = DroplessArena::new();
  
  let large_slice: Vec<u32> = (0..10000).collect();
  let allocated = arena.alloc_slice(&large_slice).expect("should allocate large slice");
  
  assert_eq!(allocated.len(), 10000);
  for (i, &val) in allocated.iter().enumerate() {
    assert_eq!(val, i as u32);
  }
  
  println!("Successfully allocated and verified large dropless arena slice of {} elements", allocated.len());
}

fn test_typed_arena_large_allocation() {
  let arena = TypedArena::<u32>::new();
  
  let large_slice: Vec<u32> = (0..10000).collect();
  let allocated = arena.alloc_slice(&large_slice).expect("should allocate large slice");
  
  assert_eq!(allocated.len(), 10000);
  for (i, &val) in allocated.iter().enumerate() {
    assert_eq!(val, i as u32);
  }
  
  println!("Successfully allocated and verified large typed arena slice of {} elements", allocated.len());
}

fn test_nested_scoped_arenas() {
  fn stage1_processing() -> Vec<i32> {
    let stage1_arena = DroplessArena::new();
    
    let numbers = stage1_arena.alloc_slice(&[1, 2, 3, 4, 5]).expect("should allocate");
    let doubled = stage1_arena.alloc_slice(&[2, 4, 6, 8, 10]).expect("should allocate");
    
    let mut result = Vec::new();
    for (&n, &d) in numbers.iter().zip(doubled.iter()) {
      result.push(n + d);
    }
    
    result
  }
  
  fn stage2_processing(input: Vec<i32>) -> Vec<String> {
    use alloc::string::ToString;
    let stage2_arena = TypedArena::<String>::new();
    let mut results = Vec::new();
    
    for &num in &input {
      let formatted = stage2_arena.alloc(alloc::format!("value: {}", num)).expect("should allocate");
      results.push(formatted.clone());
    }
    
    results
  }
  
  let stage1_results = stage1_processing();
  assert_eq!(stage1_results, vec![3, 6, 9, 12, 15]);
  
  let stage2_results = stage2_processing(stage1_results);
  assert_eq!(stage2_results.len(), 5);
  assert_eq!(stage2_results[0], "value: 3");
  assert_eq!(stage2_results[4], "value: 15");
  
  println!("Successfully completed nested scoped arena processing");
}

fn main() {
  println!("Running large allocation examples...");
  
  test_dropless_arena_large_allocation();
  test_typed_arena_large_allocation();
  test_nested_scoped_arenas();
  
  println!("All large allocation examples completed successfully");
}