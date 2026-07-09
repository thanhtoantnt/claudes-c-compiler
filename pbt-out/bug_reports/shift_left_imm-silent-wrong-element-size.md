# Bug — `encode_neon_shift_left_imm` silently re-encodes element size on over-large shift

**File:** `src/backend/arm/assembler/encoder/neon.rs:1768`
**Function:** `pub(crate) fn encode_neon_shift_left_imm(operands, u, opcode)`
**Reached via:** `SQSHL` / `UQSHL` (`encoder/mod.rs:600,605`).
**Discovering property:** `neon_shift_left_imm_pbt::prop_shift_left_imm_rejects_out_of_range_shift` (EXPECTED-FAIL).

## Minimal input

```
sqshl v0.8b, v0.8b, #8
operands = [ RegArrangement(v0,"8b"), RegArrangement(v0,"8b"), Imm(8) ]
call: encode_neon_shift_left_imm(&ops, 0, 0b01110)
```

## Expected vs. actual

- **Expected:** `Err(...)`. For `.8b` the valid shift range is `[0, 7]`
  (`0 <= shift <= esize-1`); `#8` is UNDEFINED.
- **Actual:** `Ok(Word(252736512))` (0x0F100400) — a word whose `immh = 0b0010`,
  which the ARM decoder reads as a **16-bit (halfword)** operation rather than
  the requested 8-bit operation.

Property minimal failing input:

```
Test failed: shift 8 on .8b (esize 8) is out of range [0,7] and must be rejected,
but got Ok(Word(252736512))
minimal failing input: (arr, _q, esize) = ("8b", 0, 8), too_big = 0, is_negative = false
```

## Root cause

```rust
let immhb = esize + shift;          // 8 + 8 = 16 = 0b1_0000
let immh  = (immhb >> 3) & 0xF;     // 0b0010
let immb  = immhb & 0x7;            // 0
```

There is no upper-bound check. When `shift >= esize`, `esize + shift` spills
into the next `immh` bucket; the decoder reconstructs element size as
`esize = 8 << HighestSetBit(immh)`, so `immh=0010` ⇒ **esize=16**, silently
producing an instruction with a *different element size* than requested.

## Impact

Silent miscompilation. Affects every arrangement at the boundary
(`#8` on `.8b`/`.16b`, `#16` on `.4h`/`.8h`, `#32` on `.2s`/`.4s`,
`#64` on `.2d`) and beyond, each re-encoding as the next-larger element size.
An invalid-but-unrejected input produces a wrong instruction with no diagnostic,
corrupting generated code.

## Fix

Add an upper-bound (and lower-bound) guard before computing `immhb`:

```rust
let shift_i = get_imm(operands, 2)?;
if shift_i < 0 || shift_i as u32 >= esize {
    return Err(format!(
        "shift left imm: shift {} out of range [0,{}] for {}-bit elements",
        shift_i, esize - 1, esize
    ));
}
let shift = shift_i as u32;
```
