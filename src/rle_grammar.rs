use crate::id_gen::IdGenerator;
use crate::rle_symbol::{RleDigramKey, RleSymbolNode};
use crate::symbol::Symbol;
use ahash::AHashMap as HashMap;
use slotmap::{DefaultKey, SlotMap};
use std::collections::hash_map::Entry;
use std::hash::Hash;

/// Core grammar storage for RLE-Sequitur.
///
/// Similar to the standard Grammar but with run-length encoding support:
/// - Symbols have run counts representing consecutive occurrences
/// - Digram similarity ignores run counts (a:2,b:3 is similar to a:5,b:1)
/// - Adjacent identical symbols are merged (no contiguous repeated symbols)
/// - Node splitting is performed when needed for digram uniqueness
pub(crate) struct RleGrammar<T> {
    /// Storage for all symbols with run counts
    pub symbols: SlotMap<DefaultKey, RleSymbolNode<T>>,

    /// Maps digrams to their first occurrence (ignores run counts)
    pub digram_index: HashMap<RleDigramKey, DefaultKey>,

    /// Maps rule IDs to their RuleHead keys
    pub rule_index: HashMap<u32, DefaultKey>,

    /// ID generator with reuse
    pub id_gen: IdGenerator,
}

impl<T> RleGrammar<T> {
    /// Creates a new empty RLE grammar.
    pub fn new() -> Self {
        Self {
            symbols: SlotMap::new(),
            digram_index: HashMap::default(),
            rule_index: HashMap::default(),
            id_gen: IdGenerator::new(),
        }
    }
}

impl<T: Hash + Eq + Clone> RleGrammar<T> {
    // ========================================================================
    // Run-Length Encoding Operations
    // ========================================================================

    /// Tries to merge a symbol with its next neighbor if they have the same symbol.
    ///
    /// This enforces the "no contiguous repeated symbols" property.
    /// Returns true if a merge occurred.
    pub fn try_merge_with_next(&mut self, key: DefaultKey) -> bool {
        let Some(next_key) = self.symbols[key].next else {
            return false;
        };

        // Don't merge with sequence boundaries
        if self.is_sequence_end(&self.symbols[next_key].symbol) {
            return false;
        }

        // Check if symbols are equal (excluding run counts)
        if !self.symbols[key]
            .symbol
            .equals(&self.symbols[next_key].symbol)
        {
            return false;
        }

        // Remove digrams that will be invalidated
        if let Some(prev) = self.symbols[key].prev {
            self.remove_digram_from_index(prev);
        }
        self.remove_digram_from_index(key);
        self.remove_digram_from_index(next_key);

        // Merge: add next's run to current
        let next_run = self.symbols[next_key].run;
        self.symbols[key].run += next_run;

        // Relink: skip over next_key
        let after_next = self.symbols[next_key].next;
        self.symbols[key].next = after_next;
        if let Some(after) = after_next {
            self.symbols[after].prev = Some(key);
        }

        // Note: We do NOT decrement rule count here because we're merging
        // identical symbols. The total number of references (key.run + next.run)
        // is preserved in the merged node's run, so the count stays the same.

        // Remove the merged node
        self.symbols.remove(next_key);

        true
    }

    /// Splits a node at a given run offset, creating two nodes.
    ///
    /// If the node has run=8 and we split at offset=6, we get:
    /// - First node: run=6
    /// - New second node: run=2 (inserted after first)
    ///
    /// Returns the key of the new second node.
    pub fn split_node(&mut self, key: DefaultKey, first_run: u32) -> DefaultKey {
        let total_run = self.symbols[key].run;
        debug_assert!(
            first_run > 0 && first_run < total_run,
            "Invalid split: first_run={}, total={}",
            first_run,
            total_run
        );

        // Remove affected digrams
        self.remove_digram_from_index(key);

        // Update first node's run
        self.symbols[key].run = first_run;

        // Create second node with remaining run
        let second_run = total_run - first_run;
        let second_key = self.symbols.insert(RleSymbolNode::with_run(
            self.symbols[key].symbol.clone_symbol(),
            second_run,
        ));

        // Note: We do NOT increment rule count here because we're just
        // reorganizing existing references, not creating new ones.
        // The total reference count (first_run + second_run) equals total_run,
        // which was already counted.

        // Relink
        let after_first = self.symbols[key].next;
        self.symbols[key].next = Some(second_key);
        self.symbols[second_key].prev = Some(key);
        self.symbols[second_key].next = after_first;
        if let Some(after) = after_first {
            self.symbols[after].prev = Some(second_key);
        }

        second_key
    }

