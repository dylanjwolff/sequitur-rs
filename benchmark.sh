#!/bin/bash
set -e

echo "=== Sequitur Benchmark: Rust vs C++ ==="
echo ""

# Create benchmark directory
BENCH_DIR="/tmp/sequitur-bench"
mkdir -p "$BENCH_DIR"

# Generate test files
echo "Generating test files..."

# Test 1: Small repetitive text (1KB)
python3 -c "print('abcdefgh' * 128)" > "$BENCH_DIR/small.txt"

# Test 2: Medium repetitive text (100KB)
python3 -c "print('the quick brown fox jumps over the lazy dog ' * 2000)" > "$BENCH_DIR/medium.txt"

# Test 3: Large repetitive text (1MB)
python3 -c "
text = 'Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor '
print((text * 500) * 25)
" > "$BENCH_DIR/large.txt"

# Test 4: Source code (Rust implementation itself)
cat src/*.rs > "$BENCH_DIR/source.txt"

# Test 5: Low repetition (random-ish)
head -c 50000 /dev/urandom | base64 > "$BENCH_DIR/random.txt"

echo "Test files created:"
ls -lh "$BENCH_DIR"
echo ""

CPP_BIN="./cpp-sequitur/build/sequitur"
RUST_BIN="./target/release/examples/main"

run_benchmark() {
    local name=$1
    local file=$2
    local size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file")

    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "Test: $name ($(numfmt --to=iec-i --suffix=B $size 2>/dev/null || echo "$size bytes"))"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    # C++ benchmark
    echo "C++:"
    (time "$CPP_BIN" "$file" 2>&1) 2>&1 | tail -4

    echo ""

    # Rust benchmark
    echo "Rust:"
    (time "$RUST_BIN" "$file" 2>&1) 2>&1 | tail -5

    echo ""
}

# Run benchmarks
run_benchmark "Small (1KB repetitive)" "$BENCH_DIR/small.txt"
run_benchmark "Medium (100KB repetitive)" "$BENCH_DIR/medium.txt"
run_benchmark "Large (1MB repetitive)" "$BENCH_DIR/large.txt"
run_benchmark "Source Code (Rust src)" "$BENCH_DIR/source.txt"
run_benchmark "Low Repetition (base64)" "$BENCH_DIR/random.txt"

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Benchmark complete!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
