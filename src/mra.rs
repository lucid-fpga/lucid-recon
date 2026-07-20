//! `.mra` ROM inventory + hazard flagging.
//!
//! An MRA is the MiSTer ROM-build recipe: a small XML file naming the ROM parts,
//! their pack order, and any interleave / byte-order / repeat / offset transforms.
//! Getting that transform wrong loads a subtly corrupt ROM that boots and then
//! misbehaves — the silent-corruption class that sinks a port. recon v1 does
//! **inventory + hazard-flagging**, not a full equivalence proof (that is a
//! separate checker): it lists the parts and raises a flag wherever a transform
//! exists that a port's loader must reproduce exactly.
//!
//! The parse is a lightweight tag/attribute scan of the constrained MRA subset,
//! not a general XML parser — it records what it could not read rather than
//! guessing.

use regex::Regex;
use serde::Serialize;

/// One `<part>` inside a ROM slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RomPart {
    /// `name=` (a file in the zip), if present.
    pub name: Option<String>,
    /// `crc=`, if present.
    pub crc: Option<String>,
    /// `repeat=` (a part padded/duplicated) — a transform the loader must match.
    pub repeat: Option<String>,
    /// `offset=`, if present.
    pub offset: Option<String>,
    /// True if this part sits inside an `<interleave>` group.
    pub interleaved: bool,
}

/// One `<rom index=...>` slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RomSlot {
    /// `index=` (0 = the main ROM; higher indices are nvram / secondary slots).
    pub index: Option<String>,
    /// `zip=` source archive(s), if named on the slot.
    pub zip: Option<String>,
    /// The parts, in pack order.
    pub parts: Vec<RomPart>,
    /// The `output=` widths of any interleave groups (e.g. 16, 32) — a byte-order
    /// transform the loader must reproduce.
    pub interleave_outputs: Vec<String>,
}

/// A flagged ROM-format hazard: a transform whose loader mistake corrupts silently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RomHazard {
    /// A short kind slug (e.g. `interleave`, `repeat`, `multi-slot`).
    pub kind: String,
    /// One-sentence description of what the port loader must reproduce.
    pub detail: String,
}

/// The parsed MRA inventory.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MraInventory {
    /// `<name>` of the set, if present.
    pub name: Option<String>,
    /// `<rbf>` core name, if present.
    pub rbf: Option<String>,
    /// ROM slots.
    pub slots: Vec<RomSlot>,
    /// Flagged hazards.
    pub hazards: Vec<RomHazard>,
    /// What the scan could not read (honesty over guessing).
    pub limits: Vec<String>,
}

impl MraInventory {
    /// Total part count across all slots.
    pub fn part_count(&self) -> usize {
        self.slots.iter().map(|s| s.parts.len()).sum()
    }
}

