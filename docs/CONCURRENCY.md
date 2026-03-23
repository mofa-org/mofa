# Concurrency Standards & Deadlock Prevention

## Overview

This document establishes standards for safe concurrent programming in MoFA. All code holding locks or using shared state must adhere to these guidelines.

---

## Preferred Lock Types

| Use Case | Lock Type | Rationale | Example |
|----------|-----------|-----------|---------|
| **General state protection** | `parking_lot::Mutex<T>` | Fast, no poisoning, reentrant-friendly | Agent state, configuration |
| **Read-heavy data** | `Arc<DashMap<K, V>>` | Lock-free, better scalability | Large caches, registries |
| **Async context required** | `tokio::sync::Mutex<T>` | Proper await semantics | Channel guards, async resources |
| **High-frequency reads** | `parking_lot::RwLock<T>` | Reader preference, no starvation | Event subscriptions |

---

## Core Rules

### ✅ DO

- [ ] **Drop locks before any `.await`** — Lock guards must not span async boundaries
- [ ] **Use explicit scope blocks** — `{ let _guard = lock.lock(); /* use lock */ }` clearly bounds scope
- [ ] **Re-check after upgrading** — TOCTOU: Always re-check condition after acquiring write lock
- [ ] **Document lock ordering** — If multiple locks acquired, document global order
- [ ] **Use lock-free for high-contention** — `DashMap`, `crossbeam::queue`, atomic types
- [ ] **Scope guards at smallest granularity** — Only lock what you actually use

### ❌ DON'T

- [ ] **Never hold sync lock across await** — Causes scheduler starvation on single thread
- [ ] **Never acquire locks in different orders** — Leads to circular wait deadlock
- [ ] **Never hold read lock, drop, then write** — Classic TOCTOU race condition
- [ ] **Never nest `Arc<Mutex<Arc<Mutex<...>>>>`** — Creates complex lock dependencies
- [ ] **Never let lock scope be implicit** — Avoid returning with lock held by accident
- [ ] **Never skip checking after lock upgrades** — Another thread may have modified data

---

## Anti-Patterns & Solutions

### Pattern 1: Lock Held Across Await ❌

**Problem:** Lock prevents other tasks from making progress

```rust
// ❌ DANGEROUS
async fn bad_operation(&self) {
    let lock = self.data.write().await;      // Lock acquired
    let result = external_call().await;      // AWAIT WITH LOCK — BLOCKS OTHER TASKS
    lock.update(result);
}

// ✅ CORRECT
async fn good_operation(&self) {
    let result = external_call().await;      // No lock held during await
    let mut lock = self.data.write().await;  // Acquire lock only for update
    lock.update(result);
}
```

---

### Pattern 2: TOCTOU (Time-of-Check-Time-of-Use) Race ❌

**Problem:** Between releasing read lock and acquiring write lock, another thread modifies data

```rust
// ❌ VULNERABLE - TOCTOU Race
async fn bad_insert(&mut self, key: String, value: Value) {
    if !self.data.read().await.contains_key(&key) {  // CHECK
        drop(_);  // Read lock released
        let mut data = self.data.write().await;       // RACE WINDOW
        data.insert(key, value);                       // INSERT
    }
}

// ✅ CORRECT - Atomic Check-Insert
async fn good_insert(&mut self, key: String, value: Value) {
    let mut data = self.data.write().await;
    if !data.contains_key(&key) {  // CHECK and ACT atomically
        data.insert(key, value);
    }
}

// ✅ CORRECT - Entry API Pattern
async fn better_insert(&mut self, key: String, value: Value) {
    let mut data = self.data.write().await;
    data.entry(key)
        .or_insert(value);  // Atomic check-or-insert
}
```

---

### Pattern 3: Recursive Lock Deadlock ❌

**Problem:** Method acquires lock, calls helper method that re-acquires same lock

```rust
// ❌ DEADLOCK - Recursive lock acquisition
impl Scheduler {
    async fn schedule(&mut self) {
        let lock = self.agents.write().await;  // LOCK 1
        let chosen = self.select_agent().await; // CALLS select_agent()
        // ...
    }

    async fn select_agent(&self) {
        let lock = self.agents.read().await;    // LOCK 2 — SAME LOCK! DEADLOCK!
        best_agent()
    }
}

// ✅ CORRECT - Inline or scope to avoid recursion
impl Scheduler {
    async fn schedule(&mut self) {
        let chosen = {
            let lock = self.agents.read().await;
            self.find_best_agent_locked(&lock)  // Pass lock to helper
        };
        // ...
    }

    fn find_best_agent_locked(&self, _guard: &parking_lot::RwLockReadGuard) -> Agent {
        // Uses lock held by caller, no re-acquisition
        best_agent()
    }
}
```

---

### Pattern 4: Lock Contention on Await ❌

**Problem:** Holding lock while awaiting starves other tasks

