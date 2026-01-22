use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use sequitur_rs::{Repair, Sequitur, SequiturDocuments, SequiturDocumentsRle, SequiturRle};

/// Generate repetitive text data
fn generate_repetitive_text(size: usize) -> String {
    let pattern = "the quick brown fox jumps over the lazy dog ";
    pattern.repeat(size / pattern.len())
}

/// Generate source code-like data
fn generate_source_code(size: usize) -> String {
    let patterns = [
        "fn main() {\n",
        "    let x = 42;\n",
        "    println!(\"Hello, world!\");\n",
        "    if x > 0 {\n",
        "        return x;\n",
        "    }\n",
        "}\n",
    ];

    let mut result = String::new();
    let mut i = 0;
    while result.len() < size {
        result.push_str(patterns[i % patterns.len()]);
        i += 1;
    }
    result.truncate(size);
    result
}

/// Generate low-repetition data (simulating base64)
fn generate_low_repetition(size: usize) -> String {
    let chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    let mut seed = 12345u64;

    for _ in 0..size {
        // Simple LCG random
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        let idx = (seed % chars.len() as u64) as usize;
        result.push(chars.chars().nth(idx).unwrap());
    }
    result
}

/// Generate long runs of identical characters (RLE best case)
fn generate_long_runs(size: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(size);
    let chars = [b'a', b'b', b'c', b'd'];
    let mut i = 0;
    let run_length = 100; // Each character repeats 100 times

    while result.len() < size {
        let ch = chars[i % chars.len()];
        for _ in 0..run_length.min(size - result.len()) {
            result.push(ch);
        }
        i += 1;
    }
    result
}

/// Generate (ab)^k pattern (where standard Sequitur is O(log k) but RLE is O(1))
fn generate_ab_pattern(k: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(k * 2);
    for _ in 0..k {
        result.push(b'a');
        result.push(b'b');
    }
    result
}

/// Generate difference-encoded sequence (0, 1, 1, 1, ...) - RLE best case
fn generate_difference_sequence(size: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(size);
    result.push(0);
    for _ in 1..size {
        result.push(1);
    }
    result
}

fn bench_sequitur_repetitive(c: &mut Criterion) {
    let sizes = [1_000, 10_000, 100_000];
    let mut group = c.benchmark_group("repetitive_text");

    for size in sizes.iter() {
        let data = generate_repetitive_text(*size);

        group.bench_with_input(BenchmarkId::new("Sequitur", size), &data, |b, data| {
            b.iter(|| {
                let mut seq = Sequitur::new();
                seq.extend(black_box(data.chars()));
                black_box(seq)
            });
        });

        group.bench_with_input(
            BenchmarkId::new("SequiturDocuments", size),
            &data,
            |b, data| {
                b.iter(|| {
                    let mut docs = SequiturDocuments::new();
                    docs.extend_document(black_box(1), black_box(data.chars()));
                    black_box(docs)
                });
            },
        );
    }

    group.finish();
}

fn bench_sequitur_source_code(c: &mut Criterion) {
    let sizes = [1_000, 10_000, 50_000];
    let mut group = c.benchmark_group("source_code");

    for size in sizes.iter() {
        let data = generate_source_code(*size);

        group.bench_with_input(BenchmarkId::new("Sequitur", size), &data, |b, data| {
            b.iter(|| {
                let mut seq = Sequitur::new();
                seq.extend(black_box(data.chars()));
                black_box(seq)
            });
        });

        group.bench_with_input(
            BenchmarkId::new("SequiturDocuments", size),
            &data,
            |b, data| {
                b.iter(|| {
                    let mut docs = SequiturDocuments::new();
                    docs.extend_document(black_box(1), black_box(data.chars()));
                    black_box(docs)
                });
            },
        );
    }

    group.finish();
}

