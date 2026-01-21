use crate::id_gen::IdGenerator;
use crate::symbol::{Symbol, SymbolHash, SymbolNode};
use ahash::AHashMap as HashMap;
use slotmap::{DefaultKey, SlotMap};
use std::collections::hash_map::Entry;
use std::hash::Hash;

/// A bundle of mutable references to all grammar fields.
///
/// This struct enables simultaneous mutable access to different fields,
/// working around Rust's borrow checker limitations with trait methods.
/// The algorithm is implemented as methods on this struct.
pub(crate) struct GrammarFields<'a, T> {
    pub symbols: &'a mut SlotMap<DefaultKey, SymbolNode<T>>,
    pub digram_index: &'a mut HashMap<(SymbolHash, SymbolHash), DefaultKey>,
    pub rule_index: &'a mut HashMap<u32, DefaultKey>,
    pub id_gen: &'a mut IdGenerator,
}

/// Trait for types that provide grammar storage.
///
/// This trait enables zero-cost code sharing between Sequitur and SequiturDocuments.
pub(crate) trait GrammarOps<T> {
    fn fields(&mut self) -> GrammarFields<'_, T>;
}

impl<'a, T: Hash + Eq + Clone> GrammarFields<'a, T> {
    // ========================================================================
    // Digram Operations
    // ========================================================================

    /// Finds an existing digram or adds it to the index.
    ///
    /// Returns Some(key) if a non-overlapping match exists, None otherwise.
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
        if is_sequence_start(&self.symbols[first].symbol)
            || is_sequence_end(&self.symbols[second].symbol)
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

