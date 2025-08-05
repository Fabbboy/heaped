use super::FixedAllocator;
use alloc::alloc::{
  Allocator,
  Layout,
};

#[test]
fn basic_allocation() {
  let mut buffer = [0u8; 1024];
  let allocator = FixedAllocator::new(&mut buffer);

  let layout = Layout::new::<u64>();
  let ptr = allocator.allocate(layout).unwrap();
  assert_eq!(ptr.len(), 8);
  assert_eq!(allocator.used(), 8);
  assert_eq!(allocator.available(), 1024 - 8);

  let layout2 = Layout::new::<u32>();
  let ptr2 = allocator.allocate(layout2).unwrap();
  assert_eq!(ptr2.len(), 4);
  assert_eq!(allocator.used(), 12);

  unsafe {
    allocator.deallocate(ptr2.cast(), layout2);
  }
  assert_eq!(allocator.used(), 8);
}

#[test]
fn alignment_test() {
  let mut buffer = [0u8; 1024];
  let allocator = FixedAllocator::new(&mut buffer);

  let layout1 = Layout::new::<u8>();
  let _ptr1 = allocator.allocate(layout1).unwrap();
  assert_eq!(allocator.used(), 1);

  let layout2 = Layout::new::<u64>();
  let _ptr2 = allocator.allocate(layout2).unwrap();

  assert_eq!(allocator.used(), 16);
}

#[test]
fn out_of_memory() {
  let mut buffer = [0u8; 16];
  let allocator = FixedAllocator::new(&mut buffer);

  let layout = Layout::from_size_align(32, 1).unwrap();
  let result = allocator.allocate(layout);
  assert!(result.is_err());
}

#[test]
fn reset_functionality() {
  let mut buffer = [0u8; 1024];
  let allocator = FixedAllocator::new(&mut buffer);

  let layout = Layout::new::<u64>();
  let _ptr = allocator.allocate(layout).unwrap();
  assert_eq!(allocator.used(), 8);

  unsafe {
    allocator.reset();
  }
  assert_eq!(allocator.used(), 0);
  assert_eq!(allocator.available(), 1024);
}

#[test]
fn grow_functionality() {
  let mut buffer = [0u8; 1024];
  let allocator = FixedAllocator::new(&mut buffer);

  let old_layout = Layout::from_size_align(32, 4).unwrap();
  let ptr = allocator.allocate(old_layout).unwrap();
  assert_eq!(allocator.used(), 32);

  unsafe {
    ptr.as_ptr().cast::<u8>().write(42);
  }

  let new_layout = Layout::from_size_align(64, 4).unwrap();
  let new_ptr = unsafe { allocator.grow(ptr.cast(), old_layout, new_layout).unwrap() };
  assert_eq!(allocator.used(), 64);
  assert_eq!(new_ptr.len(), 64);

  unsafe {
    assert_eq!(new_ptr.as_ptr().cast::<u8>().read(), 42);
  }

  let huge_layout = Layout::from_size_align(2048, 4).unwrap();
  let result = unsafe { allocator.grow(new_ptr.cast(), new_layout, huge_layout) };
  assert!(result.is_err());
}

#[test]
fn shrink_functionality() {
  let mut buffer = [0u8; 1024];
  let allocator = FixedAllocator::new(&mut buffer);

  let old_layout = Layout::from_size_align(64, 4).unwrap();
  let ptr = allocator.allocate(old_layout).unwrap();
  assert_eq!(allocator.used(), 64);

  unsafe {
    ptr.as_ptr().cast::<u8>().write(42);
  }

  let new_layout = Layout::from_size_align(32, 4).unwrap();
  let new_ptr = unsafe {
    allocator
      .shrink(ptr.cast(), old_layout, new_layout)
      .unwrap()
  };
  assert_eq!(new_ptr.len(), 32);
  assert_eq!(allocator.used(), 32);

  unsafe {
    assert_eq!(new_ptr.as_ptr().cast::<u8>().read(), 42);
  }
}

#[test]
fn grow_with_relocation() {
  let mut buffer = [0u8; 1024];
  let allocator = FixedAllocator::new(&mut buffer);

  let layout1 = Layout::from_size_align(32, 4).unwrap();
  let ptr1 = allocator.allocate(layout1).unwrap();
  unsafe {
    ptr1.as_ptr().cast::<u8>().write(42);
  }

  let layout2 = Layout::from_size_align(32, 4).unwrap();
  let _ptr2 = allocator.allocate(layout2).unwrap();
  assert_eq!(allocator.used(), 64);

  let new_layout = Layout::from_size_align(64, 4).unwrap();
  let new_ptr = unsafe { allocator.grow(ptr1.cast(), layout1, new_layout).unwrap() };
  assert_eq!(new_ptr.len(), 64);

  unsafe {
    assert_eq!(new_ptr.as_ptr().cast::<u8>().read(), 42);
  }
}
