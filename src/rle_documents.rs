use crate::rle_grammar::RleGrammar;
use crate::rle_symbol::RleSymbolNode;
use crate::symbol::Symbol;
use ahash::AHashMap as HashMap;
use slotmap::DefaultKey;
use std::hash::Hash;

/// Per-document metadata tracking the document's symbol sequence.
#[derive(Debug, Clone)]
pub(crate) struct RleDocumentInfo {
    /// Key to the DocHead for this document
    pub(crate) head: DefaultKey,
    /// Key to the DocTail for this document
    pub(crate) tail: DefaultKey,
    /// Number of values in this document (counting run lengths)
    pub(crate) length: usize,
}

/// Multi-document RLE-Sequitur compression with shared grammar.
///
/// Combines the benefits of multi-document compression (shared grammar across documents)
/// with run-length encoding for efficient handling of repeated symbols.
///
/// # Type Parameters
///
/// * `T` - The terminal symbol type (must be Hash + Eq + Clone)
/// * `DocId` - The document identifier type (must be Hash + Eq + Clone)
///
/// # Example
///
/// ```rust
/// use sequitur_rs::SequiturDocumentsRle;
///
/// let mut docs = SequiturDocumentsRle::<char, &str>::new();
///
/// // Add documents with repeated patterns
/// docs.extend_document("doc1", "aaabbbccc".chars());
/// docs.extend_document("doc2", "aaabbbddd".chars());
///
/// // Grammar is shared, each document can be iterated independently
/// let text1: String = docs.iter_document(&"doc1").unwrap().collect();
/// let text2: String = docs.iter_document(&"doc2").unwrap().collect();
///
/// assert_eq!(text1, "aaabbbccc");
/// assert_eq!(text2, "aaabbbddd");
/// ```
pub struct SequiturDocumentsRle<T, DocId> {
    /// Core RLE grammar storage
    pub(crate) grammar: RleGrammar<T>,

    /// Per-document sequences
    pub(crate) documents: HashMap<DocId, RleDocumentInfo>,
}

impl<T: Hash + Eq + Clone, DocId: Hash + Eq + Clone> SequiturDocumentsRle<T, DocId> {
    /// Creates a new empty SequiturDocumentsRle instance.
    pub fn new() -> Self {
        Self {
            grammar: RleGrammar::new(),
            documents: HashMap::default(),
        }
    }

    /// Adds a value to the specified document.
    ///
    /// If the document doesn't exist, it is created automatically.
    /// If the previous symbol in the document is the same value, its run count
    /// is incremented instead of creating a new node.
    pub fn push_to_document(&mut self, doc_id: DocId, value: T) {
        // Ensure document exists
        if !self.documents.contains_key(&doc_id) {
            self.create_document(doc_id.clone());
        }

        // Get document info
        let doc_info = self.documents.get(&doc_id).unwrap();
        let tail_key = doc_info.tail;
        let prev_key = self.grammar.symbols[tail_key].prev;

        // Check if we can extend the previous symbol's run
        if let Some(prev) = prev_key {
            if let Symbol::Value(ref prev_val) = self.grammar.symbols[prev].symbol {
                if prev_val == &value {
                    // Same value - just increment the run count
                    self.grammar.symbols[prev].run += 1;
                    self.documents.get_mut(&doc_id).unwrap().length += 1;
                    return;
                }
            }
        }

        // Different value or first symbol - create new node
        let new_key = self
            .grammar
            .symbols
            .insert(RleSymbolNode::new(Symbol::Value(value)));

        // Link new symbol into the list before DocTail
        self.grammar.symbols[new_key].next = Some(tail_key);
        self.grammar.symbols[new_key].prev = prev_key;
        self.grammar.symbols[tail_key].prev = Some(new_key);

        if let Some(prev) = prev_key {
            self.grammar.symbols[prev].next = Some(new_key);
        }

        self.documents.get_mut(&doc_id).unwrap().length += 1;

        // If not the first symbol, check for digram
        if let Some(prev) = prev_key {
            if !matches!(self.grammar.symbols[prev].symbol, Symbol::DocHead { .. }) {
                self.grammar.link_made(prev);
            }
        }
    }

