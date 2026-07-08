# Bug — `encode_prfm` register-offset form sets the wrong opcode bit

**File:** `src/backend/arm/assembler/encoder/load_store.rs`
**Function:** `pub(crate) fn encode_prfm` — `Operand::MemRegOffset` arm
**Differential oracle:** `llvm-mc-18 --triple=aarch64-linux-gnu --show-encoding`
**Severity:** High — every `prfm <op>, [Xn, Xm{, extend}]}` emits a wrong/illegal word.

## Root cause

```rust
let word = (0b11 << 30) | (0b111 << 27) | (0b10 << 23) | (1 << 21)
    | (rm << 16) | (option << 13) | (s_bit << 12) | (0b10 << 10) | (rn << 5) | prfop;
//                                       ^^^^^^^^^
//  0b10 << 23 sets bit 24 (0x0100_0000).  Per ARMv8-A ARM §C4.1.90
//  "PRFM (register)", opc=10 occupies bits [23:22] -> bit 23 (0x0080_0000).
```

ARM ARM §C4.1.90 PRFM (register): `11 111 0 00 10 1 Rm option S 10 Rn Rt`.
The `opc[23:22]=10` bit belongs at **bit 23**, not bit 24.

## Evidence (`llvm-mc-18`)

```
prfm pldl1keep, [x0, x1]          // encoding: [0x00,0x68,0xa1,0xf8]  == 0xF8A16800
prfm pldl1keep, [x0, x1, lsl #3]  // encoding: [0x00,0x78,0xa1,0xf8]  == 0xF8A17800
```

The crate produces `0xF9216800` for `prfm pldl1keep,[x0,x1]` — differs from the
golden by exactly `0x0180_0000` (bit 24 spurious, bit 23 missing).

## Minimal failing input (Property 4)

`pidx=0 (pldl1keep), rn=0, rm=0, opt_idx=0 (lsl), shift_amt=0`
`left (crate) = 0xF9216800` vs `right (llvm-mc ref) = 0xF8A06800`.

```
cargo test --lib prop_encode_prfm_tests::prop_prfm_register_offset_matches_reference
```

## Fix

```rust
// change  (0b10 << 23)   ->   (0b10 << 22)
let word = (0b11 << 30) | (0b111 << 27) | (0b10 << 22) | (1 << 21)
    | (rm << 16) | (option << 13) | (s_bit << 12) | (0b10 << 10) | (rn << 5) | prfop;
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/119
