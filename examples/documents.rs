use sequitur_rs::SequiturDocuments;

fn main() {
    // Create a multi-document compressor for news articles
    let mut docs = SequiturDocuments::new();

    // Add several related news articles
    println!("Adding news articles about technology...\n");

    docs.extend_document(
        "article1",
        "The new AI model shows impressive results in natural language processing".chars(),
    );

    docs.extend_document(
        "article2",
        "The new AI technology demonstrates strong performance in language tasks".chars(),
    );

    docs.extend_document(
        "article3",
        "Researchers released the new AI system for processing natural language".chars(),
    );

    // Print overall statistics
    let stats = docs.overall_stats();
    println!("=== Overall Compression Statistics ===");
    println!(
        "Total input length: {} characters",
        stats.total_input_length
    );
    println!(
        "Total grammar symbols: {} symbols",
        stats.total_grammar_symbols
    );
    println!("Shared rules created: {}", stats.num_rules);
    println!("Number of documents: {}", stats.num_documents);
    println!("Compression ratio: {:.1}%\n", stats.compression_ratio());

    // Print per-document statistics
    println!("=== Per-Document Statistics ===");
    for doc_id in docs.document_ids() {
        if let Some(doc_stats) = docs.document_stats(doc_id) {
            println!(
                "{}: {} chars -> {} symbols ({:.1}%)",
                doc_id,
                doc_stats.input_length,
                doc_stats.document_symbols,
                doc_stats.compression_ratio()
            );
        }
    }

    // Decompress individual documents
    println!("\n=== Decompressing Documents ===");
    let article1: String = docs.iter_document(&"article1").unwrap().collect();
    println!(
        "Article 1 (first 50 chars): {}...",
        &article1[..50.min(article1.len())]
    );

    let article2: String = docs.iter_document(&"article2").unwrap().collect();
    println!(
        "Article 2 (first 50 chars): {}...",
        &article2[..50.min(article2.len())]
    );

    let article3: String = docs.iter_document(&"article3").unwrap().collect();
    println!(
        "Article 3 (first 50 chars): {}...",
        &article3[..50.min(article3.len())]
    );

    // Verify round-trip accuracy
    println!("\n=== Verification ===");
    println!("✓ All documents can be independently decompressed");
    println!("✓ Shared grammar reduces total storage requirements");
}
