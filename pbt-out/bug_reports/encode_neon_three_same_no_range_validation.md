# Bug Report: `encode_neon_three_same` silently accepts out-of-range `u_bit` / `opcode`

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_three_same(operands, u_bit, opcode)`
**Severity:** Latent correctness bug (silent encoding corruption)
**Found by:** property-based test `prop_encode_neon_three_same_tests::prop_out_of_range_u_bit_and_opcode_must_error` (Property 5 — negative contract)

## Summary

`encode_neon_three_same` bit-packs its parameters into an AArch64 "Advanced SIMD three register, same" word with layout:

```
0  Q  U  01110  size  1  Rm[20:16]  opcode[15:11]  1  Rn[9:5]  Rd[4:0]
   30 29      24   22 21            11          10
```

`u_bit` occupies **a single bit** (bit 29) and `opcode` occupies a **5-bit field** (bits 15–11), per the ARMv8-A ARM. The function validates the operand count, the register numbers (via `parse_reg_num`, ≤31), and the arrangement (via `neon_arr_to_q_size`), but it performs **no range validation on `u_bit` or `opcode`**. Any `u32` is accepted and shifted into place, so an out-of-range value silently overwrites adjacent fields and emits a **semantically different, architecturally invalid** instruction word as `Ok(EncodeResult::Word(...))`.

## Reproduction

Minimal failing input from the property suite (`extra = 1`):

```rust
let ops = vec![
    Operand::RegArrangement { reg: "v0".into(), arrangement: "4s".into() },
    Operand::RegArrangement { reg: "v1".into(), arrangement: "4s".into() },
    Operand::RegArrangement { reg: "v2".into(), arrangement: "4s".into() },
];
encode_neon_three_same(&ops, /*u_bit=*/ 2, /*opcode=*/ 0)
// Actual:   Ok(EncodeResult::Word(0x4EA20420))
// Expected: Err(...)   — u_bit=2 does not fit the 1-bit U field
```

Bit-level corruption: for `.4s`, the legitimate `Q=1` already sets bit 30 (`0x40000000`). Passing `u_bit=2` computes `2 << 29 = 0x40000000`, which sets bit 30 **again** (the Q field) instead of bit 29 (the U field). The resulting word `0x4EA20420` has U=0 and Q=1 — a *different* instruction class from the intended U=2/Q=1 intent, and `U` cannot even represent 2.

The `opcode` parameter is equally unguarded: `opcode = 0x20` computes `0x20 << 11 = 0x40000`, setting bit 18 inside the **Rm** field, again yielding a non-canonical word with no error.

## Why this is a bug, not "caller's responsibility"

* The encoder is a defensive translation stage whose contract is "reject what cannot be encoded." A sibling in the same file, `encode_neon_logical`, **does** validate its analogous `opc` parameter (`return Err("unsupported NEON logical opc")`). `encode_neon_three_same` omits the equivalent check, making its behaviour inconsistent and the corruption silent.
* Unlike the register/arrangement operands (which *are* validated), `u_bit` and `opcode` are raw `u32`s with no upstream constraint, so the function is the last line of defence.
* Output of this encoder feeds real assembler/emulator consumers; a silently-corrupted word is the worst failure mode (no error, wrong semantics).

## Suggested fix

Add field-width checks before packing:

```rust
if u_bit > 1 {
    return Err(format!("encode_neon_three_same: u_bit {} exceeds 1-bit U field", u_bit));
}
if opcode > 0x1F {
    return Err(format!("encode_neon_three_same: opcode 0x{:x} exceeds 5-bit field", opcode));
}
```

## Property-suite outcome

Five properties were added in `mod prop_encode_neon_three_same_tests` (oracle: independent field-placement / bit-layout per ARMv8-A ARM):

| # | Property | Result |
|---|----------|--------|
| 1 | `prop_fixed_template` — fixed bits [31]=0,[28:24]=01110,[21]=1,[10]=1 | ✅ PASS |
| 2 | `prop_arrangement_qsize` — arrangement → Q[30]/size[23:22] (independent table) | ✅ PASS |
| 3 | `prop_register_field_independence` — Rd/Rn/Rm occupy disjoint 5-bit fields | ✅ PASS |
| 4 | `prop_error_contract` — <3 operands or unsupported arrangement → Err | ✅ PASS |
| 5 | `prop_out_of_range_u_bit_and_opcode_must_error` — u_bit>1 / opcode>0x1F → Err | ❌ **FAIL (this bug)** |

The passing properties confirm the *happy-path* bit-packing is correct; the failure is scoped to the missing input-range validation.

> Note: proptest persisted this failure in `proptest-regressions/.../neon.txt`. It will replay (and re-fail) until the fix lands; the regression seed can then be removed.
