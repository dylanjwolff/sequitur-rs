use crate::grammar::{is_sequence_start, GrammarFields, GrammarOps};
use crate::id_gen::IdGenerator;
use crate::symbol::{Symbol, SymbolHash, SymbolNode};
use ahash::AHashMap as HashMap;
use slotmap::{DefaultKey, SlotMap};
use std::hash::Hash;

/// Per-document metadata tracking the document's symbol sequence.
#[derive(Debug, Clone)]
pub(crate) struct DocumentInfo {
    /// Key to the DocHead for this document
    pub(crate) head: DefaultKey,
    /// Key to the DocTail for this document
    pub(crate) tail: DefaultKey,
    /// Number of values in this document
    pub(crate) length: usize,
}

/// Multi-document Sequitur compression with shared grammar.
///
/// Multiple documents share the same underlying grammar (rules and digram index),
/// allowing efficient compression of related content (e.g., news articles on the same topic).
/// Each document can be decompressed independently without iterating over the entire grammar.
///
/// # Type Parameters
///
/// * `T` - The terminal symbol type (must be Hash + Eq + Clone)
/// * `DocId` - The document identifier type (must be Hash + Eq + Clone)
///
/// # Example
///
/// ```rust
/// use sequitur_rs::SequiturDocuments;
///
/// let mut docs = SequiturDocuments::<char, String>::new();
///
/// // Add multiple related articles
/// docs.push_to_document("article1".to_string(), 'T');
/// docs.push_to_document("article1".to_string(), 'h');
/// docs.push_to_document("article1".to_string(), 'e');
///
/// docs.push_to_document("article2".to_string(), 'T');
/// docs.push_to_document("article2".to_string(), 'h');
/// docs.push_to_document("article2".to_string(), 'e');
///
/// // Grammar is shared, each document can be iterated independently
/// let text1: String = docs.iter_document(&"article1".to_string()).unwrap().collect();
/// let text2: String = docs.iter_document(&"article2".to_string()).unwrap().collect();
/// ```
pub struct SequiturDocuments<T, DocId> {
    /// Storage for all symbols using generational indices
    pub(crate) symbols: SlotMap<DefaultKey, SymbolNode<T>>,

    /// Maps digrams to their first occurrence
    pub(crate) digram_index: HashMap<(SymbolHash, SymbolHash), DefaultKey>,

    /// Maps rule IDs to their RuleHead keys
    pub(crate) rule_index: HashMap<u32, DefaultKey>,

    /// ID generator with reuse
    pub(crate) id_gen: IdGenerator,

    /// Per-document sequences
    pub(crate) documents: HashMap<DocId, DocumentInfo>,
}

// Implement GrammarOps trait for zero-cost code sharing
impl<T, DocId> GrammarOps<T> for SequiturDocuments<T, DocId> {
    #[inline(always)]
    fn fields(&mut self) -> GrammarFields<'_, T> {
        GrammarFields {
            symbols: &mut self.symbols,
            digram_index: &mut self.digram_index,
            rule_index: &mut self.rule_index,
            id_gen: &mut self.id_gen,
        }
    }
}

impl<T: Hash + Eq + Clone, DocId: Hash + Eq + Clone> SequiturDocuments<T, DocId> {
    /// Creates a new empty SequiturDocuments instance.
    ///
    /// No documents or rules exist initially. The grammar is built incrementally
    /// as documents are added.
    pub fn new() -> Self {
        Self {
            symbols: SlotMap::new(),
            digram_index: HashMap::default(),
            rule_index: HashMap::default(),
            id_gen: IdGenerator::new(),
            documents: HashMap::default(),
        }
    }

    /// Adds a value to the specified document.
    ///
    /// If the document doesn't exist, it is created automatically.
    /// This triggers the Sequitur algorithm to maintain grammar constraints
    /// across all documents.
    ///
    /// # Arguments
    ///
    /// * `doc_id` - Identifier for the document
    /// * `value` - Value to append to the document
    ///
    /// # Example
    ///
    /// ```rust
    /// use sequitur_rs::SequiturDocuments;
    ///
    /// let mut docs = SequiturDocuments::<char, u32>::new();
    /// docs.push_to_document(1, 'a');
    /// docs.push_to_document(1, 'b');
    /// docs.push_to_document(2, 'a');  // Creates new document
    /// ```
    pub fn push_to_document(&mut self, doc_id: DocId, value: T) {
        // Ensure document exists
        if !self.documents.contains_key(&doc_id) {
            self.create_document(doc_id.clone());
        }

        // Create new Value symbol
        let new_key = self.symbols.insert(SymbolNode::new(Symbol::Value(value)));

        // Get document info
        let doc_info = self.documents.get_mut(&doc_id).unwrap();
        let tail_key = doc_info.tail;
        let prev_key = self.symbols[tail_key].prev;

        // Link new symbol into the list before DocTail
        self.symbols[new_key].next = Some(tail_key);
        self.symbols[new_key].prev = prev_key;
        self.symbols[tail_key].prev = Some(new_key);

        if let Some(prev) = prev_key {
            self.symbols[prev].next = Some(new_key);
        }

        doc_info.length += 1;

        // If not the first symbol, check for digram
        if doc_info.length > 1 {
            if let Some(prev) = prev_key {
                // Skip if prev is DocHead (digrams don't start with DocHead)
                if !is_sequence_start(&self.symbols[prev].symbol) {
                    self.fields().link_made(prev);
                }
            }
        }
    }

