//! RePair grammar compression algorithm.
//!
//! RePair is a greedy grammar-based compression algorithm that repeatedly
//! replaces the most frequent pair of adjacent symbols with a new rule.
//!
//! Unlike Sequitur which processes input incrementally, RePair is a batch
//! algorithm that operates on the complete input sequence.
//!
//! # Example
//!
//! ```
//! use sequitur_rs::Repair;
//!
//! let mut repair = Repair::new();
//! repair.extend("abcabcabcabc".chars());
//! repair.compress();
//!
//! // Reconstructs the original sequence
//! let reconstructed: String = repair.iter().collect();
//! assert_eq!(reconstructed, "abcabcabcabc");
//! ```

use crate::id_gen::IdGenerator;
use crate::symbol::{Symbol, SymbolNode};
use ahash::AHashMap as HashMap;
use slotmap::{DefaultKey, SlotMap};
use std::collections::BinaryHeap;
use std::hash::Hash;

/// A pair of adjacent symbols with their frequency count.
#[derive(Debug, Clone, Eq, PartialEq)]
struct PairRecord {
    /// Frequency of this pair
    frequency: u32,
    /// Hash of the first symbol
    first_symbol_id: PairSymbolId,
    /// Hash of the second symbol
    second_symbol_id: PairSymbolId,
}

impl Ord for PairRecord {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher frequency = higher priority
        self.frequency.cmp(&other.frequency)
    }
}

impl PartialOrd for PairRecord {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Identifier for symbols in the pair index (either terminal value index or rule ID).
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum PairSymbolId {
    /// Terminal symbol with index into values_dedup
    Terminal(u32),
    /// Non-terminal (rule reference)
    RuleRef(u32),
}

/// Main RePair data structure.
///
/// Compresses input sequences using the RePair algorithm, which repeatedly
/// replaces the most frequent pair with a new rule.
pub struct Repair<T> {
    /// Symbol storage (doubly-linked list nodes)
    pub(crate) symbols: SlotMap<DefaultKey, SymbolNode<T>>,

    /// Maps rule IDs to their RuleHead keys
    pub(crate) rule_index: HashMap<u32, DefaultKey>,

    /// ID generator for rules
    id_gen: IdGenerator,

    /// Key to the RuleTail of Rule 0 (main sequence)
    pub(crate) sequence_end: DefaultKey,

    /// Number of values added
    length: usize,

    /// Deduplicated terminal values (for efficient pair hashing)
    values_dedup: Vec<T>,

    /// Maps terminal values to their index in values_dedup
    value_to_index: HashMap<T, u32>,

    /// Whether compression has been performed
    compressed: bool,
}

impl<T: Hash + Eq + Clone> Repair<T> {
    /// Creates a new empty Repair instance.
    pub fn new() -> Self {
        let mut symbols = SlotMap::new();
        let mut id_gen = IdGenerator::new();

        // Create Rule 0 (main sequence)
        let rule_id = id_gen.get();
        debug_assert_eq!(rule_id, 0, "First rule should have ID 0");

        // Create RuleTail first
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

        let mut rule_index = HashMap::default();
        rule_index.insert(rule_id, head_key);

        Self {
            symbols,
            rule_index,
            id_gen,
            sequence_end: tail_key,
            length: 0,
            values_dedup: Vec::new(),
            value_to_index: HashMap::default(),
            compressed: false,
        }
    }

    /// Gets or creates an index for a terminal value.
    fn get_or_create_value_index(&mut self, value: &T) -> u32 {
        if let Some(&index) = self.value_to_index.get(value) {
            index
        } else {
            let index = self.values_dedup.len() as u32;
            self.values_dedup.push(value.clone());
            self.value_to_index.insert(value.clone(), index);
            index
        }
    }

    /// Gets the PairSymbolId for a symbol at a given key.
    fn get_symbol_id(&self, key: DefaultKey) -> Option<PairSymbolId> {
        match &self.symbols[key].symbol {
            Symbol::Value(v) => {
                let index = *self.value_to_index.get(v)?;
                Some(PairSymbolId::Terminal(index))
            }
            Symbol::RuleRef { rule_id } => Some(PairSymbolId::RuleRef(*rule_id)),
            _ => None,
        }
    }

    /// Adds a value to the sequence.
    ///
    /// Must be called before `compress()`.
    pub fn push(&mut self, value: T) {
        assert!(
            !self.compressed,
            "Cannot add values after compression has been performed"
        );

        // Index the value for efficient pair tracking
        self.get_or_create_value_index(&value);

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
    }

