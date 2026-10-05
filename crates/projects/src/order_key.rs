//! t3code's fractional order keys (`threadSort.ts`), which arrange the pinned threads and the
//! rest by hand. A key is a base-26 string of `a`–`z`, read as a fraction and compared as a
//! string, so a thread moves with one new key between its new neighbors' and nothing else
//! changes: neighbors on other machines are never written.

use std::cmp::Ordering;
use std::hash::Hash;
use std::time::SystemTime;

use collections::{HashMap, HashSet};

const DIGITS: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const BASE: usize = DIGITS.len();

fn digit(character: u8) -> usize {
    (character - DIGITS[0]) as usize
}

/// Whether `key` is one the generators make. A key ending in `a` would leave no room just
/// before it, so it never is one.
pub fn is_valid(key: &str) -> bool {
    !key.is_empty()
        && key.bytes().all(|character| character.is_ascii_lowercase())
        && !key.ends_with(char::from(DIGITS[0]))
}

/// The midpoint of two digit strings read as fractions in (0, 1), where an empty `b` is the
/// open upper bound. Requires `a < b`.
fn midpoint(a: &[u8], b: &[u8]) -> Vec<u8> {
    if !b.is_empty() {
        // Past the longest common prefix, `a` padded with the lowest digit.
        let mut prefix = 0;
        while prefix < b.len() && a.get(prefix).copied().unwrap_or(DIGITS[0]) == b[prefix] {
            prefix += 1;
        }
        if prefix > 0 {
            let mut key = b[..prefix].to_vec();
            key.extend(midpoint(a.get(prefix..).unwrap_or(&[]), &b[prefix..]));
            return key;
        }
    }
    let digit_a = a.first().map_or(0, |character| digit(*character));
    let digit_b = b.first().map_or(BASE, |character| digit(*character));
    if digit_b > digit_a + 1 {
        return vec![DIGITS[(digit_a + digit_b).div_ceil(2)]];
    }
    // Consecutive leading digits: shorten into `b`'s spare digits, or extend `a`, never ending
    // on the lowest digit (the midpoint of two open bounds is the middle of the alphabet).
    if b.len() > 1 {
        return vec![b[0]];
    }
    let mut key = vec![DIGITS[digit_a]];
    key.extend(midpoint(a.get(1..).unwrap_or(&[]), &[]));
    key
}

/// A key that sorts strictly between two neighbors': `None` for `before` is the start of the
/// list, and for `after` its end. `None` when a neighbor's key is corrupt or they're out of
/// order, for the caller to rewrite the list's keys instead.
pub fn between(before: Option<&str>, after: Option<&str>) -> Option<String> {
    if before.is_some_and(|key| !is_valid(key)) || after.is_some_and(|key| !is_valid(key)) {
        return None;
    }
    if let (Some(before), Some(after)) = (before, after)
        && before >= after
    {
        return None;
    }
    let key = midpoint(
        before.unwrap_or_default().as_bytes(),
        after.unwrap_or_default().as_bytes(),
    );
    String::from_utf8(key).ok()
}

/// A key before all of `keys`: where a new pin goes, at the top of the pinned threads.
pub fn before_all<'a>(keys: impl IntoIterator<Item = &'a str>) -> Option<String> {
    between(None, keys.into_iter().filter(|key| is_valid(key)).min())
}

/// `count` evenly spaced keys, in order, for writing a whole list's order. Longer keys keep a
/// long list from running out of room between neighbors.
pub fn spread(count: usize) -> Vec<String> {
    let mut width = 2;
    let mut space = BASE * BASE;
    while space <= (count + 1) * 2 {
        width += 1;
        space *= BASE;
    }
    let step = space as f64 / (count + 1) as f64;
    (0..count)
        .map(|index| {
            let mut value = (step * (index + 1) as f64).round() as usize;
            // A value whose lowest digit is the lowest would end the key in `a`.
            if value.is_multiple_of(BASE) {
                value += 1;
            }
            let mut key = vec![DIGITS[0]; width];
            for place in (0..width).rev() {
                key[place] = DIGITS[value % BASE];
                value /= BASE;
            }
            String::from_utf8_lossy(&key).into_owned()
        })
        .collect()
}

