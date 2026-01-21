use crate::symbol::{Symbol, SymbolHash};
use slotmap::DefaultKey;
use std::hash::Hash;

/// A node in the doubly-linked list of symbols with run-length encoding.
///
/// Each node represents `run` consecutive occurrences of the same symbol.
/// For non-terminal symbols (RuleRef), the run count represents how many
/// consecutive references to the same rule.
#[derive(Debug)]
pub(crate) struct RleSymbolNode<T> {
    pub symbol: Symbol<T>,
    /// Number of consecutive occurrences (1 = single occurrence)
    pub run: u32,
    pub prev: Option<DefaultKey>,
    pub next: Option<DefaultKey>,
}

impl<T> RleSymbolNode<T> {
    pub(crate) fn new(symbol: Symbol<T>) -> Self {
        Self {
            symbol,
            run: 1,
            prev: None,
            next: None,
        }
    }

    pub(crate) fn with_run(symbol: Symbol<T>, run: u32) -> Self {
        Self {
            symbol,
            run,
            prev: None,
            next: None,
        }
    }
}

/// A digram key for RLE-Sequitur that ignores run counts.
///
/// Two digrams are "similar" if they have the same symbols, regardless of run counts.
/// For example, (a:2, b:3) is similar to (a:5, b:1).
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub(crate) struct RleDigramKey(pub SymbolHash, pub SymbolHash);

impl RleDigramKey {
    /// Creates a digram key from two symbols (ignoring run counts).
    pub(crate) fn from_symbols<T: Hash>(first: &Symbol<T>, second: &Symbol<T>) -> Self {
        RleDigramKey(
            SymbolHash::from_symbol(first),
            SymbolHash::from_symbol(second),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rle_node_creation() {
        let node = RleSymbolNode::new(Symbol::Value('x'));
        assert!(matches!(node.symbol, Symbol::Value('x')));
        assert_eq!(node.run, 1);
        assert_eq!(node.prev, None);
        assert_eq!(node.next, None);
    }

    #[test]
    fn test_rle_node_with_run() {
        let node = RleSymbolNode::with_run(Symbol::Value('a'), 5);
        assert!(matches!(node.symbol, Symbol::Value('a')));
        assert_eq!(node.run, 5);
    }

    #[test]
    fn test_digram_key_ignores_run() {
        // The digram key only depends on symbols, not runs
        let sym1 = Symbol::Value('a');
        let sym2 = Symbol::Value('b');

        let key = RleDigramKey::from_symbols(&sym1, &sym2);

        // Same symbols should produce same key
        let key2 = RleDigramKey::from_symbols(&sym1, &sym2);
        assert_eq!(key, key2);

        // Different symbols should produce different key
        let sym3 = Symbol::Value('c');
        let key3 = RleDigramKey::from_symbols(&sym1, &sym3);
        assert_ne!(key, key3);
    }
}
