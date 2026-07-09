//! Property-based tests for `encode_neon_umov`.
//!
//! `encode_neon_umov` encodes the AArch64 `UMOV` instruction (Advanced SIMD
//! copy group), which moves a single vector element to a general-purpose
//! register:
//!   * `UMOV Wd, Vn.<Ts>[index]`  (Ts in {B,H,S}; 32-bit GP destination)
//!   * `UMOV Xd, Vn.D[index]`      (64-bit GP destination)
//!
//! Encoding (single form, ARMv8 ARM, "Advanced SIMD copy"):
//!   `0 Q 0 01110 000 imm5 0 0111 1 Rn Rd`
//!    31 30 29-24 23-21 20-16 15 14-10 9-5 4-0
//!
//! `imm5` packs element size (sentinel bit) + lane index. **Q (bit 30) MUST be
//! 0 for UMOV** in both forms (the destination is a GP register, not a vector,
//! so the 64/128-bit SIMD-width Q bit is architecturally forced to 0). Golden
//! words below were derived from the ARMv8 ARM encoding and cross-checked
//! against the shape of `llvm-mc`/objdump output (`umov x0, v0.d[0]` =>
//! `0x0e083c00`, i.e. bit 30 = 0).

#![cfg(test)]

use super::encode_neon_umov;
use crate::backend::arm::assembler::encoder::EncodeResult;
use crate::backend::arm::assembler::parser::Operand;
use proptest::prelude::*;

// --- helpers --------------------------------------------------------------

/// GP destination register. `dest_x == true` => `xN` (64-bit), else `wN`.
fn gp(dest_x: bool, num: u32) -> Operand {
    let name = if dest_x { format!("x{num}") } else { format!("w{num}") };
    Operand::Reg(name)
}

/// `Operand::RegLane { reg: "v{rn}", elem_size, index }` — the source vector element.
fn lane(rn: u32, elem_size: &str, index: u32) -> Operand {
    Operand::RegLane { reg: format!("v{rn}"), elem_size: elem_size.to_string(), index }
}

fn elem_size_strategy() -> impl Strategy<Value = &'static str> {
    prop_oneof![Just("b"), Just("h"), Just("s"), Just("d")]
}

fn reg_num_strategy() -> impl Strategy<Value = u32> {
    0u32..=31u32
}

fn word_of(res: Result<EncodeResult, String>) -> u32 {
    match res {
        Ok(EncodeResult::Word(w)) => w,
        other => panic!("expected EncodeResult::Word, got {other:?}"),
    }
}

/// Per-element-size metadata derived from the ARM ARM `imm5` element-size
/// encoding: the sentinel is the single low bit of `imm5`, the lane index
/// occupies the bits above it, and `max_lane` is the architecturally valid
/// upper bound.
///   b -> sentinel 0b00001, idx shift 1, max lane 15
///   h -> sentinel 0b00010, idx shift 2, max lane 7
///   s -> sentinel 0b00100, idx shift 3, max lane 3
///   d -> sentinel 0b01000, idx shift 4, max lane 1
fn size_meta(elem_size: &str) -> Option<(u32, u32, u32)> {
    // (sentinel, index_shift, max_lane)
    Some(match elem_size {
        "b" => (0b00001, 1, 15),
        "h" => (0b00010, 2, 7),
        "s" => (0b00100, 3, 3),
        "d" => (0b01000, 4, 1),
        _ => return None,
    })
}

// --- properties -----------------------------------------------------------

