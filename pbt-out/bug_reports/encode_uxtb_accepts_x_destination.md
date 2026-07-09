# Bug: `encode_uxtb` silently accepts an invalid 64-bit (X) destination

**Law:** `UXTB` is defined by the ARMv8 ARM ONLY as `UXTB <Wd>, <Wn>` (an alias of
`UBFM <Wd>, <Wn>, #0, #7`, 32-bit). A 64-bit destination form (`UXTB <Xd>, <Xn>`)
has **no** valid encoding — the reference assembler rejects it. A conforming
encoder MUST return `Err` for an X destination.

**Impact:** An invalid `uxtb x0, x1` is accepted instead of rejected. The
emitted word (`0xD3401C20` for `uxtb x0, x1`) is a 64-bit UBFM
(`UBFM Xd, Xn, #0, #7`, alias `ubfx x0, x1, #0, #8`) — a *different*
instruction with different width semantics. Code that expects the assembler to
guard against typos gets silently mis-encoded.

**Function:** `encode_uxtb` in
`src/backend/arm/assembler/encoder/data_processing.rs`

**Detected by:** Negative error contract (differential oracle `llvm-mc-18`).

**Minimal input:** `uxtb x0, x1`  → operands `[Operand::Reg("x0"), Operand::Reg("x1")]`
  (the property's shrunk counterexample is `n = 0`, i.e. `uxtb x0, x0`).

**Expected:** `encode_uxtb` must reject an X-destination form:
```
$ echo 'uxtb x0, x1' | llvm-mc-18 --triple=aarch64
<stdin>:1:7: error: invalid operand for instruction
```
So `encode_uxtb(&[xreg(0), xreg(1)])` should return `Err`.

**Actual:** Returns `Ok(EncodeResult::Word(0xD3401C20))` — a 64-bit UBFM word
(sf=1, opc=10, N=1, imms=7) that disassembles as `ubfx x0, x1, #0, #8`, not
`uxtb`. Root cause: `encode_uxtb` derives `sf`/`N` from the destination width
(`let n = if is_64 { 1 } else { 0 };`) with no validation that the destination
is the 32-bit form the mnemonic requires.

**Severity:** medium (silent acceptance + mis-encoding of an architecturally
invalid form; sibling `encode_uxth` has the same defect, already reported in
`uxth-accepts-invalid-64bit-ubfx.md`).

**Regression test:**
`src/backend/arm/assembler/encoder/data_processing_extend_pbt.rs` — property
`uxtb_rejects_x_destination` (kept `#[ignore]`d so the default `cargo test`
stays green). Reproduce with:
```
cargo test --lib data_processing_extend_pbt::uxtb_rejects_x_destination -- --ignored
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/309
