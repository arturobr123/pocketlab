//! Adapter between PocketLab's optimizer domain model and deckgym-core.
//!
//! We deliberately run `deckgym::Game` directly instead of invoking the DeckGym CLI.
//! This keeps the hot path in-process and lets PocketLab derive a unique deterministic
//! seed per simulated game.

use crate::{
    domain::{CardId, Deck},
    simulator::{MatchupResult, SimulationArena},
};
use deckgym::{
    players::{create_players, PlayerCode},
    state::GameOutcome,
    Deck as DeckGymDeck, Game,
};
use rayon::prelude::*;

#[derive(Clone, Debug)]
pub struct DeckGymArena {
    player_a: PlayerCode,
    player_b: PlayerCode,
    parallel: bool,
}

impl Default for DeckGymArena {
    fn default() -> Self {
        Self {
            player_a: PlayerCode::W,
            player_b: PlayerCode::W,
            parallel: true,
        }
    }
}

impl DeckGymArena {
    pub fn new(player_a: PlayerCode, player_b: PlayerCode, parallel: bool) -> Self {
        Self {
            player_a,
            player_b,
            parallel,
        }
    }

    fn parse_deck(deck: &Deck) -> DeckGymDeck {
        let text = deck_to_deckgym_text(deck)
            .unwrap_or_else(|error| panic!("cannot encode PocketLab deck for DeckGym: {error}"));
        let parsed = DeckGymDeck::from_string(&text)
            .unwrap_or_else(|error| panic!("DeckGym rejected encoded deck: {error}\n{text}"));
        assert!(
            parsed.is_valid(),
            "PocketLab passed a deck that DeckGym considers invalid:\n{text}"
        );
        parsed
    }

    fn play_one(
        deck_a: DeckGymDeck,
        deck_b: DeckGymDeck,
        player_a: PlayerCode,
        player_b: PlayerCode,
        seed: u64,
    ) -> Option<GameOutcome> {
        let players = create_players(deck_a, deck_b, vec![player_a, player_b]);
        let mut game = Game::new(players, seed);
        game.play()
    }
}

impl SimulationArena for DeckGymArena {
    fn simulate(&self, deck_a: &Deck, deck_b: &Deck, games: u32, seed: u64) -> MatchupResult {
        if games == 0 {
            return MatchupResult::default();
        }

        let deck_a = Self::parse_deck(deck_a);
        let deck_b = Self::parse_deck(deck_b);
        let player_a = self.player_a.clone();
        let player_b = self.player_b.clone();

        let run = |game_index: u32| {
            let game_seed = splitmix64(seed.wrapping_add(game_index as u64));
            Self::play_one(
                deck_a.clone(),
                deck_b.clone(),
                player_a.clone(),
                player_b.clone(),
                game_seed,
            )
        };

        let outcomes: Vec<Option<GameOutcome>> = if self.parallel {
            (0..games).into_par_iter().map(run).collect()
        } else {
            (0..games).map(run).collect()
        };

        let mut result = MatchupResult {
            games: games as u64,
            ..MatchupResult::default()
        };

        for outcome in outcomes {
            match outcome {
                Some(GameOutcome::Win(0)) => result.wins_a += 1,
                Some(GameOutcome::Win(1)) => result.wins_b += 1,
                Some(GameOutcome::Win(_)) | Some(GameOutcome::Tie) | None => result.draws += 1,
            }
        }

        debug_assert_eq!(result.games, result.wins_a + result.wins_b + result.draws);
        result
    }
}

pub fn deck_to_deckgym_text(deck: &Deck) -> Result<String, String> {
    let mut lines = Vec::with_capacity(deck.cards.len() + 1);

    if !deck.energy_types.is_empty() {
        let energy = deck
            .energy_types
            .iter()
            .map(|energy| normalize_energy(energy))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("Energy: {energy}"));
    }

    for (card_id, count) in &deck.cards {
        let (set, number) = split_card_id(card_id)?;
        lines.push(format!("{count} {set} {number}"));
    }

    Ok(lines.join("\n"))
}

fn split_card_id(card_id: &CardId) -> Result<(String, String), String> {
    let raw = card_id.0.trim();
    let whitespace_parts: Vec<&str> = raw.split_whitespace().collect();
    if whitespace_parts.len() == 2 {
        return Ok((
            whitespace_parts[0].to_string(),
            whitespace_parts[1].to_string(),
        ));
    }

    if let Some((set, number)) = raw.rsplit_once('-') {
        if !set.is_empty() && !number.is_empty() {
            return Ok((set.to_string(), number.to_string()));
        }
    }

    Err(format!(
        "card id '{raw}' must look like 'A3b-041' or 'A3b 041'"
    ))
}

fn normalize_energy(energy: &str) -> String {
    match energy.trim().to_ascii_lowercase().as_str() {
        "grass" => "Grass".into(),
        "fire" => "Fire".into(),
        "water" => "Water".into(),
        "lightning" | "electric" => "Lightning".into(),
        "psychic" => "Psychic".into(),
        "fighting" => "Fighting".into(),
        "dark" | "darkness" => "Darkness".into(),
        "metal" => "Metal".into(),
        "dragon" => "Dragon".into(),
        "colorless" => "Colorless".into(),
        _ => energy.trim().to_string(),
    }
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn encodes_deckgym_text_without_card_names() {
        let deck = Deck::new(
            BTreeMap::from([(CardId("A3b-041".into()), 2), (CardId("P-A-005".into()), 1)]),
            BTreeSet::from(["lightning".into(), "water".into()]),
        );

        assert_eq!(
            deck_to_deckgym_text(&deck).unwrap(),
            "Energy: Lightning, Water\n2 A3b 041\n1 P-A 005"
        );
    }

    #[test]
    fn splitmix_is_deterministic_and_changes_with_index() {
        assert_eq!(splitmix64(42), splitmix64(42));
        assert_ne!(splitmix64(42), splitmix64(43));
    }
}
