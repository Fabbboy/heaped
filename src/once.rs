//! Single-assignment helper type.

#[derive(Debug, Default)]
/// A value that can be written to at most once.
pub enum Once<T> {
  /// The value has not been initialized.
  #[default]
  Uninit,
  /// The value has been initialized.
  Init(T),
}

impl<T> Once<T> {
  /// Create a new uninitialized instance.
  pub fn new() -> Self {
    Once::Uninit
  }

  /// Try to initialize the value, returning the input on failure.
  pub fn try_init(&mut self, value: T) -> Result<(), T> {
    match self {
      Once::Uninit => {
        *self = Once::Init(value);
        Ok(())
      }
      Once::Init(_) => Err(value),
    }
  }

  /// Initialize the value, panicking if it was already set.
  pub fn init(&mut self, value: T) {
    if self.try_init(value).is_err() {
      panic!("Once instance has already been initialized");
    }
  }

  /// Get a reference to the value if it has been initialized.
  pub fn get(&self) -> Option<&T> {
    match self {
      Once::Uninit => None,
      Once::Init(value) => Some(value),
    }
  }
}
