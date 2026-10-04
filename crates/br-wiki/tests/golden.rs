//! `fuse` and `jsort` against output recorded from the real Fuse.js 7.4.2 / V8
//! (`tests/fixtures/gen_fuse_golden.mjs`).

use br_wiki::{fuse, jsort};
use serde_json::Value;

fn golden() -> Value {
    serde_json::from_str(include_str!("fixtures/fuse_golden.json")).unwrap()
}

#[test]
fn fuse_matches_the_real_library() {
    let g = golden();
    let cases = g["fuseCases"].as_array().unwrap();
    assert!(cases.len() > 400);
    let mut compared = 0;
    for case in cases {
        let titles: Vec<&str> = case["titles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap())
            .collect();
        let query = case["query"].as_str().unwrap();
        let want = case["results"].as_array().unwrap();
        let got = fuse::search(&titles, query);
        assert_eq!(
            got.len(),
            want.len(),
            "hit count for {query:?} in {titles:?}"
        );
        for (g, w) in got.iter().zip(want) {
            assert_eq!(
                g.idx as u64,
                w["idx"].as_u64().unwrap(),
                "order for {query:?} in {titles:?}"
            );
            match (g.score, w["score"].as_f64()) {
                (Some(a), Some(b)) => assert!(
                    (a - b).abs() <= 1e-9 * b.abs().max(1.0),
                    "score {a} vs {b} for {query:?} / {:?}",
                    titles[g.idx]
                ),
                (None, None) => {}
                other => panic!("score mismatch {other:?} for {query:?}"),
            }
            compared += 1;
        }
    }
    assert!(
        compared > 300,
        "only {compared} hits compared; the generator should produce matches"
    );
}

#[test]
fn v8_sort_reproduces_nan_comparators() {
    let g = golden();
    let cases = g["sortCases"].as_array().unwrap();
    for case in cases {
        // `null` stands for NaN in the fixture.
        let scores: Vec<f64> = case["scores"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_f64().unwrap_or(f64::NAN))
            .collect();
        let want: Vec<usize> = case["order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i.as_u64().unwrap() as usize)
            .collect();
        let mut items: Vec<(usize, f64)> = scores.iter().copied().enumerate().collect();
        jsort::sort_by(&mut items, |a, b| b.1 - a.1);
        let got: Vec<usize> = items.iter().map(|x| x.0).collect();
        assert_eq!(got, want, "scores {scores:?}");
    }
}
