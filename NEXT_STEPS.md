# Next steps

## M1 — Real data ingestion

- Import every legal card into `CardRegistry`.
- Mark a card searchable only when DeckGym fully implements its mechanics.
- Normalize card IDs to `SET-NUMBER`, e.g. `A3b-041` and `P-A-005`.
- Import known decklists from Limitless and assign opponent kinds/weights.

## M2 — Matchup cache

Add SQLite with a cache key containing:

- deck A canonical hash
- deck B canonical hash
- DeckGym commit
- player policy + parameters
- rules/card database version

Store aggregate wins/losses/draws and the next deterministic seed index so later requests can add simulations instead of repeating them.

## M3 — Progressive racing

Do not fully simulate every candidate. Example budget ladder:

1. 16 opponents × 20 games
2. survivors: 32 opponents × 100 games
3. survivors: 64 opponents × 500 games
4. finalists: full pool × large budget

Promotion should use a lower-confidence-bound criterion, not point win rate alone.

## M4 — Island evolutionary search

- 8 islands × 256 active decks
- elitism within each island
- one-slot, count, trainer, evolution-package, and energy mutations
- package-aware crossover
- migration every N generations
- novelty archive to resist premature convergence

Novelty affects parent selection/exploration but never final champion ranking.

## M5 — Adversarial loop

Periodically freeze the current champion and launch a separate optimizer with objective:

`maximize P(counter beats champion)`

Add discovered counters to the opponent archive, then re-evaluate the champion. This creates the co-evolution loop that tests robustness outside the observed meta.

## M6 — Policy calibration

This is a release blocker for trustworthy deck recommendations.

For tournament archetype pairs with enough real matches:

- compute observed matchup win rates
- simulate the same archetype pairs
- measure MAE/RMSE/Spearman correlation
- compare DeckGym policies (`W`, `M`, `V`, expectiminimax depths)

Only promote a policy version for optimizer runs when it improves calibration on a held-out matchup set.
