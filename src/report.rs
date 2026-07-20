//! Rendering the [`PortPlan`]: structured JSON (shaped so downstream tools can
//! consume the plan and its CDC hotspots) and a human-readable report.

use crate::plan::PortPlan;
use serde::Serialize;

/// Tool name emitted in JSON.
pub const TOOL: &str = "lucid-recon";
/// Tool version emitted in JSON.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize)]
struct JsonEnvelope<'a> {
    tool: &'static str,
    version: &'static str,
    #[serde(flatten)]
    plan: &'a PortPlan,
}

/// The plan as pretty JSON.
pub fn to_json(plan: &PortPlan) -> String {
    let env = JsonEnvelope { tool: TOOL, version: VERSION, plan };
    serde_json::to_string_pretty(&env).expect("PortPlan serializes")
}

fn section(out: &mut String, title: &str) {
    out.push_str(&format!("\n-- {title} --\n"));
}

/// The plan as human-readable text.
pub fn to_human(plan: &PortPlan) -> String {
    let mut out = String::new();
    out.push_str(&format!("=========== port plan: {} ===========\n", plan.core));

    section(&mut out, "1. clock plan");
    match (&plan.clock_plan.family, &plan.clock_plan.core_ratio) {
        (Some(f), Some(r)) => out.push_str(&format!("  family: {f}\n  ratio : {r}\n")),
        _ => out.push_str("  family: (none matched — preserve the core's own ratios)\n"),
    }
    out.push_str(&format!("  ref   : {}\n", plan.clock_plan.ref_swap));
    out.push_str(&format!("  pixel : {}\n", plan.clock_plan.pixel_pll));
    for n in &plan.clock_plan.notes {
        out.push_str(&format!("  note  : {n}\n"));
    }

    section(&mut out, "2. CDC hotspots");
    if plan.cdc_hotspots.is_empty() {
        out.push_str("  none — no external-memory crossing detected\n");
    } else {
        for h in &plan.cdc_hotspots {
            out.push_str(&format!("  [{}] {}: {}\n", h.severity, h.kind, h.detail));
            out.push_str(&format!("       write: {}\n", h.constraint_to_write));
        }
    }

    section(&mut out, "3. memory profile");
    out.push_str(&format!("  external_memory: {}\n", plan.memory.external_memory));
    for c in &plan.memory.controllers {
        out.push_str(&format!("  controller: {c}\n"));
    }
    out.push_str(&format!("  {}\n", plan.memory.note));

    section(&mut out, "4. component BOM + sourced parts");
    if plan.bom.is_empty() {
        out.push_str("  none detected\n");
    } else {
        for b in &plan.bom {
            out.push_str(&format!("  {} [{}] (sig `{}`)\n", b.name, b.category, b.matched_signature));
            for p in &b.pocket_parts {
                out.push_str(&format!("     source: {} — {} — {} — {}\n", p.source, p.kind, p.license, p.url));
            }
        }
    }

    section(&mut out, "5. template to fork");
    match &plan.template {
        Some(t) => {
            out.push_str(&format!("  fork: {} ({})\n", t.fork, t.url));
            if let Some(c) = &t.root_commit {
                out.push_str(&format!("  root commit: {c}\n"));
            }
            out.push_str(&format!("  {}\n", t.note));
            for a in &t.alternatives {
                out.push_str(&format!("  alt: {} — {}\n", a.fork, a.framework));
            }
        }
        None => out.push_str("  (no template data)\n"),
    }

    section(&mut out, "6. ROM inventory (.mra)");
    if plan.rom_inventory.is_empty() {
        out.push_str("  no .mra supplied\n");
    } else {
        for inv in &plan.rom_inventory {
            out.push_str(&format!(
                "  {} (rbf {}): {} slot(s), {} part(s)\n",
                inv.name.as_deref().unwrap_or("?"),
                inv.rbf.as_deref().unwrap_or("?"),
                inv.slots.len(),
                inv.part_count()
            ));
            for h in &inv.hazards {
                out.push_str(&format!("     hazard [{}]: {}\n", h.kind, h.detail));
            }
            for l in &inv.limits {
                out.push_str(&format!("     limit: {l}\n"));
            }
        }
    }

    if !plan.services.is_empty() {
        section(&mut out, "MiSTer→APF services detected");
        for s in &plan.services {
            out.push_str(&format!("  {} → {} (sig `{}`)\n", s.mister, s.apf, s.matched_signature));
        }
    }

    section(&mut out, "7. risk summary");
    if plan.risks.is_empty() {
        out.push_str("  (no elevated risks flagged)\n");
    } else {
        for r in &plan.risks {
            out.push_str(&format!("  - {r}\n"));
        }
    }

    for l in &plan.limits {
        out.push_str(&format!("  limit: {l}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::recon;
    use crate::refdata::RefData;
    use crate::source::CoreFiles;

    #[test]
    fn json_round_trips_with_tool_envelope() {
        let files = CoreFiles::from_pairs([
            ("core.sdc", "set_clock_groups -asynchronous -group {core_pll}"),
            ("rtl/sdram.v", "module sdram(); endmodule"),
        ]);
        let plan = recon("t", &files, &RefData::bundled().unwrap());
        let js = to_json(&plan);
        let v: serde_json::Value = serde_json::from_str(&js).unwrap();
        assert_eq!(v["tool"], "lucid-recon");
        assert!(!v["cdc_hotspots"].as_array().unwrap().is_empty());
    }
}
