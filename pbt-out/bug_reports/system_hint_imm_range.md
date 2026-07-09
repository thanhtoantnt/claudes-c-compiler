# Bug: `hint #imm` does not reject out-of-range immediates (mask `& 0xF` / `& 0x7`)

**Law:** A valid `HINT #imm` immediate must lie in `[0, 127]` (it is split into
`CRm[3:0] = imm>>3` and `op2[2:0] = imm&7`). Any value outside that range is an
invalid operand and must be rejected with `Err`, not silently truncated.

**Impact:** `hint #128`, `hint #256`, `hint #-1`, … are accepted and encoded as
if they were `hint #0` (NOP), `hint #0`, `hint #127`, respectively. A caller
requesting a specific hint code ≥ 128 silently emits `NOP` (or another hint),
with no diagnostic. (The decoder pipeline then treats the wrong instruction as
the requested one.)

**Function:** `encode_hint` — `src/backend/arm/assembler/encoder/system.rs`

**Detected by:** Negative / error contract (range validation), differential
oracle `llvm-mc-14 --triple=aarch64-linux-gnu`.

**Minimal input:** `encode_hint(&[Operand::Imm(128)])` (also `Imm(256)`,
`Imm(-1)`, `Imm(1000)`).

**Expected:** `Err(...)` — `llvm-mc` rejects with
*"immediate must be an integer in range [0, 127]."*.

**Actual:** `Ok(EncodeResult::Word(0xD503_201F))` for `#128` — CRm/op2 are
masked with `& 0xF` / `& 0x7` (`((imm as u32) >> 3) & 0xF` and `(imm as u32) &
0x7`), so `#128` aliases `#0` (NOP). Note also that for *negative* immediates
`imm as u32` sign-extends into the high bits before masking, which still folds
into a value in `[0,127]` rather than rejecting.

**Severity:** low–medium (silent wrong hint instruction; hint codes are mostly
no-ops/diagnostics, but the miscompilation is silent).

**Regression test:** witness `b_s3_hint_rejects_out_of_range_immediate` in
`src/backend/arm/assembler/encoder/system_barriers_hints_pbt.rs` (marked
`#[ignore]`). Run with:
```
cargo test --lib system_barriers_hints::b_s3_hint_rejects_out_of_range_immediate -- --ignored
```
