# Bug: `encode_neon_sqshrun` accepts out-of-range narrowing shift amounts

**Location:** `src/backend/arm/assembler/encoder/neon.rs`, line 141, function `encode_neon_sqshrun`

## Summary

`encode_neon_sqshrun` (the AArch64 *narrowing* signed-saturating-shift-right-unsigned-
narrow encoder for `SQSHRUN`/`SQSHRUN2` and `SQRSHRUN`/`SQRSHRUN2`) permits a shift
up to `esize_src` (16/32/64) where the ARMv8 ARM narrowing form requires
`1 ..= esize_src / 2` (8/16/32). Same root defect as `encode_neon_qshrn`
(separate report), but a *different manifestation*: because this encoder
reconstructs `immh` as `(immhb >> 3) | immh_base`, an over-range shift does **not**
yield an unallocated `immh=0000`; instead it yields a *valid* `immh:immb` that
decodes to the **wrong shift value**, producing a correctly-formed instruction
whose shift silently differs from the source text.

## Doc evidence that `esize/2` is the intended narrowing bound

The immediately adjacent, identical-purpose encoder `encode_neon_shrn`
(`src/backend/arm/assembler/encoder/neon.rs:1444`) uses the *correct* bound:

```rust
let half_bits = element_bits / 2;
if shift == 0 || shift > half_bits { return Err(format!("shrn: shift {} out of range", shift)); }
```

This in-repo reference establishes that the codebase's intended narrowing-shift
range is `1 ..= esize/2`; `encode_neon_sqshrun` deviates from it.

## Root cause (single defect)

`src/backend/arm/assembler/encoder/neon.rs:141`:

```rust
if shift == 0 || shift > element_bits {        // <-- BUG: should be element_bits / 2
    return Err(format!("sqshrun: shift {} out of range for {}-bit elements", shift, element_bits));
}
```

## Minimal failing input (verified by execution)

```
operands = sqshrun v0.8b, v0.8h, #9   (is_rounding=false, is_high=false)
```

- **Expected:** `Err` — shift 9 exceeds the `.8h` legal maximum of 8.
- **Actual (executed):** `Ok(EncodeResult::Word(0x2F0F8400))`.
  `immhb = (16 - 9) & 0x7F = 7`, `immh = (7 >> 3) | 0b0001 = 0001`, `immb = 111`,
  so `immh:immb = 15`. For a `.8h` source, `immh:immb = 15` decodes as
  **shift = 16 - 15 = 1**. The emitted instruction is therefore a valid
  `sqshrun v0.8b, v0.8h, #1` — the requested shift `#9` was silently changed to `#1`.

## Impact

Shifts in `(esize/2, esize]` produce a valid-looking instruction with the wrong
shift amount. Because the encoder returns `Ok`, any consumer that trusts it
(ELF writers, JIT paths, disassemblers) emits a different operation than the
source text specifies — the most dangerous failure mode, since nothing faults:
the resulting word is a legal, runnable AArch64 instruction. GNU `as` and
`llvm-mc` reject `sqshrun v0.8b, v0.8h, #9` at assembly time.

## Evidence

- Direct execution (probe) of `encode_neon_sqshrun` on the minimal input above
  returned `Ok(0x2F0F8400)` with `immh:immb = 15` (decodes to shift #1), run
  during this campaign.
- `encode_neon_shrn` at `neon.rs:1444` demonstrates the correct `element_bits / 2`
  bound used elsewhere in the same file.

This report is confirmed by execution but the failing case is **not** left as a
permanent failing test in the suite (the campaign scope is `encode_neon_qshrn`;
a dedicated `encode_neon_sqshrun` PBT file should be added to cover it).

## Suggested fix

```rust
if shift == 0 || shift > element_bits / 2 {
    return Err(format!(
        "sqshrun: shift {} out of range for {}-bit source (narrow shift must be 1..={})",
        shift, element_bits, element_bits / 2,
    ));
}
```

The same `shift > element_bits` defect also appears in
`encode_neon_scalar_qshrun` (`neon.rs:1845`); `encode_neon_shrn` (`neon.rs:1444`)
already has the correct bound and should be used as the reference for all of
them.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/96