    /// Extends the document with multiple values.
    pub fn extend_document<I: IntoIterator<Item = T>>(&mut self, doc_id: DocId, iter: I) {
        for value in iter {
            self.push_to_document(doc_id.clone(), value);
        }
    }

    /// Returns the number of values in a document (counting run lengths).
    pub fn document_len(&self, doc_id: &DocId) -> Option<usize> {
        self.documents.get(doc_id).map(|info| info.length)
    }

    /// Returns true if the document exists and is empty.
    pub fn document_is_empty(&self, doc_id: &DocId) -> Option<bool> {
        self.documents.get(doc_id).map(|info| info.length == 0)
    }

    /// Returns an iterator over all document IDs.
    pub fn document_ids(&self) -> impl Iterator<Item = &DocId> {
        self.documents.keys()
    }

    /// Returns the number of documents.
    pub fn num_documents(&self) -> usize {
        self.documents.len()
    }

    /// Returns a reference to the rule index (shared across all documents).
    pub fn rules(&self) -> &HashMap<u32, DefaultKey> {
        &self.grammar.rule_index
    }

    /// Returns compression statistics for a specific document.
    pub fn document_stats(&self, doc_id: &DocId) -> Option<RleDocumentStats> {
        let doc_info = self.documents.get(doc_id)?;

        let mut doc_nodes = 0;
        let mut current = self.grammar.symbols[doc_info.head].next;
        while let Some(key) = current {
            match &self.grammar.symbols[key].symbol {
                Symbol::DocTail => break,
                _ => {
                    doc_nodes += 1;
                    current = self.grammar.symbols[key].next;
                }
            }
        }

        Some(RleDocumentStats {
            input_length: doc_info.length,
            document_nodes: doc_nodes,
        })
    }

    /// Returns overall compression statistics across all documents.
    pub fn overall_stats(&self) -> RleOverallStats {
        let mut total_input_length = 0;
        let mut total_grammar_nodes = 0;

        // Count nodes in all documents
        for doc_info in self.documents.values() {
            total_input_length += doc_info.length;

            let mut current = self.grammar.symbols[doc_info.head].next;
            while let Some(key) = current {
                match &self.grammar.symbols[key].symbol {
                    Symbol::DocTail => break,
                    _ => {
                        total_grammar_nodes += 1;
                        current = self.grammar.symbols[key].next;
                    }
                }
            }
        }

        // Count nodes in all rules
        for &head_key in self.grammar.rule_index.values() {
            let mut current = self.grammar.symbols[head_key].next;
            while let Some(key) = current {
                if let Some(next) = self.grammar.symbols[key].next {
                    total_grammar_nodes += 1;
                    current = Some(next);
                } else {
                    break;
                }
            }
        }

        RleOverallStats {
            total_input_length,
            total_grammar_nodes,
            num_rules: self.grammar.rule_index.len(),
            num_documents: self.documents.len(),
        }
    }

    /// Creates a new empty document.
    fn create_document(&mut self, doc_id: DocId) {
        let tail_key = self
            .grammar
            .symbols
            .insert(RleSymbolNode::new(Symbol::DocTail));

        let head_key = self
            .grammar
            .symbols
            .insert(RleSymbolNode::new(Symbol::DocHead { tail: tail_key }));

        self.grammar.symbols[head_key].next = Some(tail_key);
        self.grammar.symbols[tail_key].prev = Some(head_key);

        self.documents.insert(
            doc_id,
            RleDocumentInfo {
                head: head_key,
                tail: tail_key,
                length: 0,
            },
        );
    }
}

