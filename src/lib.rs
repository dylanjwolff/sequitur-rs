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
//! ## Performance
//!
//! - O(1) amortized time per symbol added
//! - Grammar size grows sub-linearly with input size for repetitive data
//! - Memory-efficient using generational indices (SlotMap)

mod digram;
mod documents;
mod documents_iter;
mod documents_ops;
mod id_gen;
mod iter;
mod rule;
mod sequitur;
mod symbol;

#[cfg(test)]
mod tests;

pub use documents::{DocumentStats, OverallStats, SequiturDocuments};
pub use documents_iter::DocumentIter;
pub use iter::SequiturIter;
pub use sequitur::{CompressionStats, Sequitur};