proptest! {
    // === Oracle: reference / structural (fixed opcode bits) ===============
    // For any well-formed input, the fixed fields of the UMOV encoding must be
    // exactly as defined by the ARMv8 ARM "Advanced SIMD copy" group:
    //   bit 31      = 0
    //   bits 29-21  = 0b001110000
    //   bits 15-10  = 0b001111   (the UMOV opcode within the copy group)
    // These bits do not depend on the destination register width.
    #[test]
    fn prop_fixed_opcode_bits(
        dest_x in any::<bool>(),
        rd in reg_num_strategy(),
        rn in reg_num_strategy(),
        elem in elem_size_strategy(),
        idx_mod in 0u32..=63u32,
    ) {
        let (_, _, max_lane) = size_meta(elem).unwrap();
        let idx = idx_mod % (max_lane + 1); // keep lane in range
        let w = word_of(encode_neon_umov(&[gp(dest_x, rd), lane(rn, elem, idx)]));
        prop_assert_eq!(w >> 31, 0u32, "bit 31 must be 0");
        prop_assert_eq!((w >> 21) & 0x1FF, 0b001110000u32, "bits 29-21 fixed");
        prop_assert_eq!((w >> 10) & 0x3F, 0b001111u32, "bits 15-10 = UMOV opcode 001111");
    }

    // === Oracle: algebraic (imm5 packs size + index) =====================
    // imm5 (bits 20-16) == (index << shift) | sentinel, with the size sentinel
    // uniquely tagging the element size. Holds for every in-range lane index.
    #[test]
    fn prop_imm5_packs_size_and_index(
        dest_x in any::<bool>(),
        rd in reg_num_strategy(),
        rn in reg_num_strategy(),
        elem in elem_size_strategy(),
        idx_mod in 0u32..=63u32,
    ) {
        let (sentinel, shift, max_lane) = size_meta(elem).unwrap();
        let idx = idx_mod % (max_lane + 1);
        let w = word_of(encode_neon_umov(&[gp(dest_x, rd), lane(rn, elem, idx)]));
        let imm5 = (w >> 16) & 0x1F;
        prop_assert_eq!(imm5, (idx << shift) | sentinel, "imm5 must pack index+size");
        prop_assert_eq!(imm5 & sentinel, sentinel, "size sentinel must be set");
    }

    // === Oracle: reference / round-trip (register fields) ================
    // Rd (bits 4-0) round-trips the GP destination; Rn (bits 9-5) round-trips
    // the source vector register. Placement is fixed by the ARMv8 ARM.
    #[test]
    fn prop_rd_rn_roundtrip(
        dest_x in any::<bool>(),
        rd in reg_num_strategy(),
        rn in reg_num_strategy(),
        elem in elem_size_strategy(),
        idx_mod in 0u32..=63u32,
    ) {
        let (_, _, max_lane) = size_meta(elem).unwrap();
        let idx = idx_mod % (max_lane + 1);
        let w = word_of(encode_neon_umov(&[gp(dest_x, rd), lane(rn, elem, idx)]));
        prop_assert_eq!(w & 0x1F, rd, "Rd field must equal GP destination");
        prop_assert_eq!((w >> 5) & 0x1F, rn, "Rn field must equal source vector register");
    }

    // === Oracle: reference / structural (Q bit MUST be 0) ================
    // UMOV writes a GP register, so the SIMD-width Q bit (bit 30) is
    // architecturally forced to 0 in BOTH the 32-bit and 64-bit forms
    // (ARMv8 ARM). `llvm-mc`/objdump emit e.g. `umov x0, v0.d[0]` as
    // `0x0e083c00` (bit 30 = 0). A Q=1 word is an UNDEFINED encoding here.
    //
    // EXPECTED BEHAVIOR: `(word >> 30) & 1 == 0`. The current code sets
    // `q = if is_64 { 1 } else { 0 }`, so it FAILS for every 64-bit GP
    // destination. See COVERAGE / bug report.
    #[test]
    fn prop_q_bit_must_be_zero(
        dest_x in any::<bool>(),
        rd in reg_num_strategy(),
        rn in reg_num_strategy(),
        elem in elem_size_strategy(),
        idx_mod in 0u32..=63u32,
    ) {
        let (_, _, max_lane) = size_meta(elem).unwrap();
        let idx = idx_mod % (max_lane + 1);
        let w = word_of(encode_neon_umov(&[gp(dest_x, rd), lane(rn, elem, idx)]));
        prop_assert_eq!((w >> 30) & 1, 0u32,
            "UMOV requires Q (bit 30) == 0 for both Wd and Xd forms; \
             code wrongly sets Q=1 for 64-bit GP destination");
    }

    // === Oracle: negative contract (out-of-range lane index) ============
    // The ARMv8 ARM constrains the lane index per element size:
    //   .b -> [0,15], .h -> [0,7], .s -> [0,3], .d -> [0,1].
    // `llvm-mc-18` rejects e.g. `umov w0, v0.b[16]` with
    //   "vector lane must be an integer in range [0, 15]".
    // No AArch64 spec defines wrapping/truncation as intentional, so the
    // encoder MUST return Err for an out-of-range lane. The current code
    // silently masks the index (`& 0xF`/`& 0x7`/`& 0x3`/`& 0x1`).
    //
    // EXPECTED BEHAVIOR: Err.  See COVERAGE / bug report.
    #[test]
    fn prop_rejects_out_of_range_lane_index(
        elem in elem_size_strategy(),
        over in 1u32..=16u32, // overshoot above the per-size maximum
    ) {
        let (_, _, max_lane) = size_meta(elem).unwrap();
        let bad_index = max_lane + over;
        // keep the GP width consistent with the element size so the only
        // invalid thing is the lane index.
        let dest = if elem == "d" { gp(true, 0) } else { gp(false, 0) };
        let ops: Vec<Operand> = vec![dest, lane(0, elem, bad_index)];
        prop_assert!(encode_neon_umov(&ops).is_err(),
            "out-of-range lane [{bad_index}] for .{elem} (max {max_lane}) must be rejected, \
             not silently truncated/masked");
    }

    // === Oracle: negative contract (malformed operands) ==================
    // Too few operands, a second operand that is not a RegLane, an
    // unsupported element size, or an unparseable (out-of-range) source
    // vector register must all be rejected with Err.
    #[test]
    fn prop_rejects_malformed_operands(
        // any short lowercase string that is NOT exactly b/h/s/d
        bad_size in "[a-z]{1,3}".prop_filter(
            "not a valid element size", |s| !matches!(s.as_str(), "b" | "h" | "s" | "d")),
        bad_reg in "(x|v)(3[2-9]|[4-9][0-9])", // numeric, always > 31
    ) {
        // empty operand list
        prop_assert!(encode_neon_umov(&[]).is_err());
        // single operand only
        prop_assert!(encode_neon_umov(&[gp(true, 0)]).is_err());
        // second operand is not a RegLane
        prop_assert!(encode_neon_umov(&[gp(false, 0), gp(true, 1)]).is_err());
        // unsupported element size
        let ops: Vec<Operand> = vec![
            gp(false, 0),
            Operand::RegLane { reg: "v0".into(), elem_size: bad_size.clone(), index: 0 },
        ];
        prop_assert!(encode_neon_umov(&ops).is_err(),
            "elem_size {bad_size:?} must be rejected");
        // unparseable (out-of-range) source vector register
        let ops: Vec<Operand> = vec![
            gp(false, 0),
            Operand::RegLane { reg: bad_reg.clone(), elem_size: "s".into(), index: 0 },
        ];
        prop_assert!(encode_neon_umov(&ops).is_err(),
            "source register {bad_reg:?} must be rejected");
    }
}

