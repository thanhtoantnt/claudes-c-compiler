# Bug: `encode_fp_arith` does not validate `opcode` range — silent overflow into Rm field

**File:** `src/backend/arm/assembler/encoder/fp_scalar.rs`
**Function:** `encode_fp_arith(operands: &[Operand], opcode: u32) -> Result<EncodeResult, String>`
**Severity:** Low (latent — all current callers in `arm/encoder/mod.rs:379-404` pass valid 4-bit opcodes; no guard exists.)

## Summary
The opcode is OR'd in unconditionally at `opcode << 12` with **no mask and no
range check**:

```rust
let word = (0b00011110 << 24) | (ftype << 22) | (1 << 21)
         | (rm << 16) | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
```

The opcode field is 4 bits (`[15:12]`). Any `opcode >= 16` overflows into the
adjacent 5-bit Rm field (`[20:16]`), producing a silently-corrupted word whose
effective Rm and opcode both differ from the caller's intent. A 32-bit `opcode`
can even clobber `ftype`, bit 21, and the `00011110` fixed bits.

## Minimal failing input
```
encode_fp_arith(&[Reg("d0"), Reg("d0"), Reg("d0")], 16)   // opcode = 0b10000
```
- **Expected:** `Err` (opcode exceeds the 4-bit field).
- **Actual:** `Ok(Word(...))` where `opcode<<12` contributes a `1` at bit 16,
  adding 1 to the encoded Rm value — a different, unrequested instruction.

Discovered by property `prop_fp_arith_rejects_wrong_banks_precision_and_oversized_opcode`
(oversized-opcode sub-assertion, `bad_opcode >= 16`).

## Impact
A future caller passing a wide opcode (or a mistaken constant) gets a
silently-malformed instruction rather than an error — there is no guard to catch
the mistake at the encoder boundary. Contrast the encoder-guidance rule: silent
truncation/overflow of immediate fields is not an acceptable oracle.

## Fix
Reject (preferred) or mask defensively at the top of the function:

```rust
if opcode > 0b1111 {
    return Err(format!("fp_arith opcode {} exceeds 4-bit field", opcode));
}
// …or, if wrapping is ever deemed intentional, mask explicitly:
// let opcode = opcode & 0xF;
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/148