fn bench_sequitur_low_repetition(c: &mut Criterion) {
    let sizes = [1_000, 10_000, 50_000];
    let mut group = c.benchmark_group("low_repetition");

    for size in sizes.iter() {
        let data = generate_low_repetition(*size);

        group.bench_with_input(BenchmarkId::new("Sequitur", size), &data, |b, data| {
            b.iter(|| {
                let mut seq = Sequitur::new();
                seq.extend(black_box(data.chars()));
                black_box(seq)
            });
        });

        group.bench_with_input(
            BenchmarkId::new("SequiturDocuments", size),
            &data,
            |b, data| {
                b.iter(|| {
                    let mut docs = SequiturDocuments::new();
                    docs.extend_document(black_box(1), black_box(data.chars()));
                    black_box(docs)
                });
            },
        );
    }

    group.finish();
}

fn bench_iteration(c: &mut Criterion) {
    let sizes = [1_000, 10_000, 100_000];
    let mut group = c.benchmark_group("iteration");

    for size in sizes.iter() {
        let data = generate_repetitive_text(*size);

        // Prepare pre-built structures
        let mut seq = Sequitur::new();
        seq.extend(data.chars());

        let mut docs = SequiturDocuments::new();
        docs.extend_document(1, data.chars());

        group.bench_with_input(BenchmarkId::new("Sequitur", size), &seq, |b, seq| {
            b.iter(|| {
                let count: usize = black_box(seq.iter().count());
                black_box(count)
            });
        });

        group.bench_with_input(
            BenchmarkId::new("SequiturDocuments", size),
            &docs,
            |b, docs| {
                b.iter(|| {
                    let count: usize = black_box(docs.iter_document(&1).unwrap().count());
                    black_box(count)
                });
            },
        );
    }

    group.finish();
}

// =============================================================================
// RLE-specific benchmarks
// =============================================================================

/// Benchmark long runs of identical characters - RLE best case
fn bench_long_runs(c: &mut Criterion) {
    let sizes = [1_000, 10_000, 100_000];
    let mut group = c.benchmark_group("long_runs");

    for size in sizes.iter() {
        let data = generate_long_runs(*size);

        group.bench_with_input(BenchmarkId::new("Sequitur", size), &data, |b, data| {
            b.iter(|| {
                let mut seq = Sequitur::new();
                seq.extend(black_box(data.iter().copied()));
                black_box(seq)
            });
        });

        group.bench_with_input(BenchmarkId::new("SequiturRle", size), &data, |b, data| {
            b.iter(|| {
                let mut seq = SequiturRle::new();
                seq.extend(black_box(data.iter().copied()));
                black_box(seq)
            });
        });
    }

    group.finish();
}

/// Benchmark (ab)^k pattern - demonstrates O(log k) vs O(1) rule complexity
fn bench_ab_pattern(c: &mut Criterion) {
    let ks = [100, 1_000, 10_000];
    let mut group = c.benchmark_group("ab_pattern");

    for k in ks.iter() {
        let data = generate_ab_pattern(*k);

        group.bench_with_input(BenchmarkId::new("Sequitur", k), &data, |b, data| {
            b.iter(|| {
                let mut seq = Sequitur::new();
                seq.extend(black_box(data.iter().copied()));
                black_box(seq)
            });
        });

        group.bench_with_input(BenchmarkId::new("SequiturRle", k), &data, |b, data| {
            b.iter(|| {
                let mut seq = SequiturRle::new();
                seq.extend(black_box(data.iter().copied()));
                black_box(seq)
            });
        });
    }

    group.finish();
}

/// Benchmark difference sequence (0, 1, 1, 1, ...) - RLE best case
fn bench_difference_sequence(c: &mut Criterion) {
    let sizes = [1_000, 10_000, 100_000];
    let mut group = c.benchmark_group("difference_sequence");

    for size in sizes.iter() {
        let data = generate_difference_sequence(*size);

        group.bench_with_input(BenchmarkId::new("Sequitur", size), &data, |b, data| {
            b.iter(|| {
                let mut seq = Sequitur::new();
                seq.extend(black_box(data.iter().copied()));
                black_box(seq)
            });
        });

        group.bench_with_input(BenchmarkId::new("SequiturRle", size), &data, |b, data| {
            b.iter(|| {
                let mut seq = SequiturRle::new();
                seq.extend(black_box(data.iter().copied()));
                black_box(seq)
            });
        });
    }

    group.finish();
}

