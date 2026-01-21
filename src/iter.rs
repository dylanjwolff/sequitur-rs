use crate::sequitur::Sequitur;
use crate::symbol::{Symbol, SymbolNode};
use ahash::AHashMap as HashMap;
use slotmap::{DefaultKey, SlotMap};
use std::hash::Hash;

/// Iterator that reconstructs the original sequence by expanding rules.
///
/// Uses a stack to track rule expansion depth, matching the C++ implementation.
pub struct SequiturIter<'a, T> {
    symbols: &'a SlotMap<DefaultKey, SymbolNode<T>>,
    rule_index: &'a HashMap<u32, DefaultKey>,
    current: Option<DefaultKey>,
    stack: Vec<DefaultKey>,
}

impl<'a, T: Hash + Eq + Clone> SequiturIter<'a, T> {
    pub(crate) fn new(sequitur: &'a Sequitur<T>) -> Self {
        // Start at Rule 0's first symbol
        let rule_0_head = *sequitur.rules().get(&0).expect("Rule 0 should exist");
        let start = sequitur.symbols[rule_0_head]
            .next
            .expect("Rule 0 should have content");

        let mut stack = Vec::new();
        let current =
            Self::resolve_forward(&sequitur.symbols, &sequitur.rule_index, start, &mut stack);

        Self {
            symbols: &sequitur.symbols,
            rule_index: &sequitur.rule_index,
            current,
            stack,
        }
    }

    /// Resolves forward through rules to find the next Value symbol.
    ///
    /// Matches the C++ `resolveForward` logic.
    fn resolve_forward(
        symbols: &SlotMap<DefaultKey, SymbolNode<T>>,
        rule_index: &HashMap<u32, DefaultKey>,
        key: DefaultKey,
        stack: &mut Vec<DefaultKey>,
    ) -> Option<DefaultKey> {
        match &symbols[key].symbol {
            Symbol::Value(_) => Some(key),

            Symbol::RuleRef { rule_id } => {
                // Push current position and descend into rule
                stack.push(key);
                let rule_head = *rule_index.get(rule_id)?;
                let rule_first = symbols[rule_head].next?;
                Self::resolve_forward(symbols, rule_index, rule_first, stack)
            }

            Symbol::RuleHead { .. } => {
                // Skip past RuleHead
                let next = symbols[key].next?;
                Self::resolve_forward(symbols, rule_index, next, stack)
            }

            Symbol::RuleTail => {
                // End of rule, pop stack and continue
                if let Some(parent) = stack.pop() {
                    let next = symbols[parent].next?;
                    Self::resolve_forward(symbols, rule_index, next, stack)
                } else {
                    // End of iteration
                    None
                }
            }

            Symbol::DocHead { .. } => {
                // Skip past DocHead (shouldn't appear in Rule 0, but handle defensively)
                let next = symbols[key].next?;
                Self::resolve_forward(symbols, rule_index, next, stack)
            }

            Symbol::DocTail => {
                // End of document (shouldn't appear in Rule 0, but handle defensively)
                None
            }
        }
    }
}

impl<'a, T: Hash + Eq + Clone> Iterator for SequiturIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let current_key = self.current?;

        // Extract the value
        let value = match &self.symbols[current_key].symbol {
            Symbol::Value(v) => v,
            _ => unreachable!("resolve_forward should only return Value symbols"),
        };

        // Move to next symbol
        let next_key = self.symbols[current_key].next?;
        self.current =
            Self::resolve_forward(self.symbols, self.rule_index, next_key, &mut self.stack);

        Some(value)
    }
}

impl<T: Hash + Eq + Clone> Sequitur<T> {
    /// Returns an iterator over the reconstructed sequence.
    pub fn iter(&self) -> SequiturIter<'_, T> {
        SequiturIter::new(self)
    }
}

impl<'a, T: Hash + Eq + Clone> IntoIterator for &'a Sequitur<T> {
    type Item = &'a T;
    type IntoIter = SequiturIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iter_empty() {
        let seq = Sequitur::<char>::new();
        let collected: Vec<&char> = seq.iter().collect();
        assert_eq!(collected.len(), 0);
    }

    #[test]
    fn test_iter_single() {
        let mut seq = Sequitur::new();
        seq.push('a');
        let collected: Vec<&char> = seq.iter().collect();
        assert_eq!(collected, vec![&'a']);
    }

    #[test]
    fn test_iter_multiple() {
        let mut seq = Sequitur::new();
        seq.extend(vec!['a', 'b', 'c']);
        let collected: Vec<&char> = seq.iter().collect();
        assert_eq!(collected, vec![&'a', &'b', &'c']);
    }

    #[test]
    fn test_iter_with_repetition() {
        let mut seq = Sequitur::new();
        seq.extend(vec!['a', 'b', 'a', 'b']);
        let collected: Vec<&char> = seq.iter().collect();
        assert_eq!(collected, vec![&'a', &'b', &'a', &'b']);
    }

    #[test]
    fn test_into_iterator() {
        let mut seq = Sequitur::new();
        seq.extend(vec![1, 2, 3]);
        let collected: Vec<&i32> = (&seq).into_iter().collect();
        assert_eq!(collected, vec![&1, &2, &3]);
    }
}
