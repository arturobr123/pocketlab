use crate::domain::{CardId, CardRegistry, Deck};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeckTextError {
    InvalidLine(String),
    InvalidCount(String),
    EmptyEnergy,
}

/// Parse the text format used by the Limitless Pocket deck builder and DeckGym.
///
/// Supported examples:
/// - `2 Pikachu ex A1 96`
/// - `2 A1 096`
/// - `2 Poké Ball P-A 5`
/// - `Energy: Lightning, Water`
pub fn parse_pocket_deck_text(
    text: &str,
    registry: &CardRegistry,
) -> Result<Deck, DeckTextError> {
    let mut cards = BTreeMap::new();
    let mut explicit_energy: Option<BTreeSet<String>> = None;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty()
            || line.starts_with("Pokémon:")
            || line.starts_with("Pokemon:")
            || line.starts_with("Trainer:")
        {
            continue;
        }

        if let Some(rest) = line.strip_prefix("Energy:") {
            let energy = rest
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(normalize_energy)
                .collect::<BTreeSet<_>>();
            if energy.is_empty() {
                return Err(DeckTextError::EmptyEnergy);
            }
            explicit_energy = Some(energy);
            continue;
        }

        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 3 {
            return Err(DeckTextError::InvalidLine(line.to_string()));
        }

        let count = parts[0]
            .parse::<u8>()
            .map_err(|_| DeckTextError::InvalidCount(parts[0].to_string()))?;
        let set = parts[parts.len() - 2];
        let number = normalize_card_number(parts[parts.len() - 1]);
        let id = CardId(format!("{set}-{number}"));
        *cards.entry(id).or_insert(0) += count;
    }

    let energy_types = explicit_energy.unwrap_or_else(|| {
        let inferred = registry.infer_energy_types(cards.keys());
        if inferred.is_empty() {
            // Pocket requires an Energy Zone selection. Purely colorless decks can
            // use any selectable energy, so use a deterministic fallback.
            BTreeSet::from(["Water".to_string()])
        } else {
            inferred
        }
    });

    Ok(Deck::new(cards, energy_types))
}

fn normalize_card_number(raw: &str) -> String {
    if raw.chars().all(|c| c.is_ascii_digit()) {
        format!("{raw:0>3}")
    } else {
        raw.to_string()
    }
}

fn normalize_energy(raw: &str) -> String {
    match raw.to_ascii_lowercase().as_str() {
        "grass" => "Grass".into(),
        "fire" => "Fire".into(),
        "water" => "Water".into(),
        "lightning" | "electric" => "Lightning".into(),
        "psychic" => "Psychic".into(),
        "fighting" => "Fighting".into(),
        "dark" | "darkness" => "Darkness".into(),
        "metal" => "Metal".into(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Card, CardType};

    fn registry() -> CardRegistry {
        let mut registry = CardRegistry::default();
        registry.insert(Card {
            id: CardId("A1-094".into()),
            name: "Pikachu".into(),
            card_type: CardType::Pokemon { basic: true },
            evolves_from: None,
            required_energy_types: BTreeSet::from(["Lightning".into()]),
            legal: true,
            mechanics_implemented: true,
        });
        registry.insert(Card {
            id: CardId("P-A-005".into()),
            name: "Poké Ball".into(),
            card_type: CardType::Trainer,
            evolves_from: None,
            required_energy_types: BTreeSet::new(),
            legal: true,
            mechanics_implemented: true,
        });
        registry
    }

    #[test]
    fn parses_limitless_names_and_infers_energy() {
        let deck = parse_pocket_deck_text("2 Pikachu A1 94\n2 Poké Ball P-A 5", &registry()).unwrap();
        assert_eq!(deck.cards[&CardId("A1-094".into())], 2);
        assert_eq!(deck.cards[&CardId("P-A-005".into())], 2);
        assert_eq!(deck.energy_types, BTreeSet::from(["Lightning".into()]));
    }

    #[test]
    fn explicit_energy_wins_over_inference() {
        let deck = parse_pocket_deck_text("Energy: Fire, Water\n2 Pikachu A1 94", &registry())
            .unwrap();
        assert_eq!(
            deck.energy_types,
            BTreeSet::from(["Fire".into(), "Water".into()])
        );
    }
}
