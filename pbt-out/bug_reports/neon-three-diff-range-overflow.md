# `encode_neon_three_diff`: no range validation on `u_bit` / `opcode` (silent field overflow)

- **Function:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_three_diff`
- **Signature:** `fn encode_neon_three_diff(operands: &[Operand], u_bit: u32, opcode: u32, is_high: bool) -> Result<EncodeResult, String>`
- **Severity:** latent / robustness bug (encodes the wrong instruction instead of returning `Err`); not currently reachable with wrong values from the in-tree caller table, but the API contract invites it.
- **Status:** demonstrated by the `#[ignore]`d proptest
  `backend::arm::assembler::encoder::neon_three_diff_pbt::three_diff_rejects_out_of_range_opcode_and_u_bit`
  (run with `cargo test three_diff_rejects_out_of_range_opcode_and_u_bit -- --ignored`).

## Summary

The docstring documents the inputs as a **1-bit** U field and a **4-bit** opcode:

```text
Format: 0 Q U 01110 size 1 Rm opcode 00 Rn Rd
`u_bit`: 0 for signed, 1 for unsigned
`opcode`: 4-bit opcode (bits 15-12)
```

The implementation ORs both values straight into the word with **no bounds check**:

```rust
let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22)
    | (1 << 21) | (rm << 16) | (opcode << 12) | (rn << 5) | rd;
Ok(EncodeResult::Word(word))
```

As a result, any `opcode >= 0x10` or `u_bit >= 2` is silently absorbed and
overflows into an adjacent field, producing a *valid-looking* 32-bit word that
encodes a **different instruction** rather than `Err`.

The register fields are *not* affected: `parse_reg_num` already clamps register
numbers to 0–31 (5 bits), so `rd`/`rn`/`rm` can never overflow.

## Concrete reproduction

Inputs `u_bit=0, opcode=0x10, is_high=false`, operands `v0.8h, v1.8b, v2.8b`
(the base `SADDL` opcode is `0b0000`):

| opcode | result            | decoded as                                  |
|--------|-------------------|---------------------------------------------|
| `0x0`  | `Ok(0x0E220020)`  | `saddl v0.8h, v1.8b, v2.8b` *(correct)*     |
| `0x10` | `Ok(0x0E230020)`  | `saddl v0.8h, v1.8b, v3.8b` *(Rm 2 → 3!)*   |
| `0x1F` | `Ok(0x0E3F0020)`  | `saddl v0.8h, v1.8b, v31.8b`                |

`opcode << 12` for `0x10` is `0x0001_0000`, i.e. **bit 16**, which is the low
bit of the Rm field (bits 20-16). So `opcode=0x10` quietly rewrites `Rm`.

Likewise `u_bit=2`: `2 << 29 = 0x4000_0000`, i.e. **bit 30**, which is the Q
field. So `u_bit=2` silently flips a base-form instruction into a `2` form
(`is_high` equivalent), and `u_bit=3` sets both Q and U.

## Impact

- No `Err` is returned for inputs that cannot represent a real instruction, so
  the `Result<_, String>` contract is violated. Callers that compute `opcode`
  or `u_bit` dynamically (e.g. from a parsed mnemonic table or a future
  disassembler) would get a silently wrong word with no signal.
- The 4-bit opcode field `bits 15-12` admits only `0x0..=0xF`. Anything larger
  is unallocated by the architecture for this class.

## Suggested fix

Validate the widths up-front, mirroring the architectural field sizes:

```rust
if u_bit > 1 {
    return Err(format!("encode_neon_three_diff: u_bit must be 0 or 1, got {}", u_bit));
}
if opcode > 0xF {
    return Err(format!("encode_neon_three_diff: opcode is a 4-bit field, got 0x{:x}", opcode));
}
```

Optionally also reject the `is_high == true` + narrow-source-arrangement
(`8b`/`4h`/`2s`) combination, which today produces a size/Q-inconsistent word
(Q forced to 1 while `size` still reflects the narrow element) rather than an
error.

## What the property suite verifies

In `neon_three_diff_pbt.rs` (5 passing properties + 1 `#[ignore]`d finding):

- **LLVM-anchored golden table** — 8 vectors produced by `clang --target=aarch64`
  (SADDL/UMULL/USUBL2/SMULL/SABAL/UADDL/SMLAL/UMLSL2) round-trip exactly.
- **Differential** vs. an independent field-by-field reference encoder over all
  valid `rd/rn/rm ∈ 0..=31`, `u_bit ∈ {0,1}`, `opcode ∈ 0..0xF`,
  `is_high ∈ {true,false}`, arrangement ∈ `{8b,16b,4h,8h,2s,4s}`.
- **Fixed-bits constant** (`bit31=0`, `[28:24]=01110`, `bit21=1`, `[11:10]=00`).
- **Field semantics** (Rd/Rn/Rm/opcode/U round-trip; `is_high`⇒Q=1; size from
  element width).
- **Negative contract** for validated inputs (unsupported arrangement / too-few
  operands → `Err`).
- **`#[ignore]`d finding** asserting `opcode >= 0x10` and `u_bit >= 2` return
  `Err` — this is the property this bug report documents.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/236
