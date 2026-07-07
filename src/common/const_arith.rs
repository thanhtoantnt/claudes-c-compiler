//! Shared constant-expression arithmetic helpers.
//!
//! Used by both `sema::const_eval` and `ir::lowering::const_eval` for
//! compile-time constant expression evaluation with proper C semantics.
//!
//! The functions here handle the pure arithmetic: given IrConst operands and
//! width/signedness parameters, they compute the result. The callers (sema and
//! lowering) are responsible for determining width/signedness from their own
//! type systems (CType vs IrType) before calling these shared functions.

use crate::ir::reexports::IrConst;
use crate::frontend::parser::ast::BinOp;

// === Low-level arithmetic primitives ===

/// Wrap an i64 result to 32-bit width if `is_32bit` is true, otherwise return as-is.
/// This handles the C semantics of truncating arithmetic results to `int` width.
#[inline]
fn wrap_result(v: i64, is_32bit: bool) -> i64 {
    if is_32bit { v as i32 as i64 } else { v }
}

/// Perform an unsigned binary operation, handling 32-bit vs 64-bit width.
/// Converts operands to the appropriate unsigned type, applies the operation,
/// and sign-extends the result back to i64.
#[inline]
fn unsigned_op(l: i64, r: i64, is_32bit: bool, op: fn(u64, u64) -> u64) -> i64 {
    if is_32bit {
        op(l as u32 as u64, r as u32 as u64) as u32 as i64
    } else {
        op(l as u64, r as u64) as i64
    }
}

/// Convert a boolean to i64 (1 for true, 0 for false).
#[inline]
fn bool_to_i64(b: bool) -> i64 {
    if b { 1 } else { 0 }
}

// === Shared constant binary operation evaluators ===
//
// These replace the near-identical eval_const_binop / eval_const_binop_float
// implementations that were duplicated between sema::const_eval and
// ir::lowering::const_eval. Both callers now delegate here.

/// Evaluate a constant integer binary operation with proper C width/signedness.
///
/// The caller determines `is_32bit` and `is_unsigned` from their type system:
/// - sema uses CType: `(ctype_size <= 4, ctype.is_unsigned())`
/// - lowering uses IrType: `(ir_type.size() <= 4, ir_type.is_unsigned())`
///
/// Returns `Some(IrConst::I64(result))` or `None` for division by zero.
fn eval_const_binop_int(op: &BinOp, l: i64, r: i64, is_32bit: bool, is_unsigned: bool) -> Option<IrConst> {
    let result = match op {
        BinOp::Add => wrap_result(l.wrapping_add(r), is_32bit),
        BinOp::Sub => wrap_result(l.wrapping_sub(r), is_32bit),
        BinOp::Mul => wrap_result(l.wrapping_mul(r), is_32bit),
        BinOp::Div => {
            if r == 0 { return None; }
            if is_unsigned {
                unsigned_op(l, r, is_32bit, u64::wrapping_div)
            } else {
                wrap_result(l.wrapping_div(r), is_32bit)
            }
        }
        BinOp::Mod => {
            if r == 0 { return None; }
            if is_unsigned {
                unsigned_op(l, r, is_32bit, u64::wrapping_rem)
            } else {
                wrap_result(l.wrapping_rem(r), is_32bit)
            }
        }
        BinOp::BitAnd => l & r,
        BinOp::BitOr => l | r,
        BinOp::BitXor => l ^ r,
        BinOp::Shl => wrap_result(l.wrapping_shl(r as u32), is_32bit),
        BinOp::Shr => {
            if is_unsigned {
                unsigned_op(l, r, is_32bit, |a, b| a.wrapping_shr(b as u32))
            } else if is_32bit {
                (l as i32).wrapping_shr(r as u32) as i64
            } else {
                l.wrapping_shr(r as u32)
            }
        }
        BinOp::Eq => {
            if is_32bit { bool_to_i64(l as u32 == r as u32) }
            else if is_unsigned { bool_to_i64(l as u64 == r as u64) }
            else { bool_to_i64(l == r) }
        }
        BinOp::Ne => {
            if is_32bit { bool_to_i64(l as u32 != r as u32) }
            else if is_unsigned { bool_to_i64(l as u64 != r as u64) }
            else { bool_to_i64(l != r) }
        }
        BinOp::Lt => {
            if is_32bit { if is_unsigned { bool_to_i64((l as u32) < (r as u32)) } else { bool_to_i64((l as i32) < (r as i32)) } }
            else if is_unsigned { bool_to_i64((l as u64) < (r as u64)) }
            else { bool_to_i64(l < r) }
        }
        BinOp::Gt => {
            if is_32bit { if is_unsigned { bool_to_i64((l as u32) > (r as u32)) } else { bool_to_i64((l as i32) > (r as i32)) } }
            else if is_unsigned { bool_to_i64((l as u64) > (r as u64)) }
            else { bool_to_i64(l > r) }
        }
        BinOp::Le => {
            if is_32bit { if is_unsigned { bool_to_i64((l as u32) <= (r as u32)) } else { bool_to_i64((l as i32) <= (r as i32)) } }
            else if is_unsigned { bool_to_i64((l as u64) <= (r as u64)) }
            else { bool_to_i64(l <= r) }
        }
        BinOp::Ge => {
            if is_32bit { if is_unsigned { bool_to_i64((l as u32) >= (r as u32)) } else { bool_to_i64((l as i32) >= (r as i32)) } }
            else if is_unsigned { bool_to_i64((l as u64) >= (r as u64)) }
            else { bool_to_i64(l >= r) }
        }
        BinOp::LogicalAnd => bool_to_i64(l != 0 && r != 0),
        BinOp::LogicalOr => bool_to_i64(l != 0 || r != 0),
    };
    // Preserve the result width so that downstream operations (e.g., division
    // using the result of a shift) can correctly infer the C type. This is
    // critical for expressions like (1 << 31) / N where the shift result must
    // be recognized as 32-bit (INT_MIN = -2147483648) not 64-bit (positive).
    //
    // For unsigned 32-bit results, we must use I64 with zero-extension because
    // IrConst::I32 is signed and cannot correctly represent unsigned values >= 2^31.
    // E.g., (2147483647 * 2U + 1U) = 4294967295U would be stored as I32(-1),
    // which sign-extends to -1 (0xFFFFFFFFFFFFFFFF) when widened to 64-bit.
    // Using I64(4294967295) preserves the correct unsigned value.
    if is_32bit {
        if is_unsigned {
            Some(IrConst::I64(result as u32 as i64))
        } else {
            Some(IrConst::I32(result as i32))
        }
    } else {
        Some(IrConst::I64(result))
    }
}

