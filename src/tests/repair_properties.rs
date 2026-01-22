use crate::repair::Repair;
use proptest::prelude::*;

proptest! {
    /// Property 1: Roundtrip fidelity (uncompressed)
    /// The reconstructed sequence must exactly match the input before compression.
    #[test]
    fn prop_roundtrip_uncompressed(input: Vec<u8>) {
        let mut repair = Repair::new();
        repair.extend(input.clone());

        // Without compression
        let reconstructed: Vec<u8> = repair.iter().copied().collect();
        prop_assert_eq!(reconstructed, input);
    }

    /// Property 2: Roundtrip fidelity (compressed)
    /// The reconstructed sequence must exactly match the input after compression.
    #[test]
    fn prop_roundtrip_compressed(input: Vec<u8>) {
        let mut repair = Repair::new();
        repair.extend(input.clone());
        repair.compress();

        let reconstructed: Vec<u8> = repair.iter().copied().collect();
        prop_assert_eq!(reconstructed, input);
    }

    /// Property 3: Length preservation (uncompressed)
    /// The iterator must yield exactly as many items as were added.
    #[test]
    fn prop_length_preserved_uncompressed(input: Vec<u8>) {
        let mut repair = Repair::new();
        repair.extend(input.clone());

        let count = repair.iter().count();
        prop_assert_eq!(count, input.len());
        prop_assert_eq!(repair.len(), input.len());
    }

    /// Property 4: Length preservation (compressed)
    /// The iterator must yield exactly as many items as were added after compression.
    #[test]
    fn prop_length_preserved_compressed(input: Vec<u8>) {
        let mut repair = Repair::new();
        repair.extend(input.clone());
        repair.compress();

        let count = repair.iter().count();
        prop_assert_eq!(count, input.len());
        prop_assert_eq!(repair.len(), input.len());
    }

    /// Property 5: Compression is idempotent
    /// Calling compress() multiple times should have the same effect as calling once.
    #[test]
    fn prop_compress_idempotent(input: Vec<u8>) {
        let mut repair1 = Repair::new();
        repair1.extend(input.clone());
        repair1.compress();

        let mut repair2 = Repair::new();
        repair2.extend(input.clone());
        repair2.compress();
        repair2.compress(); // Compress again
        repair2.compress(); // And again

        let result1: Vec<u8> = repair1.iter().copied().collect();
        let result2: Vec<u8> = repair2.iter().copied().collect();

        prop_assert_eq!(result1, result2);
        prop_assert_eq!(repair1.rules().len(), repair2.rules().len());
    }

    /// Property 6: Grammar size is bounded
    /// The grammar should not be larger than the input (in terms of symbols).
    #[test]
    fn prop_grammar_size_bounded(input: Vec<u8>) {
        let mut repair = Repair::new();
        repair.extend(input.clone());
        repair.compress();

        let stats = repair.stats();
        // Grammar symbols should be <= input length (compression or no change)
        // This is a loose bound; in practice it should often be smaller
        prop_assert!(
            stats.grammar_symbols <= stats.input_length + stats.num_rules * 2,
            "Grammar too large: {} symbols for {} input with {} rules",
            stats.grammar_symbols,
            stats.input_length,
            stats.num_rules
        );
    }

    /// Property 7: Statistics are consistent
    /// Stats should be internally consistent.
    #[test]
    fn prop_stats_consistent(input: Vec<u8>) {
        let mut repair = Repair::new();
        repair.extend(input.clone());
        repair.compress();

        let stats = repair.stats();
        prop_assert_eq!(stats.input_length, input.len());
        prop_assert!(stats.compressed);
        prop_assert!(stats.num_rules >= 1, "Should have at least Rule 0");
    }

    /// Property 8: Non-empty rules
    /// After compression, rules should not be empty.
    #[test]
    fn prop_nonempty_rules(input: Vec<u8>) {
        let mut repair = Repair::new();
        repair.extend(input);
        repair.compress();

        for (&rule_id, &head_key) in repair.rules() {
            if rule_id != 0 || repair.len() > 0 {
                let first = repair.symbols[head_key].next.expect("Rule should have content");
                // Check that the first symbol after head is not immediately the tail
                // (unless this is Rule 0 with no content)
                if rule_id != 0 {
                    let is_tail = matches!(
                        repair.symbols[first].symbol,
                        crate::symbol::Symbol::RuleTail
                    );
                    prop_assert!(
                        !is_tail,
                        "Rule {} is empty",
                        rule_id
                    );
                }
            }
        }
    }

    /// Property 9: Small inputs don't create unnecessary rules
    #[test]
    fn prop_small_input_reasonable_rules(input in prop::collection::vec(any::<u8>(), 0..10)) {
        let mut repair = Repair::new();
        repair.extend(input.clone());
        repair.compress();

        let rule_count = repair.rules().len();
        // For inputs < 10, we shouldn't have too many rules
        prop_assert!(
            rule_count <= input.len() + 1,
            "Small input created {} rules for {} symbols",
            rule_count,
            input.len()
        );
    }

    /// Property 10: Repeated patterns create rules
    /// Input with repeated patterns should create rules.
    #[test]
    fn prop_repeated_patterns_create_rules(pattern in prop::collection::vec(any::<u8>(), 2..5), reps in 2..10usize) {
        let mut repair = Repair::new();
        for _ in 0..reps {
            repair.extend(pattern.clone());
        }
        repair.compress();

        // With repetitions, we should create at least one non-root rule
        let stats = repair.stats();
        if pattern.len() >= 2 && reps >= 2 {
            prop_assert!(
                stats.num_rules >= 2,
                "Repeated pattern should create rules: {} reps of {:?} but only {} rules",
                reps,
                pattern,
                stats.num_rules
            );
        }
    }
}

