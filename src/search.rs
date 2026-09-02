use crate::{
    domain::{CardId, CardRegistry, Deck},
    evaluator::{evaluate_deck, DeckEvaluation, EvaluationConfig},
    opponents::OpponentPool,
    simulator::SimulationArena,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct ScoredDeck {
    pub deck: Deck,
    pub evaluation: DeckEvaluation,
}

pub struct SearchConfig {
    pub generations: usize,
    pub elite_count: usize,
    pub children_per_elite: usize,
    pub evaluation: EvaluationConfig,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            generations: 25,
            elite_count: 8,
            children_per_elite: 8,
            evaluation: EvaluationConfig::default(),
        }
    }
}

pub fn search<A: SimulationArena>(
    arena: &A,
    registry: &CardRegistry,
    pool: &OpponentPool,
    seeds: Vec<Deck>,
    config: &SearchConfig,
) -> Vec<ScoredDeck> {
    let mut population = seeds;
    let card_ids: Vec<CardId> = registry
        .iter()
        .filter(|c| c.legal && c.mechanics_implemented)
        .map(|c| c.id.clone())
        .collect();
    let mut rng = XorShift64::new(config.evaluation.master_seed);

    for _generation in 0..config.generations {
        let mut scored: Vec<ScoredDeck> = population
            .into_iter()
            .map(|deck| {
                let evaluation = evaluate_deck(arena, &deck, pool, &config.evaluation);
                ScoredDeck { deck, evaluation }
            })
            .collect();
        scored.sort_by(|a, b| {
            b.evaluation
                .robust_score
                .total_cmp(&a.evaluation.robust_score)
        });
        scored.truncate(config.elite_count.min(scored.len()));

        let mut next = scored.iter().map(|s| s.deck.clone()).collect::<Vec<_>>();
        for elite in &scored {
            for _ in 0..config.children_per_elite {
                if let Some(child) = mutate_one_slot(&elite.deck, &card_ids, registry, &mut rng) {
                    next.push(child);
                }
            }
        }
        population = dedupe(next);
    }

    let mut final_scored: Vec<_> = population
        .into_iter()
        .map(|deck| {
            let evaluation = evaluate_deck(arena, &deck, pool, &config.evaluation);
            ScoredDeck { deck, evaluation }
        })
        .collect();
    final_scored.sort_by(|a, b| {
        b.evaluation
            .robust_score
            .total_cmp(&a.evaluation.robust_score)
    });
    final_scored
}

fn mutate_one_slot(
    deck: &Deck,
    card_ids: &[CardId],
    registry: &CardRegistry,
    rng: &mut XorShift64,
) -> Option<Deck> {
    if deck.cards.is_empty() || card_ids.is_empty() {
        return None;
    }
    let existing: Vec<CardId> = deck.cards.keys().cloned().collect();
    for _ in 0..64 {
        let remove = existing[rng.index(existing.len())].clone();
        let add = card_ids[rng.index(card_ids.len())].clone();
        if remove == add {
            continue;
        }
        let mut cards = deck.cards.clone();
        decrement(&mut cards, &remove);
        *cards.entry(add).or_default() += 1;
        let child = Deck::new(cards.clone(), inferred_energy(registry, cards.keys()));
        if child.validate(registry).is_ok() {
            return Some(child);
        }
    }
    None
}

fn inferred_energy<'a>(
    registry: &CardRegistry,
    card_ids: impl IntoIterator<Item = &'a CardId>,
) -> BTreeSet<String> {
    let inferred = registry.infer_energy_types(card_ids);
    if inferred.is_empty() {
        BTreeSet::from(["Water".to_string()])
    } else {
        inferred
    }
}

fn decrement(cards: &mut BTreeMap<CardId, u8>, id: &CardId) {
    if let Some(count) = cards.get_mut(id) {
        *count -= 1;
        if *count == 0 {
            cards.remove(id);
        }
    }
}

fn dedupe(decks: Vec<Deck>) -> Vec<Deck> {
    let mut by_key = BTreeMap::new();
    for deck in decks {
        by_key.entry(deck.canonical_key()).or_insert(deck);
    }
    by_key.into_values().collect()
}

struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn index(&mut self, len: usize) -> usize {
        (self.next() as usize) % len
    }
}
