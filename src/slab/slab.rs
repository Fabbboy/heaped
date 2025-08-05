use std::alloc::{
  Allocator,
  Global,
};

pub struct Slab<T, A = Global>
where
  T: Sized,
  A: Allocator,
{
  allocator: A,
}