/// Evaluate a constant floating-point binary operation.
///
/// For LongDouble operands, uses native f128 software arithmetic on the stored
/// IEEE binary128 bytes for full 112-bit mantissa precision. This matches the
/// runtime behavior of __addtf3/__divtf3/etc. on ARM64/RISC-V targets.
/// For F32/F64, uses native Rust arithmetic.
///
/// Comparison and logical operations always return `IrConst::I64`.
pub fn eval_const_binop_float(op: &BinOp, lhs: &IrConst, rhs: &IrConst) -> Option<IrConst> {
    use crate::common::long_double;

    let use_long_double = matches!(lhs, IrConst::LongDouble(..)) || matches!(rhs, IrConst::LongDouble(..));
    let use_f32 = matches!(lhs, IrConst::F32(_)) && matches!(rhs, IrConst::F32(_));

    // For long double arithmetic, use precision matching the target:
    // - ARM64/RISC-V: f128 software arithmetic (full 112-bit mantissa)
    // - x86/i686: x87 80-bit arithmetic (64-bit mantissa)
    if use_long_double {
        if crate::common::types::target_long_double_is_f128() {
            // ARM64/RISC-V: full f128 software arithmetic
            let la = lhs.long_double_bytes().copied().unwrap_or_else(|| {
                let v = lhs.to_f64().unwrap_or(0.0);
                long_double::f64_to_f128_bytes_lossless(v)
            });
            let ra = rhs.long_double_bytes().copied().unwrap_or_else(|| {
                let v = rhs.to_f64().unwrap_or(0.0);
                long_double::f64_to_f128_bytes_lossless(v)
            });

            match op {
                BinOp::Add => {
                    let result = long_double::f128_add(&la, &ra);
                    let approx = long_double::f128_bytes_to_f64(&result);
                    Some(IrConst::long_double_with_bytes(approx, result))
                }
                BinOp::Sub => {
                    let result = long_double::f128_sub(&la, &ra);
                    let approx = long_double::f128_bytes_to_f64(&result);
                    Some(IrConst::long_double_with_bytes(approx, result))
                }
                BinOp::Mul => {
                    let result = long_double::f128_mul(&la, &ra);
                    let approx = long_double::f128_bytes_to_f64(&result);
                    Some(IrConst::long_double_with_bytes(approx, result))
                }
                BinOp::Div => {
                    let result = long_double::f128_div(&la, &ra);
                    let approx = long_double::f128_bytes_to_f64(&result);
                    Some(IrConst::long_double_with_bytes(approx, result))
                }
                BinOp::Mod => {
                    let result = long_double::f128_rem(&la, &ra);
                    let approx = long_double::f128_bytes_to_f64(&result);
                    Some(IrConst::long_double_with_bytes(approx, result))
                }
                BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                    let cmp = long_double::f128_cmp(&la, &ra);
                    let result = match op {
                        BinOp::Eq => cmp == 0,
                        BinOp::Ne => cmp != 0,
                        BinOp::Lt => cmp == -1,
                        BinOp::Gt => cmp == 1,
                        BinOp::Le => cmp == -1 || cmp == 0,
                        BinOp::Ge => cmp == 1 || cmp == 0,
                        _ => unreachable!("inner match in const_arith has already-excluded op"),
                    };
                    Some(IrConst::I64(if result { 1 } else { 0 }))
                }
                BinOp::LogicalAnd | BinOp::LogicalOr => {
                    let zero = [0u8; 16];
                    let l_nonzero = long_double::f128_cmp(&la, &zero) != 0;
                    let r_nonzero = long_double::f128_cmp(&ra, &zero) != 0;
                    let result = match op {
                        BinOp::LogicalAnd => l_nonzero && r_nonzero,
                        BinOp::LogicalOr => l_nonzero || r_nonzero,
                        _ => unreachable!("inner match in const_arith has already-excluded op"),
                    };
                    Some(IrConst::I64(if result { 1 } else { 0 }))
                }
                _ => None,
            }
        } else {
            // x86/i686: x87 80-bit arithmetic
            let la = lhs.x87_bytes();
            let ra = rhs.x87_bytes();

            match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                    let result_x87 = match op {
                        BinOp::Add => long_double::x87_add(&la, &ra),
                        BinOp::Sub => long_double::x87_sub(&la, &ra),
                        BinOp::Mul => long_double::x87_mul(&la, &ra),
                        BinOp::Div => long_double::x87_div(&la, &ra),
                        BinOp::Mod => long_double::x87_rem(&la, &ra),
                        _ => unreachable!("inner match in const_arith has already-excluded op"),
                    };
                    let result_f128 = long_double::x87_bytes_to_f128_bytes(&result_x87);
                    let approx = long_double::x87_to_f64(&result_x87);
                    Some(IrConst::long_double_with_bytes(approx, result_f128))
                }
                BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                    let l = long_double::x87_to_f64(&la);
                    let r = long_double::x87_to_f64(&ra);
                    let result = match op {
                        BinOp::Eq => l == r,
                        BinOp::Ne => l != r,
                        BinOp::Lt => l < r,
                        BinOp::Gt => l > r,
                        BinOp::Le => l <= r,
                        BinOp::Ge => l >= r,
                        _ => unreachable!("inner match in const_arith has already-excluded op"),
                    };
                    Some(IrConst::I64(if result { 1 } else { 0 }))
                }
                BinOp::LogicalAnd | BinOp::LogicalOr => {
                    let l = long_double::x87_to_f64(&la);
                    let r = long_double::x87_to_f64(&ra);
                    let result = match op {
                        BinOp::LogicalAnd => l != 0.0 && r != 0.0,
                        BinOp::LogicalOr => l != 0.0 || r != 0.0,
                        _ => unreachable!("inner match in const_arith has already-excluded op"),
                    };
                    Some(IrConst::I64(if result { 1 } else { 0 }))
                }
                _ => None,
            }
        }
    } else {
        // F32/F64 path: use native Rust arithmetic
        let l = lhs.to_f64()?;
        let r = rhs.to_f64()?;

        let make_float = |v: f64| -> IrConst {
            if use_f32 {
                IrConst::F32(v as f32)
            } else {
                IrConst::F64(v)
            }
        };

        match op {
            BinOp::Add => Some(make_float(l + r)),
            BinOp::Sub => Some(make_float(l - r)),
            BinOp::Mul => Some(make_float(l * r)),
            BinOp::Div => Some(make_float(l / r)),
            BinOp::Eq => Some(IrConst::I64(if l == r { 1 } else { 0 })),
            BinOp::Ne => Some(IrConst::I64(if l != r { 1 } else { 0 })),
            BinOp::Lt => Some(IrConst::I64(if l < r { 1 } else { 0 })),
            BinOp::Gt => Some(IrConst::I64(if l > r { 1 } else { 0 })),
            BinOp::Le => Some(IrConst::I64(if l <= r { 1 } else { 0 })),
            BinOp::Ge => Some(IrConst::I64(if l >= r { 1 } else { 0 })),
            BinOp::LogicalAnd => Some(IrConst::I64(if l != 0.0 && r != 0.0 { 1 } else { 0 })),
            BinOp::LogicalOr => Some(IrConst::I64(if l != 0.0 || r != 0.0 { 1 } else { 0 })),
            _ => None,
        }
    }
}

