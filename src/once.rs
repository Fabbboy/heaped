#[derive(Debug, Default)]
pub enum Once<T> {
  #[default]
  Uninit,
  Init(T),
}

impl<T> Once<T> {
  pub fn new() -> Self {
    Once::Uninit
  }

  pub fn init(&mut self, value: T) -> Result<(), T> {
    match self {
      Once::Uninit => {
        *self = Once::Init(value);
        Ok(())
      }
      Once::Init(_) => Err(value),
    }
  }

  pub fn get(&self) -> Option<&T> {
    match self {
      Once::Uninit => None,
      Once::Init(value) => Some(value),
    }
  }
}
