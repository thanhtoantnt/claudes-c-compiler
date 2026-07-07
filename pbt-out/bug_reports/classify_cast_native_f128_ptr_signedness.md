# Bug: native F128 `Ptr -> F128` cast is classified as signed

## Target
- File: `src/backend/cast.rs`
- Function: `classify_cast_with_f128` / `classify_f128_cast_native`

## Failing property
`native_f128_uses_softfloat_families` in `src/backend/cast.rs`.

Command:

```sh
cargo test classify_cast_properties --lib
```

Minimal counterexample from proptest:

```text
other_ty = Ptr
left:  SignedToF128 { from_ty: Ptr }
right: UnsignedToF128 { from_ty: Ptr }
```

## Expected behavior
Pointer casts should be treated as pointer-width unsigned integer casts. This is consistent with:

- `classify_cast_with_f128` documentation: Ptr normalization is part of shared cast classification.
- The non-F128 integer/float path, where `F128 -> Ptr` already uses `F128ToUnsigned { to_ty: Ptr }`.
- LP64/ILP32 pointer semantics: pointer values are addresses and should not use signed integer-to-float conversion libcalls.

## Actual behavior
For native F128 targets, `classify_cast_with_f128(Ptr, F128, true)` enters the F128-special path before pointer normalization:

```rust
if from_ty == IrType::F128 || to_ty == IrType::F128 {
    if f128_is_native {
        return classify_f128_cast_native(from_ty, to_ty);
    }
}
```

Inside `classify_f128_cast_native`, `Ptr` is not considered unsigned for `Something -> F128`:

```rust
if from_ty.is_unsigned() {
    return CastKind::UnsignedToF128 { from_ty };
}
return CastKind::SignedToF128 { from_ty };
```

Because `IrType::Ptr.is_unsigned()` is false, this returns `SignedToF128 { from_ty: Ptr }`.

## Impact
ARM/RISC-V native F128 lowering may select a signed soft-float conversion for pointer-to-long-double casts, which can miscompile high-bit pointer values by interpreting them as negative signed integers.

## Suggested fix
Handle `Ptr` as unsigned in the native F128 `to_ty == F128` branch, or normalize `Ptr` before native F128 classification. A minimal local fix is:

```rust
if from_ty.is_unsigned() || from_ty == IrType::Ptr {
    return CastKind::UnsignedToF128 { from_ty };
}
```
