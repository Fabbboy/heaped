use alloc::alloc::GlobalAlloc;
use core::{
  alloc::{
    AllocError,
    Allocator,
    Layout as RsLayout,
  },
  ptr::NonNull,
};
use std::{
  alloc::Global,
  ffi::c_void,
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

#[repr(C)]
pub struct Slice {
  pub ptr: *mut u8,
  pub len: usize,
}

impl From<&[u8]> for Slice {
  fn from(slice: &[u8]) -> Self {
    Slice {
      ptr: slice.as_ptr() as *mut u8,
      len: slice.len(),
    }
  }
}

impl From<Slice> for &[u8] {
  fn from(slice: Slice) -> Self {
    unsafe { core::slice::from_raw_parts(slice.ptr, slice.len) }
  }
}

impl From<Slice> for Layout {
  fn from(slice: Slice) -> Self {
    Layout {
      size: slice.len,
      align: core::mem::align_of::<u8>(),
    }
  }
}

#[repr(C)]
pub enum Option<T> {
  Some(T),
  None,
}

#[repr(C)]
pub struct Alloc {
  pub self_: *mut c_void,
  pub allocate: unsafe extern "C" fn(self_: *mut c_void, layout: Layout) -> Option<Slice>,
  pub deallocate: unsafe extern "C" fn(self_: *mut c_void, slice: Slice),
}

// SAFETY: The user must ensure that usage of `Alloc` is thread-safe if used in a static context.
unsafe impl Sync for Alloc {}

static GLOBAL: Global = Global;

#[unsafe(no_mangle)]
pub extern "C" fn global_allocate(_self: *mut c_void, layout: Layout) -> Option<Slice> {
  let layout: RsLayout = layout.into();
  match GLOBAL.allocate(layout) {
    Ok(non_null) => {
      let raw_ptr: *mut u8 = non_null.as_ptr() as *mut u8;
      Option::Some(Slice {
        ptr: raw_ptr,
        len: layout.size(),
      })
    }
    Err(_) => Option::None,
  }
}

#[unsafe(no_mangle)]
pub extern "C" fn global_deallocate(_self: *mut c_void, slice: Slice) {
  let ptr = slice.ptr;
  let layout: RsLayout = Layout::from(slice).into();
  unsafe {
    GLOBAL.deallocate(NonNull::new(ptr as *mut u8).unwrap(), layout);
  }
}

pub static GLOBAL_ALLOC: Alloc = Alloc {
  self_: core::ptr::null_mut(),
  allocate: global_allocate,
  deallocate: global_deallocate,
};

#[unsafe(no_mangle)]
pub extern "C" fn alloc(alloc: *mut Alloc, layout: Layout) -> Option<Slice> {
  unsafe {
    if alloc.is_null() {
      global_allocate(core::ptr::null_mut(), layout)
    } else {
      ((*alloc).allocate)((*alloc).self_, layout)
    }
  }
}

#[unsafe(no_mangle)]
pub extern "C" fn dealloc(alloc: *mut Alloc, slice: Slice) {
  unsafe {
    if alloc.is_null() {
      global_deallocate(core::ptr::null_mut(), slice);
    } else {
      ((*alloc).deallocate)((*alloc).self_, slice);
    }
  }
}