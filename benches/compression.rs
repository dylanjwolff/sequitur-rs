use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use sequitur_rs::{Sequitur, SequiturDocuments};

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

fn bench_sequitur_repetitive(c: &mut Criterion) {
    let sizes = [1_000, 10_000, 100_000];
    let mut group = c.benchmark_group("repetitive_text");

    for size in sizes.iter() {
        let data = generate_repetitive_text(*size);

        group.bench_with_input(
            BenchmarkId::new("Sequitur", size),
            &data,
            |b, data| {
                b.iter(|| {
                    let mut seq = Sequitur::new();
                    seq.extend(black_box(data.chars()));
                    black_box(seq)
                });
            },
        );

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

        group.bench_with_input(
            BenchmarkId::new("Sequitur", size),
            &data,
            |b, data| {
                b.iter(|| {
                    let mut seq = Sequitur::new();
                    seq.extend(black_box(data.chars()));
                    black_box(seq)
                });
            },
        );

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

        group.bench_with_input(
            BenchmarkId::new("Sequitur", size),
            &data,
            |b, data| {
                b.iter(|| {
                    let mut seq = Sequitur::new();
                    seq.extend(black_box(data.chars()));
                    black_box(seq)
                });
            },
        );

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

        group.bench_with_input(
            BenchmarkId::new("Sequitur", size),
            &seq,
            |b, seq| {
                b.iter(|| {
                    let count: usize = black_box(seq.iter().count());
                    black_box(count)
                });
            },
        );

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

criterion_group!(
    benches,
    bench_sequitur_repetitive,
    bench_sequitur_source_code,
    bench_sequitur_low_repetition,
    bench_iteration
);
criterion_main!(benches);
