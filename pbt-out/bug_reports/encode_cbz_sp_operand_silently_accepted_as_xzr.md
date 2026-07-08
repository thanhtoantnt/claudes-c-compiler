# BUG — `encode_cbz` / `encode_cbnz` silently accepts SP/WSP, aliased to XZR/WZR

**Severity:** High (silent mis-assembly of branch target)
**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_cbz(operands, is_nz)`
**Class:** silent SP → XZR aliasing (identical defect family to `encode_mul`,
`encode_div`, `encode_logical`, … — all driven by the shared `get_reg`→`parse_reg_num` helper)

## Summary

`cbz sp, <target>` and `cbz wsp, <target>` (and the CBNZ forms) are silently
accepted and encoded **bit-identically** to `cbz xzr, <target>` / `cbz wzr,
<target>`. The stack pointer is treated as the zero register, so the branch is
mis-assembled into a branch-on-zero of `XZR`/`WZR` (CBZ on XZR is *always*
taken), with **no error and no warning**.

## Spec basis (ARM ARM C5.6.21 / C5.6.22)

CBZ/CBNZ: `sf 011010 op imm19 Rt`. The `<Rt>` operand is a *general-purpose*
register. The Rt field value `31` denotes **XZR/WZR** — the zero register, **not**
the stack pointer. There is **no SP-using form** of CBZ/CBNZ; an Rt of SP is an
UNPREDICTABLE / unallocated encoding that a conforming assembler **must reject**.

## Root cause

`encode_cbz` resolves its register operand via the shared helper
`get_reg` → `parse_reg_num` (`encoder/mod.rs`):

```rust
pub fn parse_reg_num(name: &str) -> Option<u32> {
    let name = name.to_lowercase();
    match name.as_str() {
        "sp" | "wsp" => Some(31),   // <-- correct for SP-aware ops, wrong here
        "xzr" | "wzr" => Some(31),
        "lr" => Some(30),
        ...
    }
}
```

`parse_reg_num("sp")` returns `Some(31)` and `is_64bit_reg("sp")` returns `true`
(so `sf=1`); `parse_reg_num("wsp")` returns `Some(31)` with `sf=0`. XZR/WZR map
to exactly the same `(31, sf)`, so the resulting words collide. `encode_cbz`
performs no width/SP validation before emitting.

## Reproduction

```
cbz  sp, target   -> Ok(WordWithReloc { word: 0xB400001F, ... })   # == cbz  xzr
cbz  wsp, target  -> Ok(WordWithReloc { word: 0x3400001F, ... })   # == cbz  wzr
cbnz sp, target   -> Ok(WordWithReloc { word: 0xB500001F, ... })   # == cbnz xzr
cbnz wsp, target  -> Ok(WordWithReloc { word: 0x3500001F, ... })   # == cbnz wzr
```

Rt field == 31 in every case (XZR/WZR), sf tracks sp(64)/wsp(32).

## Property / characterization

Added to `compare_branch.rs` → `prop_encode_cbz_tests`:

- `prop_sp_silently_aliased_to_xzr` — differential characterization proving
  `cbz sp == cbz xzr` and `cbz wsp == cbz wzr` bit-for-bit, with `is_ok()`
  assertions pinning the current (buggy) acceptance. (Mirrors the
  `mul_sp_in_rm_is_silently_accepted_as_xzr` precedent.)

Run:
```
cargo test --lib backend::arm::assembler::encoder::compare_branch::prop_encode_cbz_tests::prop_sp_silently_aliased_to_xzr -- --nocapture
```

## Impact

- A program writing `cbz sp, loop` intends to branch when the stack pointer is
  zero (never, in practice) — instead it is assembled as `cbz xzr, loop`, an
  **unconditionally-taken branch**, silently changing control flow.
- Same mis-aliasing affects `cbnz` and the 32-bit `wsp` forms.
- Same shared-helper defect recurs across the whole XZR-only data-processing /
  branch family (see sibling reports).

## Regression property

Failing property: `prop_sp_silently_aliased_to_xzr`

```rust
prop_assert!(encode_cbz(&[Operand::Reg("sp".into()), Operand::Imm(0)]).is_err());
```

## Suggested fix

Reject SP/WSP (and, ideally, mixed/FP-SIMD registers) in `encode_cbz` before
encoding. The reusable fix proposed in the `encode_mul` report applies here: add
a `get_reg_no_sp` helper (parse only `xN`/`wN`/`xzr`/`wzr`, error on `sp`/`wsp`
and FP/SIMD names) and route `encode_cbz`/`encode_cbnz` (and the rest of the
XZR-only encoders) through it. Once fixed, flip the `is_ok()` assertions in
`prop_sp_silently_aliased_to_xzr` to `is_err()`.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/20
