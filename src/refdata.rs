//! Bundled **public** reference data — the distilled facts recon reasons with.
//!
//! Every table here is public-fact-only: which public cores implement which chips,
//! public repo URLs and their licenses, published clock ratios, and the
//! MiSTer→APF service mapping any porter can read off the two public frameworks.
//! No private prose, identifiers, or paths cross into this crate — the data files
//! under `data/` are the leakage boundary, and each entry carries its own public
//! provenance string.
//!
//! The data is compiled in with `include_str!`, so a released binary is
//! self-contained. Tests can build a [`RefData`] by hand (the in-memory double)
//! instead of loading the bundle.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

/// A proven Pocket implementation of a chip, with its license and reuse quality.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PocketPart {
    /// Where it lives (author / framework / repo), a public reference.
    pub source: String,
    /// `"true-drop-in"` (the same module, already proven on Pocket) or
    /// `"reference-start"` (a different implementation to adapt).
    pub kind: String,
    /// SPDX-ish license of that implementation.
    pub license: String,
    /// Public URL.
    pub url: String,
}

/// A CPU / sound / other chip family recon can detect and source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chip {
    /// Stable id.
    pub id: String,
    /// Human name.
    pub name: String,
    /// The chip category (`cpu`, `sound`, `video`, `other`).
    pub category: String,
    /// RTL tokens that identify this chip (module/instance name fragments),
    /// matched case-insensitively.
    pub signatures: Vec<String>,
    /// Known proven Pocket implementations to source from.
    pub pocket_parts: Vec<PocketPart>,
    /// Public provenance for this entry.
    pub provenance: String,
}

/// The chip/IP catalogue.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChipCatalogue {
    /// All catalogued chips.
    pub chips: Vec<Chip>,
}

/// A canonical Pocket template lineage to fork.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Template {
    /// The framework/porter this lineage belongs to.
    pub framework: String,
    /// The repo to fork.
    pub fork: String,
    /// The lineage root commit (fork from the root, not a peer-carrying descendant).
    #[serde(default)]
    pub root_commit: Option<String>,
    /// True if this is the recommended default fork.
    pub prefer: bool,
    /// When to pick this lineage.
    pub note: String,
    /// Public URL.
    pub url: String,
    /// Public provenance.
    pub provenance: String,
}

/// The template-lineage table.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageTable {
    /// All lineages.
    pub templates: Vec<Template>,
}

/// The reference-clock swap fact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefSwap {
    /// MiSTer's assumed reference (MHz).
    pub from_mhz: f64,
    /// The Pocket's supplied reference (MHz).
    pub to_mhz: f64,
    /// What to do.
    pub note: String,
    /// Public provenance.
    pub provenance: String,
}

/// The pixel-PLL-output-to-add fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelPll {
    /// Whether a pixel-clock PLL output must be added for the Pocket video path.
    pub add_output: bool,
    /// Details.
    pub note: String,
    /// Public provenance.
    pub provenance: String,
}

/// A clock-ratio family for a core/genre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockFamily {
    /// Family id.
    pub family: String,
    /// Cores in the family.
    pub cores: Vec<String>,
    /// The core clock ratio (as cited).
    pub core_ratio: String,
    /// RTL/name signatures that map a core to this family.
    pub signatures: Vec<String>,
    /// Details.
    pub note: String,
    /// Public provenance.
    pub provenance: String,
}

/// The clock reference data.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ClockData {
    /// The 50→74.25 ref swap.
    #[serde(default)]
    pub ref_swap: Option<RefSwap>,
    /// The pixel-PLL-to-add.
    #[serde(default)]
    pub pixel_pll: Option<PixelPll>,
    /// Ratio families.
    pub families: Vec<ClockFamily>,
}

/// One MiSTer→APF service equivalence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Service {
    /// The MiSTer service/interface.
    pub mister: String,
    /// The Analogue-Pocket APF equivalent.
    pub apf: String,
    /// RTL tokens that reveal the MiSTer service is used.
    #[serde(default)]
    pub signatures: Vec<String>,
    /// Porting note.
    pub note: String,
    /// Public provenance.
    pub provenance: String,
}

/// The MiSTer→APF service map.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceMap {
    /// All service mappings.
    pub services: Vec<Service>,
}

/// The full bundled reference data.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RefData {
    /// Chip/IP catalogue.
    pub chips: ChipCatalogue,
    /// Template lineages.
    pub lineage: LineageTable,
    /// Clock data.
    pub clocks: ClockData,
    /// Service map.
    pub services: ServiceMap,
}

const CHIPS_JSON: &str = include_str!("../data/chips.json");
const LINEAGE_JSON: &str = include_str!("../data/lineage.json");
const CLOCKS_JSON: &str = include_str!("../data/clocks.json");
const SERVICES_JSON: &str = include_str!("../data/services.json");

fn load<T: for<'de> Deserialize<'de>>(name: &'static str, text: &str) -> Result<T> {
    serde_json::from_str(text).map_err(|source| Error::RefData { name, source })
}

impl RefData {
    /// Load the compiled-in reference data. Fails only if a bundled file is
    /// malformed (a build bug), never on user input.
    pub fn bundled() -> Result<Self> {
        Ok(RefData {
            chips: load("chips.json", CHIPS_JSON)?,
            lineage: load("lineage.json", LINEAGE_JSON)?,
            clocks: load("clocks.json", CLOCKS_JSON)?,
            services: load("services.json", SERVICES_JSON)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_data_parses_and_is_populated() {
        let rd = RefData::bundled().expect("bundled reference data parses");
        assert!(!rd.chips.chips.is_empty(), "chip catalogue non-empty");
        assert!(!rd.lineage.templates.is_empty(), "lineage table non-empty");
        assert!(rd.clocks.ref_swap.is_some(), "ref swap present");
        assert!(!rd.services.services.is_empty(), "service map non-empty");
    }

    #[test]
    fn every_chip_signature_is_lowercaseable_and_nonempty() {
        let rd = RefData::bundled().unwrap();
        for c in &rd.chips.chips {
            assert!(!c.signatures.is_empty(), "chip {} has signatures", c.id);
            assert!(!c.provenance.is_empty(), "chip {} cites provenance", c.id);
        }
    }

    #[test]
    fn exactly_one_preferred_template() {
        let rd = RefData::bundled().unwrap();
        let preferred = rd.lineage.templates.iter().filter(|t| t.prefer).count();
        assert_eq!(preferred, 1, "exactly one default template to fork");
    }
}