    /// Extends the sequence with multiple values.
    ///
    /// Must be called before `compress()`.
    pub fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            self.push(value);
        }
    }

    /// Performs RePair compression on the sequence.
    ///
    /// This replaces frequently occurring pairs with rules until no pair
    /// occurs more than once.
    pub fn compress(&mut self) {
        if self.compressed || self.length < 2 {
            self.compressed = true;
            return;
        }

        loop {
            // Count all pairs
            let pair_counts = self.count_pairs();

            // Find the most frequent pair
            let best_pair = pair_counts.into_iter().max();

            match best_pair {
                Some(record) if record.frequency >= 2 => {
                    // Replace all occurrences of this pair with a new rule
                    self.replace_pair(record.first_symbol_id, record.second_symbol_id);
                }
                _ => break, // No pair occurs twice or more
            }
        }

        self.compressed = true;
    }

    /// Counts all pairs in Rule 0 (main sequence only).
    fn count_pairs(&self) -> BinaryHeap<PairRecord> {
        let mut pair_counts: HashMap<(PairSymbolId, PairSymbolId), u32> = HashMap::default();

        // Only count pairs in Rule 0 (the main sequence)
        let head_key = *self.rule_index.get(&0).expect("Rule 0 should exist");
        let mut current = self.symbols[head_key].next;

        while let Some(key) = current {
            let next = self.symbols[key].next;

            if let Some(next_key) = next {
                // Skip if either symbol is a sentinel
                let is_first_sentinel = matches!(
                    self.symbols[key].symbol,
                    Symbol::RuleHead { .. } | Symbol::RuleTail
                );
                let is_second_sentinel = matches!(
                    self.symbols[next_key].symbol,
                    Symbol::RuleHead { .. } | Symbol::RuleTail
                );

                if !is_first_sentinel && !is_second_sentinel {
                    if let (Some(first_id), Some(second_id)) =
                        (self.get_symbol_id(key), self.get_symbol_id(next_key))
                    {
                        *pair_counts.entry((first_id, second_id)).or_insert(0) += 1;
                    }
                }
            }

            current = next;
        }

        // Convert to priority queue
        pair_counts
            .into_iter()
            .map(|((first, second), freq)| PairRecord {
                frequency: freq,
                first_symbol_id: first,
                second_symbol_id: second,
            })
            .collect()
    }

    /// Replaces all occurrences of a pair with a new rule.
    fn replace_pair(&mut self, first_id: PairSymbolId, second_id: PairSymbolId) {
        // Create a new rule
        let rule_id = self.id_gen.get();

        // Create RuleTail
        let tail_key = self.symbols.insert(SymbolNode::new(Symbol::RuleTail));

        // Create RuleHead
        let head_key = self.symbols.insert(SymbolNode::new(Symbol::RuleHead {
            rule_id,
            count: 0,
            tail: tail_key,
        }));

        // Create the rule body (the pair of symbols)
        let rule_first = self
            .symbols
            .insert(SymbolNode::new(self.id_to_symbol(first_id)));
        let rule_second = self
            .symbols
            .insert(SymbolNode::new(self.id_to_symbol(second_id)));

        // Link rule structure: head -> first -> second -> tail
        self.symbols[head_key].next = Some(rule_first);
        self.symbols[rule_first].prev = Some(head_key);
        self.symbols[rule_first].next = Some(rule_second);
        self.symbols[rule_second].prev = Some(rule_first);
        self.symbols[rule_second].next = Some(tail_key);
        self.symbols[tail_key].prev = Some(rule_second);

        self.rule_index.insert(rule_id, head_key);

        // Find and replace all occurrences of this pair
        let mut count = 0;
        let locations = self.find_pair_locations(first_id, second_id);

        for first_key in locations {
            // Verify the pair is still valid (may have been affected by previous replacement)
            if !self.symbols.contains_key(first_key) {
                continue;
            }

            let Some(second_key) = self.symbols[first_key].next else {
                continue;
            };

            if !self.symbols.contains_key(second_key) {
                continue;
            }

            // Verify the symbols still match
            let current_first_id = self.get_symbol_id(first_key);
            let current_second_id = self.get_symbol_id(second_key);

            if current_first_id != Some(first_id) || current_second_id != Some(second_id) {
                continue;
            }

            // Skip if second is a sentinel
            if matches!(
                self.symbols[second_key].symbol,
                Symbol::RuleTail | Symbol::RuleHead { .. }
            ) {
                continue;
            }

            // Replace the pair with a RuleRef
            let before = self.symbols[first_key].prev;
            let after = self.symbols[second_key].next;

            let rule_ref_key = self
                .symbols
                .insert(SymbolNode::new(Symbol::RuleRef { rule_id }));

            self.symbols[rule_ref_key].prev = before;
            self.symbols[rule_ref_key].next = after;

            if let Some(prev) = before {
                self.symbols[prev].next = Some(rule_ref_key);
            }
            if let Some(next) = after {
                self.symbols[next].prev = Some(rule_ref_key);
            }

            // Remove the old symbols
            self.symbols.remove(first_key);
            self.symbols.remove(second_key);

            count += 1;
        }

        // Update rule count
        if let Symbol::RuleHead {
            rule_id: rid, tail, ..
        } = self.symbols[head_key].symbol
        {
            self.symbols[head_key].symbol = Symbol::RuleHead {
                rule_id: rid,
                count,
                tail,
            };
        }
    }

    /// Converts a PairSymbolId back to a Symbol.
    fn id_to_symbol(&self, id: PairSymbolId) -> Symbol<T> {
        match id {
            PairSymbolId::Terminal(index) => {
                Symbol::Value(self.values_dedup[index as usize].clone())
            }
            PairSymbolId::RuleRef(rule_id) => Symbol::RuleRef { rule_id },
        }
    }

    /// Finds all locations where a pair occurs in Rule 0 (main sequence only).
    fn find_pair_locations(
        &self,
        first_id: PairSymbolId,
        second_id: PairSymbolId,
    ) -> Vec<DefaultKey> {
        let mut locations = Vec::new();

        // Only search in Rule 0 (the main sequence)
        let head_key = *self.rule_index.get(&0).expect("Rule 0 should exist");
        let mut current = self.symbols[head_key].next;

        while let Some(key) = current {
            let next = self.symbols[key].next;

            if let Some(next_key) = next {
                let is_first_sentinel = matches!(
                    self.symbols[key].symbol,
                    Symbol::RuleHead { .. } | Symbol::RuleTail
                );
                let is_second_sentinel = matches!(
                    self.symbols[next_key].symbol,
                    Symbol::RuleHead { .. } | Symbol::RuleTail
                );

                if !is_first_sentinel && !is_second_sentinel {
                    if let (Some(fid), Some(sid)) =
                        (self.get_symbol_id(key), self.get_symbol_id(next_key))
                    {
                        if fid == first_id && sid == second_id {
                            locations.push(key);
                        }
                    }
                }
            }

            current = next;
        }

        locations
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

    /// Returns whether compression has been performed.
    pub fn is_compressed(&self) -> bool {
        self.compressed
    }

    /// Returns compression statistics.
    pub fn stats(&self) -> RepairStats {
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

        RepairStats {
            input_length: self.length,
            grammar_symbols: total_symbols,
            num_rules: self.rule_index.len(),
            compressed: self.compressed,
        }
    }
}