```rust
// ❌ POOR CONCURRENCY - Starvation
async fn process_all(&self) {
    let mut channels = self.channels.write().await;  // LOCK
    for channel in channels.iter_mut() {
        let msg = channel.recv().await;               // LONG AWAIT WITH LOCK
        // process message
    }
}

// ✅ CORRECT - Snapshot, release, then process
async fn process_all(&self) {
    let channels = {
        let guard = self.channels.read().await;
        guard.to_vec()  // Snapshot
    };  // LOCK RELEASED
    
    for mut channel in channels {
        let msg = channel.recv().await;  // No lock held
        // process message
    }
}
```

---

## Code Review Checklist

When reviewing async/concurrent code, verify:

- [ ] **No locks held across `.await`** — Every `.await` point has no active lock guards
- [ ] **Lock scope is explicit** — Locks use `{ let _g = lock(); }` or method call
- [ ] **Multiple locks ordered consistently** — If acquiring lock A then B in one place, same order everywhere
- [ ] **TOCTOU prevented** — Check and mutation happen under same lock
- [ ] **No recursive lock acquisition** — Methods don't re-acquire locks they already hold
- [ ] **Read-check-write pattern fixed** — Uses Entry API or holds write lock for both
- [ ] **No implicit lock scope** — Return statements don't rely on implicit guard drop
- [ ] **High-contention paths considered** — Are we using lock-free alternatives where needed?
- [ ] **Lock ordering documented** — If multiple locks: explicitly document global order

---

## Testing for Deadlock Safety

### Unit Tests: For Single Component

```rust
#[tokio::test]
async fn test_component_concurrent_operations() {
    let component = Arc::new(Component::new());
    
    let mut tasks = vec![];
    for i in 0..50 {
        let c = Arc::clone(&component);
        tasks.push(tokio::spawn(async move {
            // Concurrent operations
            c.operation().await;
        }));
    }
    
    for task in tasks {
        task.await.expect("Task panicked");
    }
}
```

### Stress Tests: For Deadlock Detection

```rust
#[tokio::test]
#[ignore = "stress test, run with --ignored"]
async fn stress_test_no_deadlock() {
    parking_lot::deadlock::enable_deadlock_detection();
    
    for iteration in 0..100 {
        // Create 50 concurrent tasks
        let tasks: Vec<_> = (0..50)
            .map(|_| tokio::spawn(concurrent_operation()))
            .collect();
        
        futures::future::join_all(tasks).await;
        
        // Check for deadlock
        if let Some(deadlocks) = parking_lot::deadlock::check_deadlock() {
            panic!("Iteration {}: Deadlock detected!\n{:?}", iteration, deadlocks);
        }
    }
}
```

---

## Lock Ordering Rules

If your component acquires multiple locks, define a global order:

```rust
// GLOBAL LOCK ORDER (document at crate level):
// 1. outer_cache (Arc<RwLock<Cache>>)
// 2. config_state (Arc<Mutex<Config>>)
// 3. per_item_lock (HashMap<Key, Arc<Mutex<Item>>>)

impl MyComponent {
    async fn operation(&mut self, key: &str) {
        // CORRECT: acquire in order 1 → 2 → 3
        let outer = self.outer_cache.write().await;
        let config = self.config_state.lock().await;
        let item_lock = self.items.get(key).unwrap().lock().await;
        // Use locks
    }
    
    async fn other_operation(&self, key: &str) {
        // CORRECT: Same order (skip if not needed)
        let config = self.config_state.lock().await;
        let item_lock = self.items.get(key).unwrap().lock().await;
        // Use locks (did not acquire cache, that's OK)
    }
}
```

---

## Performance Considerations

### Choosing Between Lock Types

**`parking_lot::Mutex<T>`:**
- ✅ Faster than `tokio::sync::Mutex` (no async overhead)
- ✅ No lock poisoning (shorter critical sections)
- ❌ Can't hold across `.await`
- **Use for:** State that doesn't need to be locked during async operations

**`Arc<DashMap<K, V>>`:**
- ✅ Lock-free (no lock contention)
- ✅ Excellent scalability with many readers
- ❌ Higher memory overhead than HashMap
- **Use for:** Registries, caches with high read frequency

**`tokio::sync::Mutex<T>`:**
- ✅ Can hold across `.await`
- ✅ Async-aware (proper scheduling)
- ❌ Slower than `parking_lot`
- **Use for:** State that must be protected during async operations

---

## References

- [Tokio Tutorial: Shared State](https://tokio.rs/tokio/tutorial/select#message-passing-with-channels)
- [Parking Lot Documentation](https://docs.rs/parking_lot/latest/parking_lot/)
- [DashMap: Lock-Free HashMap](https://docs.rs/dashmap/latest/dashmap/)
- [Crossbeam Synchronization](https://docs.rs/crossbeam/latest/crossbeam/)

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | March 15, 2026 | Initial concurrency standards |
