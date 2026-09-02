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
}

pub trait SimulationArena: Send + Sync {
    fn simulate(&self, deck_a: &Deck, deck_b: &Deck, games: u32, seed: u64) -> MatchupResult;
}

pub struct DeterministicMockArena;

impl SimulationArena for DeterministicMockArena {
    fn simulate(&self, deck_a: &Deck, deck_b: &Deck, games: u32, seed: u64) -> MatchupResult {
        let mut h = seed
            ^ stable_hash(&deck_a.canonical_key())
            ^ stable_hash(&deck_b.canonical_key()).rotate_left(17);
        let mut wins_a = 0;
        for _ in 0..games {
            h ^= h << 13;
            h ^= h >> 7;
            h ^= h << 17;
            if h & 1 == 0 {
                wins_a += 1;
            }
        }
        MatchupResult {
            games: games as u64,
            wins_a,
            wins_b: games as u64 - wins_a,
            draws: 0,
        }
    }
}

fn stable_hash(s: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in s.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