/// t3code's `planPinnedReorder`: the keys to write so a list shows `order`, after `moved` was
/// put where it is in it. Between keyed neighbors (or an end) that's one key, for `moved`.
/// Beside a neighbor without a key, the whole list gets new ones, once. `keys` has every keyed
/// item, including ones the list doesn't show, whose keys are kept free.
pub fn plan_reorder<T: Copy + Eq + Hash>(
    order: &[T],
    keys: &HashMap<T, String>,
    moved: T,
) -> Vec<(T, String)> {
    let shown: HashSet<T> = order.iter().copied().collect();
    let reserved: HashSet<&str> = keys
        .iter()
        .filter(|(item, _)| !shown.contains(item))
        .map(|(_, key)| key.as_str())
        .collect();
    let Some(index) = order.iter().position(|item| *item == moved) else {
        return Vec::new();
    };
    let before = index.checked_sub(1).map(|index| order[index]);
    let after = order.get(index + 1).copied();
    let before_key = before.and_then(|item| keys.get(&item)).map(String::as_str);
    let after_key = after.and_then(|item| keys.get(&item)).map(String::as_str);
    if before.is_none_or(|_| before_key.is_some()) && after.is_none_or(|_| after_key.is_some()) {
        let mut key = between(before_key, after_key);
        while let Some(taken) = key.as_deref().filter(|key| reserved.contains(key)) {
            key = between(Some(taken), after_key);
        }
        if let Some(key) = key {
            return vec![(moved, key)];
        }
    }
    let fresh: Vec<String> = spread(order.len() + reserved.len())
        .into_iter()
        .filter(|key| !reserved.contains(key.as_str()))
        .take(order.len())
        .collect();
    order
        .iter()
        .zip(fresh)
        .filter(|(item, key)| keys.get(item) != Some(key))
        .map(|(item, key)| (*item, key))
        .collect()
}

