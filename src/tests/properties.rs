use crate::sequitur::Sequitur;
use crate::symbol::Symbol;
use proptest::prelude::*;
use std::collections::HashSet;

/// Extracts all digrams from the entire grammar for testing digram uniqueness.
fn extract_all_digrams<T: Clone + Eq + std::hash::Hash>(seq: &Sequitur<T>) -> Vec<(usize, usize)> {
    let mut digrams = Vec::new();

    // Iterate through all rules
    for (&rule_id, &head_key) in seq.rules() {
        let mut current = seq.symbols[head_key].next;

        while let Some(key) = current {
            if let Some(next_key) = seq.symbols[key].next {
                // Skip RuleHead -> X and X -> RuleTail digrams
                let is_head = matches!(seq.symbols[key].symbol, Symbol::RuleHead { .. });
                let is_tail = matches!(seq.symbols[next_key].symbol, Symbol::RuleTail);

                if !is_head && !is_tail {
                    // Create a representation of the digram
                    let first_id = get_symbol_id(&seq.symbols[key].symbol);
                    let second_id = get_symbol_id(&seq.symbols[next_key].symbol);
                    digrams.push((first_id, second_id));
                }

                current = Some(next_key);
            } else {
                break;
            }
        }
    }

    digrams
}

/// Gets a unique identifier for a symbol for digram comparison.
fn get_symbol_id<T>(symbol: &Symbol<T>) -> usize {
    match symbol {
        Symbol::Value(_) => 0,  // Simplified: all values get same ID for this test
        Symbol::RuleRef { rule_id } => (*rule_id as usize) + 1000,
        Symbol::RuleHead { rule_id, .. } => (*rule_id as usize) + 2000,
        Symbol::RuleTail => 3000,
    }
}

/// Gets the reference count for a rule.
fn get_rule_count<T>(seq: &Sequitur<T>, head_key: slotmap::DefaultKey) -> u32 {
    if let Symbol::RuleHead { count, .. } = seq.symbols[head_key].symbol {
        count
    } else {
        0
    }
}

proptest! {
    /// Property 1: Roundtrip fidelity
    /// The reconstructed sequence must exactly match the input.
    #[test]
    fn prop_roundtrip(input: Vec<u8>) {
        let mut seq = Sequitur::new();
        seq.extend(input.clone());

        let reconstructed: Vec<u8> = seq.iter().copied().collect();
        prop_assert_eq!(reconstructed, input);
    }

    /// Property 2: Length preservation
    /// The iterator must yield exactly as many items as were added.
    #[test]
    fn prop_length_preserved(input: Vec<u8>) {
        let mut seq = Sequitur::new();
        seq.extend(input.clone());

        let count = seq.iter().count();
        prop_assert_eq!(count, input.len());
        prop_assert_eq!(seq.len(), input.len());
    }

    /// Property 3: Rule utility constraint
    /// Every rule (except Rule 0) must be used at least twice.
    #[test]
    fn prop_rule_utility(input: Vec<u8>) {
        let mut seq = Sequitur::new();
        seq.extend(input);

        for (&rule_id, &head_key) in seq.rules() {
            if rule_id != 0 {  // Skip root rule
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

    /// Property 4: Non-empty rules
    /// Rules should not be empty (should have at least one symbol between head and tail).
    #[test]
    fn prop_nonempty_rules(input: Vec<u8>) {
        let mut seq = Sequitur::new();
        seq.extend(input);

        for (&rule_id, &head_key) in seq.rules() {
            if rule_id != 0 {
                let first = seq.symbols[head_key].next.expect("Rule should have content");
                prop_assert!(
                    !matches!(seq.symbols[first].symbol, Symbol::RuleTail),
                    "Rule {} is empty",
                    rule_id
                );
            }
        }
    }

    /// Property 5: Small inputs don't create unnecessary rules
    /// Very short inputs shouldn't create complex grammars.
    #[test]
    fn prop_small_input_simple_grammar(input in prop::collection::vec(any::<u8>(), 0..10)) {
        let mut seq = Sequitur::new();
        seq.extend(input.clone());

        // For inputs < 10, we shouldn't have too many rules
        // This is a heuristic, not a hard constraint
        let rule_count = seq.rules().len();
        prop_assert!(
            rule_count <= input.len() + 1,
            "Small input created {} rules for {} symbols",
            rule_count,
            input.len()
        );
    }

    /// Property 6: Incremental vs batch equivalence
    /// Adding items one-by-one should produce the same result as extend.
    #[test]
    fn prop_incremental_equivalence(input: Vec<u8>) {
        let mut seq1 = Sequitur::new();
        seq1.extend(input.clone());
        let result1: Vec<u8> = seq1.iter().copied().collect();

        let mut seq2 = Sequitur::new();
        for &item in &input {
            seq2.push(item);
        }
        let result2: Vec<u8> = seq2.iter().copied().collect();

        prop_assert_eq!(result1, result2);
    }
}

/// Bolero fuzz test: No panics on arbitrary input
#[cfg(test)]
#[test]
fn fuzz_no_panic() {
    bolero::check!().with_type::<Vec<u8>>().for_each(|input| {
        let mut seq = Sequitur::new();
        seq.extend(input.iter().copied());

        // Verify basic operations don't panic
        let _ = seq.len();
        let _ = seq.is_empty();
        let count = seq.iter().count();

        // Roundtrip should work
        let reconstructed: Vec<u8> = seq.iter().copied().collect();
        assert_eq!(reconstructed.len(), input.len());
        assert_eq!(reconstructed, *input);
    });
}

/// Bolero fuzz test: Rule utility is always maintained
#[cfg(test)]
#[test]
fn fuzz_rule_utility() {
    bolero::check!()
        .with_type::<Vec<u8>>()
        .for_each(|input| {
            let mut seq = Sequitur::new();
            seq.extend(input.iter().copied());

            // Check rule utility constraint
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
    fn test_simple_repetition() {
        let mut seq = Sequitur::new();
        seq.extend(vec!['a', 'b', 'a', 'b']);

        // Should create a rule for "ab"
        assert!(seq.rules().len() >= 2, "Should have created at least one rule");

        // Verify roundtrip
        let result: Vec<char> = seq.iter().copied().collect();
        assert_eq!(result, vec!['a', 'b', 'a', 'b']);
    }

    #[test]
    fn test_nested_rules() {
        let mut seq = Sequitur::new();
        // Pattern that should create nested rules: abcabcabcabc
        seq.extend("abcabcabcabc".chars());

        // Should have multiple rules
        assert!(seq.rules().len() > 2, "Should have created nested rules");

        // Verify roundtrip
        let result: String = seq.iter().collect();
        assert_eq!(result, "abcabcabcabc");
    }

    #[test]
    fn test_all_rules_used_twice() {
        let mut seq = Sequitur::new();
        seq.extend("abracadabra".chars());

        // Verify rule utility for all rules
        for (&rule_id, &head_key) in seq.rules() {
            if rule_id != 0 {
                let count = get_rule_count(&seq, head_key);
                assert!(
                    count >= 2,
                    "Rule {} only used {} times",
                    rule_id,
                    count
                );
            }
        }
    }
}
