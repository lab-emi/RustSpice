//! Types shared between the suite engine and the deck generators/oracles. FROZEN contract: change only by
//! agreement of the orchestrator (the engine and the generators are developed in separate scratch copies).

/// Suite scale. `Small` = CI smoke, `Medium` = default, `Large` = opt-in (printed Rule-9 estimate, `--max-rss-mib`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scale {
    Small,
    Medium,
    Large,
}

impl Scale {
    pub fn parse(s: &str) -> Option<Scale> {
        match s.to_ascii_lowercase().as_str() {
            "small" => Some(Scale::Small),
            "medium" => Some(Scale::Medium),
            "large" => Some(Scale::Large),
            _ => None,
        }
    }
}

/// A generated deck. `deck` is a complete SPICE netlist INCLUDING its `* bench.*` metadata comment lines
/// (see SUITE_SPEC §2), so the engine treats generated decks exactly like static `.sp` corpus files.
#[derive(Clone, Debug)]
pub struct GeneratedDeck {
    /// Unique `[a-z0-9_]+` id, equal to the `* bench.id:` line.
    pub id: String,
    /// Family id, e.g. `ladder_dc`, `mesh_dc`, `fuzz_linear` (also in `* bench.tags:` as `family:<id>`).
    pub family: String,
    /// Scaling parameter used on the x axis of fits: MNA unknowns (preferred) or element count.
    pub size: u64,
    /// RustSpice peak-RSS estimate in MiB (Rule 9); the engine skips the case above `--max-rss-mib`.
    pub est_rss_mib: u64,
    /// Full deck text.
    pub deck: String,
}
