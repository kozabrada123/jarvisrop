use std::hash::{Hash, Hasher};

use rustc_hash::FxHasher;

pub fn compute_hash<T>(hashable: &T) -> u64
where
    T: Hash,
{
    let mut hasher = FxHasher::default();

    hashable.hash(&mut hasher);

    hasher.finish()
}
