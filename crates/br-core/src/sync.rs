//! Small blocking-thread primitives for the model layer: a counting semaphore
//! (`createConcurrencyLimiter`) and per-key de-duplication of in-flight work
//! (`identifyInFlight` / `ThumbnailModel.inFlight`).

use std::collections::HashMap;
use std::hash::Hash;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct Semaphore {
    free: Mutex<usize>,
    cv: Condvar,
}

pub struct Permit<'a>(&'a Semaphore);

impl Semaphore {
    pub fn new(permits: usize) -> Self {
        Self {
            free: Mutex::new(permits.max(1)),
            cv: Condvar::new(),
        }
    }

    pub fn acquire(&self) -> Permit<'_> {
        let mut free = lock(&self.free);
        while *free == 0 {
            free = self.cv.wait(free).unwrap_or_else(|e| e.into_inner());
        }
        *free -= 1;
        Permit(self)
    }
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        *lock(&self.0.free) += 1;
        self.0.cv.notify_one();
    }
}

struct Call<V> {
    result: Mutex<Option<Option<V>>>,
    cv: Condvar,
}

impl<V: Clone> Call<V> {
    /// `None` when the leader panicked.
    fn wait(&self) -> Option<V> {
        let mut r = lock(&self.result);
        while r.is_none() {
            r = self.cv.wait(r).unwrap_or_else(|e| e.into_inner());
        }
        r.clone().flatten()
    }
}

/// Runs `f` once per key at a time; concurrent callers with the same key wait for that run and
/// share its result.
pub struct SingleFlight<K, V> {
    calls: Mutex<HashMap<K, Arc<Call<V>>>>,
}

impl<K, V> Default for SingleFlight<K, V> {
    fn default() -> Self {
        Self {
            calls: Mutex::new(HashMap::new()),
        }
    }
}

impl<K: Hash + Eq + Clone, V: Clone> SingleFlight<K, V> {
    pub fn run(&self, key: &K, f: impl FnOnce() -> V) -> V {
        let mut f = Some(f);
        loop {
            let call = {
                let mut calls = lock(&self.calls);
                if let Some(existing) = calls.get(key) {
                    Err(existing.clone())
                } else {
                    let call = Arc::new(Call {
                        result: Mutex::new(None),
                        cv: Condvar::new(),
                    });
                    calls.insert(key.clone(), call.clone());
                    Ok(call)
                }
            };
            match call {
                Err(existing) => {
                    if let Some(v) = existing.wait() {
                        return v;
                    }
                    // The leader panicked: try to lead ourselves.
                }
                Ok(call) => {
                    let out = catch_unwind(AssertUnwindSafe(f.take().expect("leads at most once")));
                    lock(&self.calls).remove(key);
                    *lock(&call.result) = Some(out.as_ref().ok().cloned());
                    call.cv.notify_all();
                    match out {
                        Ok(v) => return v,
                        Err(panic) => resume_unwind(panic),
                    }
                }
            }
        }
    }

    /// Blocks until the run in flight for `key` (if any) has finished.
    pub fn wait_idle(&self, key: &K) {
        let call = lock(&self.calls).get(key).cloned();
        if let Some(call) = call {
            let _ = call.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn semaphore_bounds_concurrency() {
        let sem = Semaphore::new(2);
        let (active, peak) = (AtomicUsize::new(0), AtomicUsize::new(0));
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    let _p = sem.acquire();
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    active.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
        assert!(peak.load(Ordering::SeqCst) <= 2);
    }

    #[test]
    fn single_flight_shares_one_run() {
        let flight: SingleFlight<String, usize> = SingleFlight::default();
        let runs = AtomicUsize::new(0);
        let results: Vec<usize> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..6)
                .map(|_| {
                    s.spawn(|| {
                        flight.run(&"k".to_string(), || {
                            std::thread::sleep(std::time::Duration::from_millis(50));
                            runs.fetch_add(1, Ordering::SeqCst) + 100
                        })
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert_eq!(runs.load(Ordering::SeqCst), 1);
        assert!(results.iter().all(|&r| r == 100));
        // A later call runs again.
        assert_eq!(flight.run(&"k".to_string(), || 7), 7);
    }
}
