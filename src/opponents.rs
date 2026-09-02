use crate::domain::Deck;

#[derive(Clone, Debug)]
pub enum OpponentKind {
    CurrentMeta,
    Historical,
    AiChampion,
    Adversarial,
    DiverseGenerated,
    Holdout,
}

#[derive(Clone, Debug)]
pub struct WeightedOpponent {
    pub id: String,
    pub deck: Deck,
    pub kind: OpponentKind,
    pub weight: f64,
}

#[derive(Clone, Debug, Default)]
pub struct OpponentPool {
    pub opponents: Vec<WeightedOpponent>,
}

impl OpponentPool {
    pub fn normalized_weights(&self) -> Vec<f64> {
        let total: f64 = self.opponents.iter().map(|o| o.weight.max(0.0)).sum();
        if total == 0.0 {
            return vec![0.0; self.opponents.len()];
        }
        self.opponents
            .iter()
            .map(|o| o.weight.max(0.0) / total)
            .collect()
    }
}