impl<T: Hash + Eq + Clone> Default for Repair<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about RePair compression.
#[derive(Debug, Clone, Copy)]
pub struct RepairStats {
    /// Number of input symbols added
    pub input_length: usize,
    /// Total symbols in the grammar
    pub grammar_symbols: usize,
    /// Number of rules created
    pub num_rules: usize,
    /// Whether compression has been performed
    pub compressed: bool,
}

impl RepairStats {
    /// Returns the compression ratio as a percentage.
    ///
    /// Lower is better. 100% means no compression.
    pub fn compression_ratio(&self) -> f64 {
        if self.input_length == 0 {
            0.0
        } else {
            (self.grammar_symbols as f64 / self.input_length as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let repair = Repair::<char>::new();
        assert_eq!(repair.len(), 0);
        assert!(repair.is_empty());
        assert_eq!(repair.rules().len(), 1); // Rule 0 exists
        assert!(!repair.is_compressed());
    }

    #[test]
    fn test_push_single() {
        let mut repair = Repair::new();
        repair.push('a');
        assert_eq!(repair.len(), 1);
        assert!(!repair.is_empty());
    }

    #[test]
    fn test_push_multiple() {
        let mut repair = Repair::new();
        repair.push('a');
        repair.push('b');
        repair.push('c');
        assert_eq!(repair.len(), 3);
    }

    #[test]
    fn test_extend() {
        let mut repair = Repair::new();
        repair.extend(vec!['a', 'b', 'c']);
        assert_eq!(repair.len(), 3);
    }

    #[test]
    fn test_compress_no_repetition() {
        let mut repair = Repair::new();
        repair.extend("abc".chars());
        repair.compress();
        assert!(repair.is_compressed());
        // No pairs occur twice, so only Rule 0 should exist
        assert_eq!(repair.rules().len(), 1);
    }

    #[test]
    fn test_compress_simple_repetition() {
        let mut repair = Repair::new();
        repair.extend("abab".chars());
        repair.compress();
        assert!(repair.is_compressed());
        // "ab" occurs twice, should create a rule
        assert!(
            repair.rules().len() >= 2,
            "Should create at least one rule for 'ab'"
        );
    }

    #[test]
    fn test_compress_nested() {
        let mut repair = Repair::new();
        repair.extend("abcabcabcabc".chars());
        repair.compress();
        assert!(repair.is_compressed());
        // Should create multiple rules
        assert!(
            repair.rules().len() >= 2,
            "Should create rules for nested patterns"
        );
    }

    #[test]
    fn test_stats() {
        let mut repair = Repair::new();
        repair.extend("abab".chars());
        repair.compress();

        let stats = repair.stats();
        assert_eq!(stats.input_length, 4);
        assert!(stats.compressed);
    }
}
