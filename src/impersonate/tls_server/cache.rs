//! A count-bounded least-recently-used map from leaf key to leaf.

use std::collections::HashMap;

/// A least-recently-used map that holds at most `capacity` entries.
///
/// Eviction scans for the oldest entry: O(n) in the capacity (256 by default), far cheaper than
/// the certificate signature a hit saves.
#[derive(Debug)]
pub(super) struct LeafCache<V> {
    entries: HashMap<String, Slot<V>>,
    capacity: usize,
    clock: u64,
}

/// One cached value and the tick of its last use.
#[derive(Debug)]
struct Slot<V> {
    value: V,
    last_used: u64,
}

impl<V: Clone> LeafCache<V> {
    /// An empty cache that holds at most `capacity` entries (at least one).
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            capacity: capacity.max(1),
            clock: 0,
        }
    }

    /// Returns the value for `key` and marks it as the most recently used.
    pub(super) fn get(&mut self, key: &str) -> Option<V> {
        let tick = self.tick();
        let slot = self.entries.get_mut(key)?;
        slot.last_used = tick;
        Some(slot.value.clone())
    }

    /// Stores `value` under `key` as the most recently used, evicting the least recently used
    /// entry when the cache is full and `key` is new.
    pub(super) fn insert(&mut self, key: String, value: V) {
        let tick = self.tick();
        if !self.entries.contains_key(&key) && self.entries.len() >= self.capacity {
            self.evict_oldest();
        }
        self.entries.insert(
            key,
            Slot {
                value,
                last_used: tick,
            },
        );
    }

    /// How many entries the cache holds.
    pub(super) fn count(&self) -> usize {
        self.entries.len()
    }

    /// Advances the use clock. It cannot wrap in practice (one tick per lookup).
    fn tick(&mut self) -> u64 {
        self.clock = self.clock.saturating_add(1);
        self.clock
    }

    /// Drops the entry with the oldest use tick.
    fn evict_oldest(&mut self) {
        let oldest = self
            .entries
            .iter()
            .min_by_key(|(_, slot)| slot.last_used)
            .map(|(key, _)| key.clone());
        if let Some(key) = oldest {
            self.entries.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LeafCache;

    #[test]
    fn a_full_cache_evicts_the_least_recently_used_entry() {
        let mut cache = LeafCache::new(2);
        cache.insert("a".to_owned(), 1);
        cache.insert("b".to_owned(), 2);
        assert_eq!(cache.get("a"), Some(1));
        cache.insert("c".to_owned(), 3);
        assert_eq!(cache.count(), 2);
        assert_eq!(cache.get("b"), None);
        assert_eq!(cache.get("a"), Some(1));
        assert_eq!(cache.get("c"), Some(3));
    }

    #[test]
    fn replacing_a_key_never_evicts_another() {
        let mut cache = LeafCache::new(2);
        cache.insert("a".to_owned(), 1);
        cache.insert("b".to_owned(), 2);
        cache.insert("a".to_owned(), 10);
        assert_eq!(cache.count(), 2);
        assert_eq!(cache.get("a"), Some(10));
        assert_eq!(cache.get("b"), Some(2));
    }

    #[test]
    fn a_zero_capacity_still_holds_one_entry() {
        let mut cache = LeafCache::new(0);
        cache.insert("a".to_owned(), 1);
        cache.insert("b".to_owned(), 2);
        assert_eq!(cache.count(), 1);
        assert_eq!(cache.get("b"), Some(2));
    }
}
