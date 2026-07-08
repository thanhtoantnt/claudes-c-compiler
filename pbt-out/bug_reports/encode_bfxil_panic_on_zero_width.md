# Bug: `encode_bfxil` panics on `BFXIL Rd, Rn, #0, #0`

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs`, function `encode_bfxil` (line 127)
**Found by:** property `prop_encode_bfxil_tests::prop_rejects_out_of_range_operands`
**Severity:** high (panic aborts the compiler in debug builds)

## Minimal input

```
BFXIL X0, X1, #0, #0
```
i.e. `lsb == 0 && width == 0` (any register width).

## Expected

The assembler rejects the operand with a clean `Err`. `width == 0` is not a
valid bitfield width — ARM ARM (BFXIL) requires `1 <= width <= regsize - lsb`,
so `width == 0` is out of range and must be reported, never crash.

## Actual

The compiler panics:

```
panicked at src/backend/arm/assembler/encoder/bitfield.rs:127:16:
attempt to subtract with overflow
```

## Root cause

```rust
let imms = lsb + width - 1;   // line 127
```

Rust evaluates this left-to-right as `(lsb + width) - 1`. With `lsb == 0` and
`width == 0`, that is `0u32 - 1`, an arithmetic underflow that aborts in debug
builds. There is no preceding check that `width >= 1`.

## Impact

Any source containing `BFXIL Rd, Rn, #0, #0` (or any path that lowers to it,
e.g. a macro / codegen emitting a zero-width extract) crashes the compiler
instead of producing a diagnostic.

## Fix

Validate before computing:

```rust
let regsize = if is_64 { 64u32 } else { 32 };
if width == 0 || lsb >= regsize || lsb + width > regsize {
    return Err(format!("BFXIL: lsb/width out of range (lsb={}, width={}, regsize={})",
                       lsb, width, regsize));
}
let imms = lsb + width - 1; // now width >= 1, so no underflow
```

or use `width.checked_sub(1)` / `lsb.checked_add(width).and_then(|s| s.checked_sub(1))`.

## Test evidence

```
Test failed: lsb=0,width=0 (imms underflow PANIC): lsb=0 width=0 should be Err but PANICKED
minimal failing input: is_64 = false, over_lsb = 64, over_width = 65, neg = -3
```

The property wraps the call in `std::panic::catch_unwind`, so the panic is
reported as a contract failure rather than aborting the proptest run.
