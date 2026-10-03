//! Approximation of `a.localeCompare(b, undefined, { numeric: true })` (ICU root collation, default
//! sensitivity), used to order archive pages. Page order defines page numbers and therefore saved
//! reading progress, so it must match what the Bun server produced.
//!
//! Primary level: whitespace < punctuation < digits (compared by value) < letters, letters
//! case-insensitively. Ties fall back to "lowercase before uppercase", then code points.

use std::cmp::Ordering;

/// ICU's relative order of the ASCII punctuation/symbols.
const PUNCT_ORDER: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Class {
    Space,
    Punct(usize),
    Digit,
    Letter,
}

fn class(c: char) -> Class {
    if c.is_whitespace() {
        Class::Space
    } else if c.is_ascii_digit() {
        Class::Digit
    } else if let Some(i) = PUNCT_ORDER.find(c) {
        Class::Punct(i)
    } else if c.is_alphabetic() {
        Class::Letter
    } else {
        // Other symbols sort after ASCII punctuation, before digits.
        Class::Punct(PUNCT_ORDER.len() + c as usize)
    }
}

fn digit_run(it: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut s = String::new();
    while let Some(c) = it.next_if(char::is_ascii_digit) {
        s.push(c);
    }
    s.trim_start_matches('0').to_string()
}

/// Strip common Latin diacritics for the primary comparison (`é` ~ `e`), enough for file names.
fn fold(c: char) -> char {
    let l = c.to_lowercase().next().unwrap_or(c);
    match l {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ý' | 'ÿ' => 'y',
        other => other,
    }
}

pub fn locale_compare_numeric(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => break,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(p), Some(q)) => {
                let (cp, cq) = (class(p), class(q));
                if cp == Class::Digit && cq == Class::Digit {
                    let (n, m) = (digit_run(&mut x), digit_run(&mut y));
                    let ord = n.len().cmp(&m.len()).then_with(|| n.cmp(&m));
                    if ord != Ordering::Equal {
                        return ord;
                    }
                    continue;
                }
                let ord = cp.cmp(&cq).then_with(|| fold(p).cmp(&fold(q)));
                if ord != Ordering::Equal {
                    return ord;
                }
                x.next();
                y.next();
            }
        }
    }
    // Same primary key: accents, then lowercase before uppercase, then code points.
    let accents = a.chars().zip(b.chars()).find(|(p, q)| {
        let (lp, lq) = (p.to_lowercase().next(), q.to_lowercase().next());
        lp != lq
    });
    if let Some((p, q)) = accents {
        return p.to_lowercase().cmp(q.to_lowercase());
    }
    let case = a.chars().zip(b.chars()).find(|(p, q)| p != q);
    match case {
        Some((p, q)) if p.is_lowercase() && q.is_uppercase() => Ordering::Less,
        Some((p, q)) if p.is_uppercase() && q.is_lowercase() => Ordering::Greater,
        _ => a.cmp(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(mut v: Vec<&str>) -> Vec<&str> {
        v.sort_by(|a, b| locale_compare_numeric(a, b));
        v
    }

    #[test]
    fn numbers_compare_by_value() {
        assert_eq!(sorted(vec!["page10.png", "page2.png", "page1.png", "page11.png"]), ["page1.png", "page2.png", "page10.png", "page11.png"]);
    }

    #[test]
    fn leading_zeros_do_not_change_the_value() {
        assert_eq!(sorted(vec!["p010.jpg", "p002.jpg", "p100.jpg"]), ["p002.jpg", "p010.jpg", "p100.jpg"]);
    }

    #[test]
    fn case_is_secondary_lowercase_first() {
        assert_eq!(sorted(vec!["B.png", "a.png", "A.png", "b.png"]), ["a.png", "A.png", "b.png", "B.png"]);
    }

    #[test]
    fn punctuation_sorts_before_digits_and_letters() {
        assert_eq!(sorted(vec!["a1", "a_1", "a-1", "aa"]), ["a_1", "a-1", "a1", "aa"]);
    }

    #[test]
    fn prefix_sorts_first() {
        assert_eq!(sorted(vec!["page1.png", "page"]), ["page", "page1.png"]);
    }
}
