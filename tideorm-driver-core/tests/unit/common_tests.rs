use super::*;

#[test]
fn statement_cache_insert_returns_what_it_displaces() {
    // (capacity, keys inserted in order with values 0, 1, ..; what each insert returned,
    //  and the keys left, least recently used first)
    let cases: [(usize, &[&str], &[Option<usize>], &[&str]); 5] = [
        (2, &["a", "b"], &[None, None], &["a", "b"]),
        (2, &["a", "b", "c"], &[None, None, Some(0)], &["b", "c"]),
        // Replacing a key returns the old statement and evicts nothing, even when full.
        (2, &["a", "b", "a"], &[None, None, Some(0)], &["b", "a"]),
        (1, &["a", "b", "b"], &[None, Some(0), Some(1)], &["b"]),
        // Capacity 0 keeps nothing and returns nothing.
        (0, &["a", "a"], &[None, None], &[]),
    ];

    for (capacity, keys, returned, left) in cases {
        let mut cache = StatementCache::new(capacity);
        let got: Vec<_> = keys
            .iter()
            .enumerate()
            .map(|(value, key)| cache.insert(key, value))
            .collect();
        assert_eq!(got, returned, "inserting {keys:?} into capacity {capacity}");

        assert_eq!(cache.len(), left.len());
        assert_eq!(cache.is_empty(), left.is_empty());
        assert_eq!(cache.is_enabled(), capacity > 0);
        let mut drained = Vec::new();
        while let Some(value) = cache.remove_lru() {
            drained.push(keys[value]);
        }
        assert_eq!(drained, left, "inserting {keys:?} into capacity {capacity}");
    }
}

#[test]
fn statement_cache_get_mut_marks_recently_used() {
    let mut cache = StatementCache::new(2);
    cache.insert("a", 1);
    cache.insert("b", 2);

    *cache.get_mut("a").unwrap() += 10;
    assert_eq!(cache.get_mut("missing"), None);

    // "b" is now the least recently used, so it is the one evicted.
    assert_eq!(cache.insert("c", 3), Some(2));
    assert!(cache.contains_key("a") && cache.contains_key("c"));
    assert_eq!(cache.get_mut("a"), Some(&mut 11));

    cache.clear();
    assert!(cache.is_empty());
    assert_eq!(cache.capacity(), 2);
}

#[test]
fn debug_fn_prints_as_function() {
    let f = DebugFn(|x: u8| x + 1);
    assert_eq!(format!("{f:?}"), "Function");
    assert_eq!((f.0)(1), 2);
}