/// How an arranged list orders two items by their keys and creation times: by key, and those
/// without one newest first, ahead of the keyed ones (`keyless_first`, the unpinned threads,
/// so new threads lead) or after them (the pinned ones). Ties are the caller's to break.
pub fn compare(
    (a_key, a_created): (Option<&str>, Option<SystemTime>),
    (b_key, b_created): (Option<&str>, Option<SystemTime>),
    keyless_first: bool,
) -> Ordering {
    match (a_key, b_key) {
        (Some(a), Some(b)) => a.cmp(b),
        (None, None) => b_created.cmp(&a_created),
        (None, Some(_)) if keyless_first => Ordering::Less,
        (Some(_), None) if keyless_first => Ordering::Greater,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn keys_sort_between_their_bounds() {
        let middle = between(None, None).unwrap_or_default();
        let top = between(None, Some(&middle)).unwrap_or_default();
        let bottom = between(Some(&middle), None).unwrap_or_default();
        assert!(top < middle && middle < bottom, "{top} {middle} {bottom}");
        let inside = between(Some(&top), Some(&middle)).unwrap_or_default();
        assert!(top < inside && inside < middle);

        let key = between(Some("g"), Some("h")).unwrap_or_default();
        assert!("g" < key.as_str() && key.as_str() < "h", "{key}");
    }

    #[test]
    fn keys_stay_ordered_under_repeated_insertion() {
        let mut head: Option<String> = None;
        let mut seen = HashSet::default();
        for _ in 0..100 {
            let key = between(None, head.as_deref()).unwrap_or_default();
            assert!(is_valid(&key));
            if let Some(head) = &head {
                assert!(&key < head);
            }
            assert!(seen.insert(key.clone()));
            head = Some(key);
        }

        let mut low = between(None, None).unwrap_or_default();
        let mut high = between(Some(&low), None).unwrap_or_default();
        for index in 0..100 {
            let key = between(Some(&low), Some(&high)).unwrap_or_default();
            assert!(low < key && key < high, "{low} {key} {high}");
            if index % 2 == 0 {
                low = key;
            } else {
                high = key;
            }
        }
    }

    #[test]
    fn corrupt_or_unordered_bounds_have_no_key_between() {
        assert_eq!(between(Some("z"), Some("a")), None);
        assert_eq!(between(Some("A!"), None), None);
        assert_eq!(between(None, Some("ma")), None);
        assert_eq!(between(Some("m"), Some("m")), None);
        assert_eq!(
            before_all(["f", "ma", "c"]).filter(|key| key.as_str() < "c"),
            { before_all(["c"]) }
        );
    }

    #[test]
    fn spread_keys_leave_room_before_each() {
        for count in [0, 1, 650, 675, 676, 1_001, 2_000] {
            let keys = spread(count);
            assert_eq!(keys.len(), count);
            assert!(keys.windows(2).all(|pair| pair[0] < pair[1]), "{count}");
            for (index, key) in keys.iter().enumerate() {
                assert!(is_valid(key), "{key}");
                let before = index.checked_sub(1).map(|index| keys[index].as_str());
                let inside = between(before, Some(key)).unwrap_or_default();
                assert!(&inside < key);
                if let Some(before) = before {
                    assert!(inside.as_str() > before);
                }
            }
        }
    }

    fn keyed<'a>(keys: impl IntoIterator<Item = (&'a str, &'a str)>) -> HashMap<&'a str, String> {
        keys.into_iter()
            .map(|(item, key)| (item, key.to_string()))
            .collect()
    }

    #[test]
    fn a_move_between_keyed_neighbors_writes_one_key_avoiding_hidden_ones() {
        let hidden = between(Some("f"), Some("t")).unwrap_or_default();
        let keys = keyed([("a", "f"), ("b", "t"), ("moved", "z"), ("hidden", &hidden)]);
        let writes = plan_reorder(&["a", "moved", "b"], &keys, "moved");
        assert_eq!(writes.len(), 1);
        let (item, key) = &writes[0];
        assert_eq!(*item, "moved");
        assert!(key.as_str() > "f" && key.as_str() < "t" && *key != hidden);

        let keys = keyed([("a", "f"), ("b", "m"), ("c", "t")]);
        let writes = plan_reorder(&["a", "c", "b"], &keys, "c");
        assert_eq!(writes.len(), 1);
        assert!(writes[0].1.as_str() > "f" && writes[0].1.as_str() < "m");
    }

    #[test]
    fn a_keyless_neighbor_rewrites_the_list_around_hidden_keys() {
        let reserved = spread(6);
        let ids = ["h0", "h1", "h2", "h3", "h4", "h5"];
        let keys: HashMap<&str, String> = ids.into_iter().zip(reserved.iter().cloned()).collect();
        let writes = plan_reorder(&["c", "a", "b"], &keys, "c");
        assert_eq!(
            writes.iter().map(|(item, _)| *item).collect::<Vec<_>>(),
            ["c", "a", "b"]
        );
        assert!(writes.windows(2).all(|pair| pair[0].1 < pair[1].1));
        assert!(writes.iter().all(|(_, key)| !reserved.contains(key)));
    }

    /// Sorts `items` (id, key, minutes since a start) as the unpinned threads are.
    fn arranged(items: &[(&'static str, Option<String>, u64)]) -> Vec<&'static str> {
        let start = SystemTime::UNIX_EPOCH;
        let mut items = items.to_vec();
        items.sort_by(|(a, a_key, a_created), (b, b_key, b_created)| {
            compare(
                (
                    a_key.as_deref(),
                    Some(start + Duration::from_secs(a_created * 60)),
                ),
                (
                    b_key.as_deref(),
                    Some(start + Duration::from_secs(b_created * 60)),
                ),
                true,
            )
            .then(a.cmp(b))
        });
        items.into_iter().map(|(id, _, _)| id).collect()
    }

    #[test]
    fn new_threads_lead_the_arranged_ones() {
        let order = arranged(&[
            ("arranged-first", Some("f".into()), 9),
            ("new", None, 11),
            ("arranged-last", Some("t".into()), 12),
            ("older", None, 1),
        ]);
        assert_eq!(order, ["new", "older", "arranged-first", "arranged-last"]);
    }

    #[test]
    fn every_move_in_a_mixed_list_shows_as_asked() {
        let items: Vec<(&'static str, Option<String>, u64)> = vec![
            ("0", None, 6),
            ("1", None, 5),
            ("2", None, 4),
            ("3", Some("f".into()), 3),
            ("4", Some("m".into()), 2),
            ("5", Some("t".into()), 1),
        ];
        let ids: Vec<&'static str> = items.iter().map(|(id, _, _)| *id).collect();
        let keys: HashMap<&'static str, String> = items
            .iter()
            .filter_map(|(id, key, _)| Some((*id, key.clone()?)))
            .collect();
        for moved in &ids {
            for target in 0..ids.len() {
                let mut wanted: Vec<&'static str> =
                    ids.iter().copied().filter(|id| id != moved).collect();
                wanted.insert(target, moved);
                let writes: HashMap<&'static str, String> =
                    plan_reorder(&wanted, &keys, moved).into_iter().collect();
                let updated: Vec<_> = items
                    .iter()
                    .map(|(id, key, created)| {
                        (*id, writes.get(id).cloned().or(key.clone()), *created)
                    })
                    .collect();
                assert_eq!(arranged(&updated), wanted, "{moved} to {target}");
            }
        }
    }

    #[test]
    fn a_keyless_thread_moves_into_the_arranged_ones_with_one_key() {
        let keys = keyed([("first", "f"), ("last", "t")]);
        let writes = plan_reorder(&["new", "first", "moved", "last"], &keys, "moved");
        assert_eq!(writes.len(), 1);
        assert!(writes[0].1.as_str() > "f" && writes[0].1.as_str() < "t");
    }

    #[test]
    fn a_long_list_keeps_its_order_when_rewritten() {
        let ids: Vec<String> = (0..1_200).map(|index| index.to_string()).collect();
        let order: Vec<&str> = ids.iter().rev().map(String::as_str).collect();
        let writes: HashMap<&str, String> = plan_reorder(&order, &HashMap::default(), order[0])
            .into_iter()
            .collect();
        let mut sorted = order.clone();
        sorted.sort_by_key(|id| writes.get(id).cloned());
        assert_eq!(sorted, order);
    }
}
