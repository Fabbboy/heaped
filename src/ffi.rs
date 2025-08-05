use alloc::alloc::GlobalAlloc;
use core::{
  alloc::{
    AllocError,
    Allocator,
    Layout as RsLayout,
  },
  ptr::NonNull,
};

#[repr(C)]
pub struct Layout {
  pub size: usize,
  pub align: usize,
}

impl From<Layout> for RsLayout {
  fn from(layout: Layout) -> Self {
    RsLayout::array::<u8>(layout.size)
      .unwrap()
      .align_to(layout.align)
      .unwrap()
  }
}

impl From<RsLayout> for Layout {
  fn from(layout: RsLayout) -> Self {
    Layout {
      size: layout.size(),
      align: layout.align(),
    }
  }
}
