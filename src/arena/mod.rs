mod chunk;
pub mod dropless;
pub mod typed;

#[cfg(test)]
pub mod tests;

pub use dropless::DroplessArena;
pub use typed::TypedArena;

const PAGE_SIZE: usize = 4096;
const HUGE_PAGE: usize = 2 * 1024 * 1024;
