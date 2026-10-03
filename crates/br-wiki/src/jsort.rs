//! `Array.prototype.sort` as V8 runs it, for a comparator that may return `NaN`.
//!
//! The DC picker scores candidates with `parseInt("March")`, which is `NaN`, and sorts with
//! `b.score - a.score`. A `NaN` result is read as "equal", which makes the comparator
//! inconsistent, so the winner depends on the exact algorithm. V8's TimSort handles fewer than
//! 64 elements as one run (`CountAndMakeRun`) plus a binary insertion sort; that is reproduced
//! here. Longer inputs fall back to a stable sort (they never occur: a search returns <= 50 hits).

use std::cmp::Ordering;

/// Sorts `items` like `items.sort(cmp)` where `cmp` returns the comparator's number.
pub fn sort_by<T: Clone>(items: &mut Vec<T>, cmp: impl Fn(&T, &T) -> f64) {
    let order = |a: &T, b: &T| -> f64 {
        let v = cmp(a, b);
        if v.is_nan() { 0.0 } else { v }
    };
    let n = items.len();
    if n < 2 {
        return;
    }
    if n >= 64 {
        items.sort_by(|a, b| order(a, b).partial_cmp(&0.0).unwrap_or(Ordering::Equal));
        return;
    }

    // CountAndMakeRun(0, n)
    let mut run = 2;
    let descending = order(&items[1], &items[0]) < 0.0;
    let mut previous = 1;
    for idx in 2..n {
        let o = order(&items[idx], &items[previous]);
        if descending { if o >= 0.0 { break; } } else if o < 0.0 { break; }
        previous = idx;
        run += 1;
    }
    if descending {
        items[..run].reverse();
    }

    // BinaryInsertionSort(0, run, n)
    for start in run..n {
        let pivot = items[start].clone();
        let (mut left, mut right) = (0, start);
        while left < right {
            let mid = left + ((right - left) >> 1);
            if order(&pivot, &items[mid]) < 0.0 { right = mid } else { left = mid + 1 }
        }
        for p in (left + 1..=start).rev() {
            items[p] = items[p - 1].clone();
        }
        items[left] = pivot;
    }
}
