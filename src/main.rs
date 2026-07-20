//! lucid-recon CLI: analyze a MiSTer core → a Pocket port plan, or `equiv` two
//! cores to audit the shim-don't-fork invariant.
//!
//! ```text
//! lucid-recon [--json] <mister-core-dir>
//! lucid-recon equiv [--json] <mister-core-dir> <pocket-port-dir>
//! ```
//!
//! Plan mode exits non-zero on a high-severity CDC hotspot; `equiv` exits non-zero
//! when the invariant broke (a shared game-RTL module differs).

use lucid_recon::error::{Error, Result};
use lucid_recon::plan::recon_dir;
use lucid_recon::report::{equiv_human, equiv_json, to_human, to_json};
use lucid_recon::equiv_dirs;
use std::path::Path;
use std::process::ExitCode;

fn run() -> Result<i32> {
    let mut args = std::env::args().skip(1).peekable();
    if args.peek().map(|s| s.as_str()) == Some("equiv") {
        args.next();
        return run_equiv(args.collect());
    }
    run_plan(args.collect())
}

fn run_plan(args: Vec<String>) -> Result<i32> {
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
    Ok(if plan.has_high_cdc() { 1 } else { 0 })
}

fn run_equiv(args: Vec<String>) -> Result<i32> {
    const USAGE: &str = "lucid-recon equiv [--json] <mister-core-dir> <pocket-port-dir>";
    let mut json = false;
    let mut dirs: Vec<String> = Vec::new();
    for a in args {
        match a.as_str() {
            "--json" => json = true,
            "-h" | "--help" => return Err(Error::Usage(USAGE.into())),
            _ => dirs.push(a),
        }
    }
    if dirs.len() != 2 {
        return Err(Error::Usage(USAGE.into()));
    }
    let report = equiv_dirs(&dirs[0], &dirs[1])?;
    if json {
        println!("{}", equiv_json(&report));
    } else {
        print!("{}", equiv_human(&report));
    }
    Ok(if report.broke() { 1 } else { 0 })
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("lucid-recon: {e}");
            ExitCode::from(2)
        }
    }
}
