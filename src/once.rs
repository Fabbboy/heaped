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

  pub fn try_init(&mut self, value: T) -> Result<(), T> {
    match self {
      Once::Uninit => {
        *self = Once::Init(value);
        Ok(())
      }
      Once::Init(_) => Err(value),
    }
  }

  pub fn init(&mut self, value: T) {
    if self.try_init(value).is_err() {
      panic!("Once instance has already been initialized");
    }
  }

  pub fn get(&self) -> Option<&T> {
    match self {
      Once::Uninit => None,
      Once::Init(value) => Some(value),
    }
  }
}
