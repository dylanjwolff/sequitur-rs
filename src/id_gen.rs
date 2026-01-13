/// ID generator that reuses freed IDs to prevent exhaustion on long sequences.
///
/// Mimics the behavior of the C++ implementation's ID class.
#[derive(Debug)]
pub(crate) struct IdGenerator {
    next: u32,
    freed: Vec<u32>,
}

impl IdGenerator {
    /// Creates a new ID generator starting from ID 0.
    pub(crate) fn new() -> Self {
        Self {
            next: 0,
            freed: Vec::new(),
        }
    }

    /// Gets a new ID, reusing a freed one if available.
    pub(crate) fn get(&mut self) -> u32 {
        if let Some(id) = self.freed.pop() {
            id
        } else {
            let id = self.next;
            self.next += 1;
            id
        }
    }

    /// Marks an ID as freed, making it available for reuse.
    pub(crate) fn free(&mut self, id: u32) {
        assert!(id < self.next, "Cannot free ID that was never allocated");
        self.freed.push(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequential_allocation() {
        let mut gen = IdGenerator::new();
        assert_eq!(gen.get(), 0);
        assert_eq!(gen.get(), 1);
        assert_eq!(gen.get(), 2);
    }

    #[test]
    fn test_reuse_freed() {
        let mut gen = IdGenerator::new();
        let id0 = gen.get();
        let id1 = gen.get();
        let id2 = gen.get();

        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        gen.free(id1);
        assert_eq!(gen.get(), 1); // Should reuse freed ID

        gen.free(id0);
        gen.free(id2);
        assert_eq!(gen.get(), 2); // LIFO order
        assert_eq!(gen.get(), 0);
    }

    #[test]
    #[should_panic(expected = "Cannot free ID that was never allocated")]
    fn test_free_invalid_id() {
        let mut gen = IdGenerator::new();
        gen.get();
        gen.free(999); // Should panic
    }
}
