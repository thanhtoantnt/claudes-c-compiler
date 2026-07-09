# Bug: `encode_neon_ld_st_multi` does not validate post-index immediate

| Field | Value |
|---|---|
| Function | `encode_neon_ld_st_multi` |
| File | `src/backend/arm/assembler/encoder/neon.rs` (function `encode_neon_ld_st_multi`) |
| Severity | Correctness / silent mis-assembly |
| Discovered by | property `rejects_wrong_immediate_post_index` in `neon_ld_st_multi_pbt.rs` |
| Status | **Confirmed** (witness fails when un-`#[ignore]`d; 8 sibling properties + golden table pass) |

## Summary

For the post-index form `[Xn], #imm`, the ARM ARM requires the immediate to
equal the total number of bytes transferred (element size × lane count ×
structure count). The encoder binds the offset to `_imm`/`_` and
unconditionally encodes `Rm = 11111`:

```rust
if let Some(_imm) = post_index {
    // Post-index with immediate: use Rm=11111 (0x1F)
    let word = ((q << 30) | (0b001100 << 24) | (1 << 23) | (l_bit << 22))
        | (0b11111 << 16) | (opcode << 12) | (size << 10) | (rn << 5) | rt;
    return Ok(EncodeResult::Word(word));
}
```

(The same pattern repeats in the `Operand::Imm(_)` arm below.) `llvm-mc-18`
rejects a mismatched immediate with `error: invalid operand for instruction`.

## Minimal input

`ld1 {v0.8b}, [x1], #1`

## Expected vs. actual

- Expected: `Err` (`#1` is not a valid transfer size; smallest is 8 bytes).
- Actual: `Ok(EncodeResult::Word(0x0CA08020))` — identical to the valid
  `ld1 {v0.8b}, [x1], #8`.

## Impact

Codegen that constructs an incorrect post-index immediate gets a plausible
32-bit word instead of `Result::Err`, so the defect silently propagates to
emitted machine code.

## Suggested fix

Compute the expected transfer size from `(arrangement, num_regs, num_structs)`
and return `Err` when the post-index immediate differs.

## Verification

`cargo test --lib neon_ld_st_multi` → 8 passed, 2 ignored (default suite green).
`cargo test --lib neon_ld_st_multi -- --ignored` → this witness fails with the
shrunk input above. Absolute correctness is pinned by an 18-case golden table
captured from `llvm-mc-18`.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/258
