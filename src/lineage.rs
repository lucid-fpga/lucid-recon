//! Template-lineage pick: which canonical Pocket template to fork. Recommends the
//! lineage root (fork the root, not a descendant that carries a peer core's phantom
//! groups) and lists the alternatives, carrying the important public caveat that
//! the root template ships no license.

use crate::refdata::LineageTable;
use serde::Serialize;

/// An alternative lineage a porter might pick instead of the root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Alternative {
    /// The framework/porter.
    pub framework: String,
    /// The fork.
    pub fork: String,
    /// When to prefer it.
    pub note: String,
}

/// The recommended template to fork.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TemplatePick {
    /// The repo to fork.
    pub fork: String,
    /// The lineage root commit, if known.
    pub root_commit: Option<String>,
    /// Why.
    pub note: String,
    /// Public URL.
    pub url: String,
    /// Public provenance.
    pub provenance: String,
    /// Other lineages worth knowing.
    pub alternatives: Vec<Alternative>,
}

/// Pick the template to fork. `external_memory` refines the advice note (a
/// console-style SDRAM core benefits from the mf_pllbase reference), but the
/// recommended fork is always the lineage root.
pub fn pick_template(lineage: &LineageTable, external_memory: bool) -> Option<TemplatePick> {
    let root = lineage.templates.iter().find(|t| t.prefer)?;
    let mut note = root.note.clone();
    if external_memory {
        note.push_str(
            " This core has external memory — study the mf_pllbase (agg23) split-PLL reference \
             for the SDRAM clock, but still fork the root.",
        );
    }
    let alternatives = lineage
        .templates
        .iter()
        .filter(|t| !t.prefer)
        .map(|t| Alternative {
            framework: t.framework.clone(),
            fork: t.fork.clone(),
            note: t.note.clone(),
        })
        .collect();
    Some(TemplatePick {
        fork: root.fork.clone(),
        root_commit: root.root_commit.clone(),
        note,
        url: root.url.clone(),
        provenance: root.provenance.clone(),
        alternatives,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refdata::RefData;

    #[test]
    fn picks_the_root_and_lists_alternatives() {
        let lineage = RefData::bundled().unwrap().lineage;
        let pick = pick_template(&lineage, true).unwrap();
        assert!(pick.fork.contains("core-template"));
        assert_eq!(pick.root_commit.as_deref(), Some("ad20a21"));
        assert!(!pick.alternatives.is_empty());
        assert!(pick.note.contains("mf_pllbase"), "external-mem advice added");
        // the no-license caveat rides in the root note
        assert!(pick.note.to_lowercase().contains("license"));
    }
}
