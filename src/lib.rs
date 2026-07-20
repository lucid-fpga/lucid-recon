//! Port-intelligence analyzer for FPGA cores: read a MiSTer core and emit an
//! Analogue-Pocket **port plan**.
//!
//! recon PLANS a port before anyone writes it. Given a local MiSTer core directory
//! it reads the RTL, the SDC, and any `.mra`, and emits the seven-section plan from
//! the port playbook:
//!
//! 1. **clock plan** — the core's ratio family, the 50→74.25 reference swap, the
//!    pixel-PLL output to add;
//! 2. **CDC hotspots** — the real clock crossings that need datapath timing,
//!    produced by reusing the [`cdc_sentinel`] engine;
//! 3. **memory profile** — external-memory presence + the BRAM→SDRAM relocation note;
//! 4. **component BOM** — detected CPU/sound chips with sourced proven-Pocket parts
//!    and their licenses;
//! 5. **which template to fork** — the lineage root;
//! 6. **ROM inventory** — the `.mra` parts + interleave/byte-order/offset hazards;
//! 7. **risk summary**.
//!
//! recon **advises**; it does not generate RTL or edit the user's files.
//!
//! # Design
//!
//! Every analyzer is a pure function of the parsed files + the bundled
//! [`refdata::RefData`], so they unit-test against in-memory [`source::CoreFiles`]
//! doubles. The clock / CDC / memory analysis is not re-implemented — it reuses the
//! `cdc-sentinel` crate as a library.
//!
//! ```
//! use lucid_recon::{plan::recon, refdata::RefData, source::CoreFiles};
//!
//! let core = CoreFiles::from_pairs([
//!     ("core.sdc", "set_clock_groups -asynchronous -group {core_pll}"),
//!     ("rtl/sound-jt51/jt51.v", "module jt51(); endmodule"),
//!     ("rtl/sdram.v", "module sdram(); endmodule"),
//! ]);
//! let plan = recon("demo", &core, &RefData::bundled().unwrap());
//! assert!(plan.memory.external_memory);            // SDRAM crossing detected
//! assert!(plan.bom.iter().any(|b| b.id == "ym2151-jt51")); // YM2151 sourced
//! ```
//!
//! # Scope and honesty
//!
//! Heuristic scan — RTL names and paths, not an elaborated netlist or a build. The
//! bundled reference data is **public fact only** (public repos, published chips,
//! MAME XTALs), each entry carrying its own public provenance. Findings carry the
//! signature that fired and the plan records its limits.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod bom;
pub mod clockplan;
pub mod error;
pub mod lineage;
pub mod loader;
pub mod mra;
pub mod plan;
pub mod refdata;
pub mod report;
pub mod services;
pub mod source;

pub use error::{Error, Result};
pub use plan::{recon, recon_dir, PortPlan};
pub use refdata::RefData;
pub use source::CoreFiles;
