# Bug Report: `encode_ccmp_ccmn` silently truncates out-of-range `imm5` and `nzcv`

**Location:** `src/backend/arm/assembler/encoder/compare_branch.rs`, function `encode_ccmp_ccmn`

## Summary

In the immediate form (`CCMP/CCMN Rn, #imm5, #nzcv, cond`) the encoder masks the
operands with `& 0x1F` and `& 0xF` respectively, without first validating that
they fit their fields. Out-of-range and negative immediates are accepted and
silently encoded as a *different* value, with no diagnostic.

Per the ARM ARM (Conditional compare, immediate form):
- `imm5` is a **5-bit unsigned** immediate — valid range `0..=31`.
- `nzcv` is a **4-bit** field (one bit per N/Z/C/V flag) — valid range `0..=15`.

No cited spec permits wrapping for either field, so out-of-range inputs are
architecturally invalid and must be rejected.

The relevant source:

```rust
let word = (sf << 31) | op | (1 << 29) | (0b11010010 << 21)
    | ((*imm5 as u32 & 0x1F) << 16) | (cond_val << 12) | (1 << 11)
    | (rn << 5) | (*nzcv as u32 & 0xF);
```

## Reproduction

Failing property: `prop_ccmp_ccmn_tests::prop_rejects_out_of_range_immediates`

Minimal failing input (proptest-shrunk):

```text
rn = w0, imm5 = 32, nzcv = 16, cond = eq, is_ccmp = false
```

i.e. `ccmn w0, #32, #16, eq`.

The encoder returns `Ok(Word(0x3A400000))` — the same encoding as
`ccmn w0, #0, #0, eq` — instead of returning `Err`.

Concretely:
- `imm5 = 32` (0x20) is truncated to `0x20 & 0x1F = 0` (loss of the high bit).
- `nzcv = 16` (0x10) is truncated to `0x10 & 0xF = 0`.
- A *negative* `imm5` such as `-1` is accepted too: `-1i64 as u32 & 0x1F = 31`,
  so `ccmp x0, #-1, #0, eq` silently becomes `ccmp x0, #31, #0, eq`.

## Impact

Silent miscompilation / wrong code generation. An out-of-range immediate that
should be a hard assembler error is instead assembled into a plausible but
incorrect instruction, masking typos and bad codegen in upstream consumers with
no diagnostic.

## Suggested fix

Validate both operands before encoding, for both the `ccmp` and `ccmn` paths
(they share the same masking code):

```rust
if !(*imm5 >= 0 && *imm5 <= 31) {
    return Err(format!("ccmp/ccmn immediate out of range (0..=31): {}", imm5));
}
if !(*nzcv >= 0 && *nzcv <= 15) {
    return Err(format!("ccmp/ccmn nzcv out of range (0..=15): {}", nzcv));
}
```

The masked writes (`& 0x1F`, `& 0xF`) can then be left in place as a defensive
no-op or removed once range validation guarantees they are identity.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/21