    // ========================================================================
    // Digram Operations (RLE-aware)
    // ========================================================================

    /// Finds an existing similar digram or adds this one to the index.
    ///
    /// In RLE-Sequitur, digrams are "similar" if they have the same symbols,
    /// regardless of run counts.
    ///
    /// Returns Some((key, needs_split_info)) if a match exists, None otherwise.
    /// The needs_split_info indicates if node splitting is required.
    #[inline]
    pub fn find_and_add_digram(
        &mut self,
        first: DefaultKey,
        second: DefaultKey,
    ) -> Option<DefaultKey> {
        debug_assert!(
            self.symbols[first].next == Some(second),
            "Digram must be consecutive symbols"
        );

        // Don't create digrams starting/ending with sentinel nodes
        if self.is_sequence_start(&self.symbols[first].symbol)
            || self.is_sequence_end(&self.symbols[second].symbol)
        {
            return None;
        }

        // Create digram key (ignores run counts)
        let digram_key =
            RleDigramKey::from_symbols(&self.symbols[first].symbol, &self.symbols[second].symbol);

        match self.digram_index.entry(digram_key) {
            Entry::Vacant(e) => {
                e.insert(first);
                None
            }
            Entry::Occupied(mut e) => {
                let other_first = *e.get();

                // Check if it's the same digram
                if other_first == first {
                    return None;
                }

                // Check if the key is still valid
                if !self.symbols.contains_key(other_first) {
                    e.insert(first);
                    return None;
                }

                let other_second = self.symbols[other_first]
                    .next
                    .expect("Digram first should have next");

                // Check for overlap
                if other_second == first || other_first == second {
                    return None;
                }

                // Verify full equality (hash collision check)
                let symbols_equal = self.symbols[first]
                    .symbol
                    .equals(&self.symbols[other_first].symbol)
                    && self.symbols[second]
                        .symbol
                        .equals(&self.symbols[other_second].symbol);

                if symbols_equal {
                    Some(other_first)
                } else {
                    None
                }
            }
        }
    }

    /// Removes a digram from the index if it points to the given location.
    #[inline]
    pub fn remove_digram_from_index(&mut self, first: DefaultKey) {
        if self.is_sequence_start(&self.symbols[first].symbol) {
            return;
        }

        let Some(second) = self.symbols[first].next else {
            return;
        };

        if self.is_sequence_end(&self.symbols[second].symbol) {
            return;
        }

        let digram_key =
            RleDigramKey::from_symbols(&self.symbols[first].symbol, &self.symbols[second].symbol);

        if let Entry::Occupied(e) = self.digram_index.entry(digram_key) {
            if *e.get() == first {
                e.remove();
            }
        }
    }

    // ========================================================================
    // Rule Operations (RLE-aware)
    // ========================================================================

    /// Checks if a digram is a complete rule.
    #[inline]
    pub fn get_complete_rule(&self, first: DefaultKey) -> Option<DefaultKey> {
        let second = self.symbols[first].next?;

        // For RLE, we also need to check that both nodes have run=1
        // A rule with run>1 should not be reused directly
        if self.symbols[first].run != 1 || self.symbols[second].run != 1 {
            return None;
        }

        let prev = self.symbols[first].prev?;
        if !matches!(self.symbols[prev].symbol, Symbol::RuleHead { .. }) {
            return None;
        }

        let after_second = self.symbols[second].next?;
        if !matches!(self.symbols[after_second].symbol, Symbol::RuleTail) {
            return None;
        }

        if let Symbol::RuleHead { tail, .. } = self.symbols[prev].symbol {
            if tail == after_second {
                return Some(prev);
            }
        }

        None
    }

