#!/bin/bash
set -e

echo "╔═══════════════════════════════════════════════════════════════╗"
echo "║       Sequitur Performance Benchmark: Rust vs C++             ║"
echo "╚═══════════════════════════════════════════════════════════════╝"
echo ""

# Create benchmark directory
BENCH_DIR="/tmp/sequitur-bench"
mkdir -p "$BENCH_DIR"

# Generate test files
echo "📁 Generating test files..."

# Test 1: Small repetitive text (1KB)
python3 -c "print('abcdefgh' * 128)" > "$BENCH_DIR/small.txt"

# Test 2: Medium repetitive text (100KB)
python3 -c "print('the quick brown fox jumps over the lazy dog ' * 2000)" > "$BENCH_DIR/medium.txt"

# Test 3: Large repetitive text (1MB)
python3 -c "
text = 'Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor '
print((text * 500) * 25)
" > "$BENCH_DIR/large.txt"

# Test 4: Source code
cat src/*.rs > "$BENCH_DIR/source.txt"

# Test 5: Low repetition
head -c 50000 /dev/urandom | base64 > "$BENCH_DIR/random.txt"

echo "✓ Test files ready"
echo ""

CPP_BIN="./cpp-sequitur/build/sequitur"
RUST_BIN="./target/release/examples/main"

run_benchmark() {
    local name=$1
    local file=$2
    local size=$(stat -c%s "$file")

    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    printf "📊 %-50s %10s\n" "$name" "$(numfmt --to=iec-i --suffix=B $size)"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    # C++ benchmark
    local cpp_output=$( (time "$CPP_BIN" "$file" > /dev/null) 2>&1 )
    local cpp_stats=$(echo "$cpp_output" | grep "symbols\|rules" | head -3)
    local cpp_time=$(echo "$cpp_output" | grep "real" | awk '{print $2}')

    # Rust benchmark
    local rust_output=$( (time "$RUST_BIN" "$file" 2>&1) 2>&1 )
    local rust_stats=$(echo "$rust_output" | grep -E "bytes inserted|Symbols|Rules|ratio")
    local rust_time=$(echo "$rust_output" | grep "real" | awk '{print $2}')

    echo ""
    echo "C++ Implementation:"
    echo "$cpp_stats"
    echo "⏱️  Time: $cpp_time"
    echo ""

    echo "Rust Implementation:"
    echo "$rust_stats"
    echo "⏱️  Time: $rust_time"
    echo ""

    # Calculate speedup
    cpp_secs=$(echo "$cpp_time" | sed 's/[^0-9.ms]//g' | awk -F'm' '{print $1*60+$2}' | sed 's/s//')
    rust_secs=$(echo "$rust_time" | sed 's/[^0-9.ms]//g' | awk -F'm' '{print $1*60+$2}' | sed 's/s//')

    if [ -n "$cpp_secs" ] && [ -n "$rust_secs" ] && [ "$rust_secs" != "0" ]; then
        speedup=$(echo "scale=2; $cpp_secs / $rust_secs" | bc)
        if (( $(echo "$speedup > 1" | bc -l) )); then
            echo "🚀 Rust is ${speedup}x faster"
        else
            slowdown=$(echo "scale=2; $rust_secs / $cpp_secs" | bc)
            echo "⚠️  Rust is ${slowdown}x slower"
        fi
    fi

    echo ""
}

# Run benchmarks
run_benchmark "Test 1: Small (1KB repetitive)" "$BENCH_DIR/small.txt"
run_benchmark "Test 2: Medium (100KB repetitive)" "$BENCH_DIR/medium.txt"
run_benchmark "Test 3: Large (1MB repetitive)" "$BENCH_DIR/large.txt"
run_benchmark "Test 4: Source Code (Rust files)" "$BENCH_DIR/source.txt"
run_benchmark "Test 5: Low Repetition (base64)" "$BENCH_DIR/random.txt"

echo "╔═══════════════════════════════════════════════════════════════╗"
echo "║                    Benchmark Complete!                        ║"
echo "╚═══════════════════════════════════════════════════════════════╝"
