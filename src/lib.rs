//! # Sequitur - Context-Free Grammar Compression
//!
//! A Rust implementation of the Sequitur algorithm for incremental grammar compression.
//!
//! Sequitur maintains a context-free grammar that compresses input sequences while
//! enforcing two constraints:
//! 1. **Digram Uniqueness**: No digram (pair of consecutive symbols) appears more than once
//! 2. **Rule Utility**: Every rule is used at least twice
//!
//! ## Example
//!
//! ```
//! use sequitur_rs::Sequitur;
//!
//! let mut seq = Sequitur::new();
//! seq.extend("abcabcabc".chars());
//!
//! // Reconstructs the original sequence
//! let reconstructed: String = seq.iter().collect();
//! assert_eq!(reconstructed, "abcabcabc");
//!
//! println!("Created {} rules", seq.rules().len());
//! ```
//!
//! ## RLE-Sequitur
//!
//! This crate also provides [`SequiturRle`], an extension with run-length encoding
//! that efficiently handles repeated symbols. Standard Sequitur represents `(ab)^k`
//! with O(log k) rules, while RLE-Sequitur uses only 2 rules.
//!
//! ```
//! use sequitur_rs::SequiturRle;
//!
//! let mut seq = SequiturRle::new();
//!
//! // A run of 100 'x' characters uses a single node with run=100
//! for _ in 0..100 {
//!     seq.push('x');
//! }
//!
//! let stats = seq.stats();
//! assert_eq!(stats.grammar_nodes, 1); // Just one node!
//!
//! // Reconstruction still works correctly
//! let result: String = seq.iter().collect();
//! assert_eq!(result.len(), 100);
//! ```
//!
//! ## Performance
//!
//! - O(1) amortized time per symbol added
//! - Grammar size grows sub-linearly with input size for repetitive data
//! - Memory-efficient using generational indices (SlotMap)

mod documents;
mod documents_iter;
mod grammar;
mod id_gen;
mod iter;
mod sequitur;
mod symbol;

// RLE (Run-Length Encoding) Sequitur modules
mod rle_grammar;
mod rle_iter;
mod rle_sequitur;
mod rle_symbol;

#[cfg(test)]
mod tests;

pub use documents::{DocumentStats, OverallStats, SequiturDocuments};
pub use documents_iter::DocumentIter;
pub use iter::SequiturIter;
pub use sequitur::{CompressionStats, Sequitur};

// RLE exports
pub use rle_iter::RleSequiturIter;
pub use rle_sequitur::{RleCompressionStats, SequiturRle};
