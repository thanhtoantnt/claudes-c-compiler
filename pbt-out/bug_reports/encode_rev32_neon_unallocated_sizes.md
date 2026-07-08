# `encode_rev32` — NEON form silently accepts UNALLOCATED arrangements (`2s/4s/1d/2d`)

## Target
`pub(crate) fn encode_rev32(operands: &[Operand]) -> Result<EncodeResult, String>`
in `src/backend/arm/assembler/encoder/bitfield.rs`.

## Defect (severity: medium — emits architecturally invalid instruction)
The **NEON (vector)** form of `REV32` delegates arrangement parsing to
`neon_arr_to_q_size` (in `encoder/neon.rs`), which accepts **all eight**
arrangements:

```
"8b"->(Q0,size00) "16b"->(Q1,size00)
"4h"->(Q0,size01) "8h" ->(Q1,size01)
"2s"->(Q0,size10) "4s" ->(Q1,size10)
"1d"->(Q0,size11) "2d" ->(Q1,size11)
```

and then `encode_rev32` unconditionally OR-s the returned `size` into bits
[23:22]:

```rust
let (q, size) = neon_arr_to_q_size(&arr_d)?;
let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (size << 22)
    | (0b100000 << 16) | (0b000010 << 10) | (rn << 5) | rd;
```

But per the **ARM ARM (REV32 vector, "Advanced SIMD")**, the `size` field is
**constrained to `{00 (bytes), 01 (halfwords)}` only**. `size == 10` and
`size == 11` are **UNALLOCATED encodings**. `encode_rev32` performs **no
range check on `size`**, so:

- `rev32 v0.2s, v0.2s` → `0x2EA00800`  (size=10, UNALLOCATED)
- `rev32 v0.4s, v0.4s` → `0x6EA00800`  (size=10, UNALLOCATED)
- `rev32 v0.1d, v0.1d` → `0x2EA00800`+size → UNALLOCATED
- `rev32 v0.2d, v0.2d` → UNALLOCATED

These return `Ok(Word(...))` with a well-formed-looking but architecturally
invalid word that no reference assembler would ever produce. An assembler must
**reject** these inputs with `Err`.

This is a **distinct** defect from the already-known scalar-form bug
(`prop_encode_rev32_tests::prop_scalar_matches_arm_reference` — the scalar
path hardcodes sf=1/opc=000010 and discards register width). The bug here is
in the vector path's element-size validation.

## Reproduction
```bash
cargo test --lib prop_encode_rev32_neon_neg_tests::prop_neon_rejects_unallocated_sizes
```
Minimal failing input: `rd = 0, rn = 0, bad = "2s"`.
Output: `Ok(Word(782239744))` == `0x2EA00800` (size field = `10`).

## Fix
After obtaining `size` from `neon_arr_to_q_size`, reject the unallocated
element sizes before encoding, e.g.:

```rust
let (q, size) = neon_arr_to_q_size(&arr_d)?;
if size > 0b01 {
    return Err(format!(
        "REV32 vector requires byte (.8b/.16b) or halfword (.4h/.8h) \
         arrangement; size={} is UNALLOCATED", size));
}
```

(Optionally the check belongs in `neon_arr_to_q_size`'s callers per
instruction, since other instructions have different per-instruction size
constraints — e.g. `CNT` is byte-only — so a single global cap would be wrong.)

## Properties (module `prop_encode_rev32_neon_neg_tests`, 3 tests)
| # | Property | Oracle | Result |
|---|----------|--------|--------|
| P1 | `prop_neon_q_only_differs_by_width` — `8b^16b` / `4h^8h` XOR == bit30 only, size field equal | structural / field differential | **PASS** |
| P2 | `prop_neon_rejects_unallocated_sizes` — `2s/4s/1d/2d` (size 10/11) must return `Err` | negative contract (spec: UNALLOCATED) | **FAIL** (the finding) |
| P3 | `prop_neon_rejects_unknown_arrangement` — arbitrary unknown arrangement strings → `Err` | negative contract (error path) | **PASS** |

P2 is expected to fail until the range check is added; P1 and P3 pin the
correct behaviour and guard the surrounding code.

## Scope
Root cause is `neon_arr_to_q_size` being a generic helper that does not encode
per-instruction size constraints. Any NEON instruction whose valid sizes are a
strict subset of `{00,01,10,11}` and that calls this helper without a follow-up
check has the same class of defect. `encode_rev32` is the confirmed instance.