/// Evaluate a constant 128-bit integer binary operation.
///
/// Called when at least one operand is I128. Uses native Rust i128/u128 arithmetic
/// to avoid the truncation that occurs when using the i64 path.
///
/// `is_unsigned` is the signedness of the result type (for div/rem/shift/cmp).
/// `lhs_unsigned` and `rhs_unsigned` are the signedness of each operand's original
/// C type, used to correctly zero-extend or sign-extend when widening to i128.
/// Per C11 6.3.1.3, converting unsigned long long to __int128 preserves the value
/// (zero-extends), while converting signed long long sign-extends.
fn eval_const_binop_i128(op: &BinOp, lhs: &IrConst, rhs: &IrConst, is_unsigned: bool, lhs_unsigned: bool, rhs_unsigned: bool) -> Option<IrConst> {
    // When widening a non-I128 operand to 128-bit, use the operand's own signedness
    // to decide zero-extend vs sign-extend. This is critical for cases like:
    //   (i128)x << 64 | 0xFEDCBA9876543210ULL
    // where the RHS is unsigned long long (zero-extend) but the result type is
    // signed __int128.
    let l = if lhs_unsigned && !matches!(lhs, IrConst::I128(_)) {
        // Zero-extend: treat the i64 bit pattern as u64, then widen to u128
        (lhs.to_i64()? as u64 as u128) as i128
    } else {
        lhs.to_i128()?
    };
    let r = if rhs_unsigned && !matches!(rhs, IrConst::I128(_)) {
        (rhs.to_i64()? as u64 as u128) as i128
    } else {
        rhs.to_i128()?
    };

    let bool_result = |b: bool| -> Option<IrConst> {
        Some(IrConst::I64(if b { 1 } else { 0 }))
    };

    match op {
        BinOp::Add => Some(IrConst::I128(l.wrapping_add(r))),
        BinOp::Sub => Some(IrConst::I128(l.wrapping_sub(r))),
        BinOp::Mul => Some(IrConst::I128(l.wrapping_mul(r))),
        BinOp::Div => {
            if r == 0 { return None; }
            if is_unsigned {
                Some(IrConst::I128((l as u128).wrapping_div(r as u128) as i128))
            } else {
                Some(IrConst::I128(l.wrapping_div(r)))
            }
        }
        BinOp::Mod => {
            if r == 0 { return None; }
            if is_unsigned {
                Some(IrConst::I128((l as u128).wrapping_rem(r as u128) as i128))
            } else {
                Some(IrConst::I128(l.wrapping_rem(r)))
            }
        }
        BinOp::BitAnd => Some(IrConst::I128(l & r)),
        BinOp::BitOr => Some(IrConst::I128(l | r)),
        BinOp::BitXor => Some(IrConst::I128(l ^ r)),
        BinOp::Shl => Some(IrConst::I128(l.wrapping_shl(r as u32))),
        BinOp::Shr => {
            if is_unsigned {
                Some(IrConst::I128((l as u128).wrapping_shr(r as u32) as i128))
            } else {
                Some(IrConst::I128(l.wrapping_shr(r as u32)))
            }
        }
        BinOp::Eq => bool_result(l == r),
        BinOp::Ne => bool_result(l != r),
        BinOp::Lt => {
            if is_unsigned { bool_result((l as u128) < (r as u128)) }
            else { bool_result(l < r) }
        }
        BinOp::Gt => {
            if is_unsigned { bool_result((l as u128) > (r as u128)) }
            else { bool_result(l > r) }
        }
        BinOp::Le => {
            if is_unsigned { bool_result((l as u128) <= (r as u128)) }
            else { bool_result(l <= r) }
        }
        BinOp::Ge => {
            if is_unsigned { bool_result((l as u128) >= (r as u128)) }
            else { bool_result(l >= r) }
        }
        BinOp::LogicalAnd => bool_result(l != 0 && r != 0),
        BinOp::LogicalOr => bool_result(l != 0 || r != 0),
    }
}

