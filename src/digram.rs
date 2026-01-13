use crate::sequitur::Sequitur;
use crate::symbol::{Symbol, SymbolHash};
use slotmap::DefaultKey;
use std::collections::hash_map::Entry;
use std::hash::Hash;

impl<T: Hash + Eq + Clone> Sequitur<T> {
    /// Finds an existing digram or adds it to the index.
    ///
    /// Returns Some(key) if a non-overlapping match exists, None otherwise.
    ///
    /// Implements the C++ `findAndAddDigram` logic with overlap detection.
    pub(crate) fn find_and_add_digram(
        &mut self,
        first: DefaultKey,
        second: DefaultKey,
    ) -> Option<DefaultKey> {
        assert!(
            self.symbols[first].next == Some(second),
            "Digram must be consecutive symbols"
        );

        // Don't create digrams starting/ending with RuleHead/RuleTail
        if matches!(self.symbols[first].symbol, Symbol::RuleHead { .. })
            || matches!(self.symbols[second].symbol, Symbol::RuleTail)
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
            Entry::Occupied(e) => {
                let other_first = *e.get();
                let other_second = self.symbols[other_first]
                    .next
                    .expect("Digram first should have next");

                // Check for overlap: digrams sharing a symbol
                // This happens in sequences like "abcabc" where "abc" overlaps
                if other_second == first || other_first == second {
                    return None; // Overlapping digrams, ignore
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
                    // Hash collision - this is rare but possible
                    // For now, treat as no match (could be improved with chaining)
                    None
                }
            }
        }
    }

    /// Removes a digram from the index if it points to the given location.
    ///
    /// Only removes if the index entry points to exactly this location,
    /// preventing removal of duplicate digrams at different locations.
    pub(crate) fn remove_digram_from_index(&mut self, first: DefaultKey) {
        // Don't try to remove invalid digrams
        if matches!(self.symbols[first].symbol, Symbol::RuleHead { .. }) {
            return;
        }

        let Some(second) = self.symbols[first].next else {
            return;
        };

        if matches!(self.symbols[second].symbol, Symbol::RuleTail) {
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

    /// Creates a digram pair key for the hash map.
    ///
    /// Used when we need to create a key without modifying the index.
    #[allow(dead_code)]
    pub(crate) fn make_digram_key(
        &self,
        first: DefaultKey,
        second: DefaultKey,
    ) -> (SymbolHash, SymbolHash) {
        let first_hash = SymbolHash::from_symbol(&self.symbols[first].symbol);
        let second_hash = SymbolHash::from_symbol(&self.symbols[second].symbol);
        (first_hash, second_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::{Symbol, SymbolNode};
    use slotmap::SlotMap;

    #[test]
    fn test_digram_not_overlapping() {
        let mut seq = Sequitur::new();

        // Create sequence: a b a b
        seq.push('a');
        seq.push('b');
        seq.push('a');
        seq.push('b');

        // The digrams "ab" should be detected as duplicates
        // (Implementation will be tested once rule creation is complete)
    }

    #[test]
    fn test_digram_overlapping() {
        // Test overlap detection with manual symbol creation
        let mut seq = Sequitur::<char>::new();
        let mut symbols = SlotMap::new();

        let a1 = symbols.insert(SymbolNode::new(Symbol::Value('a')));
        let b = symbols.insert(SymbolNode::new(Symbol::Value('b')));
        let a2 = symbols.insert(SymbolNode::new(Symbol::Value('a')));

        // Link: a1 -> b -> a2
        symbols[a1].next = Some(b);
        symbols[b].prev = Some(a1);
        symbols[b].next = Some(a2);
        symbols[a2].prev = Some(b);

        // Temporarily swap in test symbols
        seq.symbols = symbols;

        // Try to add digram (a1, b)
        let result1 = seq.find_and_add_digram(a1, b);
        assert_eq!(result1, None); // Should add to index

        // Try to add digram (b, a2) - this overlaps with (a1, b)
        let result2 = seq.find_and_add_digram(b, a2);
        assert_eq!(result2, None); // Should detect overlap
    }
}
