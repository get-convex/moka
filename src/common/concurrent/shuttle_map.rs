// Shuttle-aware hash map that replaces `cht::SegmentedHashMap` when
// `cfg(moka_shuttle)` is active.
//
// Backed by `parking_lot::RwLock<HashMap<u64, Vec<(K, V)>>>` where the outer
// HashMap is keyed by the precomputed hash. This gives O(1) average-case
// bucket lookup while still accepting the closure-based eq API that cht uses.
// Within-bucket linear scan is O(1) in practice because hash collisions are
// rare. The RwLock is shuttle-aware via shuttle-parking_lot.

use parking_lot::RwLock;
use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};

pub(crate) struct ShuttleHashMap<K, V, S> {
    buckets: RwLock<HashMap<u64, Vec<(K, V)>>>,
    build_hasher: S,
}

impl<K, V, S> ShuttleHashMap<K, V, S> {
    pub(crate) fn len(&self) -> usize {
        self.buckets.read().values().map(|b| b.len()).sum()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.buckets.read().values().all(|b| b.is_empty())
    }

    pub(crate) fn actual_num_segments(&self) -> usize {
        1
    }
}

impl<K, V, S: BuildHasher> ShuttleHashMap<K, V, S> {
    pub(crate) fn with_num_segments_capacity_and_hasher(
        _num_segments: usize,
        _capacity: usize,
        build_hasher: S,
    ) -> Self {
        Self {
            buckets: RwLock::new(HashMap::new()),
            build_hasher,
        }
    }

    pub(crate) fn with_num_segments_and_hasher(_num_segments: usize, build_hasher: S) -> Self {
        Self {
            buckets: RwLock::new(HashMap::new()),
            build_hasher,
        }
    }

    /// Inserts `key`→`value` only if `key` is not already present.
    ///
    /// Returns `None` if the entry was inserted, or `Some(existing)` (a clone of
    /// the pre-existing value) if the key was already present.
    pub(crate) fn insert_if_not_present(&self, key: K, hash: u64, value: V) -> Option<V>
    where
        K: PartialEq,
        V: Clone,
    {
        let mut buckets = self.buckets.write();
        let bucket = buckets.entry(hash).or_default();
        if let Some((_, existing)) = bucket.iter().find(|(k, _)| k == &key) {
            Some(existing.clone())
        } else {
            bucket.push((key, value));
            None
        }
    }

    pub(crate) fn hash<Q>(&self, key: &Q) -> u64
    where
        K: std::borrow::Borrow<Q>,
        Q: Hash + ?Sized,
    {
        self.build_hasher.hash_one(key)
    }

    pub(crate) fn contains_key(&self, hash: u64, mut eq: impl FnMut(&K) -> bool) -> bool {
        let guard = self.buckets.read();
        guard
            .get(&hash)
            .map_or(false, |bucket| bucket.iter().any(|(k, _)| eq(k)))
    }

    pub(crate) fn get(&self, hash: u64, mut eq: impl FnMut(&K) -> bool) -> Option<V>
    where
        V: Clone,
    {
        let guard = self.buckets.read();
        guard
            .get(&hash)?
            .iter()
            .find(|(k, _)| eq(k))
            .map(|(_, v)| v.clone())
    }

    pub(crate) fn get_key_value_and<T>(
        &self,
        hash: u64,
        mut eq: impl FnMut(&K) -> bool,
        with_entry: impl FnOnce(&K, &V) -> T,
    ) -> Option<T> {
        let guard = self.buckets.read();
        guard
            .get(&hash)?
            .iter()
            .find(|(k, _)| eq(k))
            .map(|(k, v)| with_entry(k, v))
    }

    pub(crate) fn get_key_value_and_then<T>(
        &self,
        hash: u64,
        mut eq: impl FnMut(&K) -> bool,
        with_entry: impl FnOnce(&K, &V) -> Option<T>,
    ) -> Option<T> {
        let guard = self.buckets.read();
        guard
            .get(&hash)?
            .iter()
            .find(|(k, _)| eq(k))
            .and_then(|(k, v)| with_entry(k, v))
    }