    /// Creates a new rule from two similar digram occurrences.
    ///
    /// In RLE-Sequitur, this may involve splitting nodes if the runs don't match.
    pub fn swap_for_new_rule(
        &mut self,
        match1: DefaultKey,
        match2: DefaultKey,
    ) -> (DefaultKey, DefaultKey) {
        // First, normalize the runs so both digrams have the same run pattern
        // We need to find the minimum runs and potentially split nodes

        let match1_second = self.symbols[match1].next.unwrap();
        let match2_second = self.symbols[match2].next.unwrap();

        // Get the minimum runs for each position
        let first_run = self.symbols[match1].run.min(self.symbols[match2].run);
        let second_run = self.symbols[match1_second]
            .run
            .min(self.symbols[match2_second].run);

        // Split nodes if necessary to extract the common pattern
        let (m1_first, m1_second) = self.prepare_digram_for_rule(match1, first_run, second_run);
        let (m2_first, _m2_second) = self.prepare_digram_for_rule(match2, first_run, second_run);

        // Now create the rule with the normalized runs
        let rule_id = self.id_gen.get();

        let tail_key = self.symbols.insert(RleSymbolNode::new(Symbol::RuleTail));
        let head_key = self.symbols.insert(RleSymbolNode::new(Symbol::RuleHead {
            rule_id,
            count: 0,
            tail: tail_key,
        }));

        // Clone the digram symbols into the rule with their runs
        let rule_first = self.symbols.insert(RleSymbolNode::with_run(
            self.symbols[m1_first].symbol.clone_symbol(),
            first_run,
        ));
        let rule_second = self.symbols.insert(RleSymbolNode::with_run(
            self.symbols[m1_second].symbol.clone_symbol(),
            second_run,
        ));

        // Link rule structure
        self.symbols[head_key].next = Some(rule_first);
        self.symbols[rule_first].prev = Some(head_key);
        self.symbols[rule_first].next = Some(rule_second);
        self.symbols[rule_second].prev = Some(rule_first);
        self.symbols[rule_second].next = Some(tail_key);
        self.symbols[tail_key].prev = Some(rule_second);

        // Update digram index
        self.remove_digram_from_index(m1_first);
        self.remove_digram_from_index(m2_first);

        let digram_key = RleDigramKey::from_symbols(
            &self.symbols[rule_first].symbol,
            &self.symbols[rule_second].symbol,
        );
        self.digram_index.insert(digram_key, rule_first);

        self.rule_index.insert(rule_id, head_key);

        // Increment counts for RuleRefs in the rule body
        self.increment_if_rule(rule_first);
        self.increment_if_rule(rule_second);

        // Replace both occurrences
        let loc1 = self.swap_for_existing_rule(m1_first, head_key);
        let loc2 = self.swap_for_existing_rule(m2_first, head_key);

        (loc1, loc2)
    }

    /// Prepares a digram for rule creation by splitting nodes if needed.
    ///
    /// Returns the keys of the (possibly split) first and second nodes.
    fn prepare_digram_for_rule(
        &mut self,
        first: DefaultKey,
        target_first_run: u32,
        target_second_run: u32,
    ) -> (DefaultKey, DefaultKey) {
        let mut first_key = first;
        let mut second_key = self.symbols[first].next.unwrap();

        // Split first node if it has more run than needed
        if self.symbols[first_key].run > target_first_run {
            // We want to use the LAST target_first_run occurrences
            let remaining = self.symbols[first_key].run - target_first_run;
            let new_key = self.split_node(first_key, remaining);
            first_key = new_key;
            second_key = self.symbols[first_key].next.unwrap();
        }

        // Split second node if it has more run than needed
        if self.symbols[second_key].run > target_second_run {
            // We want to use the FIRST target_second_run occurrences
            self.split_node(second_key, target_second_run);
            // second_key remains the same, we split off the excess
        }

        (first_key, second_key)
    }

