use crate::documents::SequiturDocuments;
use crate::symbol::{Symbol, SymbolHash, SymbolNode};
use slotmap::DefaultKey;
use std::collections::hash_map::Entry;
use std::hash::Hash;

// Digram operations for SequiturDocuments
impl<T: Hash + Eq + Clone, DocId: Hash + Eq + Clone> SequiturDocuments<T, DocId> {
    /// Finds an existing digram or adds it to the index.
    ///
    /// Returns Some(key) if a non-overlapping match exists, None otherwise.
    pub(crate) fn find_and_add_digram(
        &mut self,
        first: DefaultKey,
        second: DefaultKey,
    ) -> Option<DefaultKey> {
        assert!(
            self.symbols[first].next == Some(second),
            "Digram must be consecutive symbols"
        );

        // Don't create digrams starting/ending with sentinel nodes
        if matches!(self.symbols[first].symbol, Symbol::RuleHead { .. })
            || matches!(self.symbols[second].symbol, Symbol::RuleTail)
            || matches!(self.symbols[first].symbol, Symbol::DocHead { .. })
            || matches!(self.symbols[second].symbol, Symbol::DocTail)
        {
            return None;
        }

        // Create hash pair for lookup
        let first_hash = SymbolHash::from_symbol(&self.symbols[first].symbol);
        let second_hash = SymbolHash::from_symbol(&self.symbols[second].symbol);

        match self.digram_index.entry((first_hash, second_hash)) {
            Entry::Vacant(e) => {
                // New digram, add to index
                e.insert(first);
                None
            }
            Entry::Occupied(mut e) => {
                let other_first = *e.get();

                // Check if it's the same digram (pointing to itself)
                if other_first == first {
                    return None;
                }

                // Check if the key is still valid (might have been removed)
                if !self.symbols.contains_key(other_first) {
                    // Stale entry, update it
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
    pub(crate) fn remove_digram_from_index(&mut self, first: DefaultKey) {
        // Don't try to remove invalid digrams
        if matches!(self.symbols[first].symbol, Symbol::RuleHead { .. })
            || matches!(self.symbols[first].symbol, Symbol::DocHead { .. })
        {
            return;
        }

        let Some(second) = self.symbols[first].next else {
            return;
        };

        if matches!(self.symbols[second].symbol, Symbol::RuleTail)
            || matches!(self.symbols[second].symbol, Symbol::DocTail)
        {
            return;
        }

        // Create hash key
        let first_hash = SymbolHash::from_symbol(&self.symbols[first].symbol);
        let second_hash = SymbolHash::from_symbol(&self.symbols[second].symbol);

        // Only remove if it points to this exact location
        if let Entry::Occupied(e) = self.digram_index.entry((first_hash, second_hash)) {
            if *e.get() == first {
                e.remove();
            }
        }
    }
}

// Rule operations for SequiturDocuments
impl<T: Hash + Eq + Clone, DocId: Hash + Eq + Clone> SequiturDocuments<T, DocId> {
    /// Checks if a digram match is an entire rule.
    pub(crate) fn get_complete_rule(&self, match_key: DefaultKey) -> Option<DefaultKey> {
        let first = match_key;
        let second = self.symbols[first].next?;

        let prev = self.symbols[first].prev?;
        let next = self.symbols[second].next?;

        // Check if prev is a RuleHead and next is RuleTail
        if let Symbol::RuleHead { tail, .. } = &self.symbols[prev].symbol {
            if matches!(self.symbols[next].symbol, Symbol::RuleTail) {
                // Verify they're part of the same rule
                if *tail == next {
                    return Some(prev);
                }
            }
        }

        None
    }

    /// Creates a new rule from two matching digrams.
    pub(crate) fn swap_for_new_rule(
        &mut self,
        first: DefaultKey,
        match_key: DefaultKey,
    ) -> (DefaultKey, DefaultKey) {
        assert_ne!(first, match_key, "first and match_key should be different");

        let second = self.symbols[first].next.unwrap();
        let match_second = self.symbols[match_key].next.unwrap();

        assert_ne!(first, match_second, "Digrams should not overlap");
        assert_ne!(second, match_key, "Digrams should not overlap");

        // Create new rule
        let rule_id = self.id_gen.get();
        let tail_key = self.symbols.insert(SymbolNode::new(Symbol::RuleTail));
        let head_key = self.symbols.insert(SymbolNode::new(Symbol::RuleHead {
            rule_id,
            count: 0,
            tail: tail_key,
        }));

        // Clone the two symbols into the rule
        let rule_first = self
            .symbols
            .insert(SymbolNode::new(self.symbols[first].symbol.clone_symbol()));
        let rule_second = self
            .symbols
            .insert(SymbolNode::new(self.symbols[second].symbol.clone_symbol()));

        // Link rule: head -> first -> second -> tail
        self.symbols[head_key].next = Some(rule_first);
        self.symbols[rule_first].prev = Some(head_key);
        self.symbols[rule_first].next = Some(rule_second);
        self.symbols[rule_second].prev = Some(rule_first);
        self.symbols[rule_second].next = Some(tail_key);
        self.symbols[tail_key].prev = Some(rule_second);

        // Update digram index to point to rule
        self.remove_digram_from_index(first);
        self.remove_digram_from_index(match_key);

        let first_hash = SymbolHash::from_symbol(&self.symbols[rule_first].symbol);
        let second_hash = SymbolHash::from_symbol(&self.symbols[rule_second].symbol);
        self.digram_index
            .insert((first_hash, second_hash), rule_first);

        // Add rule to index
        self.rule_index.insert(rule_id, head_key);

        // Increment rule counts for symbols in the rule
        self.increment_rule_count_if_needed(rule_first);
        self.increment_rule_count_if_needed(rule_second);

        // Replace both occurrences with RuleRefs
        // Use swap_for_existing_rule which handles the replacement safely
        let loc1 = self.swap_for_existing_rule(first, head_key);
        let loc2 = self.swap_for_existing_rule(match_key, head_key);

        (loc1, loc2)
    }

    /// Replaces a digram with a reference to an existing rule.
    pub(crate) fn swap_for_existing_rule(
        &mut self,
        first: DefaultKey,
        rule_head_key: DefaultKey,
    ) -> DefaultKey {
        let second = self.symbols[first].next.unwrap();

        let before_digram = self.symbols[first].prev;
        let after_digram = self.symbols[second].next;

        // Remove surrounding digrams from index
        if let Some(prev) = before_digram {
            self.remove_digram_from_index(prev);
        }
        self.remove_digram_from_index(second);

        // Decrement counts if symbols are RuleRefs (just decrement, don't expand)
        self.decrement_if_rule(first);
        self.decrement_if_rule(second);

        // Get rule_id from RuleHead
        let rule_id = match self.symbols[rule_head_key].symbol {
            Symbol::RuleHead { rule_id, .. } => rule_id,
            _ => panic!("Expected RuleHead"),
        };

        // Create new RuleRef symbol
        let new_rule_key = self
            .symbols
            .insert(SymbolNode::new(Symbol::RuleRef { rule_id }));

        // Link new RuleRef into the sequence
        self.symbols[new_rule_key].prev = before_digram;
        self.symbols[new_rule_key].next = after_digram;

        if let Some(prev) = before_digram {
            self.symbols[prev].next = Some(new_rule_key);
        }
        if let Some(next) = after_digram {
            self.symbols[next].prev = Some(new_rule_key);
        }

        // Increment rule count
        self.increment_rule_count(rule_head_key);

        // Remove the old digram symbols
        self.symbols.remove(first);
        self.symbols.remove(second);

        // Expand rules in the rule body if necessary
        let rule_first = self.symbols[rule_head_key].next.expect("RuleHead should have next");
        let rule_second = self.symbols[rule_first].next.expect("Rule first should have next");

        self.expand_rule_if_necessary(rule_first);
        self.expand_rule_if_necessary(rule_second);

        new_rule_key
    }


    /// Expands a rule inline if it's only used once.
    pub(crate) fn expand_rule_if_necessary(&mut self, symbol_key: DefaultKey) {
        // Check if this is a RuleRef with count == 1
        if let Symbol::RuleRef { rule_id } = self.symbols[symbol_key].symbol {
            let rule_head_key = *self
                .rule_index
                .get(&rule_id)
                .expect("Rule should exist in index");

            let count = match self.symbols[rule_head_key].symbol {
                Symbol::RuleHead { count, .. } => count,
                _ => panic!("Expected RuleHead"),
            };

            if count == 1 {
                self.expand_rule(symbol_key, rule_head_key);
            }
        }
    }

    /// Expands a rule inline.
    fn expand_rule(&mut self, rule_ref_key: DefaultKey, rule_head_key: DefaultKey) {
        let _rule_id = match self.symbols[rule_head_key].symbol {
            Symbol::RuleHead { rule_id, tail, .. } => {
                // Remove rule from index
                self.rule_index.remove(&rule_id);
                self.id_gen.free(rule_id);

                // Get symbols in rule
                let mut rule_symbols = Vec::new();
                let mut current = self.symbols[rule_head_key].next;
                while let Some(key) = current {
                    if matches!(self.symbols[key].symbol, Symbol::RuleTail) {
                        break;
                    }
                    rule_symbols.push(key);
                    current = self.symbols[key].next;
                }

                // Remove digrams from index
                for i in 0..rule_symbols.len() {
                    self.remove_digram_from_index(rule_symbols[i]);
                }

                // Clone symbols
                let cloned: Vec<_> = rule_symbols
                    .iter()
                    .map(|&k| self.symbols[k].symbol.clone_symbol())
                    .collect();

                // Remove rule structure
                self.symbols.remove(rule_head_key);
                self.symbols.remove(tail);
                for &key in &rule_symbols {
                    self.symbols.remove(key);
                }

                // Insert cloned symbols at rule_ref location
                let prev_key = self.symbols[rule_ref_key].prev;
                let next_key = self.symbols[rule_ref_key].next;
                self.symbols.remove(rule_ref_key);

                // Create new symbols and link them
                let mut new_keys = Vec::new();
                for symbol in cloned {
                    let key = self.symbols.insert(SymbolNode::new(symbol));
                    new_keys.push(key);
                }

                // Link chain
                let mut prev = prev_key;
                for &key in &new_keys {
                    self.symbols[key].prev = prev;
                    if let Some(p) = prev {
                        self.symbols[p].next = Some(key);
                    }
                    prev = Some(key);
                }

                if let Some(last) = new_keys.last() {
                    self.symbols[*last].next = next_key;
                }
                if let Some(next) = next_key {
                    self.symbols[next].prev = new_keys.last().copied();
                }

                // Check new links
                for i in 0..new_keys.len().saturating_sub(1) {
                    self.check_new_links(new_keys[i]);
                }
                if let Some(&last) = new_keys.last() {
                    self.check_new_links(last);
                }

                rule_id
            }
            _ => panic!("Expected RuleHead"),
        };
    }

    /// Checks for new digrams after inserting a symbol.
    pub(crate) fn check_new_links(&mut self, key: DefaultKey) {
        // Check if key is still valid
        if !self.symbols.contains_key(key) {
            return;
        }

        // Check digram starting at prev
        if let Some(prev) = self.symbols[key].prev {
            if !matches!(self.symbols[prev].symbol, Symbol::RuleHead { .. })
                && !matches!(self.symbols[prev].symbol, Symbol::DocHead { .. })
            {
                self.link_made(prev);
            }
        }

        // Check digram starting at key (re-check validity as link_made might have changed things)
        if !self.symbols.contains_key(key) {
            return;
        }

        if let Some(_next) = self.symbols[key].next {
            if !matches!(self.symbols[key].symbol, Symbol::RuleHead { .. })
                && !matches!(self.symbols[key].symbol, Symbol::DocHead { .. })
            {
                self.link_made(key);
            }
        }
    }

    /// Checks for new digrams after creating a rule.
    pub(crate) fn check_new_links_pair(&mut self, loc1: DefaultKey, loc2: DefaultKey) {
        self.check_new_links(loc1);
        self.check_new_links(loc2);
    }

    /// Increments a rule's reference count.
    fn increment_rule_count(&mut self, rule_head_key: DefaultKey) {
        if let Symbol::RuleHead {
            ref mut count,
            rule_id,
            tail,
        } = self.symbols[rule_head_key].symbol
        {
            *count += 1;
            // Restore the full value to avoid partial move
            self.symbols[rule_head_key].symbol = Symbol::RuleHead {
                rule_id,
                count: *count,
                tail,
            };
        }
    }

    /// Decrements a rule's reference count (without expanding).
    fn decrement_rule_count(&mut self, rule_head_key: DefaultKey) {
        if let Symbol::RuleHead { rule_id, count, tail } = self.symbols[rule_head_key].symbol {
            assert!(count > 0, "Cannot decrement count below 0");
            self.symbols[rule_head_key].symbol = Symbol::RuleHead {
                rule_id,
                count: count - 1,
                tail,
            };
        }
    }

    /// Decrements the count of a rule if the symbol is a RuleRef.
    fn decrement_if_rule(&mut self, key: DefaultKey) {
        if let Symbol::RuleRef { rule_id } = self.symbols[key].symbol {
            if let Some(&head_key) = self.rule_index.get(&rule_id) {
                self.decrement_rule_count(head_key);
            }
        }
    }

    /// Increments rule count if symbol is a RuleRef.
    fn increment_rule_count_if_needed(&mut self, symbol_key: DefaultKey) {
        if let Symbol::RuleRef { rule_id } = self.symbols[symbol_key].symbol {
            if let Some(&rule_head_key) = self.rule_index.get(&rule_id) {
                self.increment_rule_count(rule_head_key);
            }
        }
    }

}
