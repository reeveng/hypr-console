//! Fixed hash keys for caches and graph traversal that must repeat within a build.
use core::hash::BuildHasherDefault;
use std::collections::{HashMap, HashSet};
use std::hash::DefaultHasher;

pub type FixedBuildHasher = BuildHasherDefault<DefaultHasher>;
pub type FixedMap<K, V> = HashMap<K, V, FixedBuildHasher>;
pub type FixedSet<K> = HashSet<K, FixedBuildHasher>;