    pub(crate) fn insert_with_or_modify(
        &self,
        key: K,
        hash: u64,
        on_insert: impl FnOnce() -> V,
        mut on_modify: impl FnMut(&K, &V) -> V,
    ) where
        K: PartialEq,
    {
        let mut buckets = self.buckets.write();
        let bucket = buckets.entry(hash).or_default();
        match bucket.iter().position(|(k, _)| k == &key) {
            Some(pos) => {
                let new_v = {
                    let (k, v) = &bucket[pos];
                    on_modify(k, v)
                };
                bucket[pos].1 = new_v;
            }
            None => bucket.push((key, on_insert())),
        }
    }

    pub(crate) fn insert_entry_and<T>(
        &self,
        key: K,
        hash: u64,
        value: V,
        with_entry: impl FnOnce(&K, &V) -> T,
    ) -> T
    where
        K: PartialEq,
    {
        let mut buckets = self.buckets.write();
        let bucket = buckets.entry(hash).or_default();
        match bucket.iter().position(|(k, _)| k == &key) {
            Some(pos) => {
                bucket[pos].1 = value;
                let (k, v) = &bucket[pos];
                with_entry(k, v)
            }
            None => {
                bucket.push((key, value));
                let (k, v) = bucket.last().unwrap();
                with_entry(k, v)
            }
        }
    }

    pub(crate) fn remove(&self, hash: u64, mut eq: impl FnMut(&K) -> bool) -> Option<V> {
        let mut buckets = self.buckets.write();
        let bucket = buckets.get_mut(&hash)?;
        let pos = bucket.iter().position(|(k, _)| eq(k))?;
        Some(bucket.swap_remove(pos).1)
    }

    pub(crate) fn remove_entry(
        &self,
        hash: u64,
        mut eq: impl FnMut(&K) -> bool,
    ) -> Option<(K, V)> {
        let mut buckets = self.buckets.write();
        let bucket = buckets.get_mut(&hash)?;
        let pos = bucket.iter().position(|(k, _)| eq(k))?;
        Some(bucket.swap_remove(pos))
    }

    pub(crate) fn remove_if(
        &self,
        hash: u64,
        mut eq: impl FnMut(&K) -> bool,
        mut condition: impl FnMut(&K, &V) -> bool,
    ) -> Option<V> {
        let mut buckets = self.buckets.write();
        let bucket = buckets.get_mut(&hash)?;
        let pos = bucket.iter().position(|(k, v)| eq(k) && condition(k, v))?;
        Some(bucket.swap_remove(pos).1)
    }

    pub(crate) fn remove_entry_if_and<T>(
        &self,
        hash: u64,
        mut eq: impl FnMut(&K) -> bool,
        mut condition: impl FnMut(&K, &V) -> bool,
        with_previous_entry: impl FnOnce(&K, &V) -> T,
    ) -> Option<T> {
        let mut buckets = self.buckets.write();
        let bucket = buckets.get_mut(&hash)?;
        let pos = bucket.iter().position(|(k, v)| eq(k) && condition(k, v))?;
        let result = {
            let (k, v) = &bucket[pos];
            with_previous_entry(k, v)
        };
        bucket.swap_remove(pos);
        Some(result)
    }

    pub(crate) fn keys<T>(
        &self,
        segment: usize,
        mut with_key: impl FnMut(&K) -> T,
    ) -> Option<Vec<T>> {
        if segment == 0 {
            let guard = self.buckets.read();
            let mut result = Vec::new();
            for bucket in guard.values() {
                for (k, _) in bucket {
                    result.push(with_key(k));
                }
            }
            Some(result)
        } else {
            None
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (K, V)>
    where
        K: Clone,
        V: Clone,
    {
        let guard = self.buckets.read();
        let mut items = Vec::new();
        for bucket in guard.values() {
            for (k, v) in bucket {
                items.push((k.clone(), v.clone()));
            }
        }
        items.into_iter()
    }
}
