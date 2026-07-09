# Bug: `brk #imm` does not reject out-of-range immediates (mask `& 0xFFFF`)

**Law:** A valid `BRK #imm16` immediate must lie in `[0, 0xFFFF]`. Any value
outside that range is an invalid operand and must be rejected with `Err`, not
silently folded into the low 16 bits.

**Impact:** `brk #65536`, `brk #0x1_0001`, `brk #-1`, … are accepted and
encoded as if they were `brk #0`, `brk #1`, `brk #0xffff`, respectively. A
caller emitting a debug breakpoint with a 17-bit (or negative) immediate gets a
*different* breakpoint code than requested with no diagnostic.

**Function:** `encode_brk` — `src/backend/arm/assembler/encoder/system.rs`

**Detected by:** Negative / error contract (range validation), differential
oracle `llvm-mc-14 --triple=aarch64-linux-gnu`.

**Minimal input:** `encode_brk(&[Operand::Imm(65536)])` (also `Imm(-1)`,
`Imm(0x1_FFFF)`).

**Expected:** `Err(...)` — `llvm-mc` rejects with
*"immediate must be an integer in range [0, 65535]."*.

**Actual:** `Ok(EncodeResult::Word(0xD420_0000))` — the immediate is masked
with `& 0xFFFF` (`0xd4200000 | ((imm as u32 & 0xFFFF) << 5)`), so `#65536`
aliases `#0`.

**Severity:** low–medium (silent wrong breakpoint code; same defect class as
`encode_svc`, but `brk` is debugging-only so blast radius is smaller).

**Regression test:** witness `b_s2_brk_rejects_out_of_range_immediate` in
`src/backend/arm/assembler/encoder/system_barriers_hints_pbt.rs` (marked
`#[ignore]`). Run with:
```
cargo test --lib system_barriers_hints::b_s2_brk_rejects_out_of_range_immediate -- --ignored
```