    /// Replaces a digram with an existing rule reference.
    pub fn swap_for_existing_rule(
        &mut self,
        first: DefaultKey,
        rule_head: DefaultKey,
    ) -> DefaultKey {
        let second = self.symbols[first].next.expect("first should have next");

        let before_digram = self.symbols[first].prev;
        let after_digram = self.symbols[second].next;

        // Remove surrounding digrams
        if let Some(prev) = before_digram {
            self.remove_digram_from_index(prev);
        }
        self.remove_digram_from_index(second);

        // Decrement counts
        self.decrement_if_rule(first);
        self.decrement_if_rule(second);

        let rule_id = if let Symbol::RuleHead { rule_id, .. } = self.symbols[rule_head].symbol {
            rule_id
        } else {
            unreachable!();
        };

        // Create new RuleRef with run=1
        let new_rule_key = self
            .symbols
            .insert(RleSymbolNode::new(Symbol::RuleRef { rule_id }));

        // Link
        self.symbols[new_rule_key].prev = before_digram;
        self.symbols[new_rule_key].next = after_digram;

        if let Some(prev) = before_digram {
            self.symbols[prev].next = Some(new_rule_key);
        }
        if let Some(next) = after_digram {
            self.symbols[next].prev = Some(new_rule_key);
        }

        self.increment_rule_count(rule_head);

        self.symbols.remove(first);
        self.symbols.remove(second);

        // Expand rules if necessary
        // Note: We must re-fetch rule_second after expanding rule_first because
        // the expansion can trigger cascading operations that modify the structure.
        let rule_first = self.symbols[rule_head]
            .next
            .expect("RuleHead should have next");
        self.expand_rule_if_necessary(rule_first);

        // Re-fetch after potential structure changes
        if let Some(current_first) = self.symbols[rule_head].next {
            if let Some(rule_second) = self.symbols[current_first].next {
                if !matches!(self.symbols[rule_second].symbol, Symbol::RuleTail) {
                    self.expand_rule_if_necessary(rule_second);
                }
            }
        }

        new_rule_key
    }

    /// Expands a rule inline if it's only used once.
    pub fn expand_rule_if_necessary(&mut self, potential_rule: DefaultKey) {
        let Symbol::RuleRef { rule_id } = self.symbols[potential_rule].symbol else {
            return;
        };

        // Only expand if run is 1
        if self.symbols[potential_rule].run != 1 {
            return;
        }

        let Some(&rule_head) = self.rule_index.get(&rule_id) else {
            return;
        };

        let count = if let Symbol::RuleHead { count, .. } = self.symbols[rule_head].symbol {
            count
        } else {
            unreachable!();
        };

        debug_assert!(count > 0, "Rule count should never be 0");

        if count != 1 {
            return;
        }

        let rule_first = self.symbols[rule_head]
            .next
            .expect("RuleHead should have next");
        let rule_tail = if let Symbol::RuleHead { tail, .. } = self.symbols[rule_head].symbol {
            tail
        } else {
            unreachable!();
        };

        let rule_last = self.symbols[rule_tail]
            .prev
            .expect("RuleTail should have prev");

        let before_rule = self.symbols[potential_rule].prev;
        let after_rule = self.symbols[potential_rule].next;

        if let Some(prev) = before_rule {
            self.remove_digram_from_index(prev);
        }
        self.remove_digram_from_index(potential_rule);

        self.rule_index.remove(&rule_id);
        self.id_gen.free(rule_id);

        self.symbols[rule_head].next = None;
        self.symbols[rule_first].prev = None;
        self.symbols[rule_last].next = None;
        self.symbols[rule_tail].prev = None;

        self.symbols.remove(rule_head);
        self.symbols.remove(rule_tail);

        self.symbols[rule_first].prev = before_rule;
        self.symbols[rule_last].next = after_rule;

        if let Some(prev) = before_rule {
            self.symbols[prev].next = Some(rule_first);
        }
        if let Some(next) = after_rule {
            self.symbols[next].prev = Some(rule_last);
        }

        self.symbols.remove(potential_rule);

        // Check new digrams and try to merge
        if let Some(prev) = before_rule {
            if !self.is_sequence_start(&self.symbols[prev].symbol) {
                // Try to merge first
                if !self.try_merge_with_next(prev) {
                    self.link_made(prev);
                }
            }
        }

        if let Some(after) = after_rule {
            if !self.is_sequence_end(&self.symbols[after].symbol) {
                // Try to merge at rule_last
                if self.symbols.contains_key(rule_last) && !self.try_merge_with_next(rule_last) {
                    self.link_made(rule_last);
                }
            }
        }
    }

