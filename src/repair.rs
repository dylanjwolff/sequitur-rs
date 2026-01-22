//! RePair grammar compression algorithm.
//!
//! RePair is a greedy grammar-based compression algorithm that repeatedly
//! replaces the most frequent pair of adjacent symbols with a new rule.
//!
//! This implementation achieves O(n) time and space complexity by using:
//! - A hash table to track pair frequencies
//! - A bucket-based priority queue for O(1) amortized max-frequency access
//! - Occurrence threading to find all instances of a pair without rescanning
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
use std::hash::Hash;

/// Identifier for symbols in the pair index.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum PairSymbolId {
    /// Terminal symbol with index into values_dedup
    Terminal(u32),
    /// Non-terminal (rule reference)
    RuleRef(u32),
}

/// A record tracking a unique pair and its occurrences.
#[derive(Debug)]
struct PairRecord {
    /// Number of occurrences of this pair
    frequency: u32,
    /// Head of the linked list of occurrences (position of first symbol in pair)
    first_occurrence: Option<DefaultKey>,
    /// Tail of the linked list (for O(1) append)
    last_occurrence: Option<DefaultKey>,
}

/// Threading info for each position - links occurrences of the same pair.
#[derive(Debug, Clone, Copy, Default)]
struct PairThread {
    /// Next occurrence of the same (this, next) pair
    next_same_pair: Option<DefaultKey>,
    /// Previous occurrence of the same (this, next) pair
    prev_same_pair: Option<DefaultKey>,
}

/// Bucket-based priority queue for O(1) amortized max access.
#[derive(Debug)]
struct PriorityQueue {
    /// Buckets indexed by frequency, each containing list of pair keys
    buckets: Vec<Vec<(PairSymbolId, PairSymbolId)>>,
    /// Current maximum non-empty bucket
    max_bucket: usize,
}

impl PriorityQueue {
    fn new(max_frequency: usize) -> Self {
        Self {
            buckets: vec![Vec::new(); max_frequency + 1],
            max_bucket: 0,
        }
    }

    fn insert(&mut self, pair: (PairSymbolId, PairSymbolId), frequency: u32) {
        let freq = frequency as usize;
        if freq >= self.buckets.len() {
            self.buckets.resize(freq + 1, Vec::new());
        }
        self.buckets[freq].push(pair);
        if freq > self.max_bucket {
            self.max_bucket = freq;
        }
    }

    fn pop_max(&mut self) -> Option<(PairSymbolId, PairSymbolId)> {
        while self.max_bucket > 0 {
            if let Some(pair) = self.buckets[self.max_bucket].pop() {
                return Some(pair);
            }
            self.max_bucket -= 1;
        }
        // Check bucket 0 as well (though pairs with freq < 2 aren't useful)
        self.buckets[0].pop()
    }

    #[allow(dead_code)]
    fn is_empty(&self) -> bool {
        self.max_bucket == 0 && self.buckets[0].is_empty()
    }
}

