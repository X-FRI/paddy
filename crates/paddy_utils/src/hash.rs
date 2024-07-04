


/// A [`HashMap`][hashbrown::HashMap] implementing aHash, a high
/// speed keyed hashing algorithm intended for use in in-memory hashmaps.
///
/// aHash is designed for performance and is NOT cryptographically secure.
///
/// Within the same execution of the program iteration order of different
/// `HashMap`s only depends on the order of insertions and deletions,
/// but it will not be stable between multiple executions of the program.
pub type HashMap<K, V> = hashbrown::HashMap<K, V, core::hash::BuildHasherDefault<ahash::AHasher>>;