/// Evaluate a constant binary operation, dispatching to int, i128, or float as needed.
///
/// This is the top-level entry point for constant binary evaluation.
/// The caller provides `is_32bit` and `is_unsigned` for the integer path.
/// `lhs_unsigned` and `rhs_unsigned` indicate the signedness of each operand's
/// original C type, used for correct widening to i128.
pub fn eval_const_binop(op: &BinOp, lhs: &IrConst, rhs: &IrConst, is_32bit: bool, is_unsigned: bool, lhs_unsigned: bool, rhs_unsigned: bool) -> Option<IrConst> {
    let lhs_is_float = matches!(lhs, IrConst::F32(_) | IrConst::F64(_) | IrConst::LongDouble(..));
    let rhs_is_float = matches!(rhs, IrConst::F32(_) | IrConst::F64(_) | IrConst::LongDouble(..));

    if lhs_is_float || rhs_is_float {
        return eval_const_binop_float(op, lhs, rhs);
    }

    // Use native i128 arithmetic when either operand is I128 to avoid truncation.
    if matches!(lhs, IrConst::I128(_)) || matches!(rhs, IrConst::I128(_)) {
        return eval_const_binop_i128(op, lhs, rhs, is_unsigned, lhs_unsigned, rhs_unsigned);
    }

    let l = lhs.to_i64()?;
    let r = rhs.to_i64()?;
    eval_const_binop_int(op, l, r, is_32bit, is_unsigned)
}

