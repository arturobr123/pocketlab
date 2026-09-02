# PocketLab

PocketLab is an experimental search system for discovering robust Pokémon TCG Pocket decks rather than merely ranking known meta decks.

The V1 objective is:

> Search all currently simulatable legal cards for 20-card decks that maximize robust simulated win probability across a diverse opponent population.

## Current vertical slice

- PocketLab domain model (`Card`, `CardRegistry`, `Deck`)
- legality checks including the two-copies-by-name rule
- stable canonical deck identity for future matchup caching
- weighted opponent pools
- evaluator with weighted win rate, Wilson 95% lower bound, and CVaR lower-tail score
- deterministic mock arena for optimizer plumbing tests
- first elitist mutation search loop
- in-process `DeckGymArena` adapter using `deckgym::Game`
- deterministic unique seed per simulated game
- GitHub Actions CI (`fmt`, `clippy`, `test`)

## DeckGym integration

`deckgym-core` is pinned to commit:

`fda48391a4747c7d9085e6a95520b731cee0b546`

PocketLab runs DeckGym in-process; it does **not** shell out to the CLI. The adapter converts PocketLab IDs such as `A3b-041` into DeckGym text and then executes games through DeckGym's public Rust API.

The default gameplay policy is currently `WeightedRandomPlayer` (`PlayerCode::W`). This is intentionally temporary. We must calibrate simulated matchup matrices against real tournament matchups before treating optimizer results as competitive evidence.

## Build

```bash
cargo test --all-features
cargo run
```

To work only on optimizer plumbing without fetching DeckGym:

```bash
cargo test --no-default-features
```

## Why AGPL?

DeckGym is AGPL-3.0. PocketLab currently links DeckGym directly, so this repository uses AGPL-3.0 as well to keep distribution/licensing straightforward.

## Next milestone

1. ingest the real card registry and DeckGym implementation status
2. ingest current/historical Limitless decks into an `OpponentPool`
3. add SQLite matchup cache keyed by deck + engine + policy version
4. add progressive racing / successive halving
5. replace single-population mutation search with island evolution
6. add adversarial counter-deck search
7. calibrate player policies against observed tournament matchup matrices
