# Bug — `encode_neon_tbx` silently wraps out-of-range table size

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_tbx`
**Severity:** high (silent mis-assembly)
**Witness test:** `tbx_rejects_table_larger_than_four_regs`
(`#[ignore]`d in `src/backend/arm/assembler/encoder/neon_tbx_pbt.rs`)

## Defect

```rust
let len = (num_regs - 1) & 0x3;
```

`TBX` accepts **exactly 1–4 table registers** (`len ∈ 0..=3`). The `& 0x3`
mask silently wraps a 5- to 8-register list modulo 4, so e.g. a 5-register
table encodes **identically** to a 1-register table (`len`: 4 → 0). `llvm-mc`
rejects these outright:

```
$ echo 'tbx v0.16b, {v1.16b-v5.16b}, v6.16b' | llvm-mc-18 -assemble -triple=aarch64
<stdin>:1:21: error: invalid number of vectors
```

## Minimal failing input

`rd=0, rn=0, rm=0, num_regs=5`

## Expected vs. actual

- **Expected:** `Err` (TBX allows only 1–4 table registers).
- **Actual:** `Ok(Word(0x4E021020))` — byte-for-byte identical to the
  1-register form `tbx v0.16b, {v0.16b}, v0.16b`. The extra 4 table
  registers vanish.

## Impact

A source line that names a 5+ register table silently assembles to a
*different* instruction (a smaller table lookup). Code that relied on the
larger table will compute wrong bytes with no diagnostic — a correctness bug
that is invisible at the tooling level.

## Fix

Reject illegal table sizes before computing `len`:

```rust
if !(1..=4).contains(&num_regs) {
    return Err(format!("tbx: table must have 1-4 registers, got {}", num_regs));
}
let len = num_regs - 1;   // drop the `& 0x3` mask
```

## Reproduce

```bash
cargo test --lib neon_tbx_pbt -- --ignored tbx_rejects_table_larger_than_four_regs
```
