# lucid-recon

Port-intelligence analyzer for FPGA cores. Point it at a local MiSTer core and it
emits an **Analogue-Pocket port plan** — the "plan the port before you write it"
step, in one pass. It reads the RTL, the SDC, and any `.mra`, reasons over a bundled
catalogue of public facts, and prints a seven-section plan:

1. **Clock plan** — the core's clock-ratio family, the 50→74.25 MHz reference swap,
   and the pixel-clock PLL output to add.
2. **CDC hotspots** — the real clock crossings that need datapath timing, produced
   by reusing [`cdc-sentinel`](https://github.com/lucid-fpga/cdc-sentinel) as a
   library; it explicitly flags an external-memory crossing left under the template
   blanket asynchronous cut.
3. **Memory profile** — external-memory presence and the BRAM→SDRAM relocation note.
4. **Component BOM** — detected CPU / sound chips with known proven Pocket
   implementations to source from, and their licenses.
5. **Which template to fork** — the lineage root (fork the root, not a descendant
   that carries a peer core's dead constraints).
6. **ROM inventory** — the `.mra` parts plus interleave / byte-order / offset /
   multi-slot hazards a port's loader must reproduce exactly.
7. **Risk summary.**

recon **advises** — it plans a port, it does not generate RTL or edit your files.

## Status

Early development, desk-tested only. The analyzers, the bundled reference data, and
the plan synthesis are implemented and unit-tested against in-memory core doubles,
and validated end-to-end against a synthesized fixture that reproduces a studied
core's published facts (clock ratio, memory topology, BOM, template pick). The scan
is a **heuristic read of RTL names and paths, not an elaborated netlist or a
build** — it reports the signature that fired for each match so a human can confirm,
and records its limits. The public API is not stable.

## Reference data

The catalogue under [`data/`](data) is **public fact only**: which public cores
implement which chips, public repo URLs, published clock ratios derivable from
public MAME driver XTALs, and the MiSTer→APF service mapping any porter can read off
the two public frameworks. Each entry carries its own public provenance. Licenses in
the chip catalogue are pointers — confirm each in the linked repo's own `LICENSE`
file before relying on it; the openFPGA core-template notably ships **no** license
(all-rights-reserved), so clone it yourself and do not redistribute it.

## Usage

```
lucid-recon <mister-core-dir>          # human-readable port plan
lucid-recon --json <mister-core-dir>   # machine-readable plan for downstream tools
```

Exit status is non-zero when a high-severity CDC hotspot fires (a real crossing to
constrain), so recon can gate a pipeline.

### `equiv` — audit the shim-don't-fork invariant

```
lucid-recon equiv <mister-core-dir> <pocket-port-dir>          # human-readable audit
lucid-recon equiv --json <mister-core-dir> <pocket-port-dir>   # machine-readable
```

The porting method's core rule is *shim, don't fork*: a Pocket port should wrap the
game RTL in a new framework, not modify the game logic itself. `equiv` checks that
mechanically. It extracts every RTL module from each tree, sets aside the framework
modules (the platform shell — `apf/`, `sys/`, PLLs, top-levels), and structurally
diffs the **game** modules the two trees share by name.

The diff is **structural, not textual** — comments and whitespace are stripped and the
source is tokenized, so a reformat or a re-indent is not a difference; a changed token
is. A shared game module that differs means the game RTL was modified there, and the
invariant broke. Exit status is non-zero when that happens.

It **localizes, it does not classify.** A DIFFERS is "these two modules are not the
same here" — never "this is a bug." Framework-vs-game partition is a name/path
heuristic; the report states its boundary confidence so you can audit the split
yourself. Modules present in only one tree are listed, not judged.

## Testing

```
cargo test    # unit tests (in-memory doubles) + the fixture validation
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. Unless you explicitly state otherwise, any contribution
intentionally submitted for inclusion in this crate by you, as defined in the
Apache-2.0 license, shall be dual licensed as above, without any additional
terms or conditions.
