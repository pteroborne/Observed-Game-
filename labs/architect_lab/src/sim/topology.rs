//! Incremental facility topology tracking and disjoint-set primitive.

use std::collections::{BTreeMap, BTreeSet};

/// Disjoint-set forest with deterministic canonical representatives.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisjointSet<T: Ord + Copy> {
    parent: BTreeMap<T, T>,
    rank: BTreeMap<T, u32>,
}

impl<T: Ord + Copy> DisjointSet<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            parent: BTreeMap::new(),
            rank: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, item: T) {
        self.parent.entry(item).or_insert(item);
        self.rank.entry(item).or_insert(0);
    }

    pub fn find(&mut self, item: T) -> T {
        self.insert(item);
        let mut root = item;
        while let Some(&p) = self.parent.get(&root) {
            if p == root {
                break;
            }
            root = p;
        }
        // Path compression
        let mut curr = item;
        while let Some(&p) = self.parent.get(&curr) {
            if p == root {
                break;
            }
            self.parent.insert(curr, root);
            curr = p;
        }
        root
    }

    pub fn union(&mut self, a: T, b: T) -> bool {
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return false;
        }

        let rank_a = self.rank.get(&root_a).copied().unwrap_or(0);
        let rank_b = self.rank.get(&root_b).copied().unwrap_or(0);

        match rank_a.cmp(&rank_b) {
            std::cmp::Ordering::Less => {
                self.parent.insert(root_a, root_b);
            }
            std::cmp::Ordering::Greater => {
                self.parent.insert(root_b, root_a);
            }
            std::cmp::Ordering::Equal => {
                self.parent.insert(root_b, root_a);
                self.rank.insert(root_a, rank_a + 1);
            }
        }
        true
    }

    #[must_use]
    pub fn component_count(&mut self) -> usize {
        let items: Vec<T> = self.parent.keys().copied().collect();
        let mut roots = BTreeSet::new();
        for item in items {
            roots.insert(self.find(item));
        }
        roots.len()
    }

    #[must_use]
    pub fn same_component(&mut self, a: T, b: T) -> bool {
        self.find(a) == self.find(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disjoint_set_initializes_empty_and_tracks_singletons() {
        let mut ds = DisjointSet::<i32>::new();
        assert_eq!(ds.component_count(), 0);

        ds.insert(1);
        ds.insert(2);
        ds.insert(3);
        assert_eq!(ds.component_count(), 3);
        assert!(!ds.same_component(1, 2));
        assert!(!ds.same_component(2, 3));
        assert!(ds.same_component(1, 1));
    }

    #[test]
    fn disjoint_set_unions_and_merges_components() {
        let mut ds = DisjointSet::<i32>::new();
        ds.insert(1);
        ds.insert(2);
        ds.insert(3);
        ds.insert(4);

        assert!(ds.union(1, 2));
        assert!(!ds.union(1, 2)); // Redundant union returns false
        assert_eq!(ds.component_count(), 3);
        assert!(ds.same_component(1, 2));
        assert!(!ds.same_component(1, 3));

        assert!(ds.union(3, 4));
        assert_eq!(ds.component_count(), 2);
        assert!(ds.same_component(3, 4));

        assert!(ds.union(2, 3));
        assert_eq!(ds.component_count(), 1);
        assert!(ds.same_component(1, 4));
    }

    #[test]
    fn disjoint_set_find_creates_missing_item_as_singleton() {
        let mut ds = DisjointSet::<i32>::new();
        assert_eq!(ds.find(42), 42);
        assert_eq!(ds.component_count(), 1);
    }
}
