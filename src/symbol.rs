use slotmap::DefaultKey;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Symbol types in the Sequitur grammar.
///
/// Replaces the C++ inheritance hierarchy with an enum for zero-cost abstraction.
#[derive(Debug, Clone)]
pub(crate) enum Symbol<T> {
    /// A terminal symbol containing an actual value from the input.
    Value(T),

    /// A reference to a rule (non-terminal).
    RuleRef { rule_id: u32 },

    /// Marks the beginning of a rule definition.
    RuleHead {
        rule_id: u32,
        count: u32,
        tail: DefaultKey,
    },

    /// Marks the end of a rule definition.
    RuleTail,
}

/// A node in the doubly-linked list of symbols.
///
/// Replaces C++'s intrusive linked list with safe SlotMap-based indices.
#[derive(Debug)]
pub(crate) struct SymbolNode<T> {
    pub symbol: Symbol<T>,
    pub prev: Option<DefaultKey>,
    pub next: Option<DefaultKey>,
}

impl<T> SymbolNode<T> {
    pub(crate) fn new(symbol: Symbol<T>) -> Self {
        Self {
            symbol,
            prev: None,
            next: None,
        }
    }
}

/// A compact hash representation of a symbol for use in digram index keys.
///
/// Stores a 64-bit hash instead of the full symbol to allow Copy and efficient HashMap usage.
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub(crate) struct SymbolHash(u64);

impl SymbolHash {
    /// Creates a hash from a symbol.
    pub(crate) fn from_symbol<T: Hash>(symbol: &Symbol<T>) -> Self {
        let mut hasher = DefaultHasher::new();
        match symbol {
            Symbol::Value(v) => {
                0u8.hash(&mut hasher);
                v.hash(&mut hasher);
            }
            Symbol::RuleRef { rule_id } => {
                1u8.hash(&mut hasher);
                rule_id.hash(&mut hasher);
            }
            Symbol::RuleHead { rule_id, .. } => {
                2u8.hash(&mut hasher);
                rule_id.hash(&mut hasher);
            }
            Symbol::RuleTail => {
                3u8.hash(&mut hasher);
            }
        }
        SymbolHash(hasher.finish())
    }
}

impl<T: Clone> Symbol<T> {
    /// Clones the symbol for use in rule creation.
    ///
    /// Only clones the essential data, not linked list pointers.
    pub(crate) fn clone_symbol(&self) -> Symbol<T> {
        match self {
            Symbol::Value(v) => Symbol::Value(v.clone()),
            Symbol::RuleRef { rule_id } => Symbol::RuleRef { rule_id: *rule_id },
            Symbol::RuleHead {
                rule_id,
                count,
                tail,
            } => Symbol::RuleHead {
                rule_id: *rule_id,
                count: *count,
                tail: *tail,
            },
            Symbol::RuleTail => Symbol::RuleTail,
        }
    }
}

impl<T: PartialEq> Symbol<T> {
    /// Checks equality with another symbol.
    ///
    /// Used to verify hash matches in digram lookup.
    pub(crate) fn equals(&self, other: &Symbol<T>) -> bool {
        match (self, other) {
            (Symbol::Value(a), Symbol::Value(b)) => a == b,
            (Symbol::RuleRef { rule_id: a }, Symbol::RuleRef { rule_id: b }) => a == b,
            (
                Symbol::RuleHead { rule_id: a, .. },
                Symbol::RuleHead { rule_id: b, .. },
            ) => a == b,
            (Symbol::RuleTail, Symbol::RuleTail) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_hash_consistency() {
        let sym1 = Symbol::Value('a');
        let sym2 = Symbol::Value('a');
        let sym3 = Symbol::Value('b');

        let hash1 = SymbolHash::from_symbol(&sym1);
        let hash2 = SymbolHash::from_symbol(&sym2);
        let hash3 = SymbolHash::from_symbol(&sym3);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_symbol_equality() {
        let sym1 = Symbol::Value(42);
        let sym2 = Symbol::Value(42);
        let sym3 = Symbol::Value(99);

        assert!(sym1.equals(&sym2));
        assert!(!sym1.equals(&sym3));
    }

    #[test]
    fn test_rule_ref_hash() {
        let rule1 = Symbol::<()>::RuleRef { rule_id: 1 };
        let rule2 = Symbol::<()>::RuleRef { rule_id: 1 };
        let rule3 = Symbol::<()>::RuleRef { rule_id: 2 };

        let hash1 = SymbolHash::from_symbol(&rule1);
        let hash2 = SymbolHash::from_symbol(&rule2);
        let hash3 = SymbolHash::from_symbol(&rule3);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_symbol_node_creation() {
        let node = SymbolNode::new(Symbol::Value('x'));
        assert!(matches!(node.symbol, Symbol::Value('x')));
        assert_eq!(node.prev, None);
        assert_eq!(node.next, None);
    }
}