// --- golden differential regression (ARMv8 ARM / llvm-mc anchors) ---------
//
// Words computed from the ARMv8 ARM UMOV encoding and cross-checked against
// the known objdump/llvm-mc bit shape. The 32-bit-GP-destination cases (Q=0)
// are produced correctly. The 64-bit-GP-destination cases are emitted with
// Q=1 (0x4E...) by the current code, which is WRONG — the correct word has
// Q=0 (0x0E...).
#[test]
fn golden_umov() {
    // 32-bit GP destination (Wd) — Q=0, all correct:
    //  umov w0,  v0.s[0]  -> 0x0e043c00
    assert_eq!(word_of(encode_neon_umov(&[gp(false, 0), lane(0, "s", 0)])), 0x0E043C00);
    //  umov w5,  v2.b[7]  -> 0x0e0f3c45
    assert_eq!(word_of(encode_neon_umov(&[gp(false, 5), lane(2, "b", 7)])), 0x0E0F3C45);
    //  umov w15, v31.s[3] -> 0x0e1c3fef
    assert_eq!(word_of(encode_neon_umov(&[gp(false, 15), lane(31, "s", 3)])), 0x0E1C3FEF);
    //  umov w7,  v4.h[5]  -> 0x0e163c87
    assert_eq!(word_of(encode_neon_umov(&[gp(false, 7), lane(4, "h", 5)])), 0x0E163C87);

    // 64-bit GP destination (Xd) — CORRECT word has Q=0 (0x0E...).
    // Current code returns 0x4E083C00 / 0x4E183C69 (Q=1), so these FAIL:
    //  umov x0, v0.d[0] -> 0x0e083c00
    assert_eq!(word_of(encode_neon_umov(&[gp(true, 0), lane(0, "d", 0)])), 0x0E083C00,
        "umov x0, v0.d[0]: Q (bit 30) must be 0; code emits 0x4E083C00");
    //  umov x9, v3.d[1] -> 0x0e183c69
    assert_eq!(word_of(encode_neon_umov(&[gp(true, 9), lane(3, "d", 1)])), 0x0E183C69,
        "umov x9, v3.d[1]: Q (bit 30) must be 0; code emits 0x4E183C69");
}
