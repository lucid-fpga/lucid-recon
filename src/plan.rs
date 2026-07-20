//! Port-plan synthesis: assemble the seven-section plan from the sub-analyzers and
//! the reused cdc-sentinel CDC/clock/memory engine.

use crate::bom::{detect_bom, BomEntry};
use crate::clockplan::{plan_clocks, ClockPlan};
use crate::error::Result;
use crate::lineage::{pick_template, TemplatePick};
use crate::mra::{parse_mra, MraInventory};
use crate::refdata::RefData;
use crate::services::{detect_services, ServiceHit};
use crate::source::CoreFiles;
use serde::Serialize;

/// A clock-domain-crossing hotspot the port must handle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CdcHotspot {
    /// Short kind slug.
    pub kind: String,
    /// Severity (`high`/`warning`).
    pub severity: String,
    /// What the crossing is and why it matters in the port.
    pub detail: String,
    /// The constraint the port should write (recon advises; it does not generate).
    pub constraint_to_write: String,
}

/// Memory profile + relocation note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MemoryProfile {
    /// Whether an external-memory controller (SDRAM/PSRAM) is present.
    pub external_memory: bool,
    /// The controllers detected.
    pub controllers: Vec<String>,
    /// Relocation / budget guidance.
    pub note: String,
}

/// The full port plan (the seven sections).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PortPlan {
    /// Core label.
    pub core: String,
    /// (1) Clock plan.
    pub clock_plan: ClockPlan,
    /// (2) CDC hotspots.
    pub cdc_hotspots: Vec<CdcHotspot>,
    /// (3) Memory profile.
    pub memory: MemoryProfile,
    /// (4) Component BOM + sourced parts.
    pub bom: Vec<BomEntry>,
    /// (5) Which template to fork.
    pub template: Option<TemplatePick>,
    /// (6) ROM inventory (per `.mra`).
    pub rom_inventory: Vec<MraInventory>,
    /// The MiSTer services the core uses + their APF equivalents.
    pub services: Vec<ServiceHit>,
    /// (7) Risk summary.
    pub risks: Vec<String>,
    /// Honest scan limits.
    pub limits: Vec<String>,
}

impl PortPlan {
    /// True if any high-severity CDC hotspot fired.
    pub fn has_high_cdc(&self) -> bool {
        self.cdc_hotspots.iter().any(|h| h.severity == "high")
    }
}

/// Analyze a core directory and produce its port plan.
pub fn recon_dir(core: impl Into<String>, dir: impl AsRef<std::path::Path>) -> Result<PortPlan> {
    let files = CoreFiles::from_dir(dir)?;
    let refdata = RefData::bundled()?;
    Ok(recon(core, &files, &refdata))
}