/// Statistics about a single document's RLE compression.
#[derive(Debug, Clone, Copy)]
pub struct RleDocumentStats {
    /// Number of input symbols added to this document
    pub input_length: usize,
    /// Number of nodes in this document's sequence
    pub document_nodes: usize,
}

impl RleDocumentStats {
    /// Returns the document-level compression ratio as a percentage.
    pub fn compression_ratio(&self) -> f64 {
        if self.input_length == 0 {
            0.0
        } else {
            (self.document_nodes as f64 / self.input_length as f64) * 100.0
        }
    }
}

/// Overall statistics across all documents and shared RLE grammar.
#[derive(Debug, Clone, Copy)]
pub struct RleOverallStats {
    /// Total number of input symbols across all documents
    pub total_input_length: usize,
    /// Total nodes in the grammar (documents + rules)
    pub total_grammar_nodes: usize,
    /// Number of shared rules created
    pub num_rules: usize,
    /// Number of documents
    pub num_documents: usize,
}

impl RleOverallStats {
    /// Returns the overall compression ratio as a percentage.
    pub fn compression_ratio(&self) -> f64 {
        if self.total_input_length == 0 {
            0.0
        } else {
            (self.total_grammar_nodes as f64 / self.total_input_length as f64) * 100.0
        }
    }
}

impl<T: Hash + Eq + Clone, DocId: Hash + Eq + Clone> Default for SequiturDocumentsRle<T, DocId> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let docs = SequiturDocumentsRle::<char, u32>::new();
        assert_eq!(docs.num_documents(), 0);
        assert_eq!(docs.rules().len(), 0);
    }

    #[test]
    fn test_single_document() {
        let mut docs = SequiturDocumentsRle::new();
        docs.push_to_document("doc1", 'a');
        docs.push_to_document("doc1", 'b');
        docs.push_to_document("doc1", 'c');

        assert_eq!(docs.num_documents(), 1);
        assert_eq!(docs.document_len(&"doc1"), Some(3));
        assert_eq!(docs.document_is_empty(&"doc1"), Some(false));
    }

    #[test]
    fn test_run_length_encoding() {
        let mut docs = SequiturDocumentsRle::new();
        docs.push_to_document(1, 'a');
        docs.push_to_document(1, 'a');
        docs.push_to_document(1, 'a');

        assert_eq!(docs.document_len(&1), Some(3));

        // Should have only one node with run=3
        let stats = docs.document_stats(&1).unwrap();
        assert_eq!(stats.document_nodes, 1);
    }

    #[test]
    fn test_multiple_documents() {
        let mut docs = SequiturDocumentsRle::new();

        docs.push_to_document(1, 'a');
        docs.push_to_document(1, 'b');

        docs.push_to_document(2, 'c');
        docs.push_to_document(2, 'd');

        assert_eq!(docs.num_documents(), 2);
        assert_eq!(docs.document_len(&1), Some(2));
        assert_eq!(docs.document_len(&2), Some(2));
        assert_eq!(docs.document_len(&3), None);
    }

    #[test]
    fn test_shared_patterns() {
        let mut docs = SequiturDocumentsRle::new();

        // Documents with shared patterns
        docs.extend_document(1, vec!['a', 'b', 'a', 'b']);
        docs.extend_document(2, vec!['a', 'b', 'c', 'd']);

        // Grammar should have created shared rules
        let stats = docs.overall_stats();
        assert!(stats.num_rules > 0 || stats.total_grammar_nodes < stats.total_input_length);
    }

    #[test]
    fn test_document_ids() {
        let mut docs = SequiturDocumentsRle::new();
        docs.push_to_document("a", 'x');
        docs.push_to_document("b", 'y');
        docs.push_to_document("c", 'z');

        let mut ids: Vec<_> = docs.document_ids().cloned().collect();
        ids.sort();

        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_extend_document() {
        let mut docs = SequiturDocumentsRle::new();
        docs.extend_document(1, vec!['a', 'b', 'c']);

        assert_eq!(docs.document_len(&1), Some(3));
    }
}