    /// Core algorithm: Called when two symbols are linked.
    ///
    /// First tries to merge adjacent identical symbols, then checks for digram duplicates.
    #[inline]
    pub fn link_made(&mut self, first_key: DefaultKey) {
        // First, try to merge if symbols are identical (RLE property 3)
        if self.try_merge_with_next(first_key) {
            // Merged - need to check digrams around the merged node
            if let Some(prev) = self.symbols[first_key].prev {
                if !self.is_sequence_start(&self.symbols[prev].symbol) {
                    // Recursively check the new digram formed with prev
                    if let Some(_match_key) = self.find_and_add_digram(prev, first_key) {
                        // Found duplicate - handle it
                        self.handle_duplicate_digram(prev);
                    }
                }
            }
            if let Some(next) = self.symbols[first_key].next {
                if !self.is_sequence_end(&self.symbols[next].symbol) {
                    if let Some(_match_key) = self.find_and_add_digram(first_key, next) {
                        self.handle_duplicate_digram(first_key);
                    }
                }
            }
            return;
        }

        let Some(second_key) = self.symbols[first_key].next else {
            return;
        };

        // Try to find existing digram
        if let Some(match_key) = self.find_and_add_digram(first_key, second_key) {
            self.handle_duplicate_digram_with_match(first_key, match_key);
        }
    }

    fn handle_duplicate_digram(&mut self, first_key: DefaultKey) {
        let second_key = self.symbols[first_key].next.unwrap();
        let digram_key = RleDigramKey::from_symbols(
            &self.symbols[first_key].symbol,
            &self.symbols[second_key].symbol,
        );

        if let Some(&match_key) = self.digram_index.get(&digram_key) {
            if match_key != first_key && self.symbols.contains_key(match_key) {
                self.handle_duplicate_digram_with_match(first_key, match_key);
            }
        }
    }

    fn handle_duplicate_digram_with_match(&mut self, first_key: DefaultKey, match_key: DefaultKey) {
        // Check if the match is a complete rule (with run=1)
        if let Some(rule_head_key) = self.get_complete_rule(match_key) {
            // Check if our digram also has run=1 for both nodes
            let second_key = self.symbols[first_key].next.unwrap();
            if self.symbols[first_key].run == 1 && self.symbols[second_key].run == 1 {
                let new_key = self.swap_for_existing_rule(first_key, rule_head_key);
                self.check_new_links(new_key);
                return;
            }
        }

        // Create new rule
        let (loc1, loc2) = self.swap_for_new_rule(first_key, match_key);
        self.check_new_links_pair(loc1, loc2);
    }

    /// Checks newly formed links after rule insertion.
    pub fn check_new_links(&mut self, rule_key: DefaultKey) {
        if !self.symbols.contains_key(rule_key) {
            return;
        }

        // Try to merge with neighbors first
        if let Some(prev) = self.symbols[rule_key].prev {
            if !self.is_sequence_start(&self.symbols[prev].symbol) && self.try_merge_with_next(prev)
            {
                // Merged with prev, check new digrams
                self.check_new_links(prev);
                return;
            }
        }

        if !self.symbols.contains_key(rule_key) {
            return;
        }

        if let Some(next) = self.symbols[rule_key].next {
            if !self.is_sequence_end(&self.symbols[next].symbol)
                && self.try_merge_with_next(rule_key)
            {
                self.check_new_links(rule_key);
                return;
            }
        }

        // Standard digram checks
        if let Some(prev) = self.symbols[rule_key].prev {
            if !self.is_sequence_start(&self.symbols[prev].symbol) {
                self.link_made(prev);
            }
        }

        if !self.symbols.contains_key(rule_key) {
            return;
        }

        if let Some(next) = self.symbols[rule_key].next {
            if !self.is_sequence_end(&self.symbols[next].symbol)
                && !self.is_sequence_start(&self.symbols[rule_key].symbol)
            {
                self.link_made(rule_key);
            }
        }
    }

