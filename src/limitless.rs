use crate::{
    deck_text::{normalize_card_number, normalize_energy},
    domain::{CardId, CardRegistry, Deck, DeckValidationError},
    opponents::{OpponentKind, OpponentPool, WeightedOpponent},
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

const DEFAULT_BASE_URL: &str = "https://play.limitlesstcg.com/api";

#[derive(Clone, Debug)]
pub struct LimitlessClient {
    base_url: String,
    http: reqwest::blocking::Client,
}

impl Default for LimitlessClient {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

impl LimitlessClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            http: reqwest::blocking::Client::builder()
                .user_agent("pocketlab/0.1")
                .build()
                .expect("failed to create HTTP client"),
        }
    }

    pub fn pocket_tournaments(
        &self,
        limit: usize,
    ) -> Result<Vec<LimitlessTournament>, reqwest::Error> {
        self.http
            .get(format!("{}/tournaments", self.base_url))
            .query(&[("game", "POCKET"), ("limit", &limit.to_string())])
            .send()?
            .error_for_status()?
            .json()
    }

    pub fn standings(
        &self,
        tournament_id: &str,
    ) -> Result<Vec<LimitlessStanding>, reqwest::Error> {
        self.http
            .get(format!(
                "{}/tournaments/{tournament_id}/standings",
                self.base_url
            ))
            .send()?
            .error_for_status()?
            .json()
    }

    pub fn find_pocket_tournament(
        &self,
        name_fragment: &str,
        limit: usize,
    ) -> Result<Option<LimitlessTournament>, reqwest::Error> {
        let needle = name_fragment.to_ascii_lowercase();
        Ok(self
            .pocket_tournaments(limit)?
            .into_iter()
            .find(|tournament| tournament.name.to_ascii_lowercase().contains(&needle)))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct LimitlessTournament {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct LimitlessDeckArchetype {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub icons: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct LimitlessDeckCard {
    pub count: u8,
    pub set: String,
    pub number: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Default)]
pub struct LimitlessDeckList {
    #[serde(default)]
    pub pokemon: Vec<LimitlessDeckCard>,
    #[serde(default)]
    pub trainer: Vec<LimitlessDeckCard>,
    #[serde(default)]
    pub energy: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct LimitlessStanding {
    pub player: String,
    pub placing: u32,
    #[serde(default)]
    pub deck: Option<LimitlessDeckArchetype>,
    #[serde(default)]
    pub decklist: Option<LimitlessDeckList>,
}

impl LimitlessDeckList {
    pub fn to_deck(&self, registry: &CardRegistry) -> Deck {
        let mut cards = BTreeMap::new();
        for card in self.pokemon.iter().chain(self.trainer.iter()) {
            let id = CardId(format!(
                "{}-{}",
                card.set,
                normalize_card_number(&card.number)
            ));
            *cards.entry(id).or_insert(0) += card.count;
        }

        let energy_types = if self.energy.is_empty() {
            let inferred = registry.infer_energy_types(cards.keys());
            if inferred.is_empty() {
                BTreeSet::from(["Water".to_string()])
            } else {
                inferred
            }
        } else {
            self.energy
                .iter()
                .map(|energy| normalize_energy(energy))
                .collect()
        };

        Deck::new(cards, energy_types)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LimitlessImportStats {
    pub standings: usize,
    pub with_decklist: usize,
    pub accepted_entries: usize,
    pub unique_decks: usize,
    pub skipped_invalid: usize,
    pub skipped_unsupported: usize,
}

#[derive(Clone, Debug)]
pub struct LimitlessImport {
    pub pool: OpponentPool,
    pub stats: LimitlessImportStats,
}

pub fn opponent_pool_from_standings(
    standings: &[LimitlessStanding],
    registry: &CardRegistry,
    kind: OpponentKind,
) -> LimitlessImport {
    let mut stats = LimitlessImportStats {
        standings: standings.len(),
        ..LimitlessImportStats::default()
    };
    let mut aggregated: BTreeMap<String, (Deck, String, f64)> = BTreeMap::new();

    for standing in standings {
        let Some(decklist) = &standing.decklist else {
            continue;
        };
        stats.with_decklist += 1;
        let deck = decklist.to_deck(registry);

        if let Err(error) = deck.validate(registry) {
            if matches!(error, DeckValidationError::UnsupportedCard(_)) {
                stats.skipped_unsupported += 1;
            } else {
                stats.skipped_invalid += 1;
            }
            continue;
        }

        stats.accepted_entries += 1;
        let key = deck.canonical_key();
        let label = standing
            .deck
            .as_ref()
            .map(|archetype| archetype.name.clone())
            .unwrap_or_else(|| "Unclassified".to_string());
        aggregated
            .entry(key)
            .and_modify(|(_, _, weight)| *weight += 1.0)
            .or_insert((deck, label, 1.0));
    }

    let opponents = aggregated
        .into_iter()
        .enumerate()
        .map(|(index, (_key, (deck, label, weight)))| WeightedOpponent {
            id: format!("limitless:{index}:{label}"),
            deck,
            kind: kind.clone(),
            weight,
        })
        .collect::<Vec<_>>();
    stats.unique_decks = opponents.len();

    LimitlessImport {
        pool: OpponentPool { opponents },
        stats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Card, CardType};

    fn registry() -> CardRegistry {
        let mut registry = CardRegistry::default();
        registry.insert(Card {
            id: CardId("B4-010".into()),
            name: "Combee".into(),
            card_type: CardType::Pokemon { basic: true },
            evolves_from: None,
            required_energy_types: BTreeSet::from(["Grass".into()]),
            legal: true,
            mechanics_implemented: true,
        });
        registry
    }

    #[test]
    fn deserializes_real_limitless_decklist_shape() {
        let json = r#"
        {
          "player":"m8leo",
          "placing":1,
          "deck":{"id":"vespiquen-ex-b4-shuckle-ex-a4","name":"Vespiquen ex Shuckle ex","icons":["vespiquen","shuckle"]},
          "decklist":{
            "pokemon":[{"count":2,"set":"B4","number":"10","name":"Combee"}],
            "trainer":[],
            "energy":["Grass"]
          }
        }"#;
        let standing: LimitlessStanding = serde_json::from_str(json).unwrap();
        let deck = standing.decklist.unwrap().to_deck(&registry());
        assert_eq!(deck.cards[&CardId("B4-010".into())], 2);
        assert_eq!(deck.energy_types, BTreeSet::from(["Grass".into()]));
    }
}
