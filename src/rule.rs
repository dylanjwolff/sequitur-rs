use crate::sequitur::Sequitur;
use crate::symbol::{Symbol, SymbolNode};
use slotmap::DefaultKey;
use std::hash::Hash;

impl<T: Hash + Eq + Clone> Sequitur<T> {
    /// Checks if a digram is a complete rule (surrounded by RuleHead and RuleTail).
    ///
    /// Returns Some(RuleHead key) if the digram forms a complete rule.
    pub(crate) fn get_complete_rule(&self, first: DefaultKey) -> Option<DefaultKey> {
        let second = self.symbols[first].next?;

        // Check if preceded by RuleHead
        let prev = self.symbols[first].prev?;
        let is_head = matches!(self.symbols[prev].symbol, Symbol::RuleHead { .. });

        if !is_head {
            return None;
        }

        // Check if followed by RuleTail (need to look two positions ahead)
        let after_second = self.symbols[second].next?;
        let is_tail = matches!(self.symbols[after_second].symbol, Symbol::RuleTail);

        if is_tail {
            Some(prev)
        } else {
            None
        }
    }

    /// Creates a new rule from two digram occurrences.
    ///
    /// Returns the keys where the new RuleSymbols were inserted.
    pub(crate) fn swap_for_new_rule(
        &mut self,
        match1: DefaultKey,
        match2: DefaultKey,
    ) -> (DefaultKey, DefaultKey) {
        assert!(
            self.symbols[match1].next.is_some(),
            "match1 should have next"
        );
        assert!(
            self.symbols[match2].next.is_some(),
            "match2 should have next"
        );

        let match1_second = self.symbols[match1].next.unwrap();
        let _match2_second = self.symbols[match2].next.unwrap();

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

        // Clone the digram symbols into the rule
        let rule_first = self
            .symbols
            .insert(SymbolNode::new(self.symbols[match1].symbol.clone_symbol()));
        let rule_second = self.symbols.insert(SymbolNode::new(
            self.symbols[match1_second].symbol.clone_symbol(),
        ));

        // Link rule structure: head -> first -> second -> tail
        self.symbols[head_key].next = Some(rule_first);
        self.symbols[rule_first].prev = Some(head_key);
        self.symbols[rule_first].next = Some(rule_second);
        self.symbols[rule_second].prev = Some(rule_first);
        self.symbols[rule_second].next = Some(tail_key);
        self.symbols[tail_key].prev = Some(rule_second);

        // Update digram index to point to rule's copy
        let digram_key = self.make_digram_key(match1, match1_second);
        self.digram_index.insert(digram_key, rule_first);

        // Add rule to rule index
        self.rule_index.insert(rule_id, head_key);

        // Increment counts if the symbols in the rule are RuleRefs
        self.increment_if_rule(rule_first);
        self.increment_if_rule(rule_second);

        // Replace both occurrences with RuleSymbols
        let loc1 = self.swap_for_existing_rule(match1, head_key);
        let loc2 = self.swap_for_existing_rule(match2, head_key);

        (loc1, loc2)
    }

    /// Replaces a digram with an existing rule reference.
    ///
    /// Returns the key of the newly inserted RuleSymbol.
    pub(crate) fn swap_for_existing_rule(
        &mut self,
        first: DefaultKey,
        rule_head: DefaultKey,
    ) -> DefaultKey {
        let second = self.symbols[first]
            .next
            .expect("first should have next in digram");

        assert!(
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
    pub(crate) fn expand_rule_if_necessary(&mut self, potential_rule: DefaultKey) {
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

        assert!(count > 0, "Rule count should never be 0");

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
            if !matches!(self.symbols[prev].symbol, Symbol::RuleHead { .. }) {
                self.link_made(prev);
            }
        }

        // Check digram at rule_last if valid
        if let Some(after) = after_rule {
            if !matches!(self.symbols[after].symbol, Symbol::RuleTail) {
                self.link_made(rule_last);
            }
        }
    }

    /// Checks newly formed links after rule insertion.
    pub(crate) fn check_new_links(&mut self, rule_key: DefaultKey) {
        // Check digram starting at rule_key
        if let Some(next) = self.symbols[rule_key].next {
            if !matches!(self.symbols[next].symbol, Symbol::RuleTail)
                && !matches!(self.symbols[rule_key].symbol, Symbol::RuleHead { .. })
            {
                self.link_made(rule_key);
            }
        }

        // Check digram before rule_key
        if let Some(prev) = self.symbols[rule_key].prev {
            if !matches!(self.symbols[prev].symbol, Symbol::RuleHead { .. }) {
                self.link_made(prev);
            }
        }
    }

    /// Checks newly formed links after two rule insertions.
    pub(crate) fn check_new_links_pair(&mut self, rule1: DefaultKey, rule2: DefaultKey) {
        // Check at rule1
        if let Some(next) = self.symbols[rule1].next {
            if !matches!(self.symbols[next].symbol, Symbol::RuleTail)
                && !matches!(self.symbols[rule1].symbol, Symbol::RuleHead { .. })
            {
                self.link_made(rule1);
            }
        }

        // Check at rule2
        if let Some(next) = self.symbols[rule2].next {
            if !matches!(self.symbols[next].symbol, Symbol::RuleTail)
                && !matches!(self.symbols[rule2].symbol, Symbol::RuleHead { .. })
            {
                self.link_made(rule2);
            }
        }

        // Check before rule2
        if let Some(prev) = self.symbols[rule2].prev {
            if prev != rule1 && !matches!(self.symbols[prev].symbol, Symbol::RuleHead { .. }) {
                self.link_made(prev);
            }
        }

        // Check before rule1
        if let Some(prev) = self.symbols[rule1].prev {
            if prev != rule2 && !matches!(self.symbols[prev].symbol, Symbol::RuleHead { .. }) {
                self.link_made(prev);
            }
        }
    }

    /// Increments the count of a rule if the symbol is a RuleRef.
    fn increment_if_rule(&mut self, key: DefaultKey) {
        if let Symbol::RuleRef { rule_id } = self.symbols[key].symbol {
            if let Some(&head_key) = self.rule_index.get(&rule_id) {
                self.increment_rule_count(head_key);
            }
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

    /// Increments a rule's reference count.
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
    fn decrement_rule_count(&mut self, head_key: DefaultKey) {
        if let Symbol::RuleHead {
            rule_id,
            count,
            tail,
        } = self.symbols[head_key].symbol
        {
            assert!(count > 0, "Cannot decrement count below 0");
            self.symbols[head_key].symbol = Symbol::RuleHead {
                rule_id,
                count: count - 1,
                tail,
            };
        }
    }
}
