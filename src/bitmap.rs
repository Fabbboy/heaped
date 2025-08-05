use core::ptr::NonNull;

use alloc::alloc::{
  Allocator,
  Global,
  Layout,
};

#[derive(Debug)]
pub enum BitmapError {
  OutOfBounds,
  AllocError,
  InvalidSize,
}

pub struct Bitmap<'map, A = Global>
where
  A: Allocator,
{
  allocator: A,
  map: &'map mut [u8],
  layout: Layout,
  fields: usize,
}

impl<'map, A> Bitmap<'map, A>
where
  A: Allocator,
{
  pub fn try_new_in(allocator: A, size: usize) -> Result<Self, BitmapError> {
    if size % 8 != 0 {
      return Err(BitmapError::InvalidSize);
    }

    let fields = size / 8;

    let layout = Layout::array::<u8>(fields).map_err(|_| BitmapError::InvalidSize)?;
    let ptr = allocator.allocate_zeroed(layout).map_err(|_| BitmapError::AllocError)?;
    let map = unsafe { core::slice::from_raw_parts_mut(ptr.as_ptr() as *mut u8, fields) };
    Ok(Bitmap {
      allocator,
      map,
      layout,
      fields,
    })
  }

  pub fn new_in(allocator: A, size: usize) -> Self {
    Self::try_new_in(allocator, size).expect("Failed to create Bitmap")
  }
}

impl<'map> Bitmap<'map, Global> {
  pub fn new(size: usize) -> Self {
    Self::new_in(Global, size)
  }
}

impl<'map, A> Bitmap<'map, A>
where
  A: Allocator,
{
  pub fn set(&mut self, index: usize) -> Result<(), BitmapError> {
    if index >= self.fields * 8 {
      return Err(BitmapError::OutOfBounds);
    }
    let byte_index = index / 8;
    let bit_index = index % 8;
    self.map[byte_index] |= 1 << bit_index;
    Ok(())
  }

  pub fn get(&self, index: usize) -> Result<bool, BitmapError> {
    if index >= self.fields * 8 {
      return Err(BitmapError::OutOfBounds);
    }
    let byte_index = index / 8;
    let bit_index = index % 8;
    Ok((self.map[byte_index] & (1 << bit_index)) != 0)
  }

  pub fn clear(&mut self, index: usize) -> Result<(), BitmapError> {
    if index >= self.fields * 8 {
      return Err(BitmapError::OutOfBounds);
    }
    let byte_index = index / 8;
    let bit_index = index % 8;
    self.map[byte_index] &= !(1 << bit_index);
    Ok(())
  }

  pub fn resize(&mut self, new_size: usize) -> Result<(), BitmapError> {
    if new_size % 8 != 0 {
      return Err(BitmapError::InvalidSize);
    }
    let new_fields = new_size / 8;
    if new_fields > self.fields {
      let new_layout = Layout::array::<u8>(new_fields).map_err(|_| BitmapError::InvalidSize)?;
      let old_ptr = NonNull::new(self.map.as_mut_ptr()).unwrap();
      let new_ptr = unsafe {
        self
          .allocator
          .grow_zeroed(old_ptr, self.layout, new_layout).map_err(|_| BitmapError::AllocError)?
      };
      self.map =
        unsafe { core::slice::from_raw_parts_mut(new_ptr.as_ptr() as *mut u8, new_fields) };
      self.layout = new_layout;
    } else {
      for i in new_fields..self.fields {
        self.map[i] = 0;
      }
    }
    self.fields = new_fields;
    Ok(())
  }
}

//TODO: might leak investigate issues
impl<'map, A> Drop for Bitmap<'map, A>
where
  A: Allocator,
{
  fn drop(&mut self) {
    let layout = self.layout;
    unsafe {
      self.allocator.deallocate(
        NonNull::new(self.map.as_mut_ptr() as *mut u8).unwrap(),
        layout,
      );
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_bitmap() {
    let mut bitmap = Bitmap::new(64);
    assert!(bitmap.set(10).is_ok());
    assert!(bitmap.get(10).unwrap());
    assert!(bitmap.clear(10).is_ok());
    assert!(!bitmap.get(10).unwrap());
  }

  #[test]
  fn test_resize() {
    let mut bitmap = Bitmap::new(64);
    assert!(bitmap.set(10).is_ok());
    assert!(bitmap.resize(128).is_ok());
    assert!(bitmap.get(10).unwrap());
  }
}