/// Negate a constant value (unary `-`).
/// Sub-int types are promoted to i32 per C integer promotion rules.
/// Uses wrapping negation to handle MIN values (e.g. -(-2^63) wraps to -2^63 in C).
pub fn negate_const(val: IrConst) -> Option<IrConst> {
    match val {
        IrConst::I128(v) => Some(IrConst::I128(v.wrapping_neg())),
        IrConst::I64(v) => Some(IrConst::I64(v.wrapping_neg())),
        IrConst::I32(v) => Some(IrConst::I32(v.wrapping_neg())),
        IrConst::I8(v) => Some(IrConst::I32((v as i32).wrapping_neg())),
        IrConst::I16(v) => Some(IrConst::I32((v as i32).wrapping_neg())),
        IrConst::F64(v) => Some(IrConst::F64(-v)),
        IrConst::F32(v) => Some(IrConst::F32(-v)),
        IrConst::LongDouble(v, f128_bytes) => {
            // Negate by flipping the sign bit in the f128 bytes directly,
            // preserving full 112-bit precision
            let val = u128::from_le_bytes(f128_bytes);
            let neg_val = val ^ (1u128 << 127);
            Some(IrConst::long_double_with_bytes(-v, neg_val.to_le_bytes()))
        }
        _ => None,
    }
}

/// Bitwise NOT of a constant value (unary `~`).
/// Sub-int types are promoted to i32 per C integer promotion rules.
#[cfg(test)]
mod negate_const_pbt {
    use super::negate_const;
    use crate::ir::reexports::IrConst;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn i128_i64_i32_negation_matches_wrapping_neg(choice in 0u8..3, value in any::<i128>()) {
            let constant = match choice {
                0 => IrConst::I128(value),
                1 => IrConst::I64(value as i64),
                _ => IrConst::I32(value as i32),
            };

            let negated = negate_const(constant).expect("integer constants are negatable");

            match negated {
                IrConst::I128(actual) => prop_assert_eq!(actual, value.wrapping_neg()),
                IrConst::I64(actual) => prop_assert_eq!(actual, (value as i64).wrapping_neg()),
                IrConst::I32(actual) => prop_assert_eq!(actual, (value as i32).wrapping_neg()),
                other => prop_assert!(false, "unexpected negated integer variant: {:?}", other),
            }
        }

        #[test]
        fn integer_negation_is_an_involution_with_wrapping(choice in 0u8..3, value in any::<i128>()) {
            let constant = match choice {
                0 => IrConst::I128(value),
                1 => IrConst::I64(value as i64),
                _ => IrConst::I32(value as i32),
            };

            let once = negate_const(constant).expect("integer constants are negatable");
            let twice = negate_const(once).expect("negated integer constants are negatable");

            match twice {
                IrConst::I128(actual) => prop_assert_eq!(actual, value),
                IrConst::I64(actual) => prop_assert_eq!(actual, value as i64),
                IrConst::I32(actual) => prop_assert_eq!(actual, value as i32),
                other => prop_assert!(false, "unexpected double-negated integer variant: {:?}", other),
            }
        }

        #[test]
        fn sub_int_negation_promotes_to_i32(choice in 0u8..2, value in any::<i16>()) {
            let constant = if choice == 0 {
                IrConst::I8(value as i8)
            } else {
                IrConst::I16(value)
            };

            let negated = negate_const(constant).expect("sub-int constants are negatable");

            match negated {
                IrConst::I32(actual) => {
                    let expected = if choice == 0 {
                        (value as i8 as i32).wrapping_neg()
                    } else {
                        (value as i32).wrapping_neg()
                    };
                    prop_assert_eq!(actual, expected);
                }
                other => prop_assert!(false, "sub-int negation should promote to I32, got {:?}", other),
            }
        }

        #[test]
        fn float_negation_matches_native_bit_pattern(choice in 0u8..2, bits in any::<u64>()) {
            let constant = if choice == 0 {
                IrConst::F32(f32::from_bits(bits as u32))
            } else {
                IrConst::F64(f64::from_bits(bits))
            };

            let negated = negate_const(constant).expect("float constants are negatable");

            match negated {
                IrConst::F32(actual) => prop_assert_eq!(actual.to_bits(), (-f32::from_bits(bits as u32)).to_bits()),
                IrConst::F64(actual) => prop_assert_eq!(actual.to_bits(), (-f64::from_bits(bits)).to_bits()),
                other => prop_assert!(false, "unexpected negated float variant: {:?}", other),
            }
        }

