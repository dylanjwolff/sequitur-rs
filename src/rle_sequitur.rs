use crate::rle_grammar::RleGrammar;
use crate::rle_symbol::RleSymbolNode;
use crate::symbol::Symbol;
use ahash::AHashMap as HashMap;
use slotmap::DefaultKey;
use std::hash::Hash;

/// RLE-Sequitur data structure.
///
/// Run-Length Encoding Sequitur (RLESe) extends the standard Sequitur algorithm
/// with run-length encoding to efficiently handle repeated symbols.
///
/// Key differences from standard Sequitur:
/// 1. Each node stores a run count representing consecutive occurrences
/// 2. Digrams are "similar" if they have the same symbols (ignoring runs)
/// 3. No contiguous repeated symbols - they are encoded in the run count
///
/// This is particularly efficient for patterns like (ab)^k which standard
/// Sequitur represents with O(log k) rules, while RLESe uses only 2 rules.
pub struct SequiturRle<T> {
    /// Core RLE grammar storage
    pub(crate) grammar: RleGrammar<T>,

    /// Key to the RuleTail of Rule 0 (main sequence)
    pub(crate) sequence_end: DefaultKey,

    /// Number of values added (counting run lengths)
    length: usize,
}

impl<T: Hash + Eq + Clone> SequiturRle<T> {
    /// Creates a new empty RLE-Sequitur instance.
    pub fn new() -> Self {
        let mut grammar = RleGrammar::new();

        // Create Rule 0 (main sequence)
        let rule_id = grammar.id_gen.get();
        assert_eq!(rule_id, 0, "First rule should have ID 0");

        let tail_key = grammar.symbols.insert(RleSymbolNode::new(Symbol::RuleTail));

        let head_key = grammar.symbols.insert(RleSymbolNode::new(Symbol::RuleHead {
            rule_id,
            count: 0,
            tail: tail_key,
        }));

        grammar.symbols[head_key].next = Some(tail_key);
        grammar.symbols[tail_key].prev = Some(head_key);

        grammar.rule_index.insert(rule_id, head_key);

        Self {
            grammar,
            sequence_end: tail_key,
            length: 0,
        }
    }

    /// Adds a value to the sequence.
    ///
    /// If the previous symbol is the same value, its run count is incremented
    /// instead of creating a new node.
    pub fn push(&mut self, value: T) {
        let tail_key = self.sequence_end;
        let prev_key = self.grammar.symbols[tail_key].prev;

        // Check if we can extend the previous symbol's run
        if let Some(prev) = prev_key {
            if let Symbol::Value(ref prev_val) = self.grammar.symbols[prev].symbol {
                if prev_val == &value {
                    // Same value - just increment the run count
                    self.grammar.symbols[prev].run += 1;
                    self.length += 1;
                    return;
                }
            }
        }

        // Different value or first symbol - create new node
        let new_key = self
            .grammar
            .symbols
            .insert(RleSymbolNode::new(Symbol::Value(value)));

        self.grammar.symbols[new_key].next = Some(tail_key);
        self.grammar.symbols[new_key].prev = prev_key;
        self.grammar.symbols[tail_key].prev = Some(new_key);

        if let Some(prev) = prev_key {
            self.grammar.symbols[prev].next = Some(new_key);
        }

        self.length += 1;

        // Check for digram if not the first symbol
        if let Some(prev) = prev_key {
            if !matches!(self.grammar.symbols[prev].symbol, Symbol::RuleHead { .. }) {
                self.grammar.link_made(prev);
            }
        }
    }

