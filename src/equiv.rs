//! MiSTer↔Pocket RTL-equivalence audit: did the port keep the game RTL as an
//! UNTOUCHED invariant (shim-don't-fork), or did it fork/modify it?
//!
//! The port must change the *framework skin* (the Pocket top / shim + the APF-facing
//! modules) and keep the *game RTL* identical. This partitions each side's RTL into
//! framework vs game modules (heuristically — confidence recorded), then does a
//! STRUCTURAL diff (comment/whitespace-insensitive, so a reformatted top doesn't
//! false-flag) of the game modules shared by name.
//!
//! **Honesty (binding): LOCALIZE, do not CLASSIFY.** A game-RTL module that DIFFERS
//! means the core was modified — the invariant broke — and that is reported
//! neutrally. Whether the change is an intentional adaptation or an unwanted
//! divergence is the human's call; the audit never labels it "a bug".

use crate::source::CoreFiles;
use regex::Regex;
use serde::Serialize;
use std::collections::BTreeMap;

/// A parsed RTL module (or VHDL design unit) with its normalized structural body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    /// Module / entity name.
    pub name: String,
    /// The file it was found in.
    pub file: String,
    /// Comment/whitespace-normalized body (for the structural diff).
    pub normalized: String,
    /// True if classified as framework (allowed to change).
    pub framework: bool,
}

/// The verdict for one game-RTL module across the two sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModVerdict {
    /// Present on both sides, structurally identical — the invariant held.
    SharedIdentical,
    /// Present on both sides but the body DIFFERS — the invariant broke here.
    Differs,
    /// Only in the Pocket port.
    PortOnly,
    /// Only in the MiSTer source.
    MisterOnly,
}

/// One module's audit result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModuleResult {
    /// Module name.
    pub name: String,
    /// The verdict.
    pub verdict: ModVerdict,
    /// The MiSTer-side file (if present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mister_file: Option<String>,
    /// The Pocket-side file (if present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_file: Option<String>,
}

/// The equivalence report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EquivReport {
    /// True if no shared game-RTL module DIFFERS (the shim-don't-fork invariant held
    /// for the modules the two sides share).
    pub invariant_held: bool,
    /// Game modules present on both sides (compared).
    pub shared_compared: usize,
    /// Of those, how many are structurally identical.
    pub shared_identical: usize,
    /// Game-RTL modules that DIFFER (the invariant broke here) — the findings.
    pub differs: Vec<ModuleResult>,
    /// Game modules only in the port.
    pub port_only: Vec<ModuleResult>,
    /// Game modules only in the MiSTer source.
    pub mister_only: Vec<ModuleResult>,
    /// Framework modules excluded from the game-RTL comparison (per side).
    pub framework_excluded: usize,
    /// Boundary-detection confidence note.
    pub boundary_note: String,
    /// Neutral notes.
    pub notes: Vec<String>,
}

impl EquivReport {
    /// True if the invariant broke (a shared game module differs).
    pub fn broke(&self) -> bool {
        !self.invariant_held
    }
}

/// Framework directory tokens (the parts that MUST change live under these).
const FRAMEWORK_DIRS: &[&str] =
    &["/apf/", "/sys/", "/pocket/", "/bsp/", "/platform/", "/megafunctions/", "target/pocket"];
/// Framework module names / prefixes (the APF-facing / Pocket-top / PLL skin).
const FRAMEWORK_NAMES: &[&str] = &[
    "core_top", "apf_top", "emu", "hps_io", "sys_top", "mf_pllbase", "core_pll", "mf_audio_pll",
    "lucid_shim", "core_bridge_cmd", "video_mixer",
];
const FRAMEWORK_PREFIXES: &[&str] = &["io_", "mf_", "pin_ddio", "apf_"];

fn is_framework(path: &str, name: &str) -> bool {
    let p = path.to_ascii_lowercase();
    if FRAMEWORK_DIRS.iter().any(|d| p.contains(d)) {
        return true;
    }
    let n = name.to_ascii_lowercase();
    FRAMEWORK_NAMES.iter().any(|f| n == *f) || FRAMEWORK_PREFIXES.iter().any(|f| n.starts_with(f))
}

