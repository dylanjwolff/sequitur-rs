use crate::rle_documents::SequiturDocumentsRle;
use crate::rle_grammar::RleGrammar;
use crate::symbol::Symbol;
use slotmap::DefaultKey;
use std::hash::Hash;

/// Iterator over a single document in SequiturDocumentsRle.
///
/// Expands RuleRefs and run-length encoding during iteration.
pub struct RleDocumentIter<'a, T, DocId> {
    grammar: &'a RleGrammar<T>,
    current: Option<DefaultKey>,
    /// Remaining count for the current symbol's run
    remaining_run: u32,
    /// Stack for tracking rule expansion
    stack: Vec<StackEntry>,
    _doc_id: std::marker::PhantomData<DocId>,
}

/// Stack entry for tracking position during rule expansion.
struct StackEntry {
    key: DefaultKey,
    /// Remaining run count when we descended into a rule
    remaining_run: u32,
}

impl<'a, T: Hash + Eq + Clone, DocId: Hash + Eq + Clone> RleDocumentIter<'a, T, DocId> {
    /// Creates a new iterator for the specified document.
    pub(crate) fn new(
        sequitur: &'a SequiturDocumentsRle<T, DocId>,
        doc_id: &DocId,
    ) -> Option<Self> {
        let doc_info = sequitur.documents.get(doc_id)?;

        let start = sequitur.grammar.symbols[doc_info.head]
            .next
            .expect("DocHead should have next");

        let mut iter = Self {
            grammar: &sequitur.grammar,
            current: None,
            remaining_run: 0,
            stack: Vec::new(),
            _doc_id: std::marker::PhantomData,
        };

        iter.resolve_to_value(start);
        Some(iter)
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
                    let run = self.grammar.symbols[key].run;
                    self.stack.push(StackEntry {
                        key,
                        remaining_run: run,
                    });

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
                    key = self.grammar.symbols[key]
                        .next
                        .expect("Head should have next");
                }

                Symbol::RuleTail => {
                    if let Some(entry) = self.stack.pop() {
                        let new_remaining = entry.remaining_run - 1;
                        if new_remaining > 0 {
                            self.stack.push(StackEntry {
                                key: entry.key,
                                remaining_run: new_remaining,
                            });

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

                        if let Some(next) = self.grammar.symbols[entry.key].next {
                            key = next;
                            continue;
                        }
                    }

                    self.current = None;
                    self.remaining_run = 0;
                    return;
                }

                Symbol::DocTail => {
                    // End of document - but check if we're inside a rule
                    if let Some(entry) = self.stack.pop() {
                        if let Some(next) = self.grammar.symbols[entry.key].next {
                            key = next;
                            continue;
                        }
                    }
                    self.current = None;
                    self.remaining_run = 0;
                    return;
                }
            }
        }
    }

    /// Advances to the next value.
    fn advance(&mut self) {
        if self.remaining_run > 1 {
            self.remaining_run -= 1;
            return;
        }

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

impl<'a, T: Hash + Eq + Clone, DocId: Hash + Eq + Clone> Iterator
    for RleDocumentIter<'a, T, DocId>
{
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

impl<T: Hash + Eq + Clone, DocId: Hash + Eq + Clone> SequiturDocumentsRle<T, DocId> {
    /// Returns an iterator over the values in a specific document.
    ///
    /// Returns `None` if the document doesn't exist.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sequitur_rs::SequiturDocumentsRle;
    ///
    /// let mut docs = SequiturDocumentsRle::new();
    /// docs.extend_document("doc1", vec!['a', 'b', 'c']);
    ///
    /// let text: String = docs.iter_document(&"doc1").unwrap().collect();
    /// assert_eq!(text, "abc");
    /// ```
    pub fn iter_document(&self, doc_id: &DocId) -> Option<RleDocumentIter<'_, T, DocId>> {
        RleDocumentIter::new(self, doc_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iter_simple() {
        let mut docs = SequiturDocumentsRle::new();
        docs.extend_document(1, vec!['a', 'b', 'c']);

        let result: Vec<_> = docs.iter_document(&1).unwrap().copied().collect();
        assert_eq!(result, vec!['a', 'b', 'c']);
    }

    #[test]
    fn test_iter_nonexistent() {
        let docs = SequiturDocumentsRle::<char, u32>::new();
        assert!(docs.iter_document(&999).is_none());
    }

    #[test]
    fn test_iter_with_runs() {
        let mut docs = SequiturDocumentsRle::new();
        docs.extend_document(1, vec!['a', 'a', 'a', 'b', 'b']);

        let result: Vec<_> = docs.iter_document(&1).unwrap().copied().collect();
        assert_eq!(result, vec!['a', 'a', 'a', 'b', 'b']);
    }

    #[test]
    fn test_iter_multiple_documents() {
        let mut docs = SequiturDocumentsRle::new();
        docs.extend_document("a", vec!['x', 'y', 'z']);
        docs.extend_document("b", vec!['1', '2', '3']);

        let a_result: String = docs.iter_document(&"a").unwrap().collect();
        let b_result: String = docs.iter_document(&"b").unwrap().collect();

        assert_eq!(a_result, "xyz");
        assert_eq!(b_result, "123");
    }

    #[test]
    fn test_iter_with_shared_patterns() {
        let mut docs = SequiturDocumentsRle::new();

        docs.extend_document(1, vec!['a', 'b', 'a', 'b']);
        docs.extend_document(2, vec!['a', 'b', 'c']);

        let result1: Vec<_> = docs.iter_document(&1).unwrap().copied().collect();
        let result2: Vec<_> = docs.iter_document(&2).unwrap().copied().collect();

        assert_eq!(result1, vec!['a', 'b', 'a', 'b']);
        assert_eq!(result2, vec!['a', 'b', 'c']);
    }

    #[test]
    fn test_iter_long_runs() {
        let mut docs = SequiturDocumentsRle::new();

        // Add a long run
        for _ in 0..100 {
            docs.push_to_document(1, 'x');
        }

        let result: Vec<_> = docs.iter_document(&1).unwrap().copied().collect();
        assert_eq!(result.len(), 100);
        assert!(result.iter().all(|&c| c == 'x'));
    }

    #[test]
    fn test_iter_documents_with_runs() {
        let mut docs = SequiturDocumentsRle::new();

        // Document 1: "aaabbb"
        docs.extend_document(1, "aaabbb".chars());

        // Document 2: "aaaccc"
        docs.extend_document(2, "aaaccc".chars());

        let result1: String = docs.iter_document(&1).unwrap().collect();
        let result2: String = docs.iter_document(&2).unwrap().collect();

        assert_eq!(result1, "aaabbb");
        assert_eq!(result2, "aaaccc");
    }
}
