# Bug Report: `encode_prfm` register-offset form sets the wrong opcode bit

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_prfm`
**Severity:** High

## Summary

Every `prfm <op>, [Xn, Xm{, extend}]` emits a wrong/illegal word due to incorrect bit placement for the opcode field.

## Root Cause

```rust
let word = (0b11 << 30) | (0b111 << 27) | (0b10 << 23) | (1 << 21)
    | (rm << 16) | (option << 13) | (s_bit << 12) | (0b10 << 10) | (rn << 5) | prfop;
//                                       ^^^^^^^^^
//  0b10 << 23 sets bit 24 (0x0100_0000).  Per ARMv8-A ARM §C4.1.90
//  "PRFM (register)", opc=10 occupies bits [23:22] -> bit 23 (0x0080_0000).
```

ARM ARM §C4.1.90 PRFM (register): `11 111 0 00 10 1 Rm option S 10 Rn Rt`. The `opc[23:22]=10` bit belongs at **bit 23**, not bit 24.

## Reproduction

**Input:** `prfm pldl1keep, [x0, x1]`

**Expected:** `0xF8A16800` (matches llvm-mc)

**Actual:** `0xF9216800` — differs by `0x01800000` (bit 24 spurious, bit 23 missing)

**Minimal failing input:** pidx=0 (pldl1keep), rn=0, rm=0, opt_idx=0 (lsl), shift_amt=0

## Impact

Every `prfm` register-offset instruction emits a wrong/illegal word. All produced encodings are architecturally incorrect. This affects all use of the register-offset form of PRFM.

## Suggested Fix

```rust
// change (0b10 << 23)   ->   (0b10 << 22)
let word = (0b11 << 30) | (0b111 << 27) | (0b10 << 22) | (1 << 21)
    | (rm << 16) | (option << 13) | (s_bit << 12) | (0b10 << 10) | (rn << 5) | prfop;
```

## Regression Property

Failing property: `prop_register_offset_matches_reference`

```rust
prop_assert_eq!(encode_prfm(&[Operand::Reg("x0".into()), mem_reg_offset(xreg(1), xreg(2), "pldl1keep", 0, 0)], 0xF8A16800);
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/119