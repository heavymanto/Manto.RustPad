# Contributing to Manto.RustPad

Thank you for the interest. The project lives on two things: a core that is
honest about big files, and documentation whose numbers were really measured.
Before opening a PR, read the points below: they are few, and all practical.

## How to build

```sh
cd rust
cargo build --release
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

The core has one tool for looking at it, the `info` example:

```sh
cargo run --release -p mrp-core --example info -- <file>
```

The test that needs a file beyond 4 GiB is `#[ignore]`d (it takes a 6 GB
file). Generate it with `bench/gen_testfile.py` and run:

```sh
MRP_BIG_FILE=/path/to/big6g.txt cargo test --release -p mrp-core -- --ignored --nocapture
```

The benchmark scripts in `bench/` are Windows-only (Win32 + ConPTY), Python 3.12,
no third-party packages. See [bench/README.md](bench/README.md).

## Issues

Three doors: **bug**, **question**, **feature request**. A note on
expectations: this is a hobby project with one maintainer — no response times
are promised. Everything gets read; answers arrive when life allows.

## The rules of the game

1. **Never read a whole file into memory.** Files are memory-mapped read-only
   or streamed. Memory must not depend on file size; a test or a script that
   builds a big file writes it as a stream.
2. **Offsets and counts are `u64`.** No `usize` or `u32` for a position in a
   file (a 32-bit file position is exactly the bug that makes another editor
   truncate at 4 GiB). New code that handles positions comes with a test that
   crosses 4 GiB or an argument for why it cannot.
3. **Bytes, not UTF-8.** The content of a file is bytes; do not assume it is
   valid text.
4. **Tests first, with an independent oracle.** A test compares the code with
   a naive, obviously-correct implementation on small inputs, not with the
   code's own idea of the answer. Changes to concurrency come with a test that
   fails (rather than hangs forever) if the change is wrong.
5. **`unsafe` only where it cannot be avoided**, one block, with a `// SAFETY:`
   comment. Currently: the single `Mmap::map` call.
6. **Every number in the documentation was really measured**, with a script in
   `bench/` and the raw result in `bench/results/`. Say what is *not* measured.
   Cold-cache (after `flush_cache.py`) and warm-cache numbers are different
   things: never mix them.
7. **The documentation speaks English** — one language for everybody, with
   `README.it.md` as the single Italian counterpart. Code comments, test names
   and the bench scripts stay in the maintainer's voice (Italian): they are
   maintainer-facing, not user-facing.
8. **`cargo fmt` and `cargo clippy -- -D warnings` must be clean**; CI checks them.

## License of the contributions

The project is distributed under **GNU AGPLv3** (see `LICENSE`). By
contributing:

- you grant that your contribution is released under the same
  AGPL-3.0-only license as the project;
- you confirm the contribution is your original work, or material you
  have the right to submit under these terms.

No copyright assignment: the contribution stays yours. The confirmation
is needed because the project owner reserves the right to offer
commercial licenses on the code in the future (dual-licensing model),
which is only possible if contributions arrive with clear rights.
