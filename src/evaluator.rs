use crate::{
    domain::Deck,
    opponents::OpponentPool,
    simulator::{MatchupResult, SimulationArena},
};

#[derive(Clone, Debug)]
pub struct MatchupEvaluation {
    pub opponent_id: String,
    pub weight: f64,
    pub result: MatchupResult,
    pub win_rate: f64,
    pub wilson_lcb95: f64,
}

#[derive(Clone, Debug)]
pub struct DeckEvaluation {
    pub weighted_win_rate: f64,
    pub weighted_lcb95: f64,
    pub cvar10: f64,
    pub robust_score: f64,
    pub games: u64,
    pub matchups: Vec<MatchupEvaluation>,
}

#[derive(Clone, Debug)]
pub struct EvaluationConfig {
    pub games_per_matchup: u32,
    pub cvar_alpha: f64,
    pub lcb_weight: f64,
    pub cvar_weight: f64,
    pub master_seed: u64,
}

impl Default for EvaluationConfig {
    fn default() -> Self {
        Self {
            games_per_matchup: 100,
            cvar_alpha: 0.10,
            lcb_weight: 0.70,
            cvar_weight: 0.30,
            master_seed: 0x504f434b45544c41,
        }
    }
}

pub fn evaluate_deck<A: SimulationArena>(
    arena: &A,
    deck: &Deck,
    pool: &OpponentPool,
    config: &EvaluationConfig,
) -> DeckEvaluation {
    assert!(
        !pool.opponents.is_empty(),
        "opponent pool must not be empty"
    );
    let weights = pool.normalized_weights();
    let mut matchups = Vec::with_capacity(pool.opponents.len());

    for (idx, opponent) in pool.opponents.iter().enumerate() {
        let seed = config
            .master_seed
            .wrapping_add(idx as u64)
            .wrapping_add(stable_hash(&deck.canonical_key()));
        let result = arena.simulate(deck, &opponent.deck, config.games_per_matchup, seed);
        let win_rate = result.win_rate_a();
        let lcb = wilson_lower_bound95(result.wins_a, result.games);
        matchups.push(MatchupEvaluation {
            opponent_id: opponent.id.clone(),
            weight: weights[idx],
            result,
            win_rate,
            wilson_lcb95: lcb,
        });
    }

    let weighted_win_rate = matchups.iter().map(|m| m.weight * m.win_rate).sum();
    let weighted_lcb95 = matchups.iter().map(|m| m.weight * m.wilson_lcb95).sum();
    let cvar10 = weighted_lower_tail_mean(&matchups, config.cvar_alpha);
    let robust_score = config.lcb_weight * weighted_lcb95 + config.cvar_weight * cvar10;
    let games = matchups.iter().map(|m| m.result.games).sum();

    DeckEvaluation {
        weighted_win_rate,
        weighted_lcb95,
        cvar10,
        robust_score,
        games,
        matchups,
    }
}

pub fn wilson_lower_bound95(wins: u64, games: u64) -> f64 {
    if games == 0 {
        return 0.0;
    }
    let n = games as f64;
    let p = wins as f64 / n;
    let z = 1.959963984540054;
    let z2 = z * z;
    let center = p + z2 / (2.0 * n);
    let margin = z * ((p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt());
    ((center - margin) / (1.0 + z2 / n)).clamp(0.0, 1.0)
}

fn weighted_lower_tail_mean(matchups: &[MatchupEvaluation], alpha: f64) -> f64 {
    let alpha = alpha.clamp(1e-9, 1.0);
    let mut sorted: Vec<_> = matchups.iter().collect();
    sorted.sort_by(|a, b| a.win_rate.total_cmp(&b.win_rate));
    let mut remaining = alpha;
    let mut weighted_sum = 0.0;
    for m in sorted {
        if remaining <= 0.0 {
            break;
        }
        let take = m.weight.min(remaining);
        weighted_sum += take * m.win_rate;
        remaining -= take;
    }
    if remaining > 0.0 {
        weighted_sum / (alpha - remaining).max(1e-9)
    } else {
        weighted_sum / alpha
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wilson_is_conservative() {
        let lower = wilson_lower_bound95(60, 100);
        assert!(lower < 0.60 && lower > 0.49);
    }
}
