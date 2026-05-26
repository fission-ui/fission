# fission-text-engine

Text-buffer primitives for Fission editors and editable text widgets.

`fission-text-engine` wraps rope-backed text storage and editing operations used by higher-level Fission text surfaces. Most application developers should use the text widgets exposed by the `fission` facade. Depend on this crate directly when building editor features, custom text controls, or framework-level text tooling.

## What it contains

- Rope-backed text storage suitable for large editable documents.
- Line and byte indexing helpers.
- Insert, delete, and replace operations used by text input and editor-style widgets.
- Small, dependency-light primitives that can be reused by shells and tests.

## Example

```rust,ignore
use fission_text_engine::TextBuffer;

let mut buffer = TextBuffer::from("hello");
buffer.insert(5, " world");
assert_eq!(buffer.to_string(), "hello world");
```

## Documentation

Start with the Fission text and input guides at [fission.rs](https://fission.rs). API documentation and guides are available at https://fission.rs.

## License

MIT
