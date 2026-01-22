use crate::rle_sequitur::SequiturRle;
use crate::symbol::Symbol;
use proptest::prelude::*;

/// Gets the reference count for a rule in RLE-Sequitur.
fn get_rule_count<T>(seq: &SequiturRle<T>, head_key: slotmap::DefaultKey) -> u32 {
    if let Symbol::RuleHead { count, .. } = seq.grammar.symbols[head_key].symbol {
        count
    } else {
        0
    }
}

proptest! {
    /// Property 1: Roundtrip fidelity
    /// The reconstructed sequence must exactly match the input.
    #[test]
    fn prop_rle_roundtrip(input: Vec<u8>) {
        let mut seq = SequiturRle::new();
        seq.extend(input.clone());

        let reconstructed: Vec<u8> = seq.iter().copied().collect();
        prop_assert_eq!(reconstructed, input);
    }

    /// Property 2: Length preservation
    #[test]
    fn prop_rle_length_preserved(input: Vec<u8>) {
        let mut seq = SequiturRle::new();
        seq.extend(input.clone());

        let count = seq.iter().count();
        prop_assert_eq!(count, input.len());
        prop_assert_eq!(seq.len(), input.len());
    }

    /// Property 3: Rule utility constraint
    /// Every rule (except Rule 0) must be used at least twice.
    #[test]
    fn prop_rle_rule_utility(input: Vec<u8>) {
        let mut seq = SequiturRle::new();
        seq.extend(input);

        for (&rule_id, &head_key) in seq.rules() {
            if rule_id != 0 {
                let count = get_rule_count(&seq, head_key);
                prop_assert!(
                    count >= 2,
                    "Rule {} has count {}, expected >= 2",
                    rule_id,
                    count
                );
            }
        }
    }

    /// Property 4: Incremental vs batch equivalence
    #[test]
    fn prop_rle_incremental_equivalence(input: Vec<u8>) {
        let mut seq1 = SequiturRle::new();
        seq1.extend(input.clone());
        let result1: Vec<u8> = seq1.iter().copied().collect();

        let mut seq2 = SequiturRle::new();
        for &item in &input {
            seq2.push(item);
        }
        let result2: Vec<u8> = seq2.iter().copied().collect();

        prop_assert_eq!(result1, result2);
    }

    /// Property 5: RLE should be at least as efficient as standard Sequitur for runs
    #[test]
    fn prop_rle_run_compression(run_char: u8, run_len in 1usize..1000) {
        let input: Vec<u8> = vec![run_char; run_len];

        let mut seq = SequiturRle::new();
        seq.extend(input);

        let stats = seq.stats();
        // A single run should be represented by a single node
        prop_assert_eq!(
            stats.grammar_nodes, 1,
            "A run of {} should use 1 node, got {}",
            run_len, stats.grammar_nodes
        );
    }
}

/// Bolero fuzz test: No panics on arbitrary input
#[cfg(test)]
#[test]
fn fuzz_rle_no_panic() {
    bolero::check!().with_type::<Vec<u8>>().for_each(|input| {
        let mut seq = SequiturRle::new();
        seq.extend(input.iter().copied());

        // Verify basic operations don't panic
        let _ = seq.len();
        let _ = seq.is_empty();
        let _count = seq.iter().count();

        // Roundtrip should work
        let reconstructed: Vec<u8> = seq.iter().copied().collect();
        assert_eq!(reconstructed.len(), input.len());
        assert_eq!(reconstructed, *input);
    });
}

