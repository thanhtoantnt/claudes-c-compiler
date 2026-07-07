## Summary

A property-based test campaign against `encode_movz` (the ARMv8 MOVZ wide-immediate
encoder) in `src/backend/arm/assembler/encoder/data_processing.rs` (line 201). Six
properties were added inline to the module's existing `#[cfg(test)] mod tests`, reusing
its `proptest` harness and `xreg`/`expect_word`/`sf_of`/`rd_of` helpers. Three positive
properties confirm correct field placement for valid inputs. Three negative-contract
properties fail, exposing that the encoder performs **no** input validation: it silently
masks the 16-bit immediate, silently normalizes the `lsl` shift via integer division, and
ignores register width when gating the `hw` range. The defects are systemic across
`encode_movz`/`encode_movk`/`encode_movn`. Full analysis and a suggested fix are in
`BUG_REPORT.md`.

## Modules Tested

| Module (path) | Function | Properties | Pass | Fail |
|---|---|---|---|---|
| `src/backend/arm/assembler/encoder/data_processing.rs` | `encode_movz` (line 201) | 6 | 3 | 3 |

## Bugs Found

Each bug cites the failing property, the minimal failing input, and corroborating evidence
that the codebase's intended contract is to **reject** out-of-range immediates:
`encode_add_sub` returns `Err` for an un-encodable immediate at
`data_processing.rs:336`, pinned by the existing test `unencodable_immediate_returns_err`
at `data_processing.rs:1180-1189`. `encode_movz` violates this established contract.

**Bug 1 — Immediate magnitude not validated (silent truncation).**
`encode_movz` masks the immediate with `& 0xFFFF` (`data_processing.rs:230`) without any
`0..=0xFFFF` check; `get_imm` at `:213` returns the raw value. So `movz x0, #0x10000`
assembles to the same word as `movz x0, #0x0`. Confirmed by
`movz_rejects_out_of_range_immediate` (minimal input `rd=0, imm=65536`).
*Doc evidence (intended contract):* `data_processing.rs:336` + `:1180-1189`.

**Bug 2 — Shift amount not validated (silent normalization).**
The `hw` selector is computed as `*amount / 16` (`data_processing.rs:219`), so any
non-multiple-of-16 is silently floored (`lsl #1` ⇒ `hw=0`; `lsl #17` ⇒ `hw=1`). A non-`lsl`
shift kind is silently coerced to `hw=0` (the `else { 0 }` at `:221`). ARMv8 MOVZ permits
only `lsl #{0,16,32,48}`. Confirmed by `movz_rejects_non_multiple_of_16_shift` (minimal
input `rd=0, hw=0, rem=1`).
*Doc evidence (intended contract):* the sibling `encode_shift` validates `imm` against
register width (`data_processing.rs` shift encoder) and the codebase rejects bad shifts
elsewhere; MOVZ has no such gate.

**Bug 3 — 32-bit (`W`) register accepts `lsl #32` / `lsl #48` (UNDEFINED encoding).**
`movz w0, #1, lsl #32` encodes `hw = 32 / 16 = 2` (`data_processing.rs:219`). The `hw`
computation never consults `is_64`; for a 32-bit MOVZ only `hw ∈ {0,1}` is valid, so
`hw = 2`/`hw = 3` produce UNDEFINED encodings. Confirmed by both
`movz_w_reg_rejects_32_or_48_shift` (minimal input `rd=0, bad_amount=32`) and the one-shot
`movz_w_reg_lsl32_is_rejected`.
*Doc evidence (intended contract):* `data_processing.rs:336` / `:1180` establish that
un-encodable operands return `Err`; the W-register UNDEFINED case follows the same
principle.

**Systemic note.** The identical masking (`& 0xFFFF`) and shift-normalization
(`amount / 16`) patterns are duplicated in `encode_movk` (`data_processing.rs:234-262`)
and `encode_movn` (`data_processing.rs:266-285`). All three MOV-wide encoders share Bugs 1-3.

## Design Caveats

- **`Doc evidence:` — coverage gap, `:abs_g*:` modifier branch not exercised.** The
  relocation-modifier branch of `encode_movz` (`data_processing.rs:206-211`, calling
  `resolve_abs_g_modifier`) was *not* covered by this campaign. Its behavior is documented
  intentional: *Doc evidence: `data_processing.rs:179-182`* (docstring of
  `resolve_abs_g_modifier`: *"If the expression contains a symbol reference, returns None
  (needs relocation)."*). No test asserts the symbol-relocation / `WordWithReloc` path for
  MOVZ; it is a known coverage gap, not a behavior claim needing reclassification.
- **`Doc evidence:` — reference (field-placement) oracle, not `llvm-mc` differential.** The
  campaign validates fields against the ARMv8 spec rather than against a reference
  assembler. This matches the codebase's own test style: *Doc evidence:
  `data_processing.rs:1180-1189`* (`unencodable_immediate_returns_err`) and the surrounding
  `proptest!` block (`:1090-1230`) use field-extraction reference oracles, not
  differential ones. This is a test-design choice (no SUT behavior excused); noted so the
  positive properties are not mistaken for end-to-end `as`/`llvm-mc` parity.
- **`Doc evidence:` — inline test placement / shared helpers.** Properties were appended to
  the module's existing `mod tests` rather than a new file, reusing the existing
  `proptest` import and `xreg`/`expect_word`/`sf_of`/`rd_of` helpers. *Doc
  evidence: `data_processing.rs:1072`* (`use proptest::prelude::*;`) and *Doc evidence:
  `data_processing.rs:1086-1092`* (the `xreg` and `expect_word` helper definitions
  inside `mod tests`). MOVZ-specific extractors (`opc_of`, `opcode6_of`,
  `hw_of`, `imm16_of`) were added without colliding with the ADD/SUB extractors. This is a
  structural choice, not a SUT behavior claim.

No documented *intentional* code behavior in `encode_movz` itself was found to caveat: the
function has no docstring (`data_processing.rs:201`), and every un-justified behavior
(masking, division, missing width gate) was reclassified into ## Bugs Found above.

## Test Files Created

| File | Description |
|---|---|
| `src/backend/arm/assembler/encoder/data_processing.rs` (edited, not new) | Added MOVZ field extractors (`opc_of`/`opcode6_of`/`hw_of`/`imm16_of`) and 6 `proptest!`/`#[test]` cases to the existing `mod tests`. 3 reference properties pass; 3 negative-contract properties fail (exposing Bugs 1-3). No new source/test file created. |
| `BUG_REPORT.md` (created) | Full write-up of Bugs 1-3 with source excerpts, line citations, minimal failing inputs, systemic note (movk/movn), impact, and a suggested validation fix. |
| `COVERAGE.md` (created) | Property-by-property pass/fail table for `encode_movz` (6 properties). |

## Output Directories

None created. The `proptest-regressions/` directory was auto-generated during the failing
runs and deliberately removed afterward (the failures are known/expected bugs, not
flukes to persist). All test output is inline in `data_processing.rs`; all reporting
artifacts (`REPORT.md`, `BUG_REPORT.md`, `COVERAGE.md`) live at the repository root.