/// Benchmark RLE on repetitive text (compare with standard Sequitur)
fn bench_rle_repetitive_text(c: &mut Criterion) {
    let sizes = [1_000, 10_000, 100_000];
    let mut group = c.benchmark_group("rle_repetitive_text");

    for size in sizes.iter() {
        let data = generate_repetitive_text(*size);

        group.bench_with_input(BenchmarkId::new("Sequitur", size), &data, |b, data| {
            b.iter(|| {
                let mut seq = Sequitur::new();
                seq.extend(black_box(data.chars()));
                black_box(seq)
            });
        });

        group.bench_with_input(BenchmarkId::new("SequiturRle", size), &data, |b, data| {
            b.iter(|| {
                let mut seq = SequiturRle::new();
                seq.extend(black_box(data.chars()));
                black_box(seq)
            });
        });
    }

    group.finish();
}

/// Benchmark RLE iteration
fn bench_rle_iteration(c: &mut Criterion) {
    let sizes = [1_000, 10_000, 100_000];
    let mut group = c.benchmark_group("rle_iteration");

    for size in sizes.iter() {
        let data = generate_long_runs(*size);

        // Prepare pre-built structures
        let mut seq = Sequitur::new();
        seq.extend(data.iter().copied());

        let mut rle = SequiturRle::new();
        rle.extend(data.iter().copied());

        group.bench_with_input(BenchmarkId::new("Sequitur", size), &seq, |b, seq| {
            b.iter(|| {
                let count: usize = black_box(seq.iter().count());
                black_box(count)
            });
        });

        group.bench_with_input(BenchmarkId::new("SequiturRle", size), &rle, |b, rle| {
            b.iter(|| {
                let count: usize = black_box(rle.iter().count());
                black_box(count)
            });
        });
    }

    group.finish();
}