/// Strip Verilog (`//`, `/* */`) and VHDL (`--`) comments.
fn strip_comments(text: &str) -> String {
    let block = Regex::new(r"(?s)/\*.*?\*/").unwrap().replace_all(text, " ");
    Regex::new(r"//[^\n]*|--[^\n]*").unwrap().replace_all(&block, " ").into_owned()
}

/// Normalize (comment-free) RTL text for a structural (not textual) comparison:
/// TOKENIZE (word-runs + single punctuation) and re-join canonically — so a reformat
/// (spaces around `(`, `;`, operators; blank lines) yields the identical token
/// sequence, and only a change in the RTL tokens differs.
fn normalize(text: &str) -> String {
    let tok = Regex::new(r"\w+|[^\w\s]").unwrap();
    tok.find_iter(text).map(|m| m.as_str()).collect::<Vec<_>>().join(" ")
}

/// Extract modules (Verilog `module <name> (/#/;…endmodule`) and VHDL design units
/// (`entity <name> is` → to end-of-file / next entity) from a core's RTL. Comments
/// are stripped first, and a Verilog module must have a real declaration head
/// (`(`/`#`/`;` after the name), so comment prose like "module completes the load"
/// is never mistaken for a module.
pub fn extract_modules(files: &CoreFiles) -> Vec<Module> {
    // module <name> followed by a real declaration char, then a body to endmodule
    let vmod = Regex::new(r"(?is)\bmodule\s+([A-Za-z_]\w*)\s*[#(;].*?\bendmodule\b").unwrap();
    let entity = Regex::new(r"(?i)\bentity\s+([A-Za-z_]\w*)\s+is").unwrap();

    let mut out = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for f in files.rtl() {
        let clean = strip_comments(&f.text);
        let ext = f.path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        if matches!(ext.as_str(), "vhd" | "vhdl") {
            let ents: Vec<(usize, String)> =
                entity.captures_iter(&clean).map(|c| (c.get(0).unwrap().start(), c[1].to_string())).collect();
            for (i, (start, name)) in ents.iter().enumerate() {
                let end = ents.get(i + 1).map(|(s, _)| *s).unwrap_or(clean.len());
                push_module(&mut out, &mut seen, name, &f.path, &clean[*start..end]);
            }
        } else {
            for m in vmod.captures_iter(&clean) {
                let name = m[1].to_string();
                push_module(&mut out, &mut seen, &name, &f.path, m.get(0).unwrap().as_str());
            }
        }
    }
    out
}

fn push_module(
    out: &mut Vec<Module>,
    seen: &mut std::collections::HashSet<String>,
    name: &str,
    path: &str,
    body: &str,
) {
    if !seen.insert(name.to_string()) {
        return; // first definition wins; duplicates noted only by absence
    }
    out.push(Module {
        name: name.to_string(),
        file: path.to_string(),
        normalized: normalize(body),
        framework: is_framework(path, name),
    });
}

