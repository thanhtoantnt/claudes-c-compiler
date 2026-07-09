# Bug: `encode_ldr_str` masks out-of-range `imm9` offsets instead of rejecting them

## Target
`src/backend/arm/assembler/encoder/load_store.rs` — function `encode_ldr_str`

## Root cause (single defect)
`encode_ldr_str` never validates the magnitude of the signed 9-bit `imm9`
immediate. In every arm that encodes a 9-bit offset it computes

```rust
let imm9 = (*offset as i32) & 0x1FF;   // <-- no range check
```

and emits the masked value. The `imm9` field is a **signed 9-bit** field
(ARM ARM, Load/Store Register — unscaled / pre-index / post-index) whose
representable range is `[-256, 255]`. Any offset outside that range is
silently wrapped into the field instead of being rejected. This is the
same masking defect already documented for the sibling encoders
`encode_ldtr_sized` and `encode_ldrs`.

The unchecked mask occurs at the three call sites inside `encode_ldr_str`
that use `imm9`: the LDUR/STUR fallback inside the `Operand::Mem` arm, the
`Operand::MemPreIndex` arm, and the `Operand::MemPostIndex` arm. All three
share the identical one-line defect.

## Minimal failing input
```
encode_ldr_str(
    &[Operand::Reg("w0"), Operand::MemPreIndex { base: "x1", offset: 256 }],
    false /*store*/, 0b10 /*size*/, false, false,
)
```
- **Expected:** `Err` — `256` is outside the signed 9-bit range `[-256, 255]`,
  so a pre-indexed store with `#256` is unrepresentable.
- **Actual:** `Ok(Word(3088059424))` — the offset is masked to `imm9 = 0`,
  emitting `str w0, [x1]` (offset silently dropped to zero).

Found by proptest as minimal input:
`size = 2, is_load = false, form = pre-index, off = 256`.

## Spec / why wrapping is wrong
There is no wrapping semantics for `imm9`: the ARM ARM defines the field as
a signed offset in `[-256, 255]`. A conforming assembler must reject
out-of-range offsets. The current behaviour produces a correct-looking word
that accesses the **wrong address** — a silent miscompilation, not a
detected error.

## Impact
Any pre/post-indexed or oversized-offset load/store beyond ±255 bytes
(e.g. `str x0, [sp, #512]!`, `ldr x0, [x1, #32768]`) assembles without
diagnostic and accesses the wrong memory location. This corrupts code that
relies on stack frames or struct offsets larger than 255 bytes, which is
common in real C programs.

## Suggested fix
Validate the offset before masking, in each of the three arms:

```rust
let off_i = *offset as i64;
if !(-256..=255).contains(&off_i) {
    return Err(format!("ldr/str offset {} out of signed 9-bit range [-256,255]", off_i));
}
let imm9 = (*offset as i32) & 0x1FF;
```

## Evidence — property-based tests
File: `src/backend/arm/assembler/encoder/load_store_ldr_str_pbt.rs` (9 properties).

Passing (spec-conformance / reference, green by default):
- `prop_unsigned_offset_fields_round_trip`, `prop_pre_post_index_fields_round_trip`,
  `prop_ldur_fallback_imm9_round_trips` — field placement + in-range round-trip.
- `prop_out_of_range_offset_silently_corrupted` — smoking gun: an out-of-range
  offset decodes to a different value than the input.
- `prop_str_literal_rejected`, `prop_ldr_literal_emits_ldr19_reloc` — literal-form contracts.

Bug witnesses (`#[ignore]`d so `cargo test` stays green; fail under `--ignored`):
- `prop_out_of_range_imm9_rejected` — any offset outside `[-256,255]` must be `Err`.
- `prop_common_offsets_rejected` — common offsets (`256, 512, 4096, -257, -512, …`) must be `Err`.
- `prop_oversized_aligned_offset_rejected` — aligned offset overflowing the 12-bit unsigned field
  (forces the LDUR fallback) must be `Err`.

## Verification
```
cargo test --lib load_store_ldr_str                      # 6 passed; 0 failed; 3 ignored  (green)
cargo test --lib load_store_ldr_str -- --ignored         # 3 failed  (witnesses fire)
```
The three ignored witnesses all fail on the minimal input above, confirming
the defect is real and reproducible.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/275