    /// Extends the sequence with multiple values.
    pub fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            self.push(value);
        }
    }

    /// Returns the number of values added to the sequence.
    pub fn len(&self) -> usize {
        self.length
    }

    /// Returns true if no values have been added.
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Returns a reference to the rule index.
    pub fn rules(&self) -> &HashMap<u32, DefaultKey> {
        &self.grammar.rule_index
    }

    /// Returns compression statistics.
    pub fn stats(&self) -> RleCompressionStats {
        let mut total_nodes = 0;
        let mut total_run_sum = 0u64;

        for &head_key in self.grammar.rule_index.values() {
            let mut current = self.grammar.symbols[head_key].next;
            while let Some(key) = current {
                if let Some(next) = self.grammar.symbols[key].next {
                    total_nodes += 1;
                    total_run_sum += self.grammar.symbols[key].run as u64;
                    current = Some(next);
                } else {
                    break;
                }
            }
        }

        RleCompressionStats {
            input_length: self.length,
            grammar_nodes: total_nodes,
            grammar_symbols_expanded: total_run_sum,
            num_rules: self.grammar.rule_index.len(),
        }
    }

    /// Signals that the current run has finished and digram checking should occur.
    ///
    /// Call this when you know a run of repeated symbols has ended and you want
    /// to trigger grammar restructuring. This is optional - the grammar will
    /// still be correct without calling this, but it may delay some optimizations.
    pub fn end_run(&mut self) {
        // This is called to signal that the current run is finished
        // and we should check for digram patterns
        let tail_key = self.sequence_end;
        if let Some(prev) = self.grammar.symbols[tail_key].prev {
            if !matches!(self.grammar.symbols[prev].symbol, Symbol::RuleHead { .. }) {
                if let Some(prev_prev) = self.grammar.symbols[prev].prev {
                    if !matches!(
                        self.grammar.symbols[prev_prev].symbol,
                        Symbol::RuleHead { .. }
                    ) {
                        // Re-check the last digram
                        self.grammar.link_made(prev_prev);
                    }
                }
            }
        }
    }
}

/// Statistics about RLE compression.
#[derive(Debug, Clone, Copy)]
pub struct RleCompressionStats {
    /// Number of input symbols added
    pub input_length: usize,
    /// Total nodes in the grammar (not counting runs)
    pub grammar_nodes: usize,
    /// Total symbols when runs are expanded
    pub grammar_symbols_expanded: u64,
    /// Number of rules created
    pub num_rules: usize,
}

impl RleCompressionStats {
    /// Returns the compression ratio as a percentage (nodes vs input).
    pub fn compression_ratio(&self) -> f64 {
        if self.input_length == 0 {
            0.0
        } else {
            (self.grammar_nodes as f64 / self.input_length as f64) * 100.0
        }
    }

    /// Returns the compression ratio counting expanded runs.
    pub fn expanded_compression_ratio(&self) -> f64 {
        if self.input_length == 0 {
            0.0
        } else {
            (self.grammar_symbols_expanded as f64 / self.input_length as f64) * 100.0
        }
    }
}

impl<T: Hash + Eq + Clone> Default for SequiturRle<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let seq = SequiturRle::<char>::new();
        assert_eq!(seq.len(), 0);
        assert!(seq.is_empty());
        assert_eq!(seq.rules().len(), 1);
    }

    #[test]
    fn test_push_single() {
        let mut seq = SequiturRle::new();
        seq.push('a');
        assert_eq!(seq.len(), 1);
        assert!(!seq.is_empty());
    }

    #[test]
    fn test_run_length_encoding() {
        let mut seq = SequiturRle::new();

        // Push repeated values
        seq.push('a');
        seq.push('a');
        seq.push('a');

        assert_eq!(seq.len(), 3);

        // Should have only one node with run=3
        let rule_0_head = *seq.rules().get(&0).unwrap();
        let first = seq.grammar.symbols[rule_0_head].next.unwrap();

        if let Symbol::Value(v) = &seq.grammar.symbols[first].symbol {
            assert_eq!(*v, 'a');
            assert_eq!(seq.grammar.symbols[first].run, 3);
        } else {
            panic!("Expected Value symbol");
        }
    }

    #[test]
    fn test_alternating_pattern() {
        let mut seq = SequiturRle::new();

        // Push alternating pattern
        seq.extend(vec!['a', 'b', 'a', 'b']);

        // Verify reconstruction
        let result: Vec<_> = seq.iter().copied().collect();
        assert_eq!(result, vec!['a', 'b', 'a', 'b']);
    }

    #[test]
    fn test_repeated_pattern() {
        let mut seq = SequiturRle::new();

        // This is the example from RLE.md: (ab)^k should use only 2 rules
        for _ in 0..4 {
            seq.push('a');
            seq.push('b');
        }

        let result: Vec<_> = seq.iter().copied().collect();
        assert_eq!(result, vec!['a', 'b', 'a', 'b', 'a', 'b', 'a', 'b']);

        // Should have efficient representation
        let stats = seq.stats();
        println!(
            "Rules: {}, Nodes: {}, Input: {}",
            stats.num_rules, stats.grammar_nodes, stats.input_length
        );
    }

    #[test]
    fn test_long_run() {
        let mut seq = SequiturRle::new();

        // Push a long run
        for _ in 0..100 {
            seq.push('x');
        }

        assert_eq!(seq.len(), 100);

        // Should be just one node with run=100
        let stats = seq.stats();
        assert_eq!(stats.grammar_nodes, 1);
    }
}
