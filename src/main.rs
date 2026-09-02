fn main() {
    println!("pocketlab v0.1.0 — robust Pokémon TCG Pocket deck search");

    #[cfg(feature = "deckgym")]
    {
        let registry = pocketlab::deckgym_registry::build_deckgym_registry();
        let stats = pocketlab::deckgym_registry::registry_stats(&registry);
        println!(
            "DeckGym registry: {} total cards, {} simulatable, {} unsupported",
            stats.total_cards, stats.supported_cards, stats.unsupported_cards
        );
    }

    #[cfg(not(feature = "deckgym"))]
    println!("DeckGym integration is disabled; running optimizer-only build.");
}
