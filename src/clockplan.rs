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
        fam.signatures.iter().any(|s| corpus.contains(&s.to_ascii_lowercase()))
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
    fn unknown_core_still_gets_universal_transforms() {
        let files = CoreFiles::from_pairs([("rtl/mystery.v", "module mystery(); endmodule")]);
        let clocks = RefData::bundled().unwrap().clocks;
        let plan = plan_clocks("mystery", &files, &clocks);
        assert!(plan.family.is_none());
        assert!(plan.ref_swap.contains("74.25"));
        assert!(plan.notes.iter().any(|n| n.contains("preserve")));
    }
}
