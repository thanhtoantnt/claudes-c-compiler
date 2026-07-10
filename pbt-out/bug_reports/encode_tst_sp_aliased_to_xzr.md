# Bug: `encode_tst` aliases SP/WSP to XZR/WZR (no SP form exists)

## Summary
TST (ANDS) has no SP form — register 31 is always ZR. The encoder accepts `tst sp, x0` and `tst x0, sp` via `parse_reg_num` which maps sp/wsp to register 31 indistinguishably from xzr/wzr. `clang` rejects TST with SP operands.

## Witness
```
cargo test --lib div_tst_cbz_regclass_pbt -- --ignored prop_tst_rejects_sp_operands
```

## Root cause
No SP rejection after `parse_reg_num` resolves `sp`/`wsp` → 31.

## Severity
MEDIUM — silent semantic change; SP operand becomes zero-register, producing wrong results at runtime.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/347
