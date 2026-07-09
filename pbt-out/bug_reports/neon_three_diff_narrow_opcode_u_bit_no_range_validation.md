# NEON `encode_neon_three_diff_narrow` — out-of-range `opcode`/`u_bit` silently corrupts encoding

## Function
`src/backend/arm/assembler/encoder/neon.rs::encode_neon_three_diff_narrow`

Encodes the AArch64 NEON "Advanced SIMD three different" **narrowing** family:
ADDHN/RADDHN/SUBHN/RSUBHN and their `2` upper-half variants.

Layout:
```
31  30  29  28-24  23-22  21  20-16  15-12  11-10  9-5  4-0
 0   Q   U  01110   size   1   Rm    opcode   00    Rn   Rd
```

## Symptom
The docstring describes `u_bit` as a **1-bit** U field (bit 29) and `opcode` as
a **4-bit** field (bits 15-12). The implementation ORs both straight into the
word with **no range validation**:

```rust
let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22) | (1 << 21)
    | (rm << 16) | (opcode << 12) | (rn << 5) | rd;
```

Consequences:
- `opcode >= 0x10` overflows **upward** into the Rm field (bits 20-16).
- `u_bit >= 2` overflows into the Q field (bit 30).

Both produce a silently-different, architecturally unrelated instruction word
rather than `Err`. No spec cites wrapping/truncation as intentional here — the
fields have well-defined widths.

## Reproduction (property test, `#[ignore]`d)
`src/backend/arm/assembler/encoder/neon_three_diff_narrow_pbt.rs`
→ `narrow_rejects_out_of_range_opcode_and_u_bit`

Minimal failing input recorded by proptest:
```
opcode_oob = 16   =>  Ok(0x0E230020)   // opcode 0x10 leaked into Rm
u_bit_oob  = 2
```

Run with:
```
cargo test --lib neon_three_diff_narrow -- --ignored
```

## Impact
Low in practice: the function is only called from the assembler dispatch table
(`encoder/mod.rs`) with fixed in-range constants (`u_bit ∈ {0,1}`,
`opcode ∈ {0b0100, 0b0110}`). The bug is latent — it would only fire if a
future instruction were added with `opcode >= 0x10` or `u_bit >= 2`, or if the
function were reused with dynamic arguments. The encoder would then emit
garbage silently.

## Note
This is the **same** latent defect as the sibling widening encoder
`encode_neon_three_diff` (documented in `NEON_THREE_DIFF_RANGE_BUG_REPORT.md`).
The fix should add range checks (return `Err` when `u_bit > 1` or
`opcode > 0xF`) in both encoders — ideally factored into a shared helper.

## Verification of the happy path
The PBT suite also anchors eight encodings against the independent LLVM AArch64
assembler (`clang --target=aarch64 -c`) and a field-by-field reference encoder.
All 7 active properties pass:

- `narrow_matches_golden_table` — 8 LLVM-derived golden words ✓
- `narrow_matches_reference_encoder` — differential ✓
- `narrow_fixed_bits_are_constant` — bit 31=0, [28:24]=0b01110, bit 21=1, [11:10]=00 ✓
- `narrow_field_semantics` — Rd/Rn/Rm/opcode/U/Q/size round-trip ✓
- `narrow_q_depends_only_on_is_high` — Q driven solely by `is_high` (narrowing family) ✓
- `narrow_rejects_invalid_inputs` — unsupported source arrangements & too-few operands → Err ✓
- `rejects_too_few_operands` — 0/1/2 operands → Err ✓
