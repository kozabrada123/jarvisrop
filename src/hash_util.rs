use std::hash::{DefaultHasher, Hash, Hasher};

pub fn compute_hash<T>(hashable: &T) -> u64
where
    T: Hash,
{
    let mut hasher = DefaultHasher::new();

    hashable.hash(&mut hasher);

    hasher.finish()
}
