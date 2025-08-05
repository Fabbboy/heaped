## Heaped
Heaped is a collection of types providing memory handling and allocations. Primarily Heaped should be a library that solely works on `alloc` and `core`. This means heaped should be able to be used on `no_std`.

## Examples

### Once

```rust
use heaped::once::Once;

let mut value = Once::new();
assert!(value.get().is_none());

assert!(value.init(10).is_ok());
assert_eq!(value.get(), Some(&10));
```

### DroplessArena

```rust
#![no_std]
#![feature(allocator_api)]

extern crate alloc;

use alloc::alloc::{Allocator, Layout};
use heaped::arena::dropless::DroplessArena;

let arena = DroplessArena::new(1024);
let layout = Layout::array::<u8>(11).unwrap();
let mut bytes = arena.allocate_zeroed(layout).unwrap();
let slice = unsafe { bytes.as_mut() };
slice.copy_from_slice(b"Hello World");
assert_eq!(slice, b"Hello World");
```
