use sequitur_rs::Sequitur;
use std::env;
use std::fs::File;
use std::io::{BufReader, Read};

/// Example program mirroring the C++ main.cpp.
///
/// Usage: cargo run --example main <filename>
fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <filename>", args[0]);
        std::process::exit(1);
    }

    let filename = &args[1];

    // Open file
    let file = File::open(filename).unwrap_or_else(|_| {
        eprintln!("File \"{}\" not found.", filename);
        std::process::exit(1);
    });

    // Create Sequitur for char type
    let mut seq = Sequitur::new();
    let mut count = 0usize;

    // Read file byte by byte and add to Sequitur
    let reader = BufReader::new(file);
    for byte_result in reader.bytes() {
        let byte = byte_result.expect("Error reading file");
        seq.push(byte);
        count += 1;

        // Print progress every 100,000 bytes
        if count % 100_000 == 0 {
            println!("{}", count);
        }
    }

    // Verify by reconstructing
    let file = File::open(filename).expect("Cannot reopen file");
    let reader = BufReader::new(file);
    let mut seq_iter = seq.iter();
    let mut verify_count = 0;

    for byte_result in reader.bytes() {
        let file_byte = byte_result.expect("Error reading file");
        let seq_byte = seq_iter.next().expect("Sequitur ended early");

        if file_byte != *seq_byte {
            eprintln!(
                "Mismatch at position {}: file={}, seq={}",
                verify_count, file_byte, seq_byte
            );
        }

        verify_count += 1;
    }

    // Compute statistics
    let stats = seq.stats();

    println!("\n=== Statistics ===");
    println!("Total bytes inserted: {}", stats.input_length);
    println!("Symbols in grammar: {}", stats.grammar_symbols);
    println!("Rules created: {}", stats.num_rules);
    println!("Compression ratio: {:.2}%", stats.compression_ratio());
}
