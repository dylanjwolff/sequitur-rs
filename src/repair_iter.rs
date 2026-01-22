//! Iterator for reconstructing sequences from RePair grammars.

use crate::repair::Repair;
use crate::symbol::Symbol;
use slotmap::DefaultKey;
use std::hash::Hash;

/// Iterator that reconstructs the original sequence from a RePair grammar.
///
/// Uses a stack to track rule expansion depth, similar to SequiturIter.
pub struct RepairIter<'a, T> {
    repair: &'a Repair<T>,
    current: Option<DefaultKey>,
    stack: Vec<DefaultKey>,
}

impl<'a, T: Hash + Eq + Clone> RepairIter<'a, T> {
    pub(crate) fn new(repair: &'a Repair<T>) -> Self {
        // Start at Rule 0's first symbol
        let rule_0_head = *repair.rules().get(&0).expect("Rule 0 should exist");
        let start = repair.symbols[rule_0_head]
            .next
            .expect("Rule 0 should have content");

        let mut stack = Vec::new();
        let current = Self::resolve_forward(repair, start, &mut stack);

        Self {
            repair,
            current,
            stack,
        }
    }

    /// Resolves forward through rules to find the next Value symbol.
    fn resolve_forward(
        repair: &Repair<T>,
        key: DefaultKey,
        stack: &mut Vec<DefaultKey>,
    ) -> Option<DefaultKey> {
        match &repair.symbols[key].symbol {
            Symbol::Value(_) => Some(key),

            Symbol::RuleRef { rule_id } => {
                // Push current position and descend into rule
                stack.push(key);
                let rule_head = *repair.rule_index.get(rule_id)?;
                let rule_first = repair.symbols[rule_head].next?;
                Self::resolve_forward(repair, rule_first, stack)
            }

            Symbol::RuleHead { .. } => {
                // Skip past RuleHead
                let next = repair.symbols[key].next?;
                Self::resolve_forward(repair, next, stack)
            }

            Symbol::RuleTail => {
                // End of rule, pop stack and continue
                if let Some(parent) = stack.pop() {
                    let next = repair.symbols[parent].next?;
                    Self::resolve_forward(repair, next, stack)
                } else {
                    // End of iteration
                    None
                }
            }

            Symbol::DocHead { .. } => {
                // Skip past DocHead (shouldn't appear, but handle defensively)
                let next = repair.symbols[key].next?;
                Self::resolve_forward(repair, next, stack)
            }

            Symbol::DocTail => {
                // End of document (shouldn't appear, but handle defensively)
                None
            }
        }
    }
}

impl<'a, T: Hash + Eq + Clone> Iterator for RepairIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let current_key = self.current?;

        // Extract the value
        let value = match &self.repair.symbols[current_key].symbol {
            Symbol::Value(v) => v,
            _ => unreachable!("resolve_forward should only return Value symbols"),
        };

        // Move to next symbol
        let next_key = self.repair.symbols[current_key].next?;
        self.current = Self::resolve_forward(self.repair, next_key, &mut self.stack);

        Some(value)
    }
}

impl<T: Hash + Eq + Clone> Repair<T> {
    /// Returns an iterator over the reconstructed sequence.
    ///
    /// The iterator expands all rules to reconstruct the original input.
    pub fn iter(&self) -> RepairIter<'_, T> {
        RepairIter::new(self)
    }
}

impl<'a, T: Hash + Eq + Clone> IntoIterator for &'a Repair<T> {
    type Item = &'a T;
    type IntoIter = RepairIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iter_empty() {
        let repair = Repair::<char>::new();
        let collected: Vec<&char> = repair.iter().collect();
        assert_eq!(collected.len(), 0);
    }

    #[test]
    fn test_iter_single() {
        let mut repair = Repair::new();
        repair.push('a');
        let collected: Vec<&char> = repair.iter().collect();
        assert_eq!(collected, vec![&'a']);
    }

    #[test]
    fn test_iter_multiple() {
        let mut repair = Repair::new();
        repair.extend(vec!['a', 'b', 'c']);
        let collected: Vec<&char> = repair.iter().collect();
        assert_eq!(collected, vec![&'a', &'b', &'c']);
    }

    #[test]
    fn test_iter_uncompressed() {
        let mut repair = Repair::new();
        repair.extend(vec!['a', 'b', 'a', 'b']);
        // Without compression, should still iterate correctly
        let collected: Vec<&char> = repair.iter().collect();
        assert_eq!(collected, vec![&'a', &'b', &'a', &'b']);
    }

    #[test]
    fn test_iter_compressed() {
        let mut repair = Repair::new();
        repair.extend(vec!['a', 'b', 'a', 'b']);
        repair.compress();
        // After compression, should still reconstruct correctly
        let collected: Vec<&char> = repair.iter().collect();
        assert_eq!(collected, vec![&'a', &'b', &'a', &'b']);
    }

    #[test]
    fn test_iter_with_nested_rules() {
        let mut repair = Repair::new();
        repair.extend("abcabcabcabc".chars());
        repair.compress();
        let collected: String = repair.iter().collect();
        assert_eq!(collected, "abcabcabcabc");
    }

    #[test]
    fn test_into_iterator() {
        let mut repair = Repair::new();
        repair.extend(vec![1, 2, 3]);
        let collected: Vec<&i32> = (&repair).into_iter().collect();
        assert_eq!(collected, vec![&1, &2, &3]);
    }
}
