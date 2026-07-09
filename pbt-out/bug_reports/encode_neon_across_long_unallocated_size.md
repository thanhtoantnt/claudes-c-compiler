# `encode_neon_across_long` — unallocated `size=11` arrangements silently encoded

## Location
`src/backend/arm/assembler/encoder/neon.rs`, function `encode_neon_across_long`
(lines ~1724–1742).

```rust
pub(crate) fn encode_neon_across_long(operands: &[Operand], u: u32, opcode: u32) -> Result<EncodeResult, String> {
    ...
    let (rn, arr_n) = get_neon_reg(operands, 1)?;
    let (q, size) = neon_arr_to_q_size(&arr_n)?;
    let word = (q << 30) | (u << 29) | (0b01110 << 24) | (size << 22)
        | (0b11000 << 17) | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Summary
The encoder drives the AArch64 `SADDLV`/`UADDLV` "add long across vector"
reduction instructions (the "Advanced SIMD across lanes" class,
`opcode = 00011`). It converts the source arrangement `<T>` to a `(Q, size)`
pair via `neon_arr_to_q_size` and emits a 32-bit word **without checking
whether the resulting `size` field is architecturally allocated for this
instruction class**.

For the SADDLV/UADDLV class the ARMv8-A ARM defines only
`size ∈ {0b00, 0b01, 0b10}`:

| size | valid source arrangements | reduction result |
|------|---------------------------|------------------|
| 0b00 | `.8b`, `.16b`             | 16-bit (Hd)      |
| 0b01 | `.4h`, `.8h`              | 32-bit (Sd)      |
| 0b10 | `.4s`                     | 64-bit (Dd)      |
| 0b11 | **UNALLOCATED**           | —                |

`size == 0b11` is **UNALLOCATED**: the shared "Advanced SIMD across lanes"
decode (ARM DDI 0487) falls through to `UNALLOCATED` for that case. The only
arrangements `neon_arr_to_q_size` maps to `size == 0b11` are `.1d` and `.2d`.
`encode_neon_across_long` accepts them and returns `Ok(…)` instead of `Err`.

## Reproduction
```
cargo test --lib across_long_rejects_unallocated_size -- --ignored
```
Witness output (proptest, shrunk):
```
Test failed: SADDLV/UADDLV (opcode=3) does not support .1d (size=0b11,
UNALLOCATED); expected Err but got Ok(Word(250624000))
minimal failing input: rd = 0, rn = 0, u = 0, arr = "1d"
```
i.e. `saddlv d0, v0.1d` → `Ok(0x0EF03800)` (250624000; an unallocated
encoding) instead of an error. `.2d` is likewise accepted.

## Impact
A silently unallocated encoding. Any object the encoder produces for
`.1d`/`.2d` SADDLV/UADDLV operands will be rejected by the processor
(`#UC`/undefined) at runtime, or — worse, for an assembler — silently emit a
word no conforming toolchain would ever produce. Compare the structurally
identical gap already documented for `encode_neon_addv`
(see `ADDV_ENCODER_BUG_REPORT.md`, "unallocated arrangements").

## Suggested fix
After `let (q, size) = neon_arr_to_q_size(&arr_n)?;`, reject `size == 0b11`:

```rust
let (q, size) = neon_arr_to_q_size(&arr_n)?;
if size == 0b11 {
    return Err(format!(
        "saddlv/uaddlv: unsupported source arrangement {} (size=0b11 is UNALLOCATED)",
        arr_n
    ));
}
```
(Optionally also reject `.2s`, which is not a documented SADDLV/UADDLV form —
only `.4s` is valid for the `size=0b10` case.)

## Note on what is *correct*
The bit arithmetic itself is byte-for-byte identical to the tested sibling
`encode_neon_across` (`neon_across_pbt.rs`) and matches the ARMv8-A ARM layout
for all architecturally valid inputs. The 7 passing properties in
`neon_across_long_pbt.rs` (differential reference encoder, fixed-bits
invariant, field round-trip, determinism, destination-form equivalence,
unknown-arrangement rejection, and a hand-derived golden table) confirm this.
The only gap is the missing `size` validation above.

The witness is the `#[ignore]`d proptest property
`across_long_rejects_unallocated_size`, which reports `Falsifiable` with the
shrunk counterexample `rd=0, rn=0, u=0, arr="1d" → Ok(0x0EF03800)`.

## Severity
Low–medium (silent emission of an unallocated encoding; no crash, no memory
unsafety). The `#[ignore]`d witness is kept out of the default suite so
`cargo test` stays green.