/// Bolero fuzz test: Rule utility is always maintained
#[cfg(test)]
#[test]
fn fuzz_rle_rule_utility() {
    bolero::check!().with_type::<Vec<u8>>().for_each(|input| {
        let mut seq = SequiturRle::new();
        seq.extend(input.iter().copied());

        for (&rule_id, &head_key) in seq.rules() {
            if rule_id != 0 {
                let count = get_rule_count(&seq, head_key);
                assert!(
                    count >= 2,
                    "Rule {} has count {}, violates rule utility constraint",
                    rule_id,
                    count
                );
            }
        }
    });
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_run_length_encoding_basic() {
        let mut seq = SequiturRle::new();
        seq.push('a');
        seq.push('a');
        seq.push('a');

        // Should have only 1 node with run=3
        let stats = seq.stats();
        assert_eq!(stats.grammar_nodes, 1);

        // Verify roundtrip
        let result: Vec<char> = seq.iter().copied().collect();
        assert_eq!(result, vec!['a', 'a', 'a']);
    }

    #[test]
    fn test_alternating_no_merge() {
        let mut seq = SequiturRle::new();
        seq.extend(vec!['a', 'b', 'a', 'b']);

        // Verify roundtrip
        let result: Vec<char> = seq.iter().copied().collect();
        assert_eq!(result, vec!['a', 'b', 'a', 'b']);
    }

    #[test]
    fn test_ab_k_pattern() {
        // This is the key example from RLE.md
        // (ab)^k should be represented efficiently with RLE
        let mut seq = SequiturRle::new();

        // Create (ab)^8 = abababababababab
        for _ in 0..8 {
            seq.push('a');
            seq.push('b');
        }

        // Verify roundtrip
        let result: String = seq.iter().collect();
        assert_eq!(result, "abababababababab");

        // The grammar should be efficient
        let stats = seq.stats();
        println!(
            "RLE (ab)^8: {} rules, {} nodes, input_len={}",
            stats.num_rules, stats.grammar_nodes, stats.input_length
        );
    }

    #[test]
    fn test_long_run_single_node() {
        let mut seq = SequiturRle::new();

        // A long run should be a single node
        for _ in 0..1000 {
            seq.push('x');
        }

        assert_eq!(seq.len(), 1000);

        let stats = seq.stats();
        assert_eq!(stats.grammar_nodes, 1, "Long run should be 1 node");

        // Verify roundtrip
        let result: Vec<char> = seq.iter().copied().collect();
        assert_eq!(result.len(), 1000);
        assert!(result.iter().all(|&c| c == 'x'));
    }

    #[test]
    fn test_mixed_runs_and_singles() {
        let mut seq = SequiturRle::new();

        // "aaabbbccc"
        seq.extend(vec!['a', 'a', 'a', 'b', 'b', 'b', 'c', 'c', 'c']);

        let result: String = seq.iter().collect();
        assert_eq!(result, "aaabbbccc");

        // Should have 3 nodes: a:3, b:3, c:3
        let stats = seq.stats();
        assert_eq!(stats.grammar_nodes, 3);
    }

    #[test]
    fn test_difference_encoding_example() {
        // From RLE.md: (0, 1, 1, ..., 1) should compress well
        let mut seq = SequiturRle::new();
        seq.push(0u8);
        for _ in 0..9 {
            seq.push(1u8);
        }

        let result: Vec<u8> = seq.iter().copied().collect();
        assert_eq!(result, vec![0, 1, 1, 1, 1, 1, 1, 1, 1, 1]);

        // Should have 2 nodes: 0:1, 1:9
        let stats = seq.stats();
        assert_eq!(stats.grammar_nodes, 2);
    }

    #[test]
    fn test_empty_sequence() {
        let seq = SequiturRle::<char>::new();
        assert!(seq.is_empty());
        assert_eq!(seq.len(), 0);

        let result: Vec<&char> = seq.iter().collect();
        assert!(result.is_empty());
    }

    #[test]
    fn test_single_element() {
        let mut seq = SequiturRle::new();
        seq.push('x');

        assert_eq!(seq.len(), 1);
        let result: Vec<char> = seq.iter().copied().collect();
        assert_eq!(result, vec!['x']);
    }

    #[test]
    fn test_rle_vs_standard_efficiency() {
        // Compare RLE efficiency for a long run
        use crate::Sequitur;

        let input: Vec<u8> = vec![42; 100];

        let mut standard = Sequitur::new();
        standard.extend(input.clone());
        let standard_stats = standard.stats();

        let mut rle = SequiturRle::new();
        rle.extend(input);
        let rle_stats = rle.stats();

        // RLE should use fewer nodes for runs
        assert!(
            rle_stats.grammar_nodes <= standard_stats.grammar_symbols,
            "RLE ({} nodes) should be more efficient than standard ({} symbols) for runs",
            rle_stats.grammar_nodes,
            standard_stats.grammar_symbols
        );
    }

    #[test]
    fn test_rule_utility_maintained() {
        let mut seq = SequiturRle::new();
        seq.extend("abracadabra".chars());

        for (&rule_id, &head_key) in seq.rules() {
            if rule_id != 0 {
                let count = get_rule_count(&seq, head_key);
                assert!(count >= 2, "Rule {} only used {} times", rule_id, count);
            }
        }
    }
}

// =============================================================================
// SequiturDocumentsRle Tests
// =============================================================================

#[cfg(test)]
mod rle_documents_tests {
    use crate::rle_documents::SequiturDocumentsRle;

    #[test]
    fn test_documents_roundtrip() {
        let mut docs = SequiturDocumentsRle::new();

        docs.extend_document(1, "hello world".chars());
        docs.extend_document(2, "hello there".chars());

        let result1: String = docs.iter_document(&1).unwrap().collect();
        let result2: String = docs.iter_document(&2).unwrap().collect();

        assert_eq!(result1, "hello world");
        assert_eq!(result2, "hello there");
    }