        #[test]
        fn long_double_negation_flips_only_f128_sign_bit(value in any::<f64>(), bytes in any::<[u8; 16]>()) {
            let negated = negate_const(IrConst::LongDouble(value, bytes)).expect("long double constants are negatable");

            match negated {
                IrConst::LongDouble(actual_value, actual_bytes) => {
                    let mut expected_bytes = bytes;
                    expected_bytes[15] ^= 0x80;
                    prop_assert_eq!(actual_value.to_bits(), (-value).to_bits());
                    prop_assert_eq!(actual_bytes, expected_bytes);
                }
                other => prop_assert!(false, "unexpected negated long double variant: {:?}", other),
            }
        }
    }
}

/// Bitwise NOT of a constant value (unary `~`).
/// Sub-int types are promoted to i32 per C integer promotion rules.
pub fn bitnot_const(val: IrConst) -> Option<IrConst> {
    match val {
        IrConst::I128(v) => Some(IrConst::I128(!v)),
        IrConst::I64(v) => Some(IrConst::I64(!v)),
        IrConst::I32(v) => Some(IrConst::I32(!v)),
        IrConst::I8(v) => Some(IrConst::I32(!(v as i32))),
        IrConst::I16(v) => Some(IrConst::I32(!(v as i32))),
        _ => None,
    }
}

#[cfg(test)]
mod bitnot_const_pbt {
    use super::bitnot_const;
    use crate::ir::reexports::IrConst;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn integer_bitnot_matches_native_width(choice in 0u8..3, value in any::<i128>()) {
            let constant = match choice {
                0 => IrConst::I128(value),
                1 => IrConst::I64(value as i64),
                _ => IrConst::I32(value as i32),
            };

            let inverted = bitnot_const(constant).expect("integer constants support bitwise not");

            match inverted {
                IrConst::I128(actual) => prop_assert_eq!(actual, !value),
                IrConst::I64(actual) => prop_assert_eq!(actual, !(value as i64)),
                IrConst::I32(actual) => prop_assert_eq!(actual, !(value as i32)),
                other => prop_assert!(false, "unexpected bitnot integer variant: {:?}", other),
            }
        }

        #[test]
        fn integer_bitnot_is_an_involution(choice in 0u8..3, value in any::<i128>()) {
            let constant = match choice {
                0 => IrConst::I128(value),
                1 => IrConst::I64(value as i64),
                _ => IrConst::I32(value as i32),
            };

            let once = bitnot_const(constant).expect("integer constants support bitwise not");
            let twice = bitnot_const(once).expect("bitnot result supports bitwise not");

            match twice {
                IrConst::I128(actual) => prop_assert_eq!(actual, value),
                IrConst::I64(actual) => prop_assert_eq!(actual, value as i64),
                IrConst::I32(actual) => prop_assert_eq!(actual, value as i32),
                other => prop_assert!(false, "unexpected double-bitnot integer variant: {:?}", other),
            }
        }

        #[test]
        fn sub_int_bitnot_promotes_to_i32(choice in 0u8..2, value in any::<i16>()) {
            let constant = if choice == 0 {
                IrConst::I8(value as i8)
            } else {
                IrConst::I16(value)
            };

            let inverted = bitnot_const(constant).expect("sub-int constants support bitwise not");

            match inverted {
                IrConst::I32(actual) => {
                    let expected = if choice == 0 {
                        !(value as i8 as i32)
                    } else {
                        !(value as i32)
                    };
                    prop_assert_eq!(actual, expected);
                }
                other => prop_assert!(false, "sub-int bitnot should promote to I32, got {:?}", other),
            }
        }

        #[test]
        fn bitnot_matches_twos_complement_negation_identity(choice in 0u8..3, value in any::<i128>()) {
            let constant = match choice {
                0 => IrConst::I128(value),
                1 => IrConst::I64(value as i64),
                _ => IrConst::I32(value as i32),
            };

            let inverted = bitnot_const(constant).expect("integer constants support bitwise not");

            match inverted {
                IrConst::I128(actual) => prop_assert_eq!(actual, value.wrapping_neg().wrapping_sub(1)),
                IrConst::I64(actual) => prop_assert_eq!(actual, (value as i64).wrapping_neg().wrapping_sub(1)),
                IrConst::I32(actual) => prop_assert_eq!(actual, (value as i32).wrapping_neg().wrapping_sub(1)),
                other => prop_assert!(false, "unexpected bitnot identity variant: {:?}", other),
            }
        }

        #[test]
        fn non_integer_constants_do_not_support_bitnot(choice in 0u8..3, bits in any::<u64>(), bytes in any::<[u8; 16]>()) {
            let constant = match choice {
                0 => IrConst::F32(f32::from_bits(bits as u32)),
                1 => IrConst::F64(f64::from_bits(bits)),
                _ => IrConst::LongDouble(f64::from_bits(bits), bytes),
            };

            prop_assert!(bitnot_const(constant).is_none());
        }
    }
}

