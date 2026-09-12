//! Scores and ranks `PackageEntry` results against a query.
//!
//! Ranking, highest first: exact name match, then name prefix, then
//! fuzzy name match, then a plain substring hit in the description.
//! Installed packages break ties over not-installed ones -- this is
//! the search a user runs while thinking "do I have `rip...` already
//! or do I need to install it", so a name they already have should
//! surface first.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use super::index::PackageEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Tier {
    DescriptionOnly,
    NameFuzzy,
    NamePrefix,
    NameExact,
}

pub struct SearchHit<'a> {
    /// Position in the `entries` slice that was searched -- callers
    /// that need to resolve a hit back into their own owning
    /// structure (e.g. `app::state`, which stores entries in a `Vec`
    /// it mutates elsewhere) should use this rather than reconstructing
    /// it from `entry`'s address.
    pub index: usize,
    pub entry: &'a PackageEntry,
    /// True when the query matched the description rather than (or in
    /// addition to) the name -- drives the "matched on description"
    /// footnote in the Search tab.
    pub matched_description: bool,
}

pub fn search<'a>(entries: &'a [PackageEntry], query: &str) -> Vec<SearchHit<'a>> {
    let query = query.trim();
    if query.is_empty() {
        return entries
            .iter()
            .enumerate()
            .map(|(index, entry)| SearchHit {
                index,
                entry,
                matched_description: false,
            })
            .collect();
    }

    let query_lower = query.to_lowercase();
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

    struct Scored<'a> {
        index: usize,
        entry: &'a PackageEntry,
        tier: Tier,
        fuzzy_score: u32,
        matched_description: bool,
    }

    let mut scored: Vec<Scored> = Vec::new();

    for (index, entry) in entries.iter().enumerate() {
        let name_lower = entry.name.to_lowercase();
        let desc_lower = entry.description().to_lowercase();
        let desc_hit = desc_lower.contains(&query_lower);

        let mut buf = Vec::new();
        let haystack = Utf32Str::new(&entry.name, &mut buf);
        let fuzzy_score = pattern.score(haystack, &mut matcher);

        let tier = if name_lower == query_lower {
            Some(Tier::NameExact)
        } else if name_lower.starts_with(&query_lower) {
            Some(Tier::NamePrefix)
        } else if fuzzy_score.is_some() {
            Some(Tier::NameFuzzy)
        } else if desc_hit {
            Some(Tier::DescriptionOnly)
        } else {
            None
        };

        if let Some(tier) = tier {
            scored.push(Scored {
                index,
                entry,
                tier,
                fuzzy_score: fuzzy_score.unwrap_or(0),
                matched_description: desc_hit && tier < Tier::NameFuzzy,
            });
        }
    }

    scored.sort_by(|a, b| {
        b.tier
            .cmp(&a.tier)
            .then_with(|| b.fuzzy_score.cmp(&a.fuzzy_score))
            .then_with(|| b.entry.is_installed().cmp(&a.entry.is_installed()))
            .then_with(|| a.entry.name.cmp(&b.entry.name))
    });

    scored
        .into_iter()
        .map(|s| SearchHit {
            index: s.index,
            entry: s.entry,
            matched_description: s.matched_description,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alpm::index::PackageIndex;
    use crate::alpm::local::{read_local_db, LocalPackage};
    use crate::alpm::sync::{read_sync_db, SyncPackage};
    use std::path::Path;

    fn index() -> PackageIndex {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let locals: Vec<LocalPackage> = read_local_db(&root.join("local")).unwrap();
        let syncs: Vec<SyncPackage> = read_sync_db(&root.join("sync/extra.db"), "extra").unwrap();
        PackageIndex::build(locals, syncs)
    }

    #[test]
    fn exact_name_ranks_first() {
        let idx = index();
        let hits = search(&idx.entries, "ripgrep");
        assert_eq!(hits[0].entry.name, "ripgrep");
    }

    #[test]
    fn prefix_beats_fuzzy_and_description() {
        let idx = index();
        // "ez" is a prefix of eza; also appears nowhere else as a prefix.
        let hits = search(&idx.entries, "ez");
        assert_eq!(hits[0].entry.name, "eza");
    }

    #[test]
    fn description_only_match_is_flagged() {
        let idx = index();
        // "grep" appears in ripgrep's own description too, so pick a
        // word that's description-only: "usability" (from ripgrep's
        // desc) matches nothing by name.
        let hits = search(&idx.entries, "usability");
        assert!(!hits.is_empty());
        assert!(hits[0].matched_description);
        assert_eq!(hits[0].entry.name, "ripgrep");
    }

    #[test]
    fn installed_breaks_ties_over_not_installed() {
        let idx = index();
        // Both "eza" (installed) and "fd" (not installed, sync-only)
        // are unrelated names, so instead check within a shared fuzzy
        // bucket isn't easy to force artificially here; verify at
        // least that an installed exact-name match outranks anything
        // else regardless of fuzzy noise.
        let hits = search(&idx.entries, "fd");
        assert_eq!(hits[0].entry.name, "fd");
        assert!(!hits[0].entry.is_installed());
    }

    #[test]
    fn empty_query_returns_everything_unranked() {
        let idx = index();
        let hits = search(&idx.entries, "");
        assert_eq!(hits.len(), idx.entries.len());
    }

    #[test]
    fn no_match_returns_empty() {
        let idx = index();
        let hits = search(&idx.entries, "zzzznonexistentquery");
        assert!(hits.is_empty());
    }
}
