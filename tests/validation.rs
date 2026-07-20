//! Validation — reproduce the research facts on a known studied core.
//!
//! `fixtures/irem_m72_mister/` is a synthesized MiSTer-style Irem M72 core (minimal
//! stubs, no vendor RTL): the RTL module names reproduce the M72 BOM (NEC V30 main
//! CPU, Z80/T80 sound CPU, YM2151/jt51 sound, i8751/mc8051 MCU) and its multi-port
//! SDRAM controller; the `.mra` reproduces an interleaved multi-slot ROM layout.
//!
//! recon's plan is asserted to AGREE with the published facts for M72: the 3:1
//! clock ratio, the external-memory topology + CDC hotspot, the detected BOM with
//! its sourced Pocket parts, and the lineage-root template pick. Agreement with the
//! research is the correctness proof.

use lucid_recon::plan::recon_dir;
use std::path::{Path, PathBuf};

fn m72_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/irem_m72_mister")
}

#[test]
fn recon_reproduces_m72_atlas_facts() {
    let plan = recon_dir("irem_m72", m72_dir()).expect("recon runs on the fixture");

    // (1) clock plan — Irem M72 family, 3:1 ratio, 50->74.25 ref swap, pixel PLL
    assert_eq!(plan.clock_plan.family.as_deref(), Some("irem-m72"), "clock family");
    assert!(
        plan.clock_plan.core_ratio.as_deref().unwrap().contains("3:1"),
        "M72 is a 3:1 core: {:?}",
        plan.clock_plan.core_ratio
    );
    assert!(plan.clock_plan.ref_swap.contains("74.25"), "50->74.25 ref swap");
    assert!(plan.clock_plan.pixel_pll.to_lowercase().contains("pixel"), "pixel PLL to add");

    // (2)+(3) memory topology + a high CDC hotspot for the external-memory crossing
    assert!(plan.memory.external_memory, "M72 ships an external-memory (SDRAM) controller");
    assert!(plan.has_high_cdc(), "the SDRAM crossing is a high CDC hotspot to constrain");
    assert!(
        plan.cdc_hotspots.iter().any(|h| h.kind == "external-memory-crossing"),
        "the external-memory-crossing hotspot is present"
    );

    // (4) BOM — the M72 chip set, each with a sourced Pocket part
    let bom_ids: Vec<&str> = plan.bom.iter().map(|b| b.id.as_str()).collect();
    for want in ["nec-v30", "z80-t80", "ym2151-jt51", "i8051-mc8051"] {
        assert!(bom_ids.contains(&want), "BOM must detect {want}: got {bom_ids:?}");
    }
    // jt51 is the documented true-drop-in
    let jt51 = plan.bom.iter().find(|b| b.id == "ym2151-jt51").unwrap();
    assert!(
        jt51.pocket_parts.iter().any(|p| p.kind == "true-drop-in"),
        "YM2151/jt51 sourced as a true-drop-in"
    );
    assert!(!jt51.pocket_parts.is_empty() && jt51.pocket_parts.iter().all(|p| p.url.starts_with("http")));

    // (5) template — fork the lineage root
    let tpl = plan.template.as_ref().expect("a template pick");
    assert!(tpl.fork.contains("core-template"), "fork the openFPGA core-template root");
    assert_eq!(tpl.root_commit.as_deref(), Some("ad20a21"), "the lineage root commit");

    // (6) ROM inventory — the interleaved, repeated, multi-slot layout with hazards
    assert_eq!(plan.rom_inventory.len(), 1, "one .mra parsed");
    let inv = &plan.rom_inventory[0];
    assert_eq!(inv.slots.len(), 2, "two ROM slots (main + nvram)");
    let hz: Vec<&str> = inv.hazards.iter().map(|h| h.kind.as_str()).collect();
    for want in ["interleave", "repeat", "multi-slot"] {
        assert!(hz.contains(&want), "ROM hazard {want} flagged: {hz:?}");
    }

    // services — the MiSTer framework services this core reveals it uses
    let svc: Vec<&str> = plan.services.iter().map(|s| s.matched_signature.as_str()).collect();
    assert!(svc.iter().any(|s| s.starts_with("ioctl")), "ioctl service detected: {svc:?}");
    assert!(svc.contains(&"ce_pix"), "video (ce_pix) service detected");

    // (7) risk summary carries the uncovered-crossing + template-license risks
    assert!(plan.risks.iter().any(|r| r.contains("CDC")), "uncovered-crossing risk");
    assert!(
        plan.risks.iter().any(|r| r.to_lowercase().contains("license")),
        "template no-license risk surfaced"
    );
}

#[test]
fn reference_data_is_public_fact_with_provenance() {
    // A positive public-data invariant: every catalogue entry cites a provenance
    // and every sourced part points at a public URL. (Negative enforcement — that
    // no private vocabulary crossed into the crate — is the pre-commit leakguard's
    // job, run on every commit, so it is not duplicated here.)
    let rd = lucid_recon::RefData::bundled().expect("bundled data");
    assert!(rd.chips.chips.iter().all(|c| !c.provenance.is_empty()), "every chip cites provenance");
    assert!(rd.lineage.templates.iter().all(|t| !t.provenance.is_empty()), "every template cites provenance");
    assert!(rd.clocks.families.iter().all(|f| !f.provenance.is_empty()), "every clock family cites provenance");
    assert!(
        rd.chips
            .chips
            .iter()
            .flat_map(|c| &c.pocket_parts)
            .all(|p| p.url.starts_with("https://")),
        "every sourced part points at a public URL"
    );
}
