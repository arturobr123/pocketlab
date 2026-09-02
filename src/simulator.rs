use crate::domain::Deck;

#[derive(Clone, Debug, Default)]
pub struct MatchupResult {
    pub games: u64,
    pub wins_a: u64,
    pub wins_b: u64,
    pub draws: u64,
}

impl MatchupResult {
    pub fn win_rate_a(&self) -> f64 {
        if self.games == 0 {
            return 0.0;
        }
        (self.wins_a as f64 + 0.5 * self.draws as f64) / self.games as f64
    }

    pub fn merged(&self, other: &Self) -> Self {
        Self {
            games: self.games + other.games,
            wins_a: self.wins_a + other.wins_a,
            wins_b: self.wins_b + other.wins_b,
            draws: self.draws + other.draws,
        }
    }
}

/// A deterministic arena must treat `seed` as the first seed in a contiguous game
/// stream: simulating `n` games from seed `s`, then `m` games from `s + n`, must be
/// equivalent to simulating `n + m` games from `s`. The SQLite cache relies on this
/// contract to append only missing simulations.
pub trait SimulationArena: Send + Sync {
    fn simulate(&self, deck_a: &Deck, deck_b: &Deck, games: u32, seed: u64) -> MatchupResult;
}

pub struct DeterministicMockArena;

impl SimulationArena for DeterministicMockArena {
    fn simulate(&self, deck_a: &Deck, deck_b: &Deck, games: u32, seed: u64) -> MatchupResult {
        let matchup_hash = stable_hash(&deck_a.canonical_key())
            ^ stable_hash(&deck_b.canonical_key()).rotate_left(17);
        let wins_a = (0..games)
            .filter(|index| splitmix64(seed.wrapping_add(*index as u64)) ^ matchup_hash & 1 == 0)
            .count() as u64;
        MatchupResult {
            games: games as u64,
            wins_a,
            wins_b: games as u64 - wins_a,
            draws: 0,
        }
    }
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

fn stable_hash(s: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in s.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::CardId;
    use std::collections::{BTreeMap, BTreeSet};

    fn deck(id: &str) -> Deck {
        Deck::new(
            BTreeMap::from([(CardId(id.into()), 20)]),
            BTreeSet::from(["Water".into()]),
        )
    }

    #[test]
    fn deterministic_stream_can_be_extended_by_seed_offset() {
        let arena = DeterministicMockArena;
        let a = deck("A-001");
        let b = deck("B-001");
        let first = arena.simulate(&a, &b, 10, 50);
        let suffix = arena.simulate(&a, &b, 15, 60);
        let whole = arena.simulate(&a, &b, 25, 50);
        assert_eq!(first.merged(&suffix).games, whole.games);
        assert_eq!(first.merged(&suffix).wins_a, whole.wins_a);
        assert_eq!(first.merged(&suffix).wins_b, whole.wins_b);
    }
}
