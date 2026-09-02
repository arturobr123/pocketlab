use crate::{
    domain::Deck,
    simulator::{MatchupResult, SimulationArena},
};
use rusqlite::{params, Connection, OptionalExtension};
use std::{path::Path, sync::Mutex};

/// Persistent aggregate cache for deterministic matchup streams.
///
/// A row represents the prefix of the deterministic game stream for
/// `(namespace, deck_a, deck_b, base_seed)`. If a later caller requests more games,
/// PocketLab simulates only the missing suffix and merges it into the row.
pub struct SqliteMatchupCache {
    connection: Mutex<Connection>,
}

impl SqliteMatchupCache {
    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let connection = Connection::open(path)?;
        Self::from_connection(connection)
    }

    pub fn in_memory() -> rusqlite::Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> rusqlite::Result<Self> {
        connection.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;

            CREATE TABLE IF NOT EXISTS matchup_cache (
                namespace TEXT NOT NULL,
                deck_a TEXT NOT NULL,
                deck_b TEXT NOT NULL,
                base_seed TEXT NOT NULL,
                games INTEGER NOT NULL,
                wins_a INTEGER NOT NULL,
                wins_b INTEGER NOT NULL,
                draws INTEGER NOT NULL,
                PRIMARY KEY (namespace, deck_a, deck_b, base_seed)
            );
            ",
        )?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn get(
        &self,
        namespace: &str,
        deck_a: &Deck,
        deck_b: &Deck,
        base_seed: u64,
    ) -> rusqlite::Result<Option<MatchupResult>> {
        let connection = self.connection.lock().expect("SQLite cache mutex poisoned");
        connection
            .query_row(
                "
                SELECT games, wins_a, wins_b, draws
                FROM matchup_cache
                WHERE namespace = ?1 AND deck_a = ?2 AND deck_b = ?3 AND base_seed = ?4
                ",
                params![
                    namespace,
                    deck_a.canonical_key(),
                    deck_b.canonical_key(),
                    base_seed.to_string()
                ],
                |row| {
                    Ok(MatchupResult {
                        games: row.get(0)?,
                        wins_a: row.get(1)?,
                        wins_b: row.get(2)?,
                        draws: row.get(3)?,
                    })
                },
            )
            .optional()
    }

    pub fn put(
        &self,
        namespace: &str,
        deck_a: &Deck,
        deck_b: &Deck,
        base_seed: u64,
        result: &MatchupResult,
    ) -> rusqlite::Result<()> {
        let connection = self.connection.lock().expect("SQLite cache mutex poisoned");
        connection.execute(
            "
            INSERT INTO matchup_cache
                (namespace, deck_a, deck_b, base_seed, games, wins_a, wins_b, draws)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(namespace, deck_a, deck_b, base_seed) DO UPDATE SET
                games = excluded.games,
                wins_a = excluded.wins_a,
                wins_b = excluded.wins_b,
                draws = excluded.draws
            ",
            params![
                namespace,
                deck_a.canonical_key(),
                deck_b.canonical_key(),
                base_seed.to_string(),
                result.games,
                result.wins_a,
                result.wins_b,
                result.draws
            ],
        )?;
        Ok(())
    }
}

pub struct CachedArena<A> {
    inner: A,
    cache: SqliteMatchupCache,
    namespace: String,
}

impl<A> CachedArena<A> {
    pub fn new(inner: A, cache: SqliteMatchupCache, namespace: impl Into<String>) -> Self {
        Self {
            inner,
            cache,
            namespace: namespace.into(),
        }
    }
}

impl<A: SimulationArena> SimulationArena for CachedArena<A> {
    fn simulate(&self, deck_a: &Deck, deck_b: &Deck, games: u32, base_seed: u64) -> MatchupResult {
        let cached = self
            .cache
            .get(&self.namespace, deck_a, deck_b, base_seed)
            .expect("failed to read matchup cache")
            .unwrap_or_default();

        // Returning an already-computed larger prefix is intentional: callers ask
        // for a minimum simulation budget, and retaining extra evidence improves the
        // estimate without rerunning games.
        if cached.games >= games as u64 {
            return cached;
        }

        let missing = games as u64 - cached.games;
        let suffix_seed = base_seed.wrapping_add(cached.games);
        let suffix = self
            .inner
            .simulate(deck_a, deck_b, missing as u32, suffix_seed);
        let combined = cached.merged(&suffix);
        self.cache
            .put(&self.namespace, deck_a, deck_b, base_seed, &combined)
            .expect("failed to update matchup cache");
        combined
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CardId, Deck};
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::atomic::{AtomicU64, Ordering},
    };

    struct CountingArena {
        games_executed: AtomicU64,
    }

    impl SimulationArena for CountingArena {
        fn simulate(&self, _a: &Deck, _b: &Deck, games: u32, seed: u64) -> MatchupResult {
            self.games_executed
                .fetch_add(games as u64, Ordering::SeqCst);
            let wins_a = (0..games)
                .filter(|index| seed.wrapping_add(*index as u64) % 2 == 0)
                .count() as u64;
            MatchupResult {
                games: games as u64,
                wins_a,
                wins_b: games as u64 - wins_a,
                draws: 0,
            }
        }
    }

    fn deck(id: &str) -> Deck {
        Deck::new(
            BTreeMap::from([(CardId(id.into()), 20)]),
            BTreeSet::from(["Water".into()]),
        )
    }

    #[test]
    fn only_simulates_missing_suffix() {
        let cache = SqliteMatchupCache::in_memory().unwrap();
        let arena = CachedArena::new(
            CountingArena {
                games_executed: AtomicU64::new(0),
            },
            cache,
            "test-v1",
        );
        let a = deck("A-001");
        let b = deck("B-001");

        let first = arena.simulate(&a, &b, 10, 100);
        assert_eq!(first.games, 10);
        assert_eq!(arena.inner.games_executed.load(Ordering::SeqCst), 10);

        let second = arena.simulate(&a, &b, 25, 100);
        assert_eq!(second.games, 25);
        assert_eq!(arena.inner.games_executed.load(Ordering::SeqCst), 25);

        let third = arena.simulate(&a, &b, 5, 100);
        assert_eq!(third.games, 25);
        assert_eq!(arena.inner.games_executed.load(Ordering::SeqCst), 25);
    }
}