/// Analyze already-collected files against reference data — the pure seam.
pub fn recon(core: impl Into<String>, files: &CoreFiles, refdata: &RefData) -> PortPlan {
    let core = core.into();

    // Reuse cdc-sentinel for clock/CDC/memory.
    let cdc = cdc_sentinel::analyze(core.clone(), &files.cdc_source());

    // (1) clock plan
    let clock_plan = plan_clocks(&core, files, &refdata.clocks);

    // (2) CDC hotspots — the forward-looking port instruction + cdc-sentinel findings
    let mut cdc_hotspots = Vec::new();
    for mc in &cdc.summary.memory {
        if mc.kind.is_crossing() {
            cdc_hotspots.push(CdcHotspot {
                kind: "external-memory-crossing".into(),
                severity: "high".into(),
                detail: format!(
                    "core \u{2194} {:?} crossing ({}). In the Pocket port this is a real clock \
                     crossing — do NOT leave it under the template blanket -asynchronous cut with \
                     no datapath timing (a cdc-sentinel Lint B target).",
                    mc.kind, mc.evidence
                ),
                constraint_to_write:
                    "add a set_multicycle_path or set_false_path scoped to the memory (SDRAM) clock, \
                     covering the core\u{2194}memory paths"
                        .into(),
            });
        }
    }
    for f in &cdc.findings {
        cdc_hotspots.push(CdcHotspot {
            kind: format!("cdc-sentinel:{}", f.id),
            severity: match f.severity {
                cdc_sentinel::Severity::High => "high".into(),
                cdc_sentinel::Severity::Warning => "warning".into(),
            },
            detail: format!("{} — {}", f.subject, f.reason),
            constraint_to_write: f.fix_hint.clone(),
        });
    }

    // (3) memory profile
    let controllers: Vec<String> =
        cdc.summary.memory.iter().map(|m| format!("{:?} ({})", m.kind, m.evidence)).collect();
    let memory = MemoryProfile {
        external_memory: cdc.summary.external_memory,
        controllers,
        note: if cdc.summary.external_memory {
            "External memory present: plan the BRAM→SDRAM relocation for large regions, budget \
             remaining BRAM against the Cyclone V target, and pick the SDRAM controller class \
             (multi-port sdram_4w for arcade; a plain controller otherwise)."
                .into()
        } else {
            "BRAM-only: no external-memory relocation needed; the whole design can sit in one \
             clock domain (the blanket async cut is correct here)."
                .into()
        },
    };

    // (4) BOM
    let bom = detect_bom(files, &refdata.chips);

    // (5) template
    let template = pick_template(&refdata.lineage, cdc.summary.external_memory);

    // (6) ROM inventory
    let rom_inventory: Vec<MraInventory> = files.mra().map(|f| parse_mra(&f.text)).collect();

    // services
    let services = detect_services(files, &refdata.services);

    // (7) risks
    let mut risks = Vec::new();
    if cdc.summary.external_memory {
        risks.push(
            "Uncovered CDC crossing: an external-memory crossing must get datapath timing in the \
             port; the template's blanket async cut leaves it STA-blind (silent hold hazard)."
                .into(),
        );
    }
    for inv in &rom_inventory {
        for h in &inv.hazards {
            risks.push(format!("ROM ({}): {}", h.kind, h.detail));
        }
    }
    if bom.iter().flat_map(|b| &b.pocket_parts).any(|p| p.kind != "true-drop-in") {
        risks.push(
            "BOM reuse: sourced parts other than a documented true-drop-in diverge across ports — \
             diff each candidate against a proven copy before adopting it."
                .into(),
        );
    }
    if template.is_some() {
        risks.push(
            "Template license: the openFPGA core-template ships no license (all-rights-reserved) — \
             clone it yourself; do not redistribute it in a bundle."
                .into(),
        );
    }
    if bom.is_empty() {
        risks.push(
            "No BOM chips detected — the core may use unusually-named or generated CPU/sound cores; \
             identify them by hand before sourcing."
                .into(),
        );
    }

    let mut limits = vec![
        "Heuristic scan, not an elaborated netlist or a build: BOM/clock/service detection reads \
         RTL names and paths, so unusually-named or generated modules can be missed or \
         over-reported; every match carries the signature that fired for a human to confirm."
            .to_string(),
    ];
    if rom_inventory.is_empty() {
        limits.push(
            "No .mra found in the core directory — ROM inventory skipped; provide the .mra for the \
             ROM/hazard section."
                .to_string(),
        );
    }
    limits.extend(cdc.limits.iter().cloned());

    PortPlan {
        core,
        clock_plan,
        cdc_hotspots,
        memory,
        bom,
        template,
        rom_inventory,
        services,
        risks,
        limits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bram_only_core_has_no_high_cdc() {
        let files = CoreFiles::from_pairs([
            ("core.sdc", "set_clock_groups -asynchronous -group {core_pll}"),
            ("rtl/core_pll.v", "module core_pll(); endmodule"),
        ]);
        let rd = RefData::bundled().unwrap();
        let plan = recon("bram", &files, &rd);
        assert!(!plan.memory.external_memory);
        assert!(!plan.has_high_cdc());
    }

    #[test]
    fn external_mem_core_flags_high_cdc_and_relocation() {
        let files = CoreFiles::from_pairs([
            ("core.sdc", "set_clock_groups -asynchronous -group {core_pll}"),
            ("rtl/core_pll.v", "module core_pll(); endmodule"),
            ("rtl/sdram.v", "module sdram(); endmodule"),
        ]);
        let rd = RefData::bundled().unwrap();
        let plan = recon("mem", &files, &rd);
        assert!(plan.memory.external_memory);
        assert!(plan.has_high_cdc());
        assert!(plan.risks.iter().any(|r| r.contains("CDC")));
    }
}
