# BUG: `encode_ldaxr_stlxr` silently drops non-zero `[Xn, #imm]` offsets

**File:** `src/backend/arm/assembler/encoder/load_store.rs`
**Function:** `encode_ldaxr_stlxr(operands, is_load, forced_size)`
**Severity:** High — wrong machine code emitted with no diagnostic

## Summary

Per the ARMv8-A Architecture Reference Manual (§C4.1.49 LDAXR, §C4.1.116
STLXR), the exclusive load/store register instructions address memory using
**only `[Xn]`** — there is no immediate-offset, pre-index, or post-index
encoding. `llvm-mc-18` confirms this and rejects any offset:

```
$ printf 'ldaxr x0, [x1, #8]\n' | llvm-mc-18 --assemble --triple=aarch64
<stdin>:1:18: error: index must be absent or #0
```

The encoder, however, pattern-matches the memory operand with `..` and
**silently discards the offset**, emitting the `[Xn]` (offset 0) word:

```rust
let rn = match operands.get(1) {
    Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("invalid base")?,  // offset dropped
    _ => return Err("ldaxr needs memory operand".to_string()),
};
```

The store branch (`operands.get(2)`) has the identical `..` bug. The result
is a *syntactically valid* but *semantically wrong* instruction word returned
as `Ok`, so the assembler produces a program that accesses `[Xn]` instead of
the programmer's intended address, with no diagnostic.

## Reproduction

```
minimal failing input: off = 1, is_load = false
   → STLXR w0, x1, [x2, #1]
   → encoder returned Ok(Word(0xC800FC41))
   → 0xC800FC41 is exactly `stlxr w0, x1, [x2]`  (#1 became #0)
```

## Expected behaviour

Return `Err` for any non-zero offset on the `Mem` operand, e.g.:

```rust
Some(Operand::Mem { base, offset }) => {
    if *offset != 0 {
        return Err(format!(
            "ldaxr/stlxr: only [Xn] addressing is supported, offset {} is invalid",
            offset
        ));
    }
    parse_reg_num(base).ok_or("invalid base")?
}
```

(Consistent with the range validation `encode_prfm` already performs in this
same file, and with the negative-contract properties already documented on
the sibling functions `encode_ldr_str`, `encode_ldur_stur`, `encode_ldtr_sized`,
`encode_ldp_stp`, and `encode_ldnp_stnp`.)

## Verification of the finding

Run:

```
cargo test --lib prop_encode_ldaxr_stlxr_tests
```

Result: **4 passed, 1 failed**.

| Property | Result | Checks |
|---|---|---|
| `prop_layout_vs_golden_and_l_bit` | ✅ pass | full-word layout vs llvm-mc goldens; load/store differ only in L bit [22] |
| `prop_size_field_auto_and_forced` | ✅ pass | size auto-detect (x→11,w→10) + forced override; byte/halfword goldens |
| `prop_fixed_bits_o0_and_reserved` | ✅ pass | o0[15]=1 (acquire/release), Rs/Rt2 reserved = 11111, Rs carries Ws |
| `prop_malformed_operands_rejected` | ✅ pass | wrong arity / non-`Mem` operands → `Err` |
| `prop_nonzero_offset_rejected` | ❌ **FAIL** | non-zero `[Xn,#imm]` must be `Err`, is silently dropped |

The 4 passing properties confirm the *field placement and opcode bits* of the
encoder are correct (every field lands exactly where the ARM ARM mandates,
cross-checked against `llvm-mc-18`). The only defect is the missing offset
validation.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/115
