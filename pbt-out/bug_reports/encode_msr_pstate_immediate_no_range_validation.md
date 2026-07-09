# Bug Report: `encode_msr` silently accepts out-of-range PState immediate (`daifset`/`daifclr`/`spsel`)

**Target:** `src/backend/arm/assembler/encoder/system.rs` → `encode_msr`
**Severity:** Medium

## Summary

`msr daifset, #imm`, `msr daifclr, #imm` and `msr spsel, #imm` (the immediate
"PState" form of MSR) perform **no range validation** on the immediate. The
encoder masks the value with `& 0xF` and emits an instruction word, so any
immediate outside the legal range `[0, 15]` is silently accepted and folded
onto a *different* in-range value (e.g. `#16` aliases `#0`, `#255` aliases
`#15`, `#-1` aliases `#15`).

Canonical assemblers reject these inputs. `clang --target=aarch64-linux-gnu`
(LLVM-MC) reports:

```
<stdin>:1:14: error: immediate must be an integer in range [0, 15].
```

This is a negative-contract violation: invalid input that must be rejected is
instead silently mis-encoded, producing a correct-looking but semantically
wrong instruction.

## Root Cause

The immediate path in `encode_msr` narrows the immediate with `& 0xF` before
placement and never checks that the original value fits in 4 bits:

```rust
"daifset" => {
    let imm = get_imm(operands, 1)? as u32 & 0xF;          // <-- masks, never validates
    let word = 0xd5034000 | (imm << 8) | (0b110 << 5) | 0x1F;
    return Ok(EncodeResult::Word(word));
}
"daifclr" => {
    let imm = get_imm(operands, 1)? as u32 & 0xF;          // <-- same
    let word = 0xd5034000 | (imm << 8) | (0b111 << 5) | 0x1F;
    return Ok(EncodeResult::Word(word));
}
"spsel" => {
    if let Ok(imm) = get_imm(operands, 1) {
        let imm = imm as u32 & 0xF;                        // <-- same
        let word = 0xd5004000 | (imm << 8) | (0b101 << 5) | 0x1F;
        return Ok(EncodeResult::Word(word));
    }
}
```

`get_imm` returns the raw `i64` with no bounds; the subsequent `& 0xF` discards
the high bits instead of rejecting them.

## Reproduction

```
msr daifset, #16   -> encoder: Ok(0xD503_40DF)   (this is `msr daifset, #0`)
                   -> clang:    error: immediate must be an integer in range [0, 15].

msr daifset, #255  -> encoder: Ok(0xD503_4FDF)   (this is `msr daifset, #15`)
msr spsel,  #-1    -> encoder: Ok(0xD500_4FBF)   (this is `msr spsel, #15`)
```

In-range values `#0..#15` are encoded correctly and match clang exactly; only
the out-of-range half of the domain is wrong.

## Impact

An assembly source containing a typo or computed macro value outside `[0,15]`
(e.g. `msr daifset, #(1<<4)`) assembles without diagnostic and silently writes
the wrong DAIF mask / SPSel selector. Because the resulting word is a *valid*
MSR immediate instruction, the error is invisible at the object level and only
surfaces as wrong interrupt-mask / stack-pointer-selection behaviour at run
time — a silent mis-assembly.

## Suggested Fix

Validate the immediate before masking, for all three PState fields:

```rust
"daifset" => {
    let imm = get_imm(operands, 1)?;
    if !(0..=15).contains(&imm) {
        return Err(format!("daifset immediate must be in [0,15], got #{}", imm));
    }
    let word = 0xd5034000 | ((imm as u32) << 8) | (0b110 << 5) | 0x1F;
    return Ok(EncodeResult::Word(word));
}
```

(repeat for `daifclr` and the `spsel` immediate branch).

## Regression Property

Failing property: `b1_msr_rejects_out_of_range_pstate_immediate`
(file `src/backend/arm/assembler/encoder/system_msr_mrs_pbt.rs`, marked
`#[ignore]` so the default `cargo test` stays green).

Run with: `cargo test --lib system_msr_mrs -- --ignored`

```rust
#[test]
#[ignore = "documented bug: out-of-range PState immediate is masked (& 0xF) instead of rejected; clang says 'immediate must be an integer in range [0, 15].'"]
fn b1_msr_rejects_out_of_range_pstate_immediate() {
    let bad_imms = [16i64, 17, 31, 100, 255, 256, -1];
    for imm in bad_imms {
        for name in ["daifset", "daifclr", "spsel"] {
            let r = encode_msr(&[Operand::Symbol(name.into()), Operand::Imm(imm)]);
            assert!(r.is_err(),
                "msr {}, #{} should be rejected (out of [0,15]), got {:?}", name, imm, r);
        }
    }
}
```

Current behaviour: the assertion fails — e.g. `msr daifset, #16` returns
`Ok(Word(3573760223))` (`0xD503_40DF`, i.e. `daifset #0`).


**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/283
