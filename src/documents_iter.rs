use crate::documents::SequiturDocuments;
use crate::grammar::Grammar;
use crate::symbol::Symbol;
use slotmap::DefaultKey;
use std::hash::Hash;

/// Iterator over a single document in SequiturDocuments.
///
/// Expands RuleRefs recursively using a stack to reconstruct the original sequence.
pub struct DocumentIter<'a, T, DocId> {
    grammar: &'a Grammar<T>,
    current: Option<DefaultKey>,
    stack: Vec<DefaultKey>,
    _doc_id: std::marker::PhantomData<DocId>,
}

impl<'a, T: Hash + Eq + Clone, DocId: Hash + Eq + Clone> DocumentIter<'a, T, DocId> {
    /// Creates a new iterator for the specified document.
    ///
    /// Returns None if the document doesn't exist.
    pub(crate) fn new(sequitur: &'a SequiturDocuments<T, DocId>, doc_id: &DocId) -> Option<Self> {
        // Get document info
        let doc_info = sequitur
            .documents
            .get(doc_id)
            .expect("Document should exist");

        // Start from first symbol after DocHead
        let start = sequitur.grammar.symbols[doc_info.head]
            .next
            .expect("DocHead should have next");

        // Resolve forward through any rules
        let mut stack = Vec::new();
        let current = Self::resolve_forward(&sequitur.grammar, start, &mut stack);

        Some(Self {
            grammar: &sequitur.grammar,
            current,
            stack,
            _doc_id: std::marker::PhantomData,
        })
    }

    /// Resolves forward through RuleRefs to find the next Value symbol.
    ///
    /// Uses a stack to track positions within rules for proper iteration.
    fn resolve_forward(
        grammar: &'a Grammar<T>,
        mut key: DefaultKey,
        stack: &mut Vec<DefaultKey>,
    ) -> Option<DefaultKey> {
        loop {
            match &grammar.symbols[key].symbol {
                Symbol::Value(_) => return Some(key),

                Symbol::RuleRef { rule_id } => {
                    // Push current position to stack
                    stack.push(key);

                    // Jump to rule definition
                    let rule_head = *grammar
                        .rule_index
                        .get(rule_id)
                        .expect("Rule should exist in index");

                    // Move to first symbol in rule
                    key = grammar.symbols[rule_head]
                        .next
                        .expect("RuleHead should have next");
                }

                Symbol::RuleTail | Symbol::DocTail => {
                    // End of rule or document, pop from stack
                    if let Some(return_key) = stack.pop() {
                        // Return to position after RuleRef
                        key = grammar.symbols[return_key]
                            .next
                            .expect("RuleRef should have next");
                    } else {
                        // Stack empty, reached end
                        return None;
                    }
                }

                Symbol::RuleHead { .. } | Symbol::DocHead { .. } => {
                    // Skip past head
                    key = grammar.symbols[key].next.expect("Head should have next");
                }
            }
        }
    }
}

impl<'a, T: Hash + Eq + Clone, DocId: Hash + Eq + Clone> Iterator for DocumentIter<'a, T, DocId> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let current_key = self.current?;

        // Get the value
        let value = match &self.grammar.symbols[current_key].symbol {
            Symbol::Value(v) => v,
            _ => unreachable!("resolve_forward should only return Value symbols"),
        };

        // Move to next symbol
        let next_key = self.grammar.symbols[current_key].next?;
        self.current = Self::resolve_forward(self.grammar, next_key, &mut self.stack);

        Some(value)
    }
}

impl<T: Hash + Eq + Clone, DocId: Hash + Eq + Clone> SequiturDocuments<T, DocId> {
    /// Returns an iterator over the values in a specific document.
    ///
    /// Returns `None` if the document doesn't exist.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sequitur_rs::SequiturDocuments;
    ///
    /// let mut docs = SequiturDocuments::new();
    /// docs.extend_document("doc1", vec!['a', 'b', 'c']);
    ///
    /// let text: String = docs.iter_document(&"doc1").unwrap().collect();
    /// assert_eq!(text, "abc");
    /// ```
    pub fn iter_document(&self, doc_id: &DocId) -> Option<DocumentIter<'_, T, DocId>> {
        if !self.documents.contains_key(doc_id) {
            return None;
        }
        DocumentIter::new(self, doc_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iter_simple() {
        let mut docs = SequiturDocuments::new();
        docs.extend_document(1, vec!['a', 'b', 'c']);

        let result: Vec<_> = docs.iter_document(&1).unwrap().copied().collect();
        assert_eq!(result, vec!['a', 'b', 'c']);
    }

    #[test]
    fn test_iter_nonexistent() {
        let docs = SequiturDocuments::<char, u32>::new();
        assert!(docs.iter_document(&999).is_none());
    }

    #[test]
    fn test_iter_multiple_documents() {
        let mut docs = SequiturDocuments::new();
        docs.extend_document("a", vec!['x', 'y', 'z']);
        docs.extend_document("b", vec!['1', '2', '3']);

        let a_result: String = docs.iter_document(&"a").unwrap().collect();
        let b_result: String = docs.iter_document(&"b").unwrap().collect();

        assert_eq!(a_result, "xyz");
        assert_eq!(b_result, "123");
    }

    #[test]
    fn test_iter_with_shared_patterns_simple() {
        let mut docs = SequiturDocuments::new();

        // Start with a simpler test
        docs.extend_document(1, vec!['a', 'b']);
        docs.extend_document(2, vec!['a', 'b']);

        let result1: Vec<_> = docs.iter_document(&1).unwrap().copied().collect();
        let result2: Vec<_> = docs.iter_document(&2).unwrap().copied().collect();

        assert_eq!(result1, vec!['a', 'b']);
        assert_eq!(result2, vec!['a', 'b']);
    }

    #[test]
    fn test_iter_with_shared_patterns() {
        let mut docs = SequiturDocuments::new();

        // Create documents with shared patterns
        docs.extend_document(1, vec!['a', 'b', 'a', 'b']);
        docs.extend_document(2, vec!['a', 'b', 'c']);

        let result1: Vec<_> = docs.iter_document(&1).unwrap().copied().collect();
        let result2: Vec<_> = docs.iter_document(&2).unwrap().copied().collect();

        assert_eq!(result1, vec!['a', 'b', 'a', 'b']);
        assert_eq!(result2, vec!['a', 'b', 'c']);
    }
}