/// Audit two cores: partition into framework vs game RTL, then structurally diff the
/// game modules shared by name.
pub fn equiv(mister: &CoreFiles, port: &CoreFiles) -> EquivReport {
    let m_mods = extract_modules(mister);
    let p_mods = extract_modules(port);

    let m_game: BTreeMap<&str, &Module> =
        m_mods.iter().filter(|m| !m.framework).map(|m| (m.name.as_str(), m)).collect();
    let p_game: BTreeMap<&str, &Module> =
        p_mods.iter().filter(|m| !m.framework).map(|m| (m.name.as_str(), m)).collect();
    let framework_excluded =
        m_mods.iter().filter(|m| m.framework).count() + p_mods.iter().filter(|m| m.framework).count();

    let mut differs = Vec::new();
    let mut shared_identical = 0usize;
    let mut shared_compared = 0usize;
    let mut port_only = Vec::new();
    let mut mister_only = Vec::new();

    let all_names: std::collections::BTreeSet<&str> =
        m_game.keys().chain(p_game.keys()).copied().collect();
    for name in all_names {
        match (m_game.get(name), p_game.get(name)) {
            (Some(m), Some(p)) => {
                shared_compared += 1;
                if m.normalized == p.normalized {
                    shared_identical += 1;
                } else {
                    differs.push(ModuleResult {
                        name: name.to_string(),
                        verdict: ModVerdict::Differs,
                        mister_file: Some(m.file.clone()),
                        port_file: Some(p.file.clone()),
                    });
                }
            }
            (Some(m), None) => mister_only.push(ModuleResult {
                name: name.to_string(),
                verdict: ModVerdict::MisterOnly,
                mister_file: Some(m.file.clone()),
                port_file: None,
            }),
            (None, Some(p)) => port_only.push(ModuleResult {
                name: name.to_string(),
                verdict: ModVerdict::PortOnly,
                mister_file: None,
                port_file: Some(p.file.clone()),
            }),
            (None, None) => {}
        }
    }

    let invariant_held = differs.is_empty();
    let notes = vec![
        "the audit LOCALIZES game-RTL divergence — it does not classify it. A DIFFERS may be an \
         intentional port adaptation or an unwanted change; deciding which is your call, not the \
         audit's."
            .to_string(),
        "structural (comment/whitespace-insensitive) diff — a reformatted or re-commented module is \
         not flagged; only a change in the RTL tokens is."
            .to_string(),
    ];
    EquivReport {
        invariant_held,
        shared_compared,
        shared_identical,
        differs,
        port_only,
        mister_only,
        framework_excluded,
        boundary_note:
            "framework vs game-RTL split is HEURISTIC (framework = APF/Pocket/sys dirs + known \
             top/shim/PLL module names); a game module in a framework dir (or vice versa) can be \
             mis-bucketed — treat the partition as a guide, not ground truth"
                .into(),
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_game_module_identical_holds_invariant() {
        // same game module (a CPU) on both sides, reformatted on the port → SHARED
        let mister = CoreFiles::from_pairs([(
            "rtl/cpu/z80.v",
            "module z80(input clk); wire a; assign a = clk; endmodule",
        )]);
        let port = CoreFiles::from_pairs([(
            "rtl/cpu/z80.v",
            "// reformatted by the porter\nmodule z80 ( input clk );\n  wire a;\n  assign a = clk;\nendmodule",
        )]);
        let r = equiv(&mister, &port);
        assert!(r.invariant_held, "reformat is not a divergence: {:?}", r.differs);
        assert_eq!(r.shared_identical, 1);
    }

    #[test]
    fn forked_game_module_differs_breaks_invariant() {
        let mister = CoreFiles::from_pairs([("rtl/cpu/z80.v", "module z80(); assign a = 1'b0; endmodule")]);
        // the port modified the core logic
        let port = CoreFiles::from_pairs([("rtl/cpu/z80.v", "module z80(); assign a = 1'b1; endmodule")]);
        let r = equiv(&mister, &port);
        assert!(r.broke());
        assert_eq!(r.differs.len(), 1);
        assert_eq!(r.differs[0].name, "z80");
        assert_eq!(r.differs[0].verdict, ModVerdict::Differs);
    }

    #[test]
    fn framework_modules_are_excluded_from_the_invariant() {
        // core_top differs (it MUST — it's the framework skin) but that is NOT a finding
        let mister = CoreFiles::from_pairs([
            ("sys/emu.v", "module emu(); assign x = 1; endmodule"),
            ("rtl/z80.v", "module z80(); assign a = 0; endmodule"),
        ]);
        let port = CoreFiles::from_pairs([
            ("src/fpga/core/core_top.v", "module core_top(); assign x = 2; endmodule"),
            ("rtl/z80.v", "module z80(); assign a = 0; endmodule"),
        ]);
        let r = equiv(&mister, &port);
        assert!(r.invariant_held, "framework tops differ but that's expected");
        assert!(r.framework_excluded >= 2, "emu + core_top excluded");
        assert_eq!(r.shared_identical, 1, "the z80 game module is shared-identical");
    }

    #[test]
    fn port_only_and_mister_only_localized() {
        let mister = CoreFiles::from_pairs([("rtl/snd.v", "module snd(); endmodule")]);
        let port = CoreFiles::from_pairs([("rtl/vid.v", "module vid(); endmodule")]);
        let r = equiv(&mister, &port);
        assert_eq!(r.mister_only.len(), 1);
        assert_eq!(r.port_only.len(), 1);
        assert_eq!(r.mister_only[0].name, "snd");
        assert_eq!(r.port_only[0].name, "vid");
    }
}
