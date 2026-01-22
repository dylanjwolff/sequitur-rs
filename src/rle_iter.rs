use crate::rle_grammar::RleGrammar;
use crate::rle_sequitur::SequiturRle;
use crate::symbol::Symbol;
use slotmap::DefaultKey;
use std::hash::Hash;

/// Iterator that reconstructs the original sequence from RLE-Sequitur.
///
/// Expands run-length encoded symbols during iteration.
pub struct RleSequiturIter<'a, T> {
    grammar: &'a RleGrammar<T>,
    current: Option<DefaultKey>,
    /// Remaining count for the current symbol's run
    remaining_run: u32,
    /// Stack for tracking rule expansion
    stack: Vec<StackEntry>,
}

/// Stack entry for tracking position during rule expansion.
struct StackEntry {
    key: DefaultKey,
    /// Remaining run count when we descended into a rule
    remaining_run: u32,
}

impl<'a, T: Hash + Eq + Clone> RleSequiturIter<'a, T> {
    pub(crate) fn new(sequitur: &'a SequiturRle<T>) -> Self {
        let rule_0_head = *sequitur.rules().get(&0).expect("Rule 0 should exist");
        let start = sequitur.grammar.symbols[rule_0_head]
            .next
            .expect("Rule 0 should have content");

        let mut iter = Self {
            grammar: &sequitur.grammar,
            current: None,
            remaining_run: 0,
            stack: Vec::new(),
        };

        // Resolve to first Value
        iter.resolve_to_value(start);
        iter
    }

    /// Resolves forward through the grammar to find the next Value symbol.
    fn resolve_to_value(&mut self, mut key: DefaultKey) {
        loop {
            match &self.grammar.symbols[key].symbol {
                Symbol::Value(_) => {
                    self.current = Some(key);
                    self.remaining_run = self.grammar.symbols[key].run;
                    return;
                }

                Symbol::RuleRef { rule_id } => {
                    // Push current position with remaining run
                    let run = self.grammar.symbols[key].run;
                    self.stack.push(StackEntry {
                        key,
                        remaining_run: run,
                    });

                    // Descend into rule
                    let rule_head = *self
                        .grammar
                        .rule_index
                        .get(rule_id)
                        .expect("Rule should exist");
                    key = self.grammar.symbols[rule_head]
                        .next
                        .expect("Rule should have content");
                }

                Symbol::RuleHead { .. } | Symbol::DocHead { .. } => {
                    // Skip past head
                    key = self.grammar.symbols[key]
                        .next
                        .expect("Head should have next");
                }

                Symbol::RuleTail => {
                    // End of rule, pop stack
                    if let Some(entry) = self.stack.pop() {
                        // Decrement remaining run for the RuleRef
                        let new_remaining = entry.remaining_run - 1;
                        if new_remaining > 0 {
                            // Re-enter the same rule
                            self.stack.push(StackEntry {
                                key: entry.key,
                                remaining_run: new_remaining,
                            });

                            // Go back to the rule's first symbol
                            if let Symbol::RuleRef { rule_id } =
                                self.grammar.symbols[entry.key].symbol
                            {
                                let rule_head = *self
                                    .grammar
                                    .rule_index
                                    .get(&rule_id)
                                    .expect("Rule should exist");
                                key = self.grammar.symbols[rule_head]
                                    .next
                                    .expect("Rule should have content");
                                continue;
                            }
                        }

                        // Move to next symbol after the RuleRef
                        if let Some(next) = self.grammar.symbols[entry.key].next {
                            key = next;
                            continue;
                        }
                    }

                    // End of iteration
                    self.current = None;
                    self.remaining_run = 0;
                    return;
                }

                Symbol::DocTail => {
                    // End of document
                    self.current = None;
                    self.remaining_run = 0;
                    return;
                }
            }
        }
    }

    /// Advances to the next value.
    fn advance(&mut self) {
        // First check if we have more in the current run
        if self.remaining_run > 1 {
            self.remaining_run -= 1;
            return;
        }

        // Move to next symbol
        let Some(current) = self.current else {
            return;
        };

        if let Some(next) = self.grammar.symbols[current].next {
            self.resolve_to_value(next);
        } else {
            self.current = None;
            self.remaining_run = 0;
        }
    }
}

impl<'a, T: Hash + Eq + Clone> Iterator for RleSequiturIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let current_key = self.current?;

        let value = match &self.grammar.symbols[current_key].symbol {
            Symbol::Value(v) => v,
            _ => unreachable!("current should always be a Value symbol"),
        };

        self.advance();

        Some(value)
    }
}

impl<T: Hash + Eq + Clone> SequiturRle<T> {
    /// Returns an iterator over the reconstructed sequence.
    pub fn iter(&self) -> RleSequiturIter<'_, T> {
        RleSequiturIter::new(self)
    }
}

impl<'a, T: Hash + Eq + Clone> IntoIterator for &'a SequiturRle<T> {
    type Item = &'a T;
    type IntoIter = RleSequiturIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iter_empty() {
        let seq = SequiturRle::<char>::new();
        let collected: Vec<&char> = seq.iter().collect();
        assert_eq!(collected.len(), 0);
    }

    #[test]
    fn test_iter_single() {
        let mut seq = SequiturRle::new();
        seq.push('a');
        let collected: Vec<&char> = seq.iter().collect();
        assert_eq!(collected, vec![&'a']);
    }

    #[test]
    fn test_iter_run() {
        let mut seq = SequiturRle::new();
        seq.push('a');
        seq.push('a');
        seq.push('a');

        let collected: Vec<char> = seq.iter().copied().collect();
        assert_eq!(collected, vec!['a', 'a', 'a']);
    }

    #[test]
    fn test_iter_multiple() {
        let mut seq = SequiturRle::new();
        seq.extend(vec!['a', 'b', 'c']);
        let collected: Vec<&char> = seq.iter().collect();
        assert_eq!(collected, vec![&'a', &'b', &'c']);
    }

    #[test]
    fn test_iter_with_repetition() {
        let mut seq = SequiturRle::new();
        seq.extend(vec!['a', 'b', 'a', 'b']);
        let collected: Vec<&char> = seq.iter().collect();
        assert_eq!(collected, vec![&'a', &'b', &'a', &'b']);
    }

    #[test]
    fn test_iter_long_run() {
        let mut seq = SequiturRle::new();
        for _ in 0..10 {
            seq.push('x');
        }

        let collected: Vec<char> = seq.iter().copied().collect();
        assert_eq!(collected, vec!['x'; 10]);
    }

    #[test]
    fn test_iter_mixed_runs() {
        let mut seq = SequiturRle::new();

        // aaabbbccc
        for _ in 0..3 {
            seq.push('a');
        }
        for _ in 0..3 {
            seq.push('b');
        }
        for _ in 0..3 {
            seq.push('c');
        }

        let collected: String = seq.iter().collect();
        assert_eq!(collected, "aaabbbccc");
    }

    #[test]
    fn test_into_iterator() {
        let mut seq = SequiturRle::new();
        seq.extend(vec![1, 2, 3]);
        let collected: Vec<&i32> = (&seq).into_iter().collect();
        assert_eq!(collected, vec![&1, &2, &3]);
    }
}
