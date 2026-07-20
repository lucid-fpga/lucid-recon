//! ROM-loader RTL hazard scan (the format-awareness principle at the load path).
//!
//! recon's `.mra` inventory catches interleave/pack transforms the MRA *encodes*,
//! but a MiSTer core's **ROM-loader RTL** can carry address-permutation / swizzle /
//! reorder / byte-swap logic the `.mra` does not — and a port that fails to
//! reproduce it loads silently-corrupt graphics (the class the M90 port died in).
//! This scan reads the loader-context RTL and flags such patterns.
//!
//! **Honesty discipline (binding).** This is HEURISTIC and **errs toward flagging**:
//! it surfaces *candidates* ("possible address swizzle in `rtl/rom.sv` — verify"),
//! never a claimed decode of the permutation. A missed swizzle corrupts silently; a
//! false flag merely asks the porter to check. It never claims "no swizzle" — the
//! dangerous output would be false confidence.

use crate::source::CoreFiles;
use serde::Serialize;

/// A candidate ROM-format hazard found in loader RTL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoaderHazard {
    /// Short kind slug.
    pub kind: String,
    /// The RTL file it was found in.
    pub file: String,
    /// The token that fired the flag.
    pub token: String,
    /// What to verify.
    pub detail: String,
    /// Always `"candidate"` — heuristic, verify by hand.
    pub confidence: String,
}

/// Tokens that suggest an address-permutation / swizzle / byte-order transform.
const SWIZZLE_TOKENS: &[&str] = &[
    "reorder", "swizzle", "deswizzle", "byteswap", "byte_swap", "bitswap", "bit_swap",
    "permute", "interleave",
];

/// A file is loader-context if its name suggests the ROM load path. Scoping the
/// scan here keeps it quiet on unrelated RTL (e.g. a video layer that also happens
/// to reorder pixels) — a swizzle token there is not a ROM-format hazard.
fn is_loader_file(path: &str) -> bool {
    let base = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    ["rom", "load", "ioctl", "download", "data_io", "romgen"]
        .iter()
        .any(|k| base.contains(k))
}

/// Scan the core's loader-context RTL for candidate swizzle/reorder/byte-order
/// hazards. Returns one hazard per (file, token) hit.
pub fn scan_rom_loader(files: &CoreFiles) -> Vec<LoaderHazard> {
    let mut out: Vec<LoaderHazard> = Vec::new();
    for f in files.rtl().filter(|f| is_loader_file(&f.path)) {
        let low = f.text.to_ascii_lowercase();
        for tok in SWIZZLE_TOKENS {
            if !low.contains(tok) {
                continue;
            }
            // one hazard per (file, token)
            if out.iter().any(|h| h.file == f.path && h.token == *tok) {
                continue;
            }
            out.push(LoaderHazard {
                kind: "rom-loader-swizzle-candidate".into(),
                file: f.path.clone(),
                token: (*tok).to_string(),
                detail: format!(
                    "possible ROM address permutation / swizzle in `{}` (token `{tok}`) in the load \
                     path — the port loader must reproduce it exactly or graphics/ROM load corrupt. \
                     CANDIDATE: verify against the MiSTer loader; recon does not decode the \
                     permutation.",
                    f.path
                ),
                confidence: "candidate".into(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_a_swizzle_in_loader_rtl() {
        // synthesized minimal loader stub (pattern-inspired; NOT copied from any core)
        let files = CoreFiles::from_pairs([(
            "rtl/rom.sv",
            "// load path\nfunction [63:0] reorder_64(input [63:0] a); endfunction",
        )]);
        let hz = scan_rom_loader(&files);
        assert_eq!(hz.len(), 1);
        assert_eq!(hz[0].token, "reorder");
        assert_eq!(hz[0].confidence, "candidate");
        assert!(hz[0].file.contains("rom.sv"));
    }

    #[test]
    fn stays_quiet_on_non_loader_swizzle() {
        // a video layer that reorders pixels is NOT a ROM-format hazard
        let files = CoreFiles::from_pairs([(
            "rtl/ga25_layer.sv",
            "// video\nwire x = reorder_pixels(y);",
        )]);
        assert!(scan_rom_loader(&files).is_empty(), "non-loader swizzle not flagged");
    }

    #[test]
    fn stays_quiet_on_plain_loader() {
        let files = CoreFiles::from_pairs([(
            "rtl/rom.sv",
            "// plain concat load, no permutation\nassign dout = rom[addr];",
        )]);
        assert!(scan_rom_loader(&files).is_empty(), "plain loader has no swizzle candidate");
    }

    #[test]
    fn errs_toward_flag_on_byteswap() {
        let files = CoreFiles::from_pairs([("rtl/rom_loader.v", "wire [15:0] w = byteswap(d);")]);
        let hz = scan_rom_loader(&files);
        assert_eq!(hz.len(), 1);
        assert_eq!(hz[0].token, "byteswap");
    }
}
