# Bug — `encode_neon_float_cmp_zero` accepts `size_hi` that yields an UNALLOCATED `size` field

## Function
`src/backend/arm/assembler/encoder/neon.rs::encode_neon_float_cmp_zero`
```rust
pub(crate) fn encode_neon_float_cmp_zero(
    operands: &[Operand], u_bit: u32, size_hi: u32, opcode: u32,
) -> Result<EncodeResult, String>
```

Packs the floating-point compare-to-zero family
(`FCMEQ/FCMGE/FCMGT/FCMLE/FCMLT Vd.T, Vn.T, #0.0`) in the **Advanced SIMD
two-register miscellaneous** group:
```text
 31 30 29 28-24 23-22 21-17  16-12   11-10 9-5 4-0
  0  Q  U  01110  size  10000  opcode  10   Rn  Rd
```
where `size = (size_hi << 1) | sz`, and `sz` comes from the arrangement
(`2S`→sz0, `4S`→sz0, `2D`→sz1).

## The bug (single finding)
`size_hi` is OR'd into bit[23] of the `size` field with **no validation**:
```rust
let size = (size_hi << 1) | sz;
let word = ... | (size << 22) | ...;
```
Per the ARMv8-A ARM ("Advanced SIMD two-register miscellaneous",
*FCMEQ/FCMGE/FCMGT/FCMLE/FCMLT (vector) #0.0*), the `size` field bits[23:22]
is allocated **only** as:
- `00` — single-precision (`.2S`, `.4S`)
- `01` — double-precision (`.2D`)

**bit[23] must be 0.** Any non-zero `size_hi` produces `size = 0b10` or `0b11`,
both UNALLOCATED for this instruction group. The function returns `Ok(Word(..))`
for such an input instead of `Err`, so a caller can silently emit an
unallocated encoding.

## Minimal input
```
encode_neon_float_cmp_zero([v0.2s, v0.2s], u_bit=0, size_hi=1, opcode=0)
```

## Expected vs actual
- **Expected:** `Err(...)` — `size_hi = 1` is not an allocated value for this
  group, so it must be rejected.
- **Actual:** `Ok(Word(0x0EA00820))`. Decoding that word, bits[23:22] = `10`
  → `size = 0b10`, which is **UNALLOCATED** for this instruction group.

## Impact
**Silent mis-assembly (High).** The encoder produces an `Ok` word that the
hardware treats as UNALLOCATED — a UNDEF exception or, worse, a decode as an
unrelated instruction — with no error surfaced. This is reachable in practice:
`src/backend/arm/assembler/encoder/mod.rs` passes `size_hi = 1` for the
`fcmgt` (line 549) and `fcmlt` (line 554) mnemonics, so assembling e.g.
`fcmgt v0.4s, v1.4s, #0.0` or `fcmlt v0.4s, v1.4s, #0.0` silently yields an
unallocated word.

## Suggested fix (function-level)
Validate `size_hi` — it must be `0`. Reject anything else so an unallocated
`size` field can never be emitted:
```rust
if size_hi != 0 {
    return Err(format!("float cmp zero: size_hi must be 0 (got {})", size_hi));
}
```
(Equivalently, drop the `size_hi` parameter entirely and use `size = sz`.)

## Test evidence
`src/backend/arm/assembler/encoder/neon_float_cmp_zero_pbt.rs`
- `prop_float_cmp_zero_decomposes_into_documented_fields` — PASS (fields pack
  correctly when `size_hi == 0`).
- `prop_float_cmp_zero_matches_reference` — PASS (independent reassembly agrees).
- `prop_float_cmp_zero_is_deterministic` — PASS.
- `prop_float_cmp_zero_rejects_unsupported_arrangements` — PASS (the function
  *does* validate arrangements: `.8b/.16b/.4h/.8h/.1d`/garbage → `Err`).
- `prop_rejects_unallocated_size_hi` (`#[ignore]`d) — **FAILS**, demonstrating
  this finding (minimal input: `rd=0, rn=0, arr="2s", u_bit=0, size_hi=1,
  opcode=0`).
- `size_hi_nonzero_currently_emits_unallocated_size` — PASS (characterization
  test pinning the current buggy behavior; will turn red once fixed).
