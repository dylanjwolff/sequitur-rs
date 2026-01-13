# sequitur-rs

A Rust implementation of the Sequitur algorithm for incremental context-free grammar compression.

## Overview

Sequitur maintains a context-free grammar that compresses input sequences while enforcing two fundamental constraints:

1. **Digram Uniqueness**: No pair of consecutive symbols (digram) appears more than once in the grammar
2. **Rule Utility**: Every rule must be used at least twice

This implementation is a port of the C++ reference implementation, using idiomatic Rust patterns with safe memory management via SlotMap.

## Features

- **Memory Safe**: No unsafe code required - uses SlotMap for efficient generational indices
- **Generic**: Works with any type implementing `Hash + Eq + Clone`
- **Property Tested**: Comprehensive test suite with proptest and bolero
- **Performant**: O(1) amortized time per symbol added
- **Well Documented**: Extensive inline documentation and examples

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
sequitur-rs = { path = "." }
```

## Usage

### Basic Example

```rust
use sequitur_rs::Sequitur;

fn main() {
    let mut seq = Sequitur::new();

    // Add a sequence with repetition
    seq.extend("abcabcabc".chars());

    // Reconstruct the original sequence
    let reconstructed: String = seq.iter().collect();
    assert_eq!(reconstructed, "abcabcabc");

    // Get compression statistics
    let stats = seq.stats();
    println!("Grammar has {} rules", stats.num_rules);
    println!("Compression ratio: {:.2}%", stats.compression_ratio());
}
```

### Processing Files

```rust
use sequitur_rs::Sequitur;
use std::fs::File;
use std::io::{BufReader, Read};

fn compress_file(filename: &str) {
    let file = File::open(filename).unwrap();
    let reader = BufReader::new(file);

    let mut seq = Sequitur::new();

    for byte_result in reader.bytes() {
        seq.push(byte_result.unwrap());
    }

    let stats = seq.stats();
    println!("Original: {} bytes", stats.input_length);
    println!("Compressed: {} symbols", stats.grammar_symbols);
    println!("Ratio: {:.2}%", stats.compression_ratio());
}
```

## API

### Main Types

- `Sequitur<T>`: The main compression structure
- `SequiturIter<'a, T>`: Iterator that reconstructs the original sequence
- `CompressionStats`: Statistics about the compression

### Methods

- `Sequitur::new()` - Create a new empty instance
- `.push(value: T)` - Add a single value
- `.extend(iter)` - Add multiple values
- `.len()` - Number of values added
- `.iter()` - Get an iterator over the reconstructed sequence
- `.stats()` - Get compression statistics
- `.rules()` - Access to internal rule structure

## Algorithm Details

### How It Works

1. As symbols are added, the algorithm checks for repeated digrams
2. When a digram appears twice, a new rule is created
3. Both occurrences are replaced with a reference to the rule
4. If a rule is only used once, it's expanded inline (rule utility)
5. This continues recursively, potentially creating nested rules

### Example Trace

Input: `abcabcabc`

1. Add `a`, `b`, `c` → Grammar: `abc`
2. Add second `a`, `b`, `c` → Digram `ab` repeats
3. Create Rule 1: `ab` → Grammar: `Rule1 c Rule1 c`
4. Add third `abc` → Digram `Rule1 c` repeats
5. Create Rule 2: `Rule1 c` → Grammar: `Rule2 Rule2`

Final grammar:
- Rule 0: `Rule2 Rule2`
- Rule 1: `ab`
- Rule 2: `Rule1 c`

### Complexity

- **Time**: O(1) amortized per symbol (hash-based digram lookup)
- **Space**: Grammar grows sub-linearly with input size for repetitive data

## Implementation Details

### Design Decisions

1. **Enum-based Symbols**: Uses Rust enums instead of inheritance hierarchy
2. **SlotMap Storage**: Generational indices prevent use-after-free
3. **Hash-based Digrams**: 64-bit hashes for O(1) lookup with collision detection
4. **No Unsafe Code**: Entirely safe Rust in the core implementation

### Differences from C++

- Uses `SlotMap` instead of raw pointers for memory management
- Enum variants instead of virtual inheritance
- Simplified ID generation (no object pooling)
- Integrated statistics API

## Testing

The implementation includes:

- **Unit Tests**: Basic functionality and edge cases
- **Property Tests** (proptest): Roundtrip fidelity, constraint preservation
- **Fuzz Tests** (bolero): No panics on arbitrary input
- **Integration Tests**: File compression examples

Run tests:

```bash
cargo test
```

Run with more proptest cases:

```bash
cargo test -- --test-threads=1
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

### Benchmark Results

The Rust implementation **outperforms the C++ reference implementation** by **1.5-3.0x** across all test cases:

| Test Case             | Input Size | C++ Time | Rust Time | Speedup |
|-----------------------|------------|----------|-----------|---------|
| Small (repetitive)    | 1KB        | 6.0ms    | 2.0ms     | **3.0x** |
| Medium (repetitive)   | 100KB      | 122ms    | 76ms      | **1.6x** |
| Large (repetitive)    | 1MB        | 936ms    | 601ms     | **1.55x** |
| Source Code           | 38KB       | 50ms     | 32ms      | **1.56x** |
| Low Repetition        | 66KB       | 86ms     | 37ms      | **2.32x** |

### Compression Ratios

- Highly repetitive text: **0.01-2.24%** (excellent compression)
- Source code: **20-40%** (good compression)
- Random/low repetition: **60-70%** (limited compression)

See [BENCHMARK.md](BENCHMARK.md) for detailed analysis.

### Run Benchmarks

```bash
./benchmark_detailed.sh
```

## References

- [Original Sequitur Paper](http://www.sequitur.info/)
- Nevill-Manning, C.G. and Witten, I.H. (1997) "Identifying Hierarchical Structure in Sequences: A linear-time algorithm"

## License

This implementation is provided as-is for educational and research purposes.

## Contributing

Contributions welcome! Please ensure:
- All tests pass (`cargo test`)
- Code follows Rust conventions (`cargo fmt`, `cargo clippy`)
- New features include tests and documentation
