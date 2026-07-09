# `encode_neon_bic` accepts UNALLOCATED non-byte arrangements

## Summary
`encode_neon_bic` (`src/backend/arm/assembler/encoder/neon.rs:718`) silently
encodes `BIC Vd.T, Vn.T, Vm.T` for arrangements that are architecturally
**UNALLOCATED** for `BIC` (`.4h`, `.8h`, `.2s`, `.4s`, `.1d`, `.2d`). It derives
the `Q` bit solely from `arr == "16b"` and treats every other arrangement string
as `Q=0` (i.e. as `.8b`), emitting a valid-looking byte-`BIC` word instead of
returning `Err`.

## Architecture reference
`BIC` (vector) is the `(U=0, size=01)` slot of the AArch64 "Advanced SIMD three
same" logical group (ARMv8-A ARM, ARM DDI 0487, BIC row). The `size` field is
**fixed** and the group is **byte-only**: only `.8b` (Q=0) and `.16b` (Q=1) are
allocated; all other arrangements are UNALLOCATED.

## Reproduction
Differential check against LLVM's AArch64 assembler (`clang --target=aarch64`)
shows every non-byte arrangement is rejected:

```
.4h: REJECTED (invalid operand for instruction)
.8h: REJECTED
.2s: REJECTED
.4s: REJECTED
.1d: REJECTED
.2d: REJECTED
```

The Rust encoder returns `Ok` for all of them:

```rust
// encode_neon_bic, neon.rs:718
let q: u32 = if arr_d == "16b" { 1 } else { 0 };
let word = (q << 30) | (0b001110 << 24) | (0b01 << 22) | (1 << 21)
    | (rm << 16) | (0b000111 << 10) | (rn << 5) | rd;
```

e.g. `BIC V0.4h, V1.4h, V2.4h` yields `Ok(0x0E621C20)` — a perfectly valid
`bic v0.8b, v1.8b, v2.8b` — silently corrupting the instruction.

## Witness
```bash
cargo test -p ccc --lib encoder::neon_bic_pbt -- --ignored bic_rejects_non_byte
```
Expected to **fail** (the encoder does not meet the negative contract).

## Impact
A malformed instruction (e.g. from a buggy parser/frontend) passes the encoder
without diagnostics and emits an *unrelated but valid* instruction. This is a
silent correctness risk for the assembler. The same pattern affects the sibling
logical encoder `encode_neon_bsl`.

## Suggested fix
```rust
let q: u32 = match arr_d.as_str() {
    "8b" => 0,
    "16b" => 1,
    other => return Err(format!("bic requires .8b or .16b, got .{other}")),
};
```