/// Benchmark multi-document compression with RLE
fn bench_rle_documents(c: &mut Criterion) {
    let sizes = [1_000, 10_000];
    let mut group = c.benchmark_group("rle_documents");

    for size in sizes.iter() {
        // Create test data: multiple documents with shared patterns and runs
        let doc1: Vec<u8> = generate_long_runs(*size);
        let doc2: Vec<u8> = generate_long_runs(*size);
        let doc3: Vec<u8> = generate_difference_sequence(*size);

        group.bench_with_input(
            BenchmarkId::new("SequiturDocuments", size),
            &(&doc1, &doc2, &doc3),
            |b, (d1, d2, d3)| {
                b.iter(|| {
                    let mut docs = SequiturDocuments::new();
                    docs.extend_document(1, black_box(d1.iter().copied()));
                    docs.extend_document(2, black_box(d2.iter().copied()));
                    docs.extend_document(3, black_box(d3.iter().copied()));
                    black_box(docs)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("SequiturDocumentsRle", size),
            &(&doc1, &doc2, &doc3),
            |b, (d1, d2, d3)| {
                b.iter(|| {
                    let mut docs = SequiturDocumentsRle::new();
                    docs.extend_document(1, black_box(d1.iter().copied()));
                    docs.extend_document(2, black_box(d2.iter().copied()));
                    docs.extend_document(3, black_box(d3.iter().copied()));
                    black_box(docs)
                });
            },
        );
    }

    group.finish();
}

/// Print compression statistics comparison (not a timed benchmark)
fn print_compression_stats(c: &mut Criterion) {
    // This benchmark exists just to print compression statistics
    // The actual measurement is trivial
    let mut group = c.benchmark_group("compression_stats");
    group.sample_size(10);

    // Print header
    eprintln!("\n{:=^80}", " Compression Statistics Comparison ");
    eprintln!(
        "{:<25} {:>10} {:>12} {:>12} {:>12}",
        "Dataset", "Input", "Seq Rules", "Seq Syms", "RLE Nodes"
    );
    eprintln!("{:-<80}", "");

    // Long runs
    for size in [1_000, 10_000, 100_000] {
        let data = generate_long_runs(size);

        let mut seq = Sequitur::new();
        seq.extend(data.iter().copied());
        let seq_stats = seq.stats();

        let mut rle = SequiturRle::new();
        rle.extend(data.iter().copied());
        let rle_stats = rle.stats();

        eprintln!(
            "{:<25} {:>10} {:>12} {:>12} {:>12}",
            format!("long_runs_{}", size),
            size,
            seq_stats.num_rules,
            seq_stats.grammar_symbols,
            rle_stats.grammar_nodes
        );
    }

    // (ab)^k pattern
    for k in [100, 1_000, 10_000] {
        let data = generate_ab_pattern(k);

        let mut seq = Sequitur::new();
        seq.extend(data.iter().copied());
        let seq_stats = seq.stats();

        let mut rle = SequiturRle::new();
        rle.extend(data.iter().copied());
        let rle_stats = rle.stats();

        eprintln!(
            "{:<25} {:>10} {:>12} {:>12} {:>12}",
            format!("ab_pattern_{}", k),
            k * 2,
            seq_stats.num_rules,
            seq_stats.grammar_symbols,
            rle_stats.grammar_nodes
        );
    }

    // Difference sequence
    for size in [1_000, 10_000, 100_000] {
        let data = generate_difference_sequence(size);

        let mut seq = Sequitur::new();
        seq.extend(data.iter().copied());
        let seq_stats = seq.stats();

        let mut rle = SequiturRle::new();
        rle.extend(data.iter().copied());
        let rle_stats = rle.stats();

        eprintln!(
            "{:<25} {:>10} {:>12} {:>12} {:>12}",
            format!("diff_seq_{}", size),
            size,
            seq_stats.num_rules,
            seq_stats.grammar_symbols,
            rle_stats.grammar_nodes
        );
    }

    // Repetitive text
    for size in [1_000, 10_000, 100_000] {
        let data = generate_repetitive_text(size);

        let mut seq = Sequitur::new();
        seq.extend(data.chars());
        let seq_stats = seq.stats();

        let mut rle = SequiturRle::new();
        rle.extend(data.chars());
        let rle_stats = rle.stats();

        eprintln!(
            "{:<25} {:>10} {:>12} {:>12} {:>12}",
            format!("repetitive_text_{}", size),
            size,
            seq_stats.num_rules,
            seq_stats.grammar_symbols,
            rle_stats.grammar_nodes
        );
    }

    // Multi-document comparison
    eprintln!("\n{:-^80}", " Multi-Document Compression ");
    eprintln!(
        "{:<25} {:>10} {:>12} {:>12}",
        "Config", "Total Input", "Docs Symbols", "RLE Nodes"
    );
    eprintln!("{:-<80}", "");

    for size in [1_000, 10_000] {
        let doc1 = generate_long_runs(size);
        let doc2 = generate_long_runs(size);
        let doc3 = generate_difference_sequence(size);

        let mut std_docs = SequiturDocuments::new();
        std_docs.extend_document(1, doc1.iter().copied());
        std_docs.extend_document(2, doc2.iter().copied());
        std_docs.extend_document(3, doc3.iter().copied());
        let std_stats = std_docs.overall_stats();

        let mut rle_docs = SequiturDocumentsRle::new();
        rle_docs.extend_document(1, doc1.iter().copied());
        rle_docs.extend_document(2, doc2.iter().copied());
        rle_docs.extend_document(3, doc3.iter().copied());
        let rle_stats = rle_docs.overall_stats();

        eprintln!(
            "{:<25} {:>10} {:>12} {:>12}",
            format!("3_docs_{}", size),
            size * 3,
            std_stats.total_grammar_symbols,
            rle_stats.total_grammar_nodes
        );
    }

    eprintln!("{:=<80}\n", "");

    // Dummy benchmark to satisfy criterion
    group.bench_function("stats_printed", |b| b.iter(|| black_box(1)));
    group.finish();
}

// =============================================================================
// RePair benchmarks
// =============================================================================

/// Benchmark RePair vs Sequitur on repetitive text
fn bench_repair_repetitive(c: &mut Criterion) {
    let sizes = [1_000, 10_000, 50_000];
    let mut group = c.benchmark_group("repair_repetitive");

    for size in sizes.iter() {
        let data = generate_repetitive_text(*size);

        group.bench_with_input(BenchmarkId::new("Sequitur", size), &data, |b, data| {
            b.iter(|| {
                let mut seq = Sequitur::new();
                seq.extend(black_box(data.chars()));
                black_box(seq)
            });
        });

        group.bench_with_input(BenchmarkId::new("Repair", size), &data, |b, data| {
            b.iter(|| {
                let mut repair = Repair::new();
                repair.extend(black_box(data.chars()));
                repair.compress();
                black_box(repair)
            });
        });
    }

    group.finish();
}

/// Benchmark RePair vs Sequitur on source code-like data
fn bench_repair_source_code(c: &mut Criterion) {
    let sizes = [1_000, 10_000, 50_000];
    let mut group = c.benchmark_group("repair_source_code");

    for size in sizes.iter() {
        let data = generate_source_code(*size);

        group.bench_with_input(BenchmarkId::new("Sequitur", size), &data, |b, data| {
            b.iter(|| {
                let mut seq = Sequitur::new();
                seq.extend(black_box(data.chars()));
                black_box(seq)
            });
        });

        group.bench_with_input(BenchmarkId::new("Repair", size), &data, |b, data| {
            b.iter(|| {
                let mut repair = Repair::new();
                repair.extend(black_box(data.chars()));
                repair.compress();
                black_box(repair)
            });
        });
    }

    group.finish();
}

/// Benchmark RePair vs Sequitur on low-repetition data
fn bench_repair_low_repetition(c: &mut Criterion) {
    let sizes = [1_000, 10_000, 50_000];
    let mut group = c.benchmark_group("repair_low_repetition");

    for size in sizes.iter() {
        let data = generate_low_repetition(*size);

        group.bench_with_input(BenchmarkId::new("Sequitur", size), &data, |b, data| {
            b.iter(|| {
                let mut seq = Sequitur::new();
                seq.extend(black_box(data.chars()));
                black_box(seq)
            });
        });

        group.bench_with_input(BenchmarkId::new("Repair", size), &data, |b, data| {
            b.iter(|| {
                let mut repair = Repair::new();
                repair.extend(black_box(data.chars()));
                repair.compress();
                black_box(repair)
            });
        });
    }

    group.finish();
}

/// Benchmark RePair vs Sequitur on (ab)^k pattern
fn bench_repair_ab_pattern(c: &mut Criterion) {
    let ks = [100, 1_000, 5_000];
    let mut group = c.benchmark_group("repair_ab_pattern");

    for k in ks.iter() {
        let data = generate_ab_pattern(*k);

        group.bench_with_input(BenchmarkId::new("Sequitur", k), &data, |b, data| {
            b.iter(|| {
                let mut seq = Sequitur::new();
                seq.extend(black_box(data.iter().copied()));
                black_box(seq)
            });
        });

        group.bench_with_input(BenchmarkId::new("Repair", k), &data, |b, data| {
            b.iter(|| {
                let mut repair = Repair::new();
                repair.extend(black_box(data.iter().copied()));
                repair.compress();
                black_box(repair)
            });
        });
    }

    group.finish();
}

/// Benchmark RePair iteration (reconstruction)
fn bench_repair_iteration(c: &mut Criterion) {
    let sizes = [1_000, 10_000, 50_000];
    let mut group = c.benchmark_group("repair_iteration");

    for size in sizes.iter() {
        let data = generate_repetitive_text(*size);

        // Prepare pre-built structures
        let mut seq = Sequitur::new();
        seq.extend(data.chars());

        let mut repair = Repair::new();
        repair.extend(data.chars());
        repair.compress();

        group.bench_with_input(BenchmarkId::new("Sequitur", size), &seq, |b, seq| {
            b.iter(|| {
                let count: usize = black_box(seq.iter().count());
                black_box(count)
            });
        });

        group.bench_with_input(BenchmarkId::new("Repair", size), &repair, |b, repair| {
            b.iter(|| {
                let count: usize = black_box(repair.iter().count());
                black_box(count)
            });
        });
    }

    group.finish();
}

/// Print compression statistics including RePair
fn print_repair_compression_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("repair_compression_stats");
    group.sample_size(10);

    // Print header
    eprintln!("\n{:=^100}", " Sequitur vs RePair Compression Statistics ");
    eprintln!(
        "{:<25} {:>10} {:>12} {:>12} {:>12} {:>12}",
        "Dataset", "Input", "Seq Rules", "Seq Syms", "Rep Rules", "Rep Syms"
    );
    eprintln!("{:-<100}", "");

    // Repetitive text
    for size in [1_000, 10_000, 50_000] {
        let data = generate_repetitive_text(size);

        let mut seq = Sequitur::new();
        seq.extend(data.chars());
        let seq_stats = seq.stats();

        let mut repair = Repair::new();
        repair.extend(data.chars());
        repair.compress();
        let repair_stats = repair.stats();

        eprintln!(
            "{:<25} {:>10} {:>12} {:>12} {:>12} {:>12}",
            format!("repetitive_text_{}", size),
            size,
            seq_stats.num_rules,
            seq_stats.grammar_symbols,
            repair_stats.num_rules,
            repair_stats.grammar_symbols
        );
    }

    // Source code
    for size in [1_000, 10_000, 50_000] {
        let data = generate_source_code(size);

        let mut seq = Sequitur::new();
        seq.extend(data.chars());
        let seq_stats = seq.stats();

        let mut repair = Repair::new();
        repair.extend(data.chars());
        repair.compress();
        let repair_stats = repair.stats();

        eprintln!(
            "{:<25} {:>10} {:>12} {:>12} {:>12} {:>12}",
            format!("source_code_{}", size),
            size,
            seq_stats.num_rules,
            seq_stats.grammar_symbols,
            repair_stats.num_rules,
            repair_stats.grammar_symbols
        );
    }

    // (ab)^k pattern
    for k in [100, 1_000, 5_000] {
        let data = generate_ab_pattern(k);

        let mut seq = Sequitur::new();
        seq.extend(data.iter().copied());
        let seq_stats = seq.stats();

        let mut repair = Repair::new();
        repair.extend(data.iter().copied());
        repair.compress();
        let repair_stats = repair.stats();

        eprintln!(
            "{:<25} {:>10} {:>12} {:>12} {:>12} {:>12}",
            format!("ab_pattern_{}", k),
            k * 2,
            seq_stats.num_rules,
            seq_stats.grammar_symbols,
            repair_stats.num_rules,
            repair_stats.grammar_symbols
        );
    }

    // Low repetition
    for size in [1_000, 10_000, 50_000] {
        let data = generate_low_repetition(size);

        let mut seq = Sequitur::new();
        seq.extend(data.chars());
        let seq_stats = seq.stats();

        let mut repair = Repair::new();
        repair.extend(data.chars());
        repair.compress();
        let repair_stats = repair.stats();

        eprintln!(
            "{:<25} {:>10} {:>12} {:>12} {:>12} {:>12}",
            format!("low_repetition_{}", size),
            size,
            seq_stats.num_rules,
            seq_stats.grammar_symbols,
            repair_stats.num_rules,
            repair_stats.grammar_symbols
        );
    }

    eprintln!("{:=<100}\n", "");

    // Dummy benchmark to satisfy criterion
    group.bench_function("repair_stats_printed", |b| b.iter(|| black_box(1)));
    group.finish();
}

criterion_group!(
    benches,
    bench_sequitur_repetitive,
    bench_sequitur_source_code,
    bench_sequitur_low_repetition,
    bench_iteration,
    // RLE benchmarks
    bench_long_runs,
    bench_ab_pattern,
    bench_difference_sequence,
    bench_rle_repetitive_text,
    bench_rle_iteration,
    bench_rle_documents,
    // RePair benchmarks
    bench_repair_repetitive,
    bench_repair_source_code,
    bench_repair_low_repetition,
    bench_repair_ab_pattern,
    bench_repair_iteration,
    // Statistics comparison
    print_compression_stats,
    print_repair_compression_stats,
);
criterion_main!(benches);
