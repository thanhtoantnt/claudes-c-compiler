# Bug — `encode_neon_float_elem` emits bit 23 as `0` (every float-by-element word mis-assembles)

**File:** `src/backend/arm/assembler/encoder/neon.rs` — function `encode_neon_float_elem`
**Tests:** `src/backend/arm/assembler/encoder/neon_float_elem_pbt.rs`
**Oracle:** `llvm-mc-18 --triple=aarch64 --assemble --show-encoding` + ARMv8 ARM
"Advanced SIMD scalar/by-element" layout.
**Severity:** high — every emitted `fmul`/`fmla`/`fmls`-by-element word is wrong by `0x0080_0000`.

## Reference layout

```
 31 30 29 28-24 23 22 21 20-16 15-12 11 10 9-5 4-0
  0  Q  U  0 11111 sz  L  M:Rm   opcode  H  0  Rn  Rd
```

The fixed group is **`0 11111`** spanning bits **[28:23]** — **bit 23 is a
hard-wired `1`**.

## Defect

The encoder ORs in the constant as:

```rust
let word = (q << 30) | (u_bit << 29) | (0b01111 << 24) | (sz << 22)
    | (l << 21) | (m_bit << 20) | (rm_enc << 16) | (opcode << 12)
    | (h << 11) | (rn << 5) | rd;
```

`(0b01111 << 24)` sets only bits **[27:24]**, leaving **bit 23 = 0**. The
correct constant is `(0b011111 << 23)` (bits [28:23] = `0 11111`, bit 23 = 1).

## Impact

Every float by-element instruction the assembler emits is wrong by
`0x0080_0000` and decodes to an unrelated instruction, so NEON float
multiply / multiply-accumulate-by-element code assembled by `ccc` will not run
correctly on hardware.

## llvm-mc-18 differential (golden anchors)

| Mnemonic                       | llvm-mc-18 word  | encoder produces | diff        |
|--------------------------------|------------------|------------------|-------------|
| `fmul v0.4s,v1.4s,v2.s[0]`     | `0x4F829020`     | `0x4F029020`     | `0x00800000` |
| `fmul v3.2s,v5.2s,v7.s[3]`     | `0x0FA798A3`     | `0x0F2798A3`     | `0x00800000` |
| `fmul v9.2d,v2.2d,v6.d[0]`     | `0x4FC69049`     | `0x4C69049`      | `0x00800000` |
| `fmla v0.4s,v0.4s,v0.s[1]`     | `0x4FA01000`     | `0x4F201000`     | `0x00800000` |
| `fmla v31.4s,v30.4s,v29.s[2]`  | `0x4F9D1BDF`     | `0x4F1D1BDF`     | `0x00800000` |
| `fmls v5.2d,v7.2d,v9.d[1]`     | `0x4FC958E5`     | `0x4C958E5`      | `0x00800000` |
| `fmls v10.4s,v11.4s,v12.s[3]`  | `0x4FAC596A`     | `0x4F2C596A`     | `0x00800000` |

## PBT witness

Surfaced by the FAILING proptest property `prop_matches_arm_reference` (#[ignore]d).

- **reproduce=** `cargo test --lib neon_float_elem_pbt::prop_matches_arm_reference -- --ignored`
- **shrunk counterexample (proptest "minimal failing input"):**
  `(q, sz) = (0, 0), (u, opcode) = (0, 9), rd = 0, rn = 0, rm = 0, idx_mod = 0`
  → mnemonic `fmul v0.2s, v0.2s, v0.s[0]`
- **Falsifiable:** encoder `left = 251695104` (`0x0F009000`), ARM ARM reference
  `right = 260083712` (`0x0F809000`); diff = `0x00800000` = bit 23.

Absolute llvm-mc-18 golden anchors (also #[ignore]d, all FAIL by `0x00800000`):
- `golden_fmul_by_element`, `golden_fmla_by_element`, `golden_fmls_by_element`.

- `prop_matches_arm_reference` — every valid operand set must equal the ARM
  ARM template (bit 23 = 1); fails with a `0x00800000` diff.
- `golden_fmul_by_element` / `golden_fmla_by_element` /
  `golden_fmls_by_element` — absolute llvm-mc-18 anchors, all fail by
  `0x00800000`.

Concrete observed failure:
```
golden_fmul_by_element: assertion `left == right` failed
  left:  1325568032   (0x4F029020 — encoder)
  right: 1333956640   (0x4F829020 — llvm-mc-18)
```

## Suggested fix

```rust
- (0b01111 << 24)
+ (0b011111 << 23)   // bits [28:23] = 0 11111  -> bit 23 = 1
```
