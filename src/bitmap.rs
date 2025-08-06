//! Bitmap structure for tracking fixed-size boolean flags.

use core::ptr::NonNull;

use alloc::alloc::{
  Allocator,
  Global,
  Layout,
};

#[derive(Debug)]
/// Errors that can occur while operating on a [`Bitmap`].
pub enum BitmapError {
  /// Index was outside the bitmap bounds.
  OutOfBounds,
  /// Allocation from the underlying allocator failed.
  AllocError,
  /// Requested size was not a multiple of 8.
  InvalidSize,
}

#[derive(Debug)]
/// A dynamically sizable bitmap.
pub struct Bitmap<'map, A = Global>
where
  A: Allocator,
{
  /// Allocator used for backing storage.
  allocator: A,
  /// Slice holding the bitmap bits.
  map: &'map mut [u8],
  /// Layout used for the allocation.
  layout: Layout,
  /// Number of bytes in the bitmap.
  fields: usize,
}

impl<'map, A> Bitmap<'map, A>
where
  A: Allocator,
{
  /// Try to create a new bitmap in the given allocator.
  pub fn try_new_in(allocator: A, size: usize) -> Result<Self, BitmapError> {
    if !size.is_multiple_of(8) {
      return Err(BitmapError::InvalidSize);
    }

    let fields = size / 8;

    let layout = Layout::array::<u8>(fields).map_err(|_| BitmapError::InvalidSize)?;
    let ptr = allocator
      .allocate_zeroed(layout)
      .map_err(|_| BitmapError::AllocError)?;
    let map = unsafe { core::slice::from_raw_parts_mut(ptr.as_ptr() as *mut u8, fields) };
    Ok(Bitmap {
      allocator,
      map,
      layout,
      fields,
    })
  }

  /// Create a new bitmap, panicking on failure.
  pub fn new_in(allocator: A, size: usize) -> Self {
    Self::try_new_in(allocator, size).expect("Failed to create Bitmap")
  }
}

impl<'map> Bitmap<'map, Global> {
  /// Create a new bitmap using the global allocator.
  pub fn new(size: usize) -> Self {
    Self::new_in(Global, size)
  }
}

impl<'map, A> Bitmap<'map, A>
where
  A: Allocator,
{
  /// Try to set the bit at the given index.
  pub fn try_set(&mut self, index: usize) -> Result<(), BitmapError> {
    if index >= self.fields * 8 {
      return Err(BitmapError::OutOfBounds);
    }
    let byte_index = index / 8;
    let bit_index = index % 8;
    self.map[byte_index] |= 1 << bit_index;
    Ok(())
  }

  /// Set the bit at the given index, panicking on out-of-bounds.
  pub fn set(&mut self, index: usize) {
    self.try_set(index).expect("Bitmap index out of bounds");
  }

  /// Try to get the bit at the given index.
  pub fn try_get(&self, index: usize) -> Result<bool, BitmapError> {
    if index >= self.fields * 8 {
      return Err(BitmapError::OutOfBounds);
    }
    let byte_index = index / 8;
    let bit_index = index % 8;
    Ok((self.map[byte_index] & (1 << bit_index)) != 0)
  }

  /// Get the bit at the given index, panicking on out-of-bounds.
  pub fn get(&self, index: usize) -> bool {
    self.try_get(index).expect("Bitmap index out of bounds")
  }

  /// Try to clear the bit at the given index.
  pub fn try_clear(&mut self, index: usize) -> Result<(), BitmapError> {
    if index >= self.fields * 8 {
      return Err(BitmapError::OutOfBounds);
    }
    let byte_index = index / 8;
    let bit_index = index % 8;
    self.map[byte_index] &= !(1 << bit_index);
    Ok(())
  }

  /// Clear the bit at the given index, panicking on out-of-bounds.
  pub fn clear(&mut self, index: usize) {
    self.try_clear(index).expect("Bitmap index out of bounds");
  }

  /// Try to resize the bitmap to a new bit count.
  pub fn try_resize(&mut self, new_size: usize) -> Result<(), BitmapError> {
    if !new_size.is_multiple_of(8) {
      return Err(BitmapError::InvalidSize);
    }
    let new_fields = new_size / 8;
    if new_fields > self.fields {
      let new_layout = Layout::array::<u8>(new_fields).map_err(|_| BitmapError::InvalidSize)?;
      let old_ptr = NonNull::new(self.map.as_mut_ptr()).unwrap();
      let new_ptr = unsafe {
        self
          .allocator
          .grow_zeroed(old_ptr, self.layout, new_layout)
          .map_err(|_| BitmapError::AllocError)?
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

  /// Resize the bitmap, panicking on failure.
  pub fn resize(&mut self, new_size: usize) {
    self.try_resize(new_size).expect("Failed to resize Bitmap");
  }
}

impl<'map, A> Drop for Bitmap<'map, A>
where
  A: Allocator,
{
  fn drop(&mut self) {
    let layout = self.layout;
    unsafe {
      self.allocator.deallocate(
        NonNull::new(self.map.as_mut_ptr()).unwrap(),
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
    assert!(bitmap.try_set(10).is_ok());
    assert!(bitmap.try_get(10).unwrap());
    assert!(bitmap.try_clear(10).is_ok());
    assert!(!bitmap.try_get(10).unwrap());
  }

  #[test]
  fn test_resize() {
    let mut bitmap = Bitmap::new(64);
    assert!(bitmap.try_set(10).is_ok());
    assert!(bitmap.try_resize(128).is_ok());
    assert!(bitmap.try_get(10).unwrap());
  }
}
