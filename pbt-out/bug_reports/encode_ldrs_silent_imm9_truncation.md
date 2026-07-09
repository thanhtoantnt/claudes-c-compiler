# Bug Report: `encode_ldrs` silently truncates out-of-range signed imm9 offset

- **Function:** `encode_ldrs` (`src/backend/arm/assembler/encoder/load_store.rs`)
- **File:** `src/backend/arm/assembler/encoder/load_store.rs`
- **API:** `pub(crate) fn encode_ldrs(operands: &[Operand], size: u32) -> Result<EncodeResult, String>`
- **Instructions:** LDRSB (`size=00`) / LDRSH (`size=01`) — sign-extending byte/halfword loads
- **Severity:** High — silent mis-assembly of memory offsets; produces a valid-looking encoding that computes the *wrong* effective address.
- **Root-cause class:** Missing range validation on a signed immediate field; raw bitwise masking (`& 0x1FF`) used in place of a bounds check.

## One-line summary

`encode_ldrs` computes the signed 9-bit immediate offset field as
`(*offset as i32) & 0x1FF` with **no range check**. The ARM ARM defines this
field as a *signed* 9-bit value over **[-256, 255]**; offsets outside that range
are unrepresentable and must be rejected, but the encoder silently masks them
down to their low 9 bits and returns `Ok`.

## Minimal failing input

```rust
// ldrsb x0, [x1], #256   (post-indexed)
let ops = vec![
    Operand::Reg("x0".to_string()),
    Operand::MemPostIndex { base: "x1".to_string(), offset: 256 },
];
encode_ldrs(&ops, 0b00)   // size=00 → LDRSB
```

- **Expected:** `Err(...)` — `+256` is outside the signed 9-bit range
  `[-256, 255]` and is rejected by every mainstream AArch64 assembler.
- **Actual:**   `Ok(Word(0x389FC420))` — the low 9 bits of `256` (`0x100`)
  decode back to **-256**, so the emitted instruction computes a *different*
  address with no diagnostic.

Reference behavior (LLVM `llvm-mc`):

```
$ echo 'ldrsb x0, [x1], #256' | llvm-mc -triple=aarch64 -show-encoding
error: immediate must be an integer in range [-256, 255].
```

## Spec reference

LDRSB / LDRSH (immediate, post-/pre-indexed and unscaled LDUR forms), ARM ARM
Armv8-A §C6.2.118 / §C6.2.124: the `imm9` field (bits [20:12]) is *"the signed
immediate byte offset, in the range -256 to 255."* Values outside this range are
not a valid encoding.

## Where the guard is missing

The same unguarded masking line appears in three addressing forms inside
`encode_ldrs` (the pre/post/unscaled branches). It is a single defect — one
missing bounds check — repeated across the branches:

```rust
let imm9 = (*offset as i32) & 0x1FF;   // no range check
```

(For comparison: the unsigned-offset `Mem` branch is correctly guarded by
`imm12 < 4096`, and the register-offset branch has no immediate field — neither
is affected.)

## How the corruption manifests

| Intended offset | low 9 bits | Decoded imm9 | Outcome                |
|----------------:|-----------:|-------------:|------------------------|
|          +256   |     0x100  |         -256 | wrong offset, no error |
|          +512   |     0x000  |           0  | offset silently zeroed |
|          +1000  |     0x1E8  |          -24 | wrong sign/offset      |
|          -257   |     0x1FF  |          -1  | wrong offset, no error |
|          -300   |     0x114  |         +276 | wrong offset, no error |

## Properties (added in module `prop_encode_ldrs_tests`)

Spec-conformance / negative-contract oracle:

1. `prop_fields_round_trip` — **PASS.** Rt/Rn/size/opc field placement for the
   post-index form over the legal range.
2. `prop_opc_selects_width` — **PASS.** opc=10 for Xt, opc=11 for Wt.
3. `prop_out_of_range_imm9_rejected` — **FAIL (this bug).** Negative contract:
   any offset outside [-256, 255] must be `Err` for pre/post-index forms.
   Minimal failing input: `size = 0, form = 0, off = 256`.
4. `prop_out_of_range_offset_silently_corrupted` — **PASS.** Documents the
   mechanism: a negative offset past -256 cannot be recovered from the encoded
   9-bit field (smoking gun for the masking).
5. `prop_common_offsets_rejected` — **FAIL (this bug).** Concrete regression
   anchors `{256, 257, 512, 1000, 4096, -257, -258, -512}`, all must be
   rejected.

## Suggested fix

Validate the offset before masking, in each of the three affected branches:

```rust
let imm9_val = *offset as i32;
if !(-256..=255).contains(&imm9_val) {
    return Err(format!(
        "ldrsb/ldrsh immediate offset {} out of range [-256, 255]",
        imm9_val
    ));
}
let imm9 = (imm9_val & 0x1FF) as u32;
```

With the guard in place, properties 3 and 5 pass; properties 1 and 2 are
unchanged, and property 4 (the mechanism demonstration) becomes moot since the
encoder no longer accepts out-of-range inputs.

## Impact / blast radius

Any program emitting LDRSB/LDRSH with a post-indexed, pre-indexed, or unscaled
offset outside [-256, 255] will assemble without error but compute the wrong
effective address at runtime. This is a silent correctness defect in the
generated machine code — the worst failure mode for an assembler.