/// Bolero fuzz test: No panics on arbitrary input
#[cfg(test)]
#[test]
fn fuzz_repair_no_panic() {
    bolero::check!().with_type::<Vec<u8>>().for_each(|input| {
        let mut repair = Repair::new();
        repair.extend(input.iter().copied());

        // Basic operations shouldn't panic
        let _ = repair.len();
        let _ = repair.is_empty();
        let _ = repair.is_compressed();
        let _count = repair.iter().count();

        // Compression shouldn't panic
        repair.compress();

        let _ = repair.stats();
        let _count = repair.iter().count();

        // Roundtrip should work
        let reconstructed: Vec<u8> = repair.iter().copied().collect();
        assert_eq!(reconstructed.len(), input.len());
        assert_eq!(reconstructed, *input);
    });
}

/// Bolero fuzz test: Roundtrip correctness
#[cfg(test)]
#[test]
fn fuzz_repair_roundtrip() {
    bolero::check!().with_type::<Vec<u8>>().for_each(|input| {
        let mut repair = Repair::new();
        repair.extend(input.iter().copied());
        repair.compress();

        let reconstructed: Vec<u8> = repair.iter().copied().collect();
        assert_eq!(
            reconstructed,
            *input,
            "Roundtrip failed for input of length {}",
            input.len()
        );
    });
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_simple_repetition() {
        let mut repair = Repair::new();
        repair.extend(vec!['a', 'b', 'a', 'b']);
        repair.compress();

        // Should create a rule for "ab"
        assert!(
            repair.rules().len() >= 2,
            "Should have created at least one rule"
        );

        // Verify roundtrip
        let result: Vec<char> = repair.iter().copied().collect();
        assert_eq!(result, vec!['a', 'b', 'a', 'b']);
    }

    #[test]
    fn test_nested_rules() {
        let mut repair = Repair::new();
        repair.extend("abcabcabcabc".chars());
        repair.compress();

        // Should have multiple rules
        assert!(repair.rules().len() > 2, "Should have created nested rules");

        // Verify roundtrip
        let result: String = repair.iter().collect();
        assert_eq!(result, "abcabcabcabc");
    }

    #[test]
    fn test_long_repetition() {
        let mut repair = Repair::new();
        let pattern = "hello";
        for _ in 0..100 {
            repair.extend(pattern.chars());
        }
        repair.compress();

        // Verify roundtrip
        let result: String = repair.iter().collect();
        assert_eq!(result.len(), 500);
        assert_eq!(&result[..5], "hello");

        // Should compress significantly
        let stats = repair.stats();
        assert!(
            stats.grammar_symbols < stats.input_length,
            "Should compress: {} symbols vs {} input",
            stats.grammar_symbols,
            stats.input_length
        );
    }

    #[test]
    fn test_no_repetition() {
        let mut repair = Repair::new();
        repair.extend("abcdefgh".chars());
        repair.compress();

        // No pairs repeat, so only Rule 0 should exist
        assert_eq!(
            repair.rules().len(),
            1,
            "No rules needed without repetition"
        );

        // Verify roundtrip
        let result: String = repair.iter().collect();
        assert_eq!(result, "abcdefgh");
    }

    #[test]
    fn test_all_same() {
        let mut repair = Repair::new();
        repair.extend(vec!['a'; 100]);
        repair.compress();

        // Should create rules for repeated 'a's
        assert!(repair.rules().len() > 1, "Should create rules for repeats");

        // Verify roundtrip
        let result: Vec<char> = repair.iter().copied().collect();
        assert_eq!(result.len(), 100);
        assert!(result.iter().all(|&c| c == 'a'));
    }

    #[test]
    fn test_empty_input() {
        let mut repair = Repair::<u8>::new();
        repair.compress();

        assert!(repair.is_empty());
        assert!(repair.is_compressed());
        assert_eq!(repair.iter().count(), 0);
    }

    #[test]
    fn test_single_element() {
        let mut repair = Repair::new();
        repair.push(42u8);
        repair.compress();

        let result: Vec<u8> = repair.iter().copied().collect();
        assert_eq!(result, vec![42]);
    }

    #[test]
    fn test_two_elements_no_repeat() {
        let mut repair = Repair::new();
        repair.extend(vec!['a', 'b']);
        repair.compress();

        let result: Vec<char> = repair.iter().copied().collect();
        assert_eq!(result, vec!['a', 'b']);
        // Only Rule 0, no additional rules since pair only occurs once
        assert_eq!(repair.rules().len(), 1);
    }

    #[test]
    fn test_binary_data() {
        let mut repair = Repair::new();
        let data: Vec<u8> = (0..=255).cycle().take(1000).collect();
        repair.extend(data.clone());
        repair.compress();

        let result: Vec<u8> = repair.iter().copied().collect();
        assert_eq!(result, data);
    }

    #[test]
    fn test_compression_ratio() {
        let mut repair = Repair::new();
        // Highly compressible data
        let pattern = vec![1u8, 2, 3, 4, 5];
        for _ in 0..100 {
            repair.extend(pattern.clone());
        }
        repair.compress();

        let stats = repair.stats();
        assert!(
            stats.compression_ratio() < 50.0,
            "Should compress well: {}%",
            stats.compression_ratio()
        );
    }
}
