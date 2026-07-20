//! Component BOM detection + sourcing. Scans the core's RTL for chip signatures
//! from the bundled catalogue and, for each hit, reports the chip plus its known
//! proven Pocket implementations to source from. Heuristic: a signature is matched
//! as an RTL path substring or a module/identifier token — an unusual naming or a
//! chip built from an unseen `.qip`/`.ip` can be missed, and a coincidental token
//! can over-report; matches carry the signature that fired so a human can confirm.

use crate::refdata::{Chip, ChipCatalogue, PocketPart};
use crate::source::CoreFiles;
use regex::Regex;
use serde::Serialize;

/// One detected component and where to source it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BomEntry {
    /// Catalogue chip id.
    pub id: String,
    /// Human name.
    pub name: String,
    /// Category (`cpu`/`sound`/…).
    pub category: String,
    /// The signature token that fired the match.
    pub matched_signature: String,
    /// Proven Pocket implementations to source from.
    pub pocket_parts: Vec<PocketPart>,
    /// Public provenance for the catalogue entry.
    pub provenance: String,
}

/// Lowercased identifier tokens from the RTL: module declaration names and
/// instantiated module types.
fn rtl_identifiers(files: &CoreFiles) -> Vec<String> {
    let module_re = Regex::new(r"(?i)\bmodule\s+([A-Za-z_][\w]*)").unwrap();
    let inst_re = Regex::new(r"(?im)^\s*([A-Za-z_][\w]*)\s+(?:#\s*\([^;]*\)\s*)?[A-Za-z_][\w]*\s*\(").unwrap();
    let mut ids: Vec<String> = Vec::new();
    for f in files.rtl() {
        for c in module_re.captures_iter(&f.text) {
            ids.push(c[1].to_ascii_lowercase());
        }
        for c in inst_re.captures_iter(&f.text) {
            ids.push(c[1].to_ascii_lowercase());
        }
    }
    ids.sort();
    ids.dedup();
    ids
}

fn signature_hit(sig: &str, path_corpus: &str, idents: &[String]) -> bool {
    let s = sig.to_ascii_lowercase();
    if path_corpus.contains(&s) {
        return true;
    }
    idents.iter().any(|id| id == &s || id.contains(&s))
}

/// Detect the component BOM of a core against the catalogue.
pub fn detect_bom(files: &CoreFiles, catalogue: &ChipCatalogue) -> Vec<BomEntry> {
    let path_corpus = files
        .files
        .iter()
        .map(|f| f.path.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("\n");
    let idents = rtl_identifiers(files);

    let mut out = Vec::new();
    for chip in &catalogue.chips {
        if let Some(sig) = chip
            .signatures
            .iter()
            .find(|s| signature_hit(s, &path_corpus, &idents))
        {
            out.push(entry(chip, sig));
        }
    }
    out
}

fn entry(chip: &Chip, sig: &str) -> BomEntry {
    BomEntry {
        id: chip.id.clone(),
        name: chip.name.clone(),
        category: chip.category.clone(),
        matched_signature: sig.to_string(),
        pocket_parts: chip.pocket_parts.clone(),
        provenance: chip.provenance.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refdata::RefData;

    #[test]
    fn detects_irem_m72_bom_from_module_names() {
        // synthesized MiSTer-like core: V30 main, Z80 sound, YM2151 (jt51), i8751 (mc8051)
        let files = CoreFiles::from_pairs([
            ("rtl/cpu-v30/v30.v", "module v30(); endmodule"),
            ("rtl/t80/t80.v", "module t80(); endmodule"),
            ("rtl/sound-jt51/jt51.v", "module jt51(); endmodule"),
            ("rtl/cpu-mc8051/mc8051.v", "module mc8051(); endmodule"),
        ]);
        let cat = RefData::bundled().unwrap().chips;
        let bom = detect_bom(&files, &cat);
        let ids: Vec<&str> = bom.iter().map(|b| b.id.as_str()).collect();
        assert!(ids.contains(&"nec-v30"), "V30 detected: {ids:?}");
        assert!(ids.contains(&"z80-t80"), "Z80/T80 detected");
        assert!(ids.contains(&"ym2151-jt51"), "YM2151/jt51 detected");
        assert!(ids.contains(&"i8051-mc8051"), "i8751/mc8051 detected");
    }

    #[test]
    fn jt51_is_flagged_true_drop_in() {
        let files = CoreFiles::from_pairs([("rtl/jt51.v", "module jt51(); endmodule")]);
        let cat = RefData::bundled().unwrap().chips;
        let bom = detect_bom(&files, &cat);
        let jt51 = bom.iter().find(|b| b.id == "ym2151-jt51").unwrap();
        assert!(jt51.pocket_parts.iter().any(|p| p.kind == "true-drop-in"));
    }

    #[test]
    fn empty_core_has_empty_bom() {
        let files = CoreFiles::from_pairs([("rtl/top.v", "module top(); endmodule")]);
        let cat = RefData::bundled().unwrap().chips;
        assert!(detect_bom(&files, &cat).is_empty());
    }
}