    /// Checks newly formed links after two rule insertions.
    pub fn check_new_links_pair(&mut self, rule1: DefaultKey, rule2: DefaultKey) {
        // Try merging first
        for &key in &[rule1, rule2] {
            if !self.symbols.contains_key(key) {
                continue;
            }
            if let Some(prev) = self.symbols[key].prev {
                if !self.is_sequence_start(&self.symbols[prev].symbol) {
                    self.try_merge_with_next(prev);
                }
            }
        }

        for &key in &[rule1, rule2] {
            if !self.symbols.contains_key(key) {
                continue;
            }
            if let Some(next) = self.symbols[key].next {
                if !self.is_sequence_end(&self.symbols[next].symbol) {
                    self.try_merge_with_next(key);
                }
            }
        }

        // Now check digrams
        if self.symbols.contains_key(rule1) {
            if let Some(next) = self.symbols[rule1].next {
                if !self.is_sequence_end(&self.symbols[next].symbol)
                    && !self.is_sequence_start(&self.symbols[rule1].symbol)
                {
                    self.link_made(rule1);
                }
            }
        }

        if self.symbols.contains_key(rule2) {
            if let Some(next) = self.symbols[rule2].next {
                if !self.is_sequence_end(&self.symbols[next].symbol)
                    && !self.is_sequence_start(&self.symbols[rule2].symbol)
                {
                    self.link_made(rule2);
                }
            }
        }

        if self.symbols.contains_key(rule2) {
            if let Some(prev) = self.symbols[rule2].prev {
                if self.symbols.contains_key(rule1)
                    && prev != rule1
                    && !self.is_sequence_start(&self.symbols[prev].symbol)
                {
                    self.link_made(prev);
                }
            }
        }

        if self.symbols.contains_key(rule1) {
            if let Some(prev) = self.symbols[rule1].prev {
                if self.symbols.contains_key(rule2)
                    && prev != rule2
                    && !self.is_sequence_start(&self.symbols[prev].symbol)
                {
                    self.link_made(prev);
                }
            }
        }
    }

    // ========================================================================
    // Helper methods
    // ========================================================================

    #[inline]
    fn is_sequence_start(&self, symbol: &Symbol<T>) -> bool {
        matches!(symbol, Symbol::RuleHead { .. } | Symbol::DocHead { .. })
    }

    #[inline]
    fn is_sequence_end(&self, symbol: &Symbol<T>) -> bool {
        matches!(symbol, Symbol::RuleTail | Symbol::DocTail)
    }

    #[inline]
    fn increment_if_rule(&mut self, key: DefaultKey) {
        if let Symbol::RuleRef { rule_id } = self.symbols[key].symbol {
            // Increment by run count
            let run = self.symbols[key].run;
            if let Some(&head_key) = self.rule_index.get(&rule_id) {
                for _ in 0..run {
                    self.increment_rule_count(head_key);
                }
            }
        }
    }

    #[inline]
    fn decrement_if_rule(&mut self, key: DefaultKey) {
        if let Symbol::RuleRef { rule_id } = self.symbols[key].symbol {
            // Decrement by run count
            let run = self.symbols[key].run;
            if let Some(&head_key) = self.rule_index.get(&rule_id) {
                for _ in 0..run {
                    self.decrement_rule_count(head_key);
                }
            }
        }
    }

    #[inline]
    fn increment_rule_count(&mut self, head_key: DefaultKey) {
        if let Symbol::RuleHead {
            rule_id,
            count,
            tail,
        } = self.symbols[head_key].symbol
        {
            self.symbols[head_key].symbol = Symbol::RuleHead {
                rule_id,
                count: count + 1,
                tail,
            };
        }
    }

    #[inline]
    fn decrement_rule_count(&mut self, head_key: DefaultKey) {
        if let Symbol::RuleHead {
            rule_id,
            count,
            tail,
        } = self.symbols[head_key].symbol
        {
            debug_assert!(count > 0, "Cannot decrement count below 0");
            self.symbols[head_key].symbol = Symbol::RuleHead {
                rule_id,
                count: count - 1,
                tail,
            };
        }
    }
}

impl<T> Default for RleGrammar<T> {
    fn default() -> Self {
        Self::new()
    }
}
