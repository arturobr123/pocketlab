use std::env;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        None | Some("registry") => show_registry(),
        Some("meta") => {
            let fragment = args
                .get(1)
                .ok_or_else(|| "usage: pocketlab meta <tournament-name-fragment>".to_string())?;
            show_meta(fragment)
        }
        Some("evaluate") => {
            let deck_path = args.get(1).ok_or_else(|| {
                "usage: pocketlab evaluate <deck-file> <tournament-name-fragment> [games-per-matchup]"
                    .to_string()
            })?;
            let tournament = args.get(2).ok_or_else(|| {
                "usage: pocketlab evaluate <deck-file> <tournament-name-fragment> [games-per-matchup]"
                    .to_string()
            })?;
            let games = args
                .get(3)
                .map(|value| value.parse::<u32>())
                .transpose()
                .map_err(|error| format!("invalid games-per-matchup: {error}"))?
                .unwrap_or(100);
            evaluate_deck(deck_path, tournament, games)
        }
        Some(command) => Err(format!(
            "unknown command '{command}'. Try: registry | meta <name> | evaluate <deck-file> <name> [games]"
        )),
    }
}

#[cfg(feature = "deckgym")]
fn build_registry() -> pocketlab::domain::CardRegistry {
    pocketlab::deckgym_registry::build_deckgym_registry()
}

#[cfg(feature = "deckgym")]
fn show_registry() -> Result<(), String> {
    let registry = build_registry();
    let stats = pocketlab::deckgym_registry::registry_stats(&registry);
    println!("pocketlab v0.1.0 — robust Pokémon TCG Pocket deck search");
    println!(
        "DeckGym registry: {} total cards, {} simulatable, {} unsupported",
        stats.total_cards, stats.supported_cards, stats.unsupported_cards
    );
    Ok(())
}

#[cfg(not(feature = "deckgym"))]
fn show_registry() -> Result<(), String> {
    println!("PocketLab built without DeckGym integration.");
    Ok(())
}

#[cfg(feature = "deckgym")]
fn load_meta(
    fragment: &str,
    registry: &pocketlab::domain::CardRegistry,
) -> Result<
    (
        pocketlab::limitless::LimitlessTournament,
        pocketlab::limitless::LimitlessImport,
    ),
    String,
> {
    use pocketlab::{
        limitless::{opponent_pool_from_standings, LimitlessClient},
        opponents::OpponentKind,
    };

    let client = LimitlessClient::default();
    let tournament = client
        .find_pocket_tournament(fragment, 100)
        .map_err(|error| format!("Limitless tournament lookup failed: {error}"))?
        .ok_or_else(|| format!("no recent Pocket tournament matching '{fragment}'"))?;
    let standings = client
        .standings(&tournament.id)
        .map_err(|error| format!("Limitless standings lookup failed: {error}"))?;
    let import = opponent_pool_from_standings(&standings, registry, OpponentKind::CurrentMeta);
    if import.pool.opponents.is_empty() {
        return Err(format!(
            "tournament '{}' produced no DeckGym-simulatable decks",
            tournament.name
        ));
    }
    Ok((tournament, import))
}

#[cfg(feature = "deckgym")]
fn show_meta(fragment: &str) -> Result<(), String> {
    let registry = build_registry();
    let (tournament, import) = load_meta(fragment, &registry)?;
    println!("Tournament: {} ({})", tournament.name, tournament.id);
    println!(
        "Standings: {} | decklists: {} | accepted: {} | unique: {} | unsupported: {} | invalid: {}",
        import.stats.standings,
        import.stats.with_decklist,
        import.stats.accepted_entries,
        import.stats.unique_decks,
        import.stats.skipped_unsupported,
        import.stats.skipped_invalid
    );

    let mut opponents = import.pool.opponents;
    opponents.sort_by(|a, b| b.weight.total_cmp(&a.weight));
    println!("\nMost common exact decklists:");
    for opponent in opponents.iter().take(10) {
        println!("  {:>4.0}x  {}", opponent.weight, opponent.id);
    }
    Ok(())
}

#[cfg(not(feature = "deckgym"))]
fn show_meta(_fragment: &str) -> Result<(), String> {
    Err("meta import requires the 'deckgym' feature".to_string())
}

#[cfg(feature = "deckgym")]
fn evaluate_deck(deck_path: &str, tournament_fragment: &str, games: u32) -> Result<(), String> {
    use pocketlab::{
        cache::{CachedArena, SqliteMatchupCache},
        deck_text::parse_pocket_deck_text,
        deckgym_adapter::DeckGymArena,
        evaluator::{evaluate_deck as evaluate, EvaluationConfig},
    };
    use std::path::Path;

    let registry = build_registry();
    let text = std::fs::read_to_string(deck_path)
        .map_err(|error| format!("failed to read '{deck_path}': {error}"))?;
    let deck = parse_pocket_deck_text(&text, &registry)
        .map_err(|error| format!("failed to parse deck: {error:?}"))?;
    deck.validate(&registry)
        .map_err(|error| format!("invalid or unsupported deck: {error:?}"))?;

    let (tournament, import) = load_meta(tournament_fragment, &registry)?;
    let cache_path = env::var("POCKETLAB_CACHE")
        .unwrap_or_else(|_| ".pocketlab/matchups.sqlite".to_string());
    if let Some(parent) = Path::new(&cache_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create cache directory: {error}"))?;
        }
    }
    let cache = SqliteMatchupCache::open(&cache_path)
        .map_err(|error| format!("failed to open matchup cache '{cache_path}': {error}"))?;
    let arena = CachedArena::new(
        DeckGymArena::default(),
        cache,
        "deckgym:fda48391:weighted-random-vs-weighted-random:v1",
    );
    let config = EvaluationConfig {
        games_per_matchup: games,
        ..EvaluationConfig::default()
    };
    let result = evaluate(&arena, &deck, &import.pool, &config);

    println!("Deck: {}", deck.canonical_key());
    println!("Opponent pool: {}", tournament.name);
    println!("Unique opponent decks: {}", import.pool.opponents.len());
    println!("Simulated games: {}", result.games);
    println!("Matchup cache: {cache_path}");
    println!(
        "Weighted win rate: {:.2}%",
        result.weighted_win_rate * 100.0
    );
    println!("Wilson LCB95: {:.2}%", result.weighted_lcb95 * 100.0);
    println!("CVaR10: {:.2}%", result.cvar10 * 100.0);
    println!("Robust score: {:.4}", result.robust_score);

    let mut matchups = result.matchups;
    matchups.sort_by(|a, b| a.win_rate.total_cmp(&b.win_rate));
    println!("\nWeakest matchups:");
    for matchup in matchups.iter().take(10) {
        println!(
            "  {:>6.2}%  {:>6} games  {}",
            matchup.win_rate * 100.0,
            matchup.result.games,
            matchup.opponent_id
        );
    }
    Ok(())
}

#[cfg(not(feature = "deckgym"))]
fn evaluate_deck(_deck_path: &str, _tournament_fragment: &str, _games: u32) -> Result<(), String> {
    Err("deck evaluation requires the 'deckgym' feature".to_string())
}
