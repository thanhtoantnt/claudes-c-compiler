# Bug: `svc #imm` does not reject out-of-range immediates (mask `& 0xFFFF`)

**Law:** A valid `SVC #imm16` immediate must lie in `[0, 0xFFFF]`. Any value
outside that range is an invalid operand and must be rejected with `Err`, not
silently folded into the low 16 bits.

**Impact:** `svc #65536`, `svc #0x1_0001`, `svc #-1`, … are accepted and
encoded as if they were `svc #0`, `svc #1`, `svc #0xffff`, respectively. A
caller passing a 17-bit (or negative) syscall number gets a *different* syscall
than requested with no diagnostic — a silent miscompilation of the program's
entry into the kernel.

**Function:** `encode_svc` — `src/backend/arm/assembler/encoder/system.rs`

**Detected by:** Negative / error contract (range validation), differential
oracle `llvm-mc-14 --triple=aarch64-linux-gnu`.

**Minimal input:** `encode_svc(&[Operand::Imm(65536)])` (also `Imm(-1)`,
`Imm(0x1_FFFF)`).

**Expected:** `Err(...)` — `llvm-mc` rejects with
*"immediate must be an integer in range [0, 65535]."*.

**Actual:** `Ok(EncodeResult::Word(0xD400_0001))` — the immediate is masked
with `& 0xFFFF` (`0xd4000001 | ((imm as u32 & 0xFFFF) << 5)`), so `#65536`
aliases `#0`.

**Severity:** medium (silent wrong syscall number; no crash, but wrong runtime
behaviour).

**Regression test:** witness `b_s1_svc_rejects_out_of_range_immediate` in
`src/backend/arm/assembler/encoder/system_barriers_hints_pbt.rs` (marked
`#[ignore]`). Run with:
```
cargo test --lib system_barriers_hints::b_s1_svc_rejects_out_of_range_immediate -- --ignored
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/331