/// Check if an AST expression is a zero literal (0 or cast of 0).
/// Used for offsetof pattern detection: `&((type*)0)->member`.
pub fn is_zero_expr(expr: &crate::frontend::parser::ast::Expr) -> bool {
    use crate::frontend::parser::ast::Expr;
    match expr {
        Expr::IntLiteral(0, _) | Expr::UIntLiteral(0, _)
        | Expr::LongLiteral(0, _) | Expr::ULongLiteral(0, _)
        | Expr::LongLongLiteral(0, _) | Expr::ULongLongLiteral(0, _) => true,
        Expr::Cast(_, inner, _) => is_zero_expr(inner),
        _ => false,
    }
}

/// Check if an AST expression is a null pointer constant per C11 6.3.2.3p3:
/// "An integer constant expression with the value 0, or such an expression
///  cast to type void *, is called a null pointer constant."
///
/// This is stricter than is_zero_expr because it requires the expression to be
/// an integer constant expression (ICE), not just evaluating to zero at runtime.
/// For example, `(void*)((long)(volatile_var) * 0l)` is NOT an NPC because
/// `(long)(volatile_var) * 0l` is not an ICE (it involves a volatile variable).
pub fn is_null_pointer_constant(expr: &crate::frontend::parser::ast::Expr) -> bool {
    use crate::frontend::parser::ast::Expr;
    use crate::frontend::parser::ast::TypeSpecifier;
    match expr {
        // Integer literal 0 is an NPC
        Expr::IntLiteral(0, _) | Expr::UIntLiteral(0, _)
        | Expr::LongLiteral(0, _) | Expr::ULongLiteral(0, _)
        | Expr::LongLongLiteral(0, _) | Expr::ULongLongLiteral(0, _) => true,
        // (void*)ICE_zero is an NPC
        Expr::Cast(ts, inner, _) => {
            if let TypeSpecifier::Pointer(p, _) = ts {
                if matches!(p.as_ref(), TypeSpecifier::Void) {
                    // Cast to void*: inner must be an integer constant expression with value 0
                    return is_integer_constant_expr_zero(inner);
                }
            }
            // Cast to non-void-pointer type: still check if it's an NPC
            // (e.g. (int)0 is still an ICE zero)
            is_null_pointer_constant(inner)
        }
        _ => false,
    }
}

/// Check if an expression is an integer constant expression that evaluates to 0.
/// An ICE can only contain literals, sizeof, alignof, enum constants, and operations
/// on these. It cannot contain variable references, function calls, or volatile accesses.
fn is_integer_constant_expr_zero(expr: &crate::frontend::parser::ast::Expr) -> bool {
    // First check if the expression is purely constant (no variable refs)
    if !is_syntactically_constant(expr) {
        return false;
    }
    // Then check if it evaluates to zero
    is_zero_valued_constant(expr)
}

/// Check if an expression is syntactically constant (contains no variable references,
/// function calls, or other non-constant elements). This is a conservative check for
/// integer constant expressions per C11 6.6.
fn is_syntactically_constant(expr: &crate::frontend::parser::ast::Expr) -> bool {
    use crate::frontend::parser::ast::Expr;
    match expr {
        // Literals are constant
        Expr::IntLiteral(_, _) | Expr::UIntLiteral(_, _)
        | Expr::LongLiteral(_, _) | Expr::ULongLiteral(_, _)
        | Expr::LongLongLiteral(_, _) | Expr::ULongLongLiteral(_, _)
        | Expr::CharLiteral(_, _) | Expr::FloatLiteral(_, _)
        | Expr::FloatLiteralF32(_, _) | Expr::FloatLiteralLongDouble(_, _, _) => true,

        // sizeof and alignof are constant (even when applied to expressions)
        Expr::Sizeof(_, _) | Expr::Alignof(_, _)
        | Expr::AlignofExpr(_, _) | Expr::GnuAlignof(_, _)
        | Expr::GnuAlignofExpr(_, _) => true,

        // Casts of constant expressions are constant
        Expr::Cast(_, inner, _) => is_syntactically_constant(inner),

        // Unary ops on constant expressions are constant
        Expr::UnaryOp(_, inner, _) | Expr::PostfixOp(_, inner, _) => {
            is_syntactically_constant(inner)
        }

        // Binary ops on constant expressions are constant
        Expr::BinaryOp(_, lhs, rhs, _) => {
            is_syntactically_constant(lhs) && is_syntactically_constant(rhs)
        }

        // Conditional on constant expressions is constant
        Expr::Conditional(cond, then_e, else_e, _) => {
            is_syntactically_constant(cond) &&
            is_syntactically_constant(then_e) &&
            is_syntactically_constant(else_e)
        }

        // Variable references, function calls, etc. are NOT constant
        // This includes identifiers that might be enum constants, but
        // for the NPC check we're conservative and treat them as non-constant.
        // This is fine because enum-constant-based NPCs would be caught by
        // the literal zero check above.
        _ => false,
    }
}

