use super::SlabAllocator;
use alloc::alloc::{
  Allocator,
  Layout,
};

#[test]
fn insert_and_get() {
  let mut slab = SlabAllocator::new();
  let idx = slab.insert(10);
  assert_eq!(slab.get(idx), Some(&10));
}

#[test]
fn remove_and_reuse() {
  let mut slab = SlabAllocator::new();
  let a = slab.insert(1);
  let b = slab.insert(2);
  assert_eq!(slab.remove(a), Some(1));
  let c = slab.insert(3);
  assert_eq!(c, a);
  assert_eq!(slab.get(b), Some(&2));
  assert_eq!(slab.get(c), Some(&3));
  assert_eq!(slab.len(), 2);
}

#[test]
fn allocator_api() {
  let slab = SlabAllocator::<u64>::new();
  let layout = Layout::new::<u64>();
  let ptr = slab.allocate(layout).expect("allocation failed");
  let raw = core::ptr::NonNull::new(ptr.as_ptr() as *mut u8).unwrap();
  unsafe { slab.deallocate(raw, layout) };
  assert_eq!(slab.len(), 0);
}
