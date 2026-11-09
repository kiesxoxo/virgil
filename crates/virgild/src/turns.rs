//! Taking turns on the request line. The songs still to play form groups: a run of songs from one
//! request (an album stays whole), or, for songs nobody requested, a run from one album. A group's
//! round is how many groups its owner has before it. A new request goes in the next round of the
//! person asking, after every group of that round or an earlier one.

use std::collections::HashMap;
use std::ops::Range;

use virgil_core::QueueItem;

#[derive(PartialEq)]
enum Key<'a> {
    Request(u32),
    Album(&'a str),
    Song(u32),
}

fn key(item: &QueueItem) -> Key<'_> {
    match (&item.pick, &item.album_id) {
        (Some(pick), _) => Key::Request(pick.request),
        (None, Some(album)) => Key::Album(album),
        (None, None) => Key::Song(item.id),
    }
}

/// Whose turn a song is: a person's id, or `None` for songs nobody requested.
fn owner(item: &QueueItem) -> Option<&str> {
    item.pick.as_ref().map(|pick| pick.by.as_str())
}

/// The groups of `items`, as index ranges in order.
pub fn groups(items: &[QueueItem]) -> Vec<Range<usize>> {
    let mut groups: Vec<Range<usize>> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        match groups.last_mut() {
            Some(last) if key(&items[last.start]) == key(item) => last.end = i + 1,
            _ => groups.push(i..i + 1),
        }
    }
    groups
}

/// Where in `upcoming` (the songs after the current one) a new request from `who` goes.
pub fn place(upcoming: &[QueueItem], who: &str) -> usize {
    let groups = groups(upcoming);
    let turn = groups.iter().filter(|g| owner(&upcoming[g.start]) == Some(who)).count();
    let mut rounds: HashMap<Option<&str>, usize> = HashMap::new();
    let mut at = 0;
    for group in groups {
        let round = rounds.entry(owner(&upcoming[group.start])).or_default();
        if *round <= turn {
            at = group.end;
        }
        *round += 1;
    }
    at
}

#[cfg(test)]
mod tests {
    use virgil_core::Pick;

    use super::*;

    /// `spec` is one song per word: `ola:1` is Ola's request 1, `-:x` a song nobody requested from
    /// album x, `-` one from no known album.
    fn queue(spec: &str) -> Vec<QueueItem> {
        spec.split_whitespace()
            .enumerate()
            .map(|(i, word)| {
                let (who, what) = word.split_once(':').unwrap_or((word, ""));
                let pick = (who != "-").then(|| Pick {
                    by: who.to_string(),
                    request: what.parse().unwrap(),
                    at: 0,
                    note: None,
                });
                QueueItem {
                    id: i as u32,
                    uri: format!("song{i}"),
                    title: String::new(),
                    artist: String::new(),
                    album: String::new(),
                    duration: 0.0,
                    format: None,
                    album_id: (who == "-" && !what.is_empty()).then(|| what.to_string()),
                    art: None,
                    pick,
                }
            })
            .collect()
    }

    #[test]
    fn an_empty_queue_takes_it_at_once() {
        assert_eq!(place(&[], "ola"), 0);
    }

    #[test]
    fn albums_stay_whole_and_people_take_turns() {
        // Ola's album is one turn: Jan's song goes after all of it.
        assert_eq!(place(&queue("ola:1 ola:1 ola:1 ola:1"), "jan"), 4);
        // Kai's first request comes before anyone's second.
        assert_eq!(place(&queue("ola:1 ola:1 jan:2"), "kai"), 3);
        // Out of turn already (the line opened late): Kai still follows the last first request.
        assert_eq!(place(&queue("ola:1 ola:2 jan:3"), "kai"), 3);
        // Ola's second request waits for Jan's and Kai's first.
        assert_eq!(place(&queue("ola:1 ola:1 jan:2 kai:3"), "ola"), 4);
        // Jan's second goes after Ola's second, which is in the same round.
        assert_eq!(place(&queue("ola:1 jan:2 kai:3 ola:4"), "jan"), 4);
    }

    #[test]
    fn songs_nobody_requested_take_turns_album_by_album() {
        // The queue from before the line opened: a guest goes after its first album, not all of it.
        assert_eq!(place(&queue("-:x -:x -:x -:y -:y"), "ola"), 3);
        assert_eq!(place(&queue("-:x -:x ola:1 -:y"), "jan"), 3);
        // Songs from no known album are a turn each.
        assert_eq!(place(&queue("- - -"), "ola"), 1);
    }

    #[test]
    fn a_request_split_by_a_move_counts_twice() {
        assert_eq!(groups(&queue("ola:1 jan:2 ola:1")), [0..1, 1..2, 2..3]);
        assert_eq!(place(&queue("ola:1 jan:2 ola:1"), "kai"), 2);
    }
}