/// Main RePair data structure.
///
/// Compresses input sequences using the RePair algorithm with O(n) complexity.
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
    pub fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            self.push(value);
        }
    }

    /// Performs RePair compression on the sequence.
    ///
    /// This implementation uses O(n) time and space:
    /// - Hash table for pair → record mapping
    /// - Bucket-based priority queue for max-frequency access
    /// - Occurrence threading for efficient pair replacement
    pub fn compress(&mut self) {
        if self.compressed || self.length < 2 {
            self.compressed = true;
            return;
        }

        // Phase 1: Initialize data structures - O(n)
        let (mut pair_records, mut pair_threads) = self.initialize_pair_structures();

        // Build priority queue from pair records
        let max_freq = pair_records
            .values()
            .map(|r| r.frequency)
            .max()
            .unwrap_or(0);
        let mut pq = PriorityQueue::new(max_freq as usize);

        for (&pair, record) in &pair_records {
            if record.frequency >= 2 {
                pq.insert(pair, record.frequency);
            }
        }

        // Phase 2: Main compression loop - O(n) total
        while let Some(pair) = pq.pop_max() {
            // Get current record (may have been updated)
            let Some(record) = pair_records.get(&pair) else {
                continue;
            };

            // Skip if frequency dropped below 2
            if record.frequency < 2 {
                continue;
            }

            let first_occurrence = match record.first_occurrence {
                Some(k) => k,
                None => continue,
            };

            // Create new rule for this pair
            let rule_id = self.create_rule_for_pair(pair);

            // Replace all occurrences, updating adjacent pairs
            self.replace_all_occurrences(
                pair,
                rule_id,
                first_occurrence,
                &mut pair_records,
                &mut pair_threads,
                &mut pq,
            );
        }

        self.compressed = true;
    }

    /// Initialize pair records and threading structures - O(n).
    fn initialize_pair_structures(
        &self,
    ) -> (
        HashMap<(PairSymbolId, PairSymbolId), PairRecord>,
        HashMap<DefaultKey, PairThread>,
    ) {
        let mut pair_records: HashMap<(PairSymbolId, PairSymbolId), PairRecord> =
            HashMap::default();
        let mut pair_threads: HashMap<DefaultKey, PairThread> = HashMap::default();

        // Scan Rule 0 and build pair records
        let head_key = *self.rule_index.get(&0).expect("Rule 0 should exist");
        let mut current = self.symbols[head_key].next;

        while let Some(key) = current {
            let next_key = match self.symbols[key].next {
                Some(n) => n,
                None => break,
            };

            // Skip sentinels
            if self.is_sentinel(key) || self.is_sentinel(next_key) {
                current = Some(next_key);
                continue;
            }

            let first_id = match self.get_symbol_id(key) {
                Some(id) => id,
                None => {
                    current = Some(next_key);
                    continue;
                }
            };
            let second_id = match self.get_symbol_id(next_key) {
                Some(id) => id,
                None => {
                    current = Some(next_key);
                    continue;
                }
            };

            let pair = (first_id, second_id);

            // Update or create record
            let record = pair_records.entry(pair).or_insert(PairRecord {
                frequency: 0,
                first_occurrence: None,
                last_occurrence: None,
            });

            // Thread this occurrence - O(1) using last_occurrence
            let mut thread = PairThread::default();

            if let Some(last) = record.last_occurrence {
                // Link this occurrence after last
                if let Some(t) = pair_threads.get_mut(&last) {
                    t.next_same_pair = Some(key);
                }
                thread.prev_same_pair = Some(last);
            } else {
                record.first_occurrence = Some(key);
            }
            record.last_occurrence = Some(key);

            pair_threads.insert(key, thread);
            record.frequency += 1;

            current = Some(next_key);
        }

        (pair_records, pair_threads)
    }

    fn is_sentinel(&self, key: DefaultKey) -> bool {
        matches!(
            self.symbols[key].symbol,
            Symbol::RuleHead { .. } | Symbol::RuleTail
        )
    }

    /// Create a new rule for a pair and return its ID.
    fn create_rule_for_pair(&mut self, pair: (PairSymbolId, PairSymbolId)) -> u32 {
        let rule_id = self.id_gen.get();

        // Create RuleTail
        let tail_key = self.symbols.insert(SymbolNode::new(Symbol::RuleTail));

        // Create RuleHead
        let head_key = self.symbols.insert(SymbolNode::new(Symbol::RuleHead {
            rule_id,
            count: 0,
            tail: tail_key,
        }));

        // Create the rule body
        let rule_first = self
            .symbols
            .insert(SymbolNode::new(self.id_to_symbol(pair.0)));
        let rule_second = self
            .symbols
            .insert(SymbolNode::new(self.id_to_symbol(pair.1)));

        // Link rule structure: head -> first -> second -> tail
        self.symbols[head_key].next = Some(rule_first);
        self.symbols[rule_first].prev = Some(head_key);
        self.symbols[rule_first].next = Some(rule_second);
        self.symbols[rule_second].prev = Some(rule_first);
        self.symbols[rule_second].next = Some(tail_key);
        self.symbols[tail_key].prev = Some(rule_second);

        self.rule_index.insert(rule_id, head_key);
        rule_id
    }

    /// Replace all occurrences of a pair using the thread structure.
    fn replace_all_occurrences(
        &mut self,
        pair: (PairSymbolId, PairSymbolId),
        rule_id: u32,
        first_occurrence: DefaultKey,
        pair_records: &mut HashMap<(PairSymbolId, PairSymbolId), PairRecord>,
        pair_threads: &mut HashMap<DefaultKey, PairThread>,
        pq: &mut PriorityQueue,
    ) {
        let mut count = 0u32;
        let mut current_occ = Some(first_occurrence);

        while let Some(first_key) = current_occ {
            // Get next occurrence before we potentially invalidate the thread
            let next_occ = pair_threads.get(&first_key).and_then(|t| t.next_same_pair);

            // Verify this occurrence is still valid
            if !self.symbols.contains_key(first_key) {
                current_occ = next_occ;
                continue;
            }

            let second_key = match self.symbols[first_key].next {
                Some(k) if self.symbols.contains_key(k) => k,
                _ => {
                    current_occ = next_occ;
                    continue;
                }
            };

            // Verify symbols still match the pair
            let current_first = self.get_symbol_id(first_key);
            let current_second = self.get_symbol_id(second_key);

            if current_first != Some(pair.0) || current_second != Some(pair.1) {
                current_occ = next_occ;
                continue;
            }

            // Get adjacent positions for updating neighbor pairs
            let before_key = self.symbols[first_key].prev;
            let after_key = self.symbols[second_key].next;

            // Decrease frequency of adjacent pairs (before, first) and (second, after)
            if let Some(before) = before_key {
                if !self.is_sentinel(before) {
                    if let (Some(bid), Some(fid)) =
                        (self.get_symbol_id(before), self.get_symbol_id(first_key))
                    {
                        Self::decrease_pair_frequency(pair_records, (bid, fid));
                    }
                }
            }

            if let Some(after) = after_key {
                if !self.is_sentinel(after) {
                    if let (Some(sid), Some(aid)) =
                        (self.get_symbol_id(second_key), self.get_symbol_id(after))
                    {
                        Self::decrease_pair_frequency(pair_records, (sid, aid));
                    }
                }
            }

            // Create RuleRef to replace the pair
            let rule_ref_key = self
                .symbols
                .insert(SymbolNode::new(Symbol::RuleRef { rule_id }));

            // Link into sequence
            self.symbols[rule_ref_key].prev = before_key;
            self.symbols[rule_ref_key].next = after_key;

            if let Some(prev) = before_key {
                self.symbols[prev].next = Some(rule_ref_key);
            }
            if let Some(next) = after_key {
                self.symbols[next].prev = Some(rule_ref_key);
            }

            // Remove old symbols
            self.symbols.remove(first_key);
            self.symbols.remove(second_key);
            pair_threads.remove(&first_key);

            // Increase frequency of new adjacent pairs and add to PQ if needed
            let new_id = PairSymbolId::RuleRef(rule_id);

            if let Some(before) = before_key {
                if !self.is_sentinel(before) {
                    if let Some(bid) = self.get_symbol_id(before) {
                        let new_pair = (bid, new_id);
                        let freq = Self::increase_pair_frequency(
                            pair_records,
                            pair_threads,
                            new_pair,
                            before,
                        );
                        if freq == 2 {
                            pq.insert(new_pair, freq);
                        }
                    }
                }
            }

            if let Some(after) = after_key {
                if !self.is_sentinel(after) {
                    if let Some(aid) = self.get_symbol_id(after) {
                        let new_pair = (new_id, aid);
                        let freq = Self::increase_pair_frequency(
                            pair_records,
                            pair_threads,
                            new_pair,
                            rule_ref_key,
                        );
                        if freq == 2 {
                            pq.insert(new_pair, freq);
                        }
                    }
                }
            }

            count += 1;
            current_occ = next_occ;
        }

        // Update rule count
        if let Some(&head_key) = self.rule_index.get(&rule_id) {
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

        // Remove the pair record
        pair_records.remove(&pair);
    }

    fn decrease_pair_frequency(
        pair_records: &mut HashMap<(PairSymbolId, PairSymbolId), PairRecord>,
        pair: (PairSymbolId, PairSymbolId),
    ) {
        if let Some(record) = pair_records.get_mut(&pair) {
            record.frequency = record.frequency.saturating_sub(1);
        }
    }

    fn increase_pair_frequency(
        pair_records: &mut HashMap<(PairSymbolId, PairSymbolId), PairRecord>,
        pair_threads: &mut HashMap<DefaultKey, PairThread>,
        pair: (PairSymbolId, PairSymbolId),
        position: DefaultKey,
    ) -> u32 {
        let record = pair_records.entry(pair).or_insert(PairRecord {
            frequency: 0,
            first_occurrence: None,
            last_occurrence: None,
        });

        // Thread this occurrence - O(1) using last_occurrence
        let mut thread = PairThread::default();

        if let Some(last) = record.last_occurrence {
            // Link after last
            if let Some(t) = pair_threads.get_mut(&last) {
                t.next_same_pair = Some(position);
            }
            thread.prev_same_pair = Some(last);
        } else {
            record.first_occurrence = Some(position);
        }
        record.last_occurrence = Some(position);

        pair_threads.insert(position, thread);
        record.frequency += 1;
        record.frequency
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
