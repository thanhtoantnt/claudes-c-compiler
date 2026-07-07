# Bug Report: `encode_smull` silently accepts SP/WSP operands as zero registers

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_smull`

## Summary

`encode_smull` uses the shared generic register parser, which maps `sp`/`wsp` to register number 31. SMULL is an alias of SMADDL and does not permit SP operands; register 31 in these fields is the zero register. The encoder accepts invalid SP/WSP operands and silently encodes them as XZR/WZR.

## Reproduction

Characterization from the PBT campaign:

```text
smull x0, wsp, w2
```

Actual behavior: returns `Ok(Word(_))` with `Rn = 31`, equivalent to `smull x0, wzr, w2`.

## Impact

Invalid source is accepted and assembled as a different instruction than written, changing multiplication semantics without a diagnostic.

## Suggested fix

For multiply/long-multiply instruction classes, reject `sp`/`wsp` before converting to a register number:

```rust
if reg.eq_ignore_ascii_case("sp") || reg.eq_ignore_ascii_case("wsp") {
    return Err("smull operands cannot use SP/WSP".to_string());
}
```
