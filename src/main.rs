//! lucid-recon CLI: analyze a MiSTer core directory and emit a port plan.
//!
//! ```text
//! lucid-recon [--json] <mister-core-dir>
//! ```
//!
//! Human report by default; `--json` emits the machine-readable plan. Exit status
//! is non-zero if a high-severity CDC hotspot fired (a real crossing to constrain),
//! so it can gate a pipeline.

use lucid_recon::error::{Error, Result};
use lucid_recon::plan::recon_dir;
use lucid_recon::report::{to_human, to_json};
use std::path::Path;
use std::process::ExitCode;

fn run() -> Result<bool> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut json = false;
    let mut dir: Option<String> = None;
    for a in args {
        match a.as_str() {
            "--json" => json = true,
            "-h" | "--help" => return Err(Error::Usage("lucid-recon [--json] <mister-core-dir>".into())),
            _ if dir.is_none() => dir = Some(a),
            _ => return Err(Error::Usage("one core directory at a time: lucid-recon [--json] <dir>".into())),
        }
    }
    let dir = dir.ok_or_else(|| Error::Usage("lucid-recon [--json] <mister-core-dir>".into()))?;
    let name = Path::new(&dir)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.clone());

    let plan = recon_dir(name, &dir)?;
    if json {
        println!("{}", to_json(&plan));
    } else {
        print!("{}", to_human(&plan));
    }
    Ok(plan.has_high_cdc())
}

fn main() -> ExitCode {
    match run() {
        Ok(high) => {
            if high {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("lucid-recon: {e}");
            ExitCode::from(2)
        }
    }
}
