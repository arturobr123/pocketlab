# Architecture

## Optimization objective

Primary objective: maximize robust blind win probability, not just win rate against today's meta.

Initial score:

```
robust_score = 0.70 * weighted_wilson_lcb95 + 0.30 * cvar_10
```

- `weighted_wilson_lcb95`: penalizes uncertain estimates.
- `cvar_10`: weighted average performance in the bottom 10% of matchup mass.
- novelty is used only for exploration, never final ranking.

## Planned phases

1. DeckGym adapter and engine calibration.
2. Matchup cache with engine/policy/card-db versioning.
3. Progressive racing / successive halving.
4. Island genetic algorithm and semantic mutation packages.
5. Adversarial counter-deck search and opponent archive.
6. Holdout opponent pool for generalization checks.
7. Stronger player policies (MCTS/self-play) only after search is trustworthy.