    #[test]
    fn test_documents_with_runs() {
        let mut docs = SequiturDocumentsRle::new();

        // Document with runs
        docs.extend_document("doc1", "aaabbbccc".chars());
        docs.extend_document("doc2", "xxxyyyzzz".chars());

        let result1: String = docs.iter_document(&"doc1").unwrap().collect();
        let result2: String = docs.iter_document(&"doc2").unwrap().collect();

        assert_eq!(result1, "aaabbbccc");
        assert_eq!(result2, "xxxyyyzzz");

        // Check compression - each doc should have 3 nodes (a:3, b:3, c:3)
        let stats1 = docs.document_stats(&"doc1").unwrap();
        let stats2 = docs.document_stats(&"doc2").unwrap();

        assert_eq!(stats1.document_nodes, 3);
        assert_eq!(stats2.document_nodes, 3);
    }

    #[test]
    fn test_documents_shared_patterns() {
        let mut docs = SequiturDocumentsRle::new();

        // Documents with shared patterns
        docs.extend_document(1, vec!['a', 'b', 'a', 'b']);
        docs.extend_document(2, vec!['a', 'b', 'c', 'd']);
        docs.extend_document(3, vec!['a', 'b', 'a', 'b', 'a', 'b']);

        let result1: Vec<_> = docs.iter_document(&1).unwrap().copied().collect();
        let result2: Vec<_> = docs.iter_document(&2).unwrap().copied().collect();
        let result3: Vec<_> = docs.iter_document(&3).unwrap().copied().collect();

        assert_eq!(result1, vec!['a', 'b', 'a', 'b']);
        assert_eq!(result2, vec!['a', 'b', 'c', 'd']);
        assert_eq!(result3, vec!['a', 'b', 'a', 'b', 'a', 'b']);
    }

    #[test]
    fn test_documents_long_runs() {
        let mut docs = SequiturDocumentsRle::new();

        // Document with long runs
        for _ in 0..1000 {
            docs.push_to_document(1, 'x');
        }

        assert_eq!(docs.document_len(&1), Some(1000));

        let result: Vec<_> = docs.iter_document(&1).unwrap().copied().collect();
        assert_eq!(result.len(), 1000);
        assert!(result.iter().all(|&c| c == 'x'));

        // Should be just 1 node
        let stats = docs.document_stats(&1).unwrap();
        assert_eq!(stats.document_nodes, 1);
    }

    #[test]
    fn test_documents_difference_sequence() {
        let mut docs = SequiturDocumentsRle::new();

        // (0, 1, 1, 1, ..., 1) pattern
        docs.push_to_document("diff", 0u8);
        for _ in 0..99 {
            docs.push_to_document("diff", 1u8);
        }

        let result: Vec<u8> = docs.iter_document(&"diff").unwrap().copied().collect();
        assert_eq!(result.len(), 100);
        assert_eq!(result[0], 0);
        assert!(result[1..].iter().all(|&x| x == 1));

        // Should be just 2 nodes: 0:1, 1:99
        let stats = docs.document_stats(&"diff").unwrap();
        assert_eq!(stats.document_nodes, 2);
    }

    #[test]
    fn test_documents_overall_stats() {
        let mut docs = SequiturDocumentsRle::new();

        docs.extend_document(1, "aaa".chars());
        docs.extend_document(2, "bbb".chars());
        docs.extend_document(3, "ccc".chars());

        let stats = docs.overall_stats();
        assert_eq!(stats.num_documents, 3);
        assert_eq!(stats.total_input_length, 9);
        // Each document has 1 node (a:3, b:3, c:3)
        assert!(stats.total_grammar_nodes >= 3);
    }

    #[test]
    fn test_documents_empty() {
        let docs = SequiturDocumentsRle::<char, u32>::new();
        assert_eq!(docs.num_documents(), 0);
        assert!(docs.iter_document(&1).is_none());
    }

    #[test]
    fn test_documents_ab_pattern() {
        let mut docs = SequiturDocumentsRle::new();

        // (ab)^k pattern in multiple documents
        for _ in 0..100 {
            docs.push_to_document(1, 'a');
            docs.push_to_document(1, 'b');
        }

        for _ in 0..50 {
            docs.push_to_document(2, 'a');
            docs.push_to_document(2, 'b');
        }

        let result1: String = docs.iter_document(&1).unwrap().collect();
        let result2: String = docs.iter_document(&2).unwrap().collect();

        assert_eq!(result1.len(), 200);
        assert_eq!(result2.len(), 100);

        // Verify the pattern
        assert!(result1.chars().enumerate().all(|(i, c)| {
            if i % 2 == 0 {
                c == 'a'
            } else {
                c == 'b'
            }
        }));
    }
}
