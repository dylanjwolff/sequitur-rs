use crate::grammar::{is_sequence_start, GrammarFields, GrammarOps};
use crate::id_gen::IdGenerator;
use crate::symbol::{Symbol, SymbolHash, SymbolNode};
use ahash::AHashMap as HashMap;
use slotmap::{DefaultKey, SlotMap};
use std::hash::Hash;

/// Main Sequitur data structure.
///
/// Maintains a context-free grammar that incrementally compresses input sequences
/// while enforcing two constraints:
/// 1. Digram Uniqueness: No digram appears more than once
/// 2. Rule Utility: Every rule is used at least twice
pub struct Sequitur<T> {
    /// Storage for all symbols using generational indices
    pub(crate) symbols: SlotMap<DefaultKey, SymbolNode<T>>,

    /// Maps digrams to their first occurrence
    pub(crate) digram_index: HashMap<(SymbolHash, SymbolHash), DefaultKey>,

    /// Maps rule IDs to their RuleHead keys
    pub(crate) rule_index: HashMap<u32, DefaultKey>,

    /// ID generator with reuse
    pub(crate) id_gen: IdGenerator,

    /// Key to the RuleTail of Rule 0 (main sequence)
    pub(crate) sequence_end: DefaultKey,

    /// Number of values added
    length: usize,
}

// Implement GrammarOps trait for zero-cost code sharing
impl<T> GrammarOps<T> for Sequitur<T> {
    #[inline(always)]
    fn fields(&mut self) -> GrammarFields<'_, T> {
        GrammarFields {
            symbols: &mut self.symbols,
            digram_index: &mut self.digram_index,
            rule_index: &mut self.rule_index,
            id_gen: &mut self.id_gen,
        }
    }
}

impl<T: Hash + Eq + Clone> Sequitur<T> {
    /// Creates a new empty Sequitur instance.
    ///
    /// Initializes with Rule 0 (the main sequence).
    pub fn new() -> Self {
        let mut symbols = SlotMap::new();
        let mut id_gen = IdGenerator::new();
        let mut rule_index = HashMap::default();

        // Create Rule 0 (main sequence)
        let rule_id = id_gen.get();
        assert_eq!(rule_id, 0, "First rule should have ID 0");

        // Create RuleTail first (will be updated with RuleHead reference)
        let tail_key = symbols.insert(SymbolNode::new(Symbol::RuleTail));

        // Create RuleHead with reference to tail
        let head_key = symbols.insert(SymbolNode::new(Symbol::RuleHead {
            rule_id,
            count: 0,
            tail: tail_key,
        }));

        // Link them together
        symbols[head_key].next = Some(tail_key);
        symbols[tail_key].prev = Some(head_key);

        rule_index.insert(rule_id, head_key);

        Self {
            symbols,
            digram_index: HashMap::default(),
            rule_index,
            id_gen,
            sequence_end: tail_key,
            length: 0,
        }
    }

    /// Adds a value to the sequence.
    ///
    /// This triggers the Sequitur algorithm to maintain the grammar constraints.
    pub fn push(&mut self, value: T) {
        // Create new Value symbol
        let new_key = self.symbols.insert(SymbolNode::new(Symbol::Value(value)));

        // Insert before sequence_end (RuleTail of Rule 0)
        let tail_key = self.sequence_end;
        let prev_key = self.symbols[tail_key].prev;

        // Link new symbol into the list
        self.symbols[new_key].next = Some(tail_key);
        self.symbols[new_key].prev = prev_key;
        self.symbols[tail_key].prev = Some(new_key);

        if let Some(prev) = prev_key {
            self.symbols[prev].next = Some(new_key);
        }

        self.length += 1;

        // If not the first symbol, check for digram
        if self.length > 1 {
            if let Some(prev) = prev_key {
                // Skip if prev is RuleHead (digrams don't start with RuleHead)
                if !is_sequence_start(&self.symbols[prev].symbol) {
                    self.fields().link_made(prev);
                }
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
        &self.rule_index
    }

    /// Returns compression statistics.
    pub fn stats(&self) -> CompressionStats {
        let mut total_symbols = 0;

        for &head_key in self.rule_index.values() {
            // Count symbols between RuleHead and RuleTail
            let mut current = self.symbols[head_key].next;
            while let Some(key) = current {
                if let Some(next) = self.symbols[key].next {
                    total_symbols += 1;
                    current = Some(next);
                } else {
                    break;
                }
            }
        }

        CompressionStats {
            input_length: self.length,
            grammar_symbols: total_symbols,
            num_rules: self.rule_index.len(),
        }
    }
}

/// Statistics about the compression.
#[derive(Debug, Clone, Copy)]
pub struct CompressionStats {
    /// Number of input symbols added
    pub input_length: usize,
    /// Total symbols in the grammar
    pub grammar_symbols: usize,
    /// Number of rules created
    pub num_rules: usize,
}

impl CompressionStats {
    /// Returns the compression ratio as a percentage.
    pub fn compression_ratio(&self) -> f64 {
        if self.input_length == 0 {
            0.0
        } else {
            (self.grammar_symbols as f64 / self.input_length as f64) * 100.0
        }
    }
}

impl<T: Hash + Eq + Clone> Default for Sequitur<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let seq = Sequitur::<char>::new();
        assert_eq!(seq.len(), 0);
        assert!(seq.is_empty());
        assert_eq!(seq.rules().len(), 1); // Rule 0 exists
    }

    #[test]
    fn test_push_single() {
        let mut seq = Sequitur::new();
        seq.push('a');
        assert_eq!(seq.len(), 1);
        assert!(!seq.is_empty());
    }

    #[test]
    fn test_push_multiple() {
        let mut seq = Sequitur::new();
        seq.push('a');
        seq.push('b');
        seq.push('c');
        assert_eq!(seq.len(), 3);
    }

    #[test]
    fn test_abab_pattern() {
        let mut seq = Sequitur::new();
        seq.extend(vec!['a', 'b', 'a', 'b']);
        let result: Vec<_> = seq.iter().copied().collect();
        assert_eq!(result, vec!['a', 'b', 'a', 'b']);
    }

    #[test]
    fn test_extend() {
        let mut seq = Sequitur::new();
        seq.extend(vec!['a', 'b', 'c']);
        assert_eq!(seq.len(), 3);
    }

    #[test]
    fn test_rule_0_structure() {
        let seq = Sequitur::<u8>::new();
        let rule_0_head = *seq.rules().get(&0).expect("Rule 0 should exist");

        // Verify structure: RuleHead -> RuleTail
        let head_node = &seq.symbols[rule_0_head];
        assert!(matches!(
            head_node.symbol,
            Symbol::RuleHead { rule_id: 0, .. }
        ));

        let tail_key = head_node.next.expect("Head should have next");
        let tail_node = &seq.symbols[tail_key];
        assert!(matches!(tail_node.symbol, Symbol::RuleTail));
        assert_eq!(tail_key, seq.sequence_end);
    }
}