fn attr(re_cache: &mut Vec<(String, Regex)>, tag: &str, key: &str) -> Option<String> {
    // build/cache a `key="..."` extractor
    let pat = format!(r#"{key}\s*=\s*"([^"]*)""#);
    let re = re_cache
        .iter()
        .find(|(p, _)| p == &pat)
        .map(|(_, r)| r.clone())
        .unwrap_or_else(|| {
            let r = Regex::new(&pat).unwrap();
            re_cache.push((pat.clone(), r.clone()));
            r
        });
    re.captures(tag).map(|c| c[1].to_string())
}

/// Parse MRA text into an inventory with hazards. Never panics on odd input; what
/// it cannot read becomes a `limits` note.
pub fn parse_mra(text: &str) -> MraInventory {
    let mut inv = MraInventory::default();
    let mut cache: Vec<(String, Regex)> = Vec::new();

    let name_re = Regex::new(r"(?is)<name>\s*(.*?)\s*</name>").unwrap();
    let rbf_re = Regex::new(r"(?is)<rbf>\s*(.*?)\s*</rbf>").unwrap();
    inv.name = name_re.captures(text).map(|c| c[1].trim().to_string());
    inv.rbf = rbf_re.captures(text).map(|c| c[1].trim().to_string());

    let rom_re = Regex::new(r"(?is)<rom\b([^>]*)>(.*?)</rom>").unwrap();
    let self_rom_re = Regex::new(r#"(?is)<rom\b([^>]*?)/>"#).unwrap();
    let interleave_re = Regex::new(r"(?is)<interleave\b([^>]*)>(.*?)</interleave>").unwrap();
    let part_open_re = Regex::new(r#"(?is)<part\b([^>]*?)/?>"#).unwrap();

    let has_rom = rom_re.is_match(text);
    if !has_rom && !self_rom_re.is_match(text) {
        inv.limits.push("no <rom> slot found — not a recognizable MRA, or an unusual layout".into());
    }

    for cap in rom_re.captures_iter(text) {
        let attrs = &cap[1];
        let body = &cap[2];
        let mut slot = RomSlot {
            index: attr(&mut cache, attrs, "index"),
            zip: attr(&mut cache, attrs, "zip"),
            parts: Vec::new(),
            interleave_outputs: Vec::new(),
        };

        // interleave groups first (their parts are byte-order-transformed)
        let mut interleaved_spans: Vec<(usize, usize)> = Vec::new();
        for il in interleave_re.captures_iter(body) {
            if let Some(out) = attr(&mut cache, &il[1], "output") {
                slot.interleave_outputs.push(out);
            }
            let m = il.get(0).unwrap();
            interleaved_spans.push((m.start(), m.end()));
            for p in part_open_re.captures_iter(&il[2]) {
                slot.parts.push(make_part(&mut cache, &p[1], true));
            }
        }
        // parts outside interleave groups
        for p in part_open_re.captures_iter(body) {
            let m = p.get(0).unwrap();
            let inside =
                interleaved_spans.iter().any(|(s, e)| m.start() >= *s && m.end() <= *e);
            if inside {
                continue;
            }
            slot.parts.push(make_part(&mut cache, &p[1], false));
        }
        inv.slots.push(slot);
    }

    derive_hazards(&mut inv);
    inv
}

fn make_part(cache: &mut Vec<(String, Regex)>, attrs: &str, interleaved: bool) -> RomPart {
    RomPart {
        name: attr(cache, attrs, "name"),
        crc: attr(cache, attrs, "crc"),
        repeat: attr(cache, attrs, "repeat"),
        offset: attr(cache, attrs, "offset"),
        interleaved,
    }
}

fn derive_hazards(inv: &mut MraInventory) {
    for slot in &inv.slots {
        if !slot.interleave_outputs.is_empty() {
            inv.hazards.push(RomHazard {
                kind: "interleave".into(),
                detail: format!(
                    "slot {} interleaves parts at output width {} — the port loader must \
                     reproduce the exact byte/word interleave or the ROM loads corrupt",
                    slot.index.as_deref().unwrap_or("?"),
                    slot.interleave_outputs.join(",")
                ),
            });
        }
        if slot.parts.iter().any(|p| p.repeat.is_some()) {
            inv.hazards.push(RomHazard {
                kind: "repeat".into(),
                detail: format!(
                    "slot {} has repeated/padded parts (repeat=) — pad size and order must match",
                    slot.index.as_deref().unwrap_or("?")
                ),
            });
        }
        if slot.parts.iter().any(|p| p.offset.is_some()) {
            inv.hazards.push(RomHazard {
                kind: "offset".into(),
                detail: format!(
                    "slot {} places parts at explicit offsets (offset=) — the load map is not a plain concat",
                    slot.index.as_deref().unwrap_or("?")
                ),
            });
        }
    }
    if inv.slots.len() > 1 {
        inv.hazards.push(RomHazard {
            kind: "multi-slot".into(),
            detail: format!(
                "{} ROM slots — secondary slots (nvram / a second device) each need their own load path",
                inv.slots.len()
            ),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const M72_LIKE: &str = r#"
<misterromdescription>
  <name>Sample Arcade</name>
  <rbf>sample</rbf>
  <rom index="0" zip="sample.zip">
    <interleave output="16">
      <part name="cpu.h" crc="1111"/>
      <part name="cpu.l" crc="2222"/>
    </interleave>
    <part name="snd.rom" crc="3333" repeat="2"/>
  </rom>
  <rom index="1">
    <part name="nvram.bin"/>
  </rom>
</misterromdescription>
"#;

    #[test]
    fn parses_parts_and_metadata() {
        let inv = parse_mra(M72_LIKE);
        assert_eq!(inv.name.as_deref(), Some("Sample Arcade"));
        assert_eq!(inv.rbf.as_deref(), Some("sample"));
        assert_eq!(inv.slots.len(), 2);
        assert_eq!(inv.part_count(), 4);
        // first two parts are interleaved
        assert!(inv.slots[0].parts[0].interleaved);
        assert!(inv.slots[0].parts[1].interleaved);
        assert!(!inv.slots[0].parts[2].interleaved);
        assert_eq!(inv.slots[0].interleave_outputs, vec!["16"]);
    }

    #[test]
    fn flags_interleave_repeat_and_multislot() {
        let inv = parse_mra(M72_LIKE);
        let kinds: Vec<&str> = inv.hazards.iter().map(|h| h.kind.as_str()).collect();
        assert!(kinds.contains(&"interleave"));
        assert!(kinds.contains(&"repeat"));
        assert!(kinds.contains(&"multi-slot"));
    }

    #[test]
    fn plain_concat_has_no_transform_hazard() {
        let inv = parse_mra(
            r#"<misterromdescription><rom index="0"><part name="a"/><part name="b"/></rom></misterromdescription>"#,
        );
        assert_eq!(inv.slots.len(), 1);
        assert!(inv.hazards.is_empty(), "a plain concat single slot has no hazard");
    }

    #[test]
    fn non_mra_records_a_limit() {
        let inv = parse_mra("<html>not an mra</html>");
        assert!(!inv.limits.is_empty());
    }
}
