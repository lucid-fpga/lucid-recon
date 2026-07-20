//! Clock plan. Matches the core to a known clock-ratio family and emits the port's
//! clock plan: the preserved core ratio, the 50→74.25 reference swap, and the
//! pixel-clock PLL output to add. If no family matches, the universal transforms
//! still apply and the ratio is left to be derived from the core's own spec.

use crate::refdata::ClockData;
use crate::source::CoreFiles;
use serde::Serialize;

/// The clock plan for a port.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClockPlan {
    /// The matched ratio family, if any.
    pub family: Option<String>,
    /// The core clock ratio (from the family), if matched.
    pub core_ratio: Option<String>,
    /// The reference-clock swap instruction.
    pub ref_swap: String,
    /// The pixel-PLL instruction.
    pub pixel_pll: String,
    /// Notes and honest gaps.
    pub notes: Vec<String>,
    /// Public provenance for whatever was matched.
    pub provenance: Vec<String>,
}

/// Word-boundary substring match: `sig` must appear in `corpus` (already
/// lowercased) bounded by non-alphanumeric characters on both sides, so a short
/// signature like `nes` does not match inside a longer token (`scanlines`). `_` and
/// every other non-alphanumeric are separators, so `m72` still matches `irem_m72`.
fn token_match(corpus: &str, sig: &str) -> bool {
    if sig.is_empty() {
        return false;
    }
    let bytes = corpus.as_bytes();
    let n = sig.len();
    let is_word = |c: u8| c.is_ascii_alphanumeric();
    let mut start = 0;
    while let Some(pos) = corpus[start..].find(sig) {
        let i = start + pos;
        let before_ok = i == 0 || !is_word(bytes[i - 1]);
        let after = i + n;
        let after_ok = after >= bytes.len() || !is_word(bytes[after]);
        if before_ok && after_ok {
            return true;
        }
        start = i + 1;
    }
    false
}

/// Build the clock plan for `core_name` given its files and the clock data.
pub fn plan_clocks(core_name: &str, files: &CoreFiles, clocks: &ClockData) -> ClockPlan {
    let corpus = {
        let mut c = core_name.to_ascii_lowercase();
        c.push('\n');
        c.push_str(
            &files
                .files
                .iter()
                .map(|f| f.path.to_ascii_lowercase())
                .collect::<Vec<_>>()
                .join("\n"),
        );
        c
    };

    let matched = clocks.families.iter().find(|fam| {
        fam.signatures.iter().any(|s| token_match(&corpus, &s.to_ascii_lowercase()))
    });

    let mut notes = Vec::new();
    let mut provenance = Vec::new();

    let ref_swap = match &clocks.ref_swap {
        Some(r) => {
            provenance.push(r.provenance.clone());
            format!("{} MHz \u{2192} {} MHz: {}", r.from_mhz, r.to_mhz, r.note)
        }
        None => "reference swap: (no data)".to_string(),
    };
    let pixel_pll = match &clocks.pixel_pll {
        Some(p) => {
            provenance.push(p.provenance.clone());
            p.note.clone()
        }
        None => "pixel PLL: (no data)".to_string(),
    };

    let (family, core_ratio) = match matched {
        Some(fam) => {
            notes.push(fam.note.clone());
            provenance.push(fam.provenance.clone());
            (Some(fam.family.clone()), Some(fam.core_ratio.clone()))
        }
        None => {
            notes.push(
                "no known clock-ratio family matched — preserve the core's own ratios verbatim \
                 (derive from its system spec) and apply the universal ref-swap + pixel-PLL"
                    .to_string(),
            );
            (None, None)
        }
    };

    ClockPlan { family, core_ratio, ref_swap, pixel_pll, notes, provenance }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refdata::RefData;

    #[test]
    fn matches_irem_m72_family_and_ratio() {
        let files = CoreFiles::from_pairs([("rtl/m72_core.v", "module m72(); endmodule")]);
        let clocks = RefData::bundled().unwrap().clocks;
        let plan = plan_clocks("irem_m72", &files, &clocks);
        assert_eq!(plan.family.as_deref(), Some("irem-m72"));
        assert!(plan.core_ratio.as_deref().unwrap().contains("3:1"));
        assert!(plan.ref_swap.contains("74.25"));
        assert!(plan.pixel_pll.to_lowercase().contains("pixel"));
    }

    #[test]
    fn token_match_respects_word_boundaries() {
        // the real-core regression: `nes` must NOT match inside `scanlines`
        assert!(!token_match("sys/scanlines.v", "nes"));
        assert!(!token_match("genesis", "nes"));
        // but real tokens still match (underscore / slash are separators)
        assert!(token_match("rtl/m72_core.v", "m72"));
        assert!(token_match("arcade-irem_m72", "m72"));
        assert!(token_match("rtl/nes_top.v", "nes"));
    }

    #[test]
    fn m90_resolves_to_irem_3to1_not_nintendo_nes() {
        // real M90 tree carries sys/scanlines.v; with the fix it must land on irem-m90 3:1
        let files = CoreFiles::from_pairs([
            ("rtl/ga25_sdram.sv", "module ga25_sdram(); endmodule"),
            ("sys/scanlines.v", "// stock mister scanlines"),
        ]);
        let clocks = RefData::bundled().unwrap().clocks;
        let plan = plan_clocks("Arcade-IremM90_MiSTer", &files, &clocks);
        assert_eq!(plan.family.as_deref(), Some("irem-m90"), "M90 → irem-m90, not nintendo-nes");
        assert!(plan.core_ratio.as_deref().unwrap().contains("3:1"), "M90 is 3:1");
    }

    #[test]
    fn no_regression_m72_m92_nes_families() {
        let clocks = RefData::bundled().unwrap().clocks;
        let m72 = plan_clocks("irem_m72", &CoreFiles::from_pairs([("rtl/m72.v", "")]), &clocks);
        assert_eq!(m72.family.as_deref(), Some("irem-m72"));
        assert!(m72.core_ratio.as_deref().unwrap().contains("3:1"));
        let m92 = plan_clocks("irem_m92", &CoreFiles::from_pairs([("rtl/m92.v", "")]), &clocks);
        assert_eq!(m92.family.as_deref(), Some("irem-m92"));
        // NES must still be 4:1, and must NOT be shadowed by anything
        let nes = plan_clocks("NES_MiSTer", &CoreFiles::from_pairs([("rtl/nes.v", "")]), &clocks);
        assert_eq!(nes.family.as_deref(), Some("nintendo-nes"));
        assert!(nes.core_ratio.as_deref().unwrap().contains("4:1"));
    }

    #[test]
    fn unknown_core_still_gets_universal_transforms() {
        let files = CoreFiles::from_pairs([("rtl/mystery.v", "module mystery(); endmodule")]);
        let clocks = RefData::bundled().unwrap().clocks;
        let plan = plan_clocks("mystery", &files, &clocks);
        assert!(plan.family.is_none());
        assert!(plan.ref_swap.contains("74.25"));
        assert!(plan.notes.iter().any(|n| n.contains("preserve")));
    }
}
