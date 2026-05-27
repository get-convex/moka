#![cfg(moka_shuttle)]
// These tests mostly just check that nothing deadlocks or panics during concurrent operations.

use moka::sync::Cache;
use shuttle::thread;

#[test]
fn concurrent_inserts() {
    shuttle::check_random(
        || {
            let cache: Cache<u32, u32> = Cache::new(100);

            let c1 = cache.clone();
            let t1 = thread::spawn(move || {
                c1.insert(1, 10);
            });

            let c2 = cache.clone();
            let t2 = thread::spawn(move || {
                c2.insert(2, 20);
            });

            t1.join().unwrap();
            t2.join().unwrap();

            let _ = cache.get(&1);
            let _ = cache.get(&2);
        },
        100,
    );
}

#[test]
fn insert_and_invalidate() {
    shuttle::check_random(
        || {
            let cache: Cache<u32, String> = Cache::new(10);

            let c1 = cache.clone();
            let t1 = thread::spawn(move || {
                c1.insert(42, "hello".to_string());
            });

            let c2 = cache.clone();
            let t2 = thread::spawn(move || {
                c2.invalidate(&42);
            });

            t1.join().unwrap();
            t2.join().unwrap();
        },
        100,
    );
}

#[test]
fn insert_with_eviction_listener() {
    use std::sync::{Arc, Mutex};

    shuttle::check_random(
        || {
            let evicted: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(Vec::new()));
            let evicted_clone = Arc::clone(&evicted);

            let cache: Cache<u32, u32> = Cache::builder()
                .max_capacity(2)
                .eviction_listener(move |k, _v, _cause| {
                    evicted_clone.lock().unwrap().push(*k);
                })
                .build();

            let c1 = cache.clone();
            let t1 = thread::spawn(move || {
                c1.insert(1, 10);
                c1.insert(2, 20);
                c1.insert(3, 30); // should trigger eviction of one entry
            });

            let c2 = cache.clone();
            let t2 = thread::spawn(move || {
                let _ = c2.get(&1);
                let _ = c2.get(&2);
            });

            t1.join().unwrap();
            t2.join().unwrap();

            cache.run_pending_tasks();
        },
        50,
    );
}
