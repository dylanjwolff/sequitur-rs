# sequitur-rs

A Rust implementation of the Sequitur algorithm for incremental context-free grammar compression.

## Overview

Sequitur maintains a context-free grammar that compresses input sequences while enforcing two fundamental constraints:

1. **Digram Uniqueness**: No pair of consecutive symbols (digram) appears more than once in the grammar
2. **Rule Utility**: Every rule must be used at least twice

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
sequitur-rs = { path = "." }
```

## Examples

Run the file compression example:

```bash
cargo run --example main <filename>
```

This will:
1. Read the file byte-by-byte
2. Build the grammar
3. Verify reconstruction matches original
4. Print compression statistics

## Performance

## References

- [Original Sequitur Website](http://www.sequitur.info/)
- Nevill-Manning, C.G. and Witten, I.H. (1997) "Identifying Hierarchical Structure in Sequences: A linear-time algorithm"

## Contributing

Contributions welcome! Please ensure:
- All tests pass (`cargo test`)
- Code follows Rust conventions (`cargo fmt`, `cargo clippy`)
- New features include tests and documentation