                // Check for overlap: digrams sharing a symbol
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
                    // Hash collision - treat as no match
                    None
                }
            }
        }
    }

    /// Removes a digram from the index if it points to the given location.
    #[inline]
    pub fn remove_digram_from_index(&mut self, first: DefaultKey) {
        // Don't try to remove invalid digrams
        if is_sequence_start(&self.symbols[first].symbol) {
            return;
        }

        let Some(second) = self.symbols[first].next else {
            return;
        };

        if is_sequence_end(&self.symbols[second].symbol) {
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

    // ========================================================================
    // Rule Operations
    // ========================================================================

    /// Checks if a digram is a complete rule (surrounded by RuleHead and RuleTail).
    ///
    /// Returns Some(RuleHead key) if the digram forms a complete rule.
    #[inline]
    pub fn get_complete_rule(&self, first: DefaultKey) -> Option<DefaultKey> {
        let second = self.symbols[first].next?;

        // Check if preceded by RuleHead
        let prev = self.symbols[first].prev?;
        if !matches!(self.symbols[prev].symbol, Symbol::RuleHead { .. }) {
            return None;
        }

        // Check if followed by RuleTail
        let after_second = self.symbols[second].next?;
        if !matches!(self.symbols[after_second].symbol, Symbol::RuleTail) {
            return None;
        }

        // Verify they're part of the same rule
        if let Symbol::RuleHead { tail, .. } = self.symbols[prev].symbol {
            if tail == after_second {
                return Some(prev);
            }
        }

        None
    }

    /// Creates a new rule from two digram occurrences.
    ///
    /// Returns the keys where the new RuleRefs were inserted.
    pub fn swap_for_new_rule(
        &mut self,
        match1: DefaultKey,
        match2: DefaultKey,
    ) -> (DefaultKey, DefaultKey) {
        debug_assert!(
            self.symbols[match1].next.is_some(),
            "match1 should have next"
        );
        debug_assert!(
            self.symbols[match2].next.is_some(),
            "match2 should have next"
        );
        debug_assert_ne!(match1, match2, "match1 and match2 should be different");

        let match1_second = self.symbols[match1].next.unwrap();

        // Clone the symbols we need before mutating
        let first_symbol = self.symbols[match1].symbol.clone_symbol();
        let second_symbol = self.symbols[match1_second].symbol.clone_symbol();

        // Create new rule
        let rule_id = self.id_gen.get();

        // Create RuleTail
        let tail_key = self.symbols.insert(SymbolNode::new(Symbol::RuleTail));

        // Create RuleHead
        let head_key = self.symbols.insert(SymbolNode::new(Symbol::RuleHead {
            rule_id,
            count: 0,
            tail: tail_key,
        }));

        // Insert the cloned symbols into the rule
        let rule_first = self.symbols.insert(SymbolNode::new(first_symbol));
        let rule_second = self.symbols.insert(SymbolNode::new(second_symbol));

        // Link rule structure: head -> first -> second -> tail
        self.symbols[head_key].next = Some(rule_first);
        self.symbols[rule_first].prev = Some(head_key);
        self.symbols[rule_first].next = Some(rule_second);
        self.symbols[rule_second].prev = Some(rule_first);
        self.symbols[rule_second].next = Some(tail_key);
        self.symbols[tail_key].prev = Some(rule_second);

        // Update digram index to point to rule's copy
        self.remove_digram_from_index(match1);
        self.remove_digram_from_index(match2);

        let first_hash = SymbolHash::from_symbol(&self.symbols[rule_first].symbol);
        let second_hash = SymbolHash::from_symbol(&self.symbols[rule_second].symbol);
        self.digram_index
            .insert((first_hash, second_hash), rule_first);

        // Add rule to rule index
        self.rule_index.insert(rule_id, head_key);

        // Increment counts if the symbols in the rule are RuleRefs
        self.increment_if_rule(rule_first);
        self.increment_if_rule(rule_second);

        // Replace both occurrences with RuleRefs
        let loc1 = self.swap_for_existing_rule(match1, head_key);
        let loc2 = self.swap_for_existing_rule(match2, head_key);

        (loc1, loc2)
    }

    /// Replaces a digram with an existing rule reference.
    ///
    /// Returns the key of the newly inserted RuleRef.
    pub fn swap_for_existing_rule(
        &mut self,
        first: DefaultKey,
        rule_head: DefaultKey,
    ) -> DefaultKey {
        let second = self.symbols[first]
            .next
            .expect("first should have next in digram");

        debug_assert!(
            matches!(self.symbols[rule_head].symbol, Symbol::RuleHead { .. }),
            "rule_head must be a RuleHead"
        );

        let before_digram = self.symbols[first].prev;
        let after_digram = self.symbols[second].next;

        // Remove surrounding digrams from index
        if let Some(prev) = before_digram {
            self.remove_digram_from_index(prev);
        }
        self.remove_digram_from_index(second);

        // Decrement counts if symbols are RuleRefs
        self.decrement_if_rule(first);
        self.decrement_if_rule(second);

        // Get rule_id from RuleHead
        let rule_id = if let Symbol::RuleHead { rule_id, .. } = self.symbols[rule_head].symbol {
            rule_id
        } else {
            unreachable!();
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
        self.increment_rule_count(rule_head);

        // Remove the old digram symbols
        self.symbols.remove(first);
        self.symbols.remove(second);

        // Expand rules in the rule body if necessary
        let rule_first = self.symbols[rule_head]
            .next
            .expect("RuleHead should have next");
        let rule_second = self.symbols[rule_first]
            .next
            .expect("Rule first should have next");

        self.expand_rule_if_necessary(rule_first);
        self.expand_rule_if_necessary(rule_second);

        new_rule_key
    }

    /// Expands a rule inline if it's only used once (rule utility constraint).
    pub fn expand_rule_if_necessary(&mut self, potential_rule: DefaultKey) {
        // Only RuleRef symbols can be expanded
        let Symbol::RuleRef { rule_id } = self.symbols[potential_rule].symbol else {
            return;
        };

        // Get the rule head
        let Some(&rule_head) = self.rule_index.get(&rule_id) else {
            return;
        };

        // Check rule count
        let count = if let Symbol::RuleHead { count, .. } = self.symbols[rule_head].symbol {
            count
        } else {
            unreachable!();
        };

        debug_assert!(count > 0, "Rule count should never be 0");

        if count != 1 {
            return; // Rule is used more than once, keep it
        }

        // Rule is only used once, expand it inline
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

        // Get surrounding symbols
        let before_rule = self.symbols[potential_rule].prev;
        let after_rule = self.symbols[potential_rule].next;

        // Remove digrams pointing into this area
        if let Some(prev) = before_rule {
            self.remove_digram_from_index(prev);
        }
        self.remove_digram_from_index(potential_rule);

        // Remove rule from indices
        self.rule_index.remove(&rule_id);
        self.id_gen.free(rule_id);

        // Unlink rule head and tail
        self.symbols[rule_head].next = None;
        self.symbols[rule_first].prev = None;
        self.symbols[rule_last].next = None;
        self.symbols[rule_tail].prev = None;

        // Remove head and tail
        self.symbols.remove(rule_head);
        self.symbols.remove(rule_tail);

        // Link rule contents into original sequence
        self.symbols[rule_first].prev = before_rule;
        self.symbols[rule_last].next = after_rule;

        if let Some(prev) = before_rule {
            self.symbols[prev].next = Some(rule_first);
        }
        if let Some(next) = after_rule {
            self.symbols[next].prev = Some(rule_last);
        }

        // Remove the RuleRef symbol
        self.symbols.remove(potential_rule);

        // Check new digrams formed
        if let Some(prev) = before_rule {
            if !is_sequence_start(&self.symbols[prev].symbol) {
                self.link_made(prev);
            }
        }

        // Check digram at rule_last if valid
        if let Some(after) = after_rule {
            if !is_sequence_end(&self.symbols[after].symbol) {
                self.link_made(rule_last);
            }
        }
    }

    /// Core algorithm: Called when two symbols are linked.
    ///
    /// Checks for digram duplicates and creates/reuses rules as needed.
    #[inline]
    pub fn link_made(&mut self, first_key: DefaultKey) {
        debug_assert!(
            self.symbols[first_key].next.is_some(),
            "link_made called on symbol without next"
        );

        let second_key = self.symbols[first_key].next.unwrap();

        // Try to find existing digram or add to index
        if let Some(match_key) = self.find_and_add_digram(first_key, second_key) {
            // Check if the match is a complete rule
            if let Some(rule_head_key) = self.get_complete_rule(match_key) {
                // Replace with existing rule
                let new_key = self.swap_for_existing_rule(first_key, rule_head_key);
                self.check_new_links(new_key);
            } else {
                // Create new rule from both occurrences
                let (loc1, loc2) = self.swap_for_new_rule(first_key, match_key);
                self.check_new_links_pair(loc1, loc2);
            }
        }
    }

    /// Checks newly formed links after rule insertion.
    #[inline]
    pub fn check_new_links(&mut self, rule_key: DefaultKey) {
        // Check if key is still valid
        if !self.symbols.contains_key(rule_key) {
            return;
        }

        // Check digram before rule_key
        if let Some(prev) = self.symbols[rule_key].prev {
            if !is_sequence_start(&self.symbols[prev].symbol) {
                self.link_made(prev);
            }
        }

        // Re-check validity after link_made might have changed things
        if !self.symbols.contains_key(rule_key) {
            return;
        }

        // Check digram starting at rule_key
        if let Some(next) = self.symbols[rule_key].next {
            if !is_sequence_end(&self.symbols[next].symbol)
                && !is_sequence_start(&self.symbols[rule_key].symbol)
            {
                self.link_made(rule_key);
            }
        }
    }

    /// Checks newly formed links after two rule insertions.
    #[inline]
    pub fn check_new_links_pair(&mut self, rule1: DefaultKey, rule2: DefaultKey) {
        // Check at rule1
        if let Some(next) = self.symbols[rule1].next {
            if !is_sequence_end(&self.symbols[next].symbol)
                && !is_sequence_start(&self.symbols[rule1].symbol)
            {
                self.link_made(rule1);
            }
        }

        // Check at rule2
        if let Some(next) = self.symbols[rule2].next {
            if !is_sequence_end(&self.symbols[next].symbol)
                && !is_sequence_start(&self.symbols[rule2].symbol)
            {
                self.link_made(rule2);
            }
        }

        // Check before rule2
        if let Some(prev) = self.symbols[rule2].prev {
            if prev != rule1 && !is_sequence_start(&self.symbols[prev].symbol) {
                self.link_made(prev);
            }
        }

        // Check before rule1
        if let Some(prev) = self.symbols[rule1].prev {
            if prev != rule2 && !is_sequence_start(&self.symbols[prev].symbol) {
                self.link_made(prev);
            }
        }
    }

    // ========================================================================
    // Helper methods
    // ========================================================================

    /// Increments the count of a rule if the symbol is a RuleRef.
    #[inline]
    fn increment_if_rule(&mut self, key: DefaultKey) {
        if let Symbol::RuleRef { rule_id } = self.symbols[key].symbol {
            if let Some(&head_key) = self.rule_index.get(&rule_id) {
                self.increment_rule_count(head_key);
            }
        }
    }

    /// Decrements the count of a rule if the symbol is a RuleRef.
    #[inline]
    fn decrement_if_rule(&mut self, key: DefaultKey) {
        if let Symbol::RuleRef { rule_id } = self.symbols[key].symbol {
            if let Some(&head_key) = self.rule_index.get(&rule_id) {
                self.decrement_rule_count(head_key);
            }
        }
    }

    /// Increments a rule's reference count.
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

    /// Decrements a rule's reference count.
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

/// Checks if a symbol marks the start of a sequence (RuleHead or DocHead).
#[inline(always)]
pub(crate) fn is_sequence_start<T>(symbol: &Symbol<T>) -> bool {
    matches!(symbol, Symbol::RuleHead { .. } | Symbol::DocHead { .. })
}

/// Checks if a symbol marks the end of a sequence (RuleTail or DocTail).
#[inline(always)]
pub(crate) fn is_sequence_end<T>(symbol: &Symbol<T>) -> bool {
    matches!(symbol, Symbol::RuleTail | Symbol::DocTail)
}