    /// Extends the document with multiple values.
    ///
    /// # Arguments
    ///
    /// * `doc_id` - Identifier for the document
    /// * `iter` - Iterator of values to append
    pub fn extend_document<I: IntoIterator<Item = T>>(&mut self, doc_id: DocId, iter: I) {
        for value in iter {
            self.push_to_document(doc_id.clone(), value);
        }
    }

    /// Returns the number of values in a document.
    ///
    /// Returns `None` if the document doesn't exist.
    pub fn document_len(&self, doc_id: &DocId) -> Option<usize> {
        self.documents.get(doc_id).map(|info| info.length)
    }

    /// Returns true if the document exists and is empty.
    ///
    /// Returns `None` if the document doesn't exist.
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
        &self.rule_index
    }

    /// Returns compression statistics for a specific document.
    ///
    /// Returns `None` if the document doesn't exist.
    pub fn document_stats(&self, doc_id: &DocId) -> Option<DocumentStats> {
        let doc_info = self.documents.get(doc_id)?;

        // Count symbols in this document's sequence
        let mut doc_symbols = 0;
        let mut current = self.symbols[doc_info.head].next;
        while let Some(key) = current {
            match &self.symbols[key].symbol {
                Symbol::DocTail => break,
                _ => {
                    doc_symbols += 1;
                    current = self.symbols[key].next;
                }
            }
        }

        Some(DocumentStats {
            input_length: doc_info.length,
            document_symbols: doc_symbols,
        })
    }

    /// Returns overall compression statistics across all documents.
    pub fn overall_stats(&self) -> OverallStats {
        let mut total_input_length = 0;
        let mut total_grammar_symbols = 0;

        // Count symbols in all documents
        for doc_info in self.documents.values() {
            total_input_length += doc_info.length;

            let mut current = self.symbols[doc_info.head].next;
            while let Some(key) = current {
                match &self.symbols[key].symbol {
                    Symbol::DocTail => break,
                    _ => {
                        total_grammar_symbols += 1;
                        current = self.symbols[key].next;
                    }
                }
            }
        }

        // Count symbols in all rules
        for &head_key in self.rule_index.values() {
            let mut current = self.symbols[head_key].next;
            while let Some(key) = current {
                if let Some(next) = self.symbols[key].next {
                    total_grammar_symbols += 1;
                    current = Some(next);
                } else {
                    break;
                }
            }
        }

        OverallStats {
            total_input_length,
            total_grammar_symbols,
            num_rules: self.rule_index.len(),
            num_documents: self.documents.len(),
        }
    }

    /// Creates a new empty document.
    fn create_document(&mut self, doc_id: DocId) {
        // Create DocTail first
        let tail_key = self.symbols.insert(SymbolNode::new(Symbol::DocTail));

        // Create DocHead with reference to tail
        let head_key = self
            .symbols
            .insert(SymbolNode::new(Symbol::DocHead { tail: tail_key }));

        // Link them together
        self.symbols[head_key].next = Some(tail_key);
        self.symbols[tail_key].prev = Some(head_key);

        self.documents.insert(
            doc_id,
            DocumentInfo {
                head: head_key,
                tail: tail_key,
                length: 0,
            },
        );
    }
}

/// Statistics about a single document's compression.
#[derive(Debug, Clone, Copy)]
pub struct DocumentStats {
    /// Number of input symbols added to this document
    pub input_length: usize,
    /// Number of symbols in this document's sequence (including rule references)
    pub document_symbols: usize,
}

impl DocumentStats {
    /// Returns the document-level compression ratio as a percentage.
    pub fn compression_ratio(&self) -> f64 {
        if self.input_length == 0 {
            0.0
        } else {
            (self.document_symbols as f64 / self.input_length as f64) * 100.0
        }
    }
}

/// Overall statistics across all documents and shared grammar.
#[derive(Debug, Clone, Copy)]
pub struct OverallStats {
    /// Total number of input symbols across all documents
    pub total_input_length: usize,
    /// Total symbols in the grammar (documents + rules)
    pub total_grammar_symbols: usize,
    /// Number of shared rules created
    pub num_rules: usize,
    /// Number of documents
    pub num_documents: usize,
}

impl OverallStats {
    /// Returns the overall compression ratio as a percentage.
    pub fn compression_ratio(&self) -> f64 {
        if self.total_input_length == 0 {
            0.0
        } else {
            (self.total_grammar_symbols as f64 / self.total_input_length as f64) * 100.0
        }
    }
}

impl<T: Hash + Eq + Clone, DocId: Hash + Eq + Clone> Default for SequiturDocuments<T, DocId> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let docs = SequiturDocuments::<char, u32>::new();
        assert_eq!(docs.num_documents(), 0);
        assert_eq!(docs.rules().len(), 0);
    }

    #[test]
    fn test_single_document() {
        let mut docs = SequiturDocuments::new();
        docs.push_to_document("doc1", 'a');
        docs.push_to_document("doc1", 'b');
        docs.push_to_document("doc1", 'c');

        assert_eq!(docs.num_documents(), 1);
        assert_eq!(docs.document_len(&"doc1"), Some(3));
        assert_eq!(docs.document_is_empty(&"doc1"), Some(false));
    }

    #[test]
    fn test_multiple_documents() {
        let mut docs = SequiturDocuments::new();

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
    fn test_document_ids() {
        let mut docs = SequiturDocuments::new();
        docs.push_to_document("a", 'x');
        docs.push_to_document("b", 'y');
        docs.push_to_document("c", 'z');

        let mut ids: Vec<_> = docs.document_ids().cloned().collect();
        ids.sort();

        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_extend_document() {
        let mut docs = SequiturDocuments::new();
        docs.extend_document(1, vec!['a', 'b', 'c']);

        assert_eq!(docs.document_len(&1), Some(3));
    }
}