/// Check if a syntactically-constant expression evaluates to zero.
/// Only call this after is_syntactically_constant returns true.
fn is_zero_valued_constant(expr: &crate::frontend::parser::ast::Expr) -> bool {
    use crate::frontend::parser::ast::Expr;
    match expr {
        Expr::IntLiteral(v, _) => *v == 0,
        Expr::UIntLiteral(v, _) => *v == 0,
        Expr::LongLiteral(v, _) => *v == 0,
        Expr::ULongLiteral(v, _) => *v == 0,
        Expr::LongLongLiteral(v, _) => *v == 0,
        Expr::ULongLongLiteral(v, _) => *v == 0,
        Expr::CharLiteral(v, _) => *v == '\0',
        // Cast of zero is zero
        Expr::Cast(_, inner, _) => is_zero_valued_constant(inner),
        // x * 0 = 0 for any constant x; 0 * x = 0
        Expr::BinaryOp(BinOp::Mul, lhs, rhs, _) => {
            is_zero_valued_constant(lhs) || is_zero_valued_constant(rhs)
        }
        // 0 + 0, 0 - 0, 0 & x, etc. - be conservative, only handle * 0
        // For other ops, we'd need full evaluation which is complex.
        // The * 0 case is the critical one for __is_constexpr.
        _ => false,
    }
}

/// Evaluate raw u64 bit truncation and sign extension for a cast chain.
///
/// Given raw bits from a source value, truncates to `target_width` bits
/// and optionally sign-extends back to 64 bits. Returns `(result_bits, target_signed)`.
///
/// The caller determines `target_width` and `target_signed` from their type system.
pub fn truncate_and_extend_bits(bits: u64, target_width: usize, target_signed: bool) -> (u64, bool) {
    // Truncate to target width
    let truncated = if target_width >= 64 || target_width == 0 {
        bits
    } else {
        bits & ((1u64 << target_width) - 1)
    };

    // If target is signed, sign-extend to 64 bits
    let result = if target_signed && target_width < 64 && target_width > 0 {
        let sign_bit = 1u64 << (target_width - 1);
        if truncated & sign_bit != 0 {
            truncated | !((1u64 << target_width) - 1)
        } else {
            truncated
        }
    } else {
        truncated
    };

    (result, target_signed)
}

#[cfg(test)]
mod truncate_and_extend_bits_pbt {
    use super::truncate_and_extend_bits;
    use proptest::prelude::*;

    fn sign_extend_masked(bits: u64, width: usize) -> u64 {
        if width == 0 || width >= 64 {
            bits
        } else {
            let mask = (1u64 << width) - 1;
            let truncated = bits & mask;
            let sign_bit = 1u64 << (width - 1);
            if truncated & sign_bit != 0 {
                truncated | !mask
            } else {
                truncated
            }
        }
    }

    proptest! {
        #[test]
        fn zero_width_is_a_noop(bits in any::<u64>(), signed in any::<bool>()) {
            let (result, result_signed) = truncate_and_extend_bits(bits, 0, signed);
            prop_assert_eq!(result, bits);
            prop_assert_eq!(result_signed, signed);
        }

        #[test]
        fn widths_at_least_64_are_a_noop(bits in any::<u64>(), width in 64usize..256usize, signed in any::<bool>()) {
            let (result, result_signed) = truncate_and_extend_bits(bits, width, signed);
            prop_assert_eq!(result, bits);
            prop_assert_eq!(result_signed, signed);
        }

        #[test]
        fn unsigned_results_match_low_bits(bits in any::<u64>(), width in 1usize..64usize) {
            let (result, result_signed) = truncate_and_extend_bits(bits, width, false);
            let mask = (1u64 << width) - 1;
            prop_assert_eq!(result, bits & mask);
            prop_assert!(!result_signed);
        }

        #[test]
        fn signed_results_match_twos_complement_sign_extension(bits in any::<u64>(), width in 1usize..64usize) {
            let (result, result_signed) = truncate_and_extend_bits(bits, width, true);
            prop_assert_eq!(result, sign_extend_masked(bits, width));
            prop_assert!(result_signed);
        }

        #[test]
        fn applying_the_helper_twice_is_idempotent(bits in any::<u64>(), width in 0usize..256usize, signed in any::<bool>()) {
            let once = truncate_and_extend_bits(bits, width, signed);
            let twice = truncate_and_extend_bits(once.0, width, signed);
            prop_assert_eq!(twice, once);
        }
    }
}
