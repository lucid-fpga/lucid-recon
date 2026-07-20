//! File collection for a MiSTer core directory. recon reads SDC + RTL (handed to
//! cdc-sentinel for the clock/CDC/memory analysis) **and** `.mra` recipes (for the
//! ROM inventory), so it collects a superset of what cdc-sentinel reads. The
//! [`cdc_sentinel::SourceFile`] type is reused; a [`CoreFiles`] can be built from a
//! directory or, in tests, directly from in-memory `(path, text)` pairs — the
//! testable seam.

use crate::error::{Error, Result};
use cdc_sentinel::source::{CoreSource, MemSource, SourceFile};
use std::path::Path;

/// A MiSTer core's collected source files.
#[derive(Debug, Clone, Default)]
pub struct CoreFiles {
    /// All collected files (SDC, RTL, MRA).
    pub files: Vec<SourceFile>,
}

fn ext_of(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default()
}

fn is_wanted(path: &str) -> bool {
    // Include VHDL (.vhd/.vhdl): a large fraction of real MiSTer CPU/sound cores
    // (T80, V30, 65xx, 8051, ...) are VHDL, each in a dir named for the chip, so
    // collecting them lets BOM detection find the component by path. Without this,
    // a VHDL main CPU (e.g. rtl/v30/cpu.vhd) is silently missed.
    matches!(
        ext_of(path).as_str(),
        "sdc" | "v" | "sv" | "vh" | "svh" | "vhd" | "vhdl" | "mra"
    )
}

impl CoreFiles {
    /// The in-memory constructor (test double): each `(path, text)` becomes a file.
    pub fn from_pairs<I, P, T>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (P, T)>,
        P: Into<String>,
        T: Into<String>,
    {
        CoreFiles {
            files: pairs
                .into_iter()
                .map(|(p, t)| SourceFile::new(p.into(), t.into()))
                .collect(),
        }
    }

    /// Walk a core directory, collecting SDC/RTL/MRA files. Skips `target/` and
    /// dotted directories; individual unreadable files are skipped. Fails only if
    /// `root` itself cannot be read.
    pub fn from_dir(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let mut files = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            let entries = std::fs::read_dir(&dir).map_err(|source| Error::Source {
                path: dir.display().to_string(),
                source,
            })?;
            for entry in entries.flatten() {
                let p = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if p.is_dir() {
                    if name == "target" || name.starts_with('.') {
                        continue;
                    }
                    stack.push(p);
                    continue;
                }
                let rel = p
                    .strip_prefix(&root)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .replace('\\', "/");
                if !is_wanted(&rel) {
                    continue;
                }
                if let Ok(bytes) = std::fs::read(&p) {
                    files.push(SourceFile::new(rel, String::from_utf8_lossy(&bytes).into_owned()));
                }
            }
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(CoreFiles { files })
    }

    /// A cdc-sentinel source over these files (it internally reads only SDC/RTL).
    pub fn cdc_source(&self) -> MemSource {
        let mut s = MemSource::new();
        for f in &self.files {
            s = s.with(f.path.clone(), f.text.clone());
        }
        s
    }

    /// The RTL files (for token scanning).
    pub fn rtl(&self) -> impl Iterator<Item = &SourceFile> {
        self.files.iter().filter(|f| f.is_rtl())
    }

    /// Combined RTL text (for service-signature matching).
    pub fn rtl_text(&self) -> String {
        self.rtl().map(|f| f.text.as_str()).collect::<Vec<_>>().join("\n")
    }

    /// The `.mra` files.
    pub fn mra(&self) -> impl Iterator<Item = &SourceFile> {
        self.files.iter().filter(|f| ext_of(&f.path) == "mra")
    }
}

/// Assert the reused cdc-sentinel source behaves — a compile-and-behavior guard.
impl CoreSource for CoreFiles {
    fn files(&self) -> Vec<SourceFile> {
        self.files.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_dir_collects_vhdl_cpu_cores() {
        // Regression (real MiSTer core): CPU/sound cores are often VHDL in a
        // chip-named dir (rtl/v30/cpu.vhd). from_dir must collect .vhd so BOM
        // detection can find the component by path — else a VHDL main CPU is missed.
        let dir = std::env::temp_dir().join(format!("recon-vhd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("rtl/v30")).unwrap();
        std::fs::write(dir.join("rtl/v30/cpu.vhd"), "-- vhdl\nentity cpu is end entity;").unwrap();
        let cf = CoreFiles::from_dir(&dir).unwrap();
        assert!(
            cf.files.iter().any(|f| f.path.contains("rtl/v30/cpu.vhd")),
            "VHDL cores must be collected: {:?}",
            cf.files.iter().map(|f| &f.path).collect::<Vec<_>>()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn collects_and_partitions() {
        let cf = CoreFiles::from_pairs([
            ("core.sdc", "set_clock_groups -asynchronous -group {core_pll}"),
            ("rtl/cpu-v30.v", "module v30(); endmodule"),
            ("game.mra", "<misterromdescription></misterromdescription>"),
        ]);
        assert_eq!(cf.rtl().count(), 1);
        assert_eq!(cf.mra().count(), 1);
        assert!(cf.rtl_text().contains("v30"));
    }
}
