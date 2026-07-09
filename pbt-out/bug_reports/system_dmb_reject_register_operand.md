# Bug: `dmb` accepts a register operand, silently encoding `DMB SY`

**Law:** A `DMB` operand that is not a recognised barrier option name is
invalid and must be rejected with `Err`. A register operand is never a valid
barrier option.

**Impact:** `dmb x0` is silently encoded as `DMB SY` instead of being rejected.
`llvm-mc` rejects it with *"invalid barrier option name"*. An invalid operand is
accepted as valid output with no diagnostic, so a malformed program assembles
successfully and silently runs a full barrier rather than failing fast.

**Function:** `encode_dmb` — `src/backend/arm/assembler/encoder/system.rs`

**Detected by:** Negative / error contract (invalid operand rejection),
differential oracle `llvm-mc-14 --triple=aarch64-linux-gnu`.

**Minimal input:** `encode_dmb(&[Operand::Reg("x0".into())])`.

**Expected:** `Err(...)` (matches `llvm-mc` *"invalid barrier option name"*).

**Actual:** `Ok(EncodeResult::Word(0xD503_3FBF))` — the catch-all
`_ => 0b1111` arm of `encode_dmb` defaults any non-`Barrier`/`Symbol` operand to
the `sy` option (CRm = 0xF).

**Severity:** medium (invalid operand silently accepted; misplaced barrier).

**Regression test:** witness `b_s4a_dmb_dsb_reject_register_operand` in
`src/backend/arm/assembler/encoder/system_barriers_hints_pbt.rs` (marked
`#[ignore]`). Run with:
```
cargo test --lib system_barriers_hints::b_s4a_dmb_dsb_reject_register_operand -- --ignored
```
