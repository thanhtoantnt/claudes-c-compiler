use super::*;
use crate::backend::arm::assembler::parser::Operand;

// ── Bitfield extract/insert ──────────────────────────────────────────────

/// Encode UBFX Rd, Rn, #lsb, #width -> UBFM Rd, Rn, #lsb, #(lsb+width-1)
pub(crate) fn encode_ubfx(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let lsb = get_imm(operands, 2)? as u32;
    let width = get_imm(operands, 3)? as u32;
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0u32 };
    let immr = lsb;
    let imms = lsb + width - 1;
    // UBFM: sf 10 100110 N immr imms Rn Rd
    let word = (sf << 31) | (0b10 << 29) | (0b100110 << 23) | (n << 22) | (immr << 16) | (imms << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode SBFX Rd, Rn, #lsb, #width -> SBFM Rd, Rn, #lsb, #(lsb+width-1)
pub(crate) fn encode_sbfx(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let lsb = get_imm(operands, 2)? as u32;
    let width = get_imm(operands, 3)? as u32;
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0u32 };
    let immr = lsb;
    let imms = lsb + width - 1;
    // SBFM: sf 00 100110 N immr imms Rn Rd
    let word = (sf << 31) | (0b100110 << 23) | (n << 22) | (immr << 16) | (imms << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode UBFM Rd, Rn, #immr, #imms (raw form)
pub(crate) fn encode_ubfm(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let immr = get_imm(operands, 2)? as u32;
    let imms = get_imm(operands, 3)? as u32;
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0u32 };
    let word = (sf << 31) | (0b10 << 29) | (0b100110 << 23) | (n << 22) | (immr << 16) | (imms << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode SBFM Rd, Rn, #immr, #imms (raw form)
pub(crate) fn encode_sbfm(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let immr = get_imm(operands, 2)? as u32;
    let imms = get_imm(operands, 3)? as u32;
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0u32 };
    let word = (sf << 31) | (0b100110 << 23) | (n << 22) | (immr << 16) | (imms << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode SBFIZ Rd, Rn, #lsb, #width — alias for SBFM Rd, Rn, #(-lsb MOD regsize), #(width-1)
pub(crate) fn encode_sbfiz(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let lsb = get_imm(operands, 2)? as u32;
    let width = get_imm(operands, 3)? as u32;
    let regsize = if is_64 { 64u32 } else { 32 };
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0u32 };
    let immr = (regsize.wrapping_sub(lsb)) & (regsize - 1);
    let imms = width - 1;
    let word = (sf << 31) | (0b100110 << 23) | (n << 22) | (immr << 16) | (imms << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode UBFIZ Rd, Rn, #lsb, #width — alias for UBFM Rd, Rn, #(-lsb MOD regsize), #(width-1)
pub(crate) fn encode_ubfiz(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let lsb = get_imm(operands, 2)? as u32;
    let width = get_imm(operands, 3)? as u32;
    let regsize = if is_64 { 64u32 } else { 32 };
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0u32 };
    let immr = (regsize.wrapping_sub(lsb)) & (regsize - 1);
    let imms = width - 1;
    let word = (sf << 31) | (0b10 << 29) | (0b100110 << 23) | (n << 22) | (immr << 16) | (imms << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode BFM Rd, Rn, #immr, #imms (bitfield move)
pub(crate) fn encode_bfm(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let immr = get_imm(operands, 2)? as u32;
    let imms = get_imm(operands, 3)? as u32;
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0u32 };
    // BFM: sf 01 100110 N immr imms Rn Rd
    let word = (sf << 31) | (0b01 << 29) | (0b100110 << 23) | (n << 22) | (immr << 16) | (imms << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode BFI Rd, Rn, #lsb, #width -> BFM Rd, Rn, #(-lsb mod width_reg), #(width-1)
pub(crate) fn encode_bfi(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let lsb = get_imm(operands, 2)? as u32;
    let width = get_imm(operands, 3)? as u32;
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0u32 };
    let reg_width = if is_64 { 64u32 } else { 32u32 };
    let immr = (reg_width - lsb) % reg_width;
    let imms = width - 1;
    let word = (sf << 31) | (0b01 << 29) | (0b100110 << 23) | (n << 22) | (immr << 16) | (imms << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode BFXIL Rd, Rn, #lsb, #width -> BFM Rd, Rn, #lsb, #(lsb+width-1)
pub(crate) fn encode_bfxil(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let lsb = get_imm(operands, 2)? as u32;
    let width = get_imm(operands, 3)? as u32;
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0u32 };
    let immr = lsb;
    let imms = lsb + width - 1;
    let word = (sf << 31) | (0b01 << 29) | (0b100110 << 23) | (n << 22) | (immr << 16) | (imms << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode EXTR Rd, Rn, Rm, #lsb
pub(crate) fn encode_extr(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let lsb = get_imm(operands, 3)? as u32;
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0u32 };
    // EXTR: sf 0 0 100111 N 0 Rm imms Rn Rd
    let word = (sf << 31) | (0b00100111 << 23) | (n << 22) | (rm << 16)
        | (lsb << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── Bit manipulation ─────────────────────────────────────────────────────

pub(crate) fn encode_clz(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let sf = sf_bit(is_64);
    // CLZ: sf 1 0 11010110 00000 00010 0 Rn Rd
    let word = ((sf << 31) | (1 << 30) | (0b011010110 << 21))
        | (0b000100 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_cls(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let sf = sf_bit(is_64);
    let word = ((sf << 31) | (1 << 30) | (0b011010110 << 21))
        | (0b000101 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_rbit(operands: &[Operand]) -> Result<EncodeResult, String> {
    // NEON vector form: RBIT Vd.T, Vn.T (reverse bits in each byte)
    if let Some(Operand::RegArrangement { .. }) = operands.first() {
        let (rd, arr_d) = get_neon_reg(operands, 0)?;
        let (rn, _) = get_neon_reg(operands, 1)?;
        let q: u32 = if arr_d == "16b" { 1 } else { 0 };
        // RBIT (vector): 0 Q 1 01110 01 10000 00101 10 Rn Rd
        let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (0b01 << 22)
            | (0b10000 << 17) | (0b00101 << 12) | (0b10 << 10) | (rn << 5) | rd;
        return Ok(EncodeResult::Word(word));
    }
    // Scalar form: RBIT Rd, Rn
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let sf = sf_bit(is_64);
    let word = ((sf << 31) | (1 << 30) | (0b011010110 << 21)) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_rev(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let sf = sf_bit(is_64);
    let opc = if is_64 { 0b000011 } else { 0b000010 };
    let word = ((sf << 31) | (1 << 30) | (0b011010110 << 21))
        | (opc << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_rev16(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let sf = sf_bit(is_64);
    let word = ((sf << 31) | (1 << 30) | (0b011010110 << 21))
        | (0b000001 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_rev32(operands: &[Operand]) -> Result<EncodeResult, String> {
    // Check for NEON vector form: REV32 Vd.T, Vn.T
    if let Some(Operand::RegArrangement { .. }) = operands.first() {
        let (rd, arr_d) = get_neon_reg(operands, 0)?;
        let (rn, _) = get_neon_reg(operands, 1)?;
        let (q, size) = neon_arr_to_q_size(&arr_d)?;
        // REV32 Vd.T, Vn.T: 0 Q 1 01110 size 10 0000 0000 10 Rn Rd
        let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (size << 22)
            | (0b100000 << 16) | (0b000010 << 10) | (rn << 5) | rd;
        return Ok(EncodeResult::Word(word));
    }
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    // REV32 is 64-bit only: 1 1 0 11010110 00000 000010 Rn Rd
    let word = ((1u32 << 31) | (1 << 30) | (0b011010110 << 21))
        | (0b000010 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── CRC32 ────────────────────────────────────────────────────────────────

pub(crate) fn encode_crc32(mnemonic: &str, operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;

    let is_c = mnemonic.contains("crc32c");
    let c_bit = if is_c { 1u32 } else { 0 };

    let (sf, sz) = match mnemonic {
        "crc32b" | "crc32cb" => (0u32, 0b00u32),
        "crc32h" | "crc32ch" => (0, 0b01),
        "crc32w" | "crc32cw" => (0, 0b10),
        "crc32x" | "crc32cx" => (1, 0b11),
        _ => (0, 0b00),
    };

    // CRC32: sf 0 0 11010110 Rm 010 C sz Rn Rd
    let word = (sf << 31) | (0b0011010110 << 21) | (rm << 16) | (0b010 << 13)
        | (c_bit << 12) | (sz << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

#[cfg(test)]
mod prop_encode_ubfx_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Field layout of the UBFM (UBFX alias) word ─────────────────────
    //   sf [31] | 10 [30:29] | 100110 [28:23] | N [22]
    //   | immr [21:16] | imms [15:10] | Rn [9:5] | Rd [4:0]
    const MASK_SF: u32 = 0x8000_0000;
    const FIXED_30_29: u32 = 0b10 << 29; // 0x4000_0000
    const MASK_30_29: u32 = 0b11 << 29; // 0x6000_0000
    const FIXED_28_23: u32 = 0b100110 << 23; // 0x1300_0000
    const MASK_28_23: u32 = 0b111111 << 23; // 0x1F80_0000
    const MASK_N: u32 = 1 << 22; // 0x0040_0000
    const MASK_IMMR: u32 = 0x003F_0000; // bits [21:16]
    const MASK_IMMS: u32 = 0x0000_FC00; // bits [15:10]
    const MASK_RN: u32 = 0x0000_03E0; // bits [9:5]
    const MASK_RD: u32 = 0x0000_001F; // bits [4:0]

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word(encode_ubfx(ops))
    }

    fn reg_name(num: u32, is_64: bool) -> String {
        if is_64 {
            format!("x{}", num)
        } else {
            format!("w{}", num)
        }
    }

    /// (rd_name, rd_num, rn_name, rn_num, lsb, width, is_64) with lsb/width
    /// constrained to architecturally-valid ranges so imms = lsb+width-1 fits
    /// its 6-bit field without wrapping.
    fn arb_valid_case() -> impl Strategy<Value = (String, u32, String, u32, u32, u32, bool)> {
        (any::<bool>(), 0u32..=30u32, 0u32..=30u32, 0u32..63u32, 1u32..=64u32)
            .prop_filter(
                "lsb+width must fit regsize",
                |&(is_64, _rd, _rn, lsb, width)| {
                    let max = if is_64 { 64 } else { 32 };
                    lsb < max && lsb + width <= max
                },
            )
            .prop_map(|(is_64, rd, rn, lsb, width)| {
                (reg_name(rd, is_64), rd, reg_name(rn, is_64), rn, lsb, width, is_64)
            })
    }

    /// Broad (possibly out-of-range) lsb/width for differential + invariant
    /// tests, where wrapping behaviour is part of what we compare.
    fn arb_broad_case() -> impl Strategy<Value = (String, u32, String, u32, u32, u32, bool)> {
        (any::<bool>(), 0u32..=30u32, 0u32..=30u32, 0u32..63u32, 1u32..=64u32).prop_map(
            |(is_64, rd, rn, lsb, width)| {
                (reg_name(rd, is_64), rd, reg_name(rn, is_64), rn, lsb, width, is_64)
            },
        )
    }

    proptest! {
        // Property A — structural / field-placement oracle.
        // Every fixed opcode bit and every variable field lands in its
        // mandated position; imms reconstructs exactly to lsb+width-1.
        #[test]
        fn prop_ubfx_field_placement(c in arb_valid_case()) {
            let (rd_name, rd, rn_name, rn, lsb, width, is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let w = enc(&ops);

            prop_assert_eq!(w & MASK_30_29, FIXED_30_29);
            prop_assert_eq!(w & MASK_28_23, FIXED_28_23);
            prop_assert_eq!(w & MASK_SF, if is_64 { MASK_SF } else { 0 });
            prop_assert_eq!(w & MASK_N, if is_64 { MASK_N } else { 0 });
            prop_assert_eq!((w & MASK_IMMR) >> 16, lsb);
            prop_assert_eq!((w & MASK_IMMS) >> 10, lsb + width - 1);
            prop_assert_eq!((w & MASK_RN) >> 5, rn);
            prop_assert_eq!(w & MASK_RD, rd);
        }

        // Property B — N tracks sf invariant. For UBFX the N bit must equal
        // the sf bit: both derive solely from the destination register width.
        #[test]
        fn prop_n_equals_sf(c in arb_broad_case()) {
            let (rd_name, _rd, rn_name, _rn, lsb, width, _is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let w = enc(&ops);
            prop_assert_eq!((w >> 31) & 1, (w >> 22) & 1);
        }

        // Property C — differential oracle against the sibling UBFM encoder.
        // UBFX Rd, Rn, #lsb, #width must be bit-identical to
        // UBFM Rd, Rn, #lsb, #(lsb+width-1). Holds even for out-of-range
        // immediates because both encoders wrap identically.
        #[test]
        fn prop_ubfx_equals_ubfm_with_converted_immediates(c in arb_broad_case()) {
            let (rd_name, _rd, rn_name, _rn, lsb, width, _is_64) = c;
            let ubfx = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Reg(rn_name.clone()),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let ubfm = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(lsb as i64),
                Operand::Imm((lsb + width - 1) as i64),
            ];
            prop_assert_eq!(enc(&ubfx), word(encode_ubfm(&ubfm)));
        }

        // Property D — register-width differential. Encoding with x{N} vs w{N}
        // (same numeric register, same lsb/width) must differ in exactly the
        // sf bit [31] and the N bit [22], and nowhere else.
        #[test]
        fn prop_width_changes_only_sf_and_n(
            num in 0u32..=30u32,
            lsb in 0u32..63u32,
            width in 1u32..=64u32,
        ) {
            let ops64 = vec![
                Operand::Reg(format!("x{}", num)),
                Operand::Reg("x0".into()),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let ops32 = vec![
                Operand::Reg(format!("w{}", num)),
                Operand::Reg("w0".into()),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let diff = enc(&ops64) ^ enc(&ops32);
            prop_assert_eq!(diff, MASK_SF | MASK_N);
        }

        // Property E — error / negative contract. Any operand list that is
        // missing a required operand or has a non-register/non-immediate in a
        // fixed slot must be rejected with Err.
        #[test]
        fn prop_rejects_malformed_operands(
            bad in prop_oneof![
                Just(0u8), Just(1u8), Just(2u8), Just(3u8), Just(4u8), Just(5u8),
            ],
            n in 0u32..=30u32,
            v in -16i64..=16i64,
        ) {
            let r = match bad {
                0 => encode_ubfx(&[]),
                1 => encode_ubfx(&[
                    Operand::Reg(reg_name(n, true)),
                    Operand::Reg("x1".into()),
                    Operand::Imm(v),
                ]),
                2 => encode_ubfx(&[
                    Operand::Imm(v),
                    Operand::Reg("x1".into()),
                    Operand::Imm(0),
                    Operand::Imm(1),
                ]),
                3 => encode_ubfx(&[
                    Operand::Reg("x0".into()),
                    Operand::Imm(v),
                    Operand::Imm(0),
                    Operand::Imm(1),
                ]),
                4 => encode_ubfx(&[
                    Operand::Reg("x0".into()),
                    Operand::Reg("x1".into()),
                    Operand::Reg(reg_name(n, true)),
                    Operand::Imm(1),
                ]),
                _ => encode_ubfx(&[
                    Operand::Reg("x0".into()),
                    Operand::Reg("x1".into()),
                    Operand::Imm(0),
                    Operand::Reg(reg_name(n, true)),
                ]),
            };
            prop_assert!(r.is_err(), "expected Err, got {:?}", r);
        }

        // Property F — NEGATIVE CONTRACT (the finding).
        // UBFX Rd, Rn, #lsb, #width maps to UBFM with imms = lsb+width-1.
        // ARM ARM operand constraints (Bitfield, §C4.1.69):
        //   64-bit: 0 <= lsb <= 63, 1 <= width <= 64 - lsb
        //   32-bit: 0 <= lsb <= 31, 1 <= width <= 32 - lsb
        // so that immr (=lsb) and imms (=lsb+width-1) each fit their 6-bit
        // fields ([21:16] / [15:10]). An assembler MUST reject out-of-range
        // immediates rather than silently truncating: today the `as u32`
        // cast wraps negatives into the upper opcode bits, and the OR into
        // the word lets immr>=64 overflow into the N bit [22] and imms>=64
        // overflow into the Rn field [9:5]. width==0 with lsb==0 underflows
        // imms to u32::MAX. The current encoder performs NO range validation,
        // so this property is EXPECTED TO FAIL and documents the bug shared
        // with encode_ubfm/encode_sbfm/encode_bfm.
        #[test]
        fn prop_rejects_out_of_range_immediates(
            is_64 in any::<bool>(),
            bad_lsb in 64u32..=1023u32,
            bad_width in 65u32..=1023u32,
            neg_imm in (-1024i64)..(-1i64),
        ) {
            let mk = |lsb: i64, width: i64, w64: bool| {
                encode_ubfx(&[
                    Operand::Reg(reg_name(0, w64)),
                    Operand::Reg("x1".into()),
                    Operand::Imm(lsb),
                    Operand::Imm(width),
                ])
            };
            // lsb beyond the 6-bit / register-width field must be rejected.
            prop_assert!(mk(bad_lsb as i64, 1, is_64).is_err(),
                "lsb={} (>{}) should be rejected, got {:?}",
                bad_lsb, if is_64 { 63 } else { 31 }, mk(bad_lsb as i64, 1, is_64));
            // width that pushes imms = lsb+width-1 out of range must be rejected.
            prop_assert!(mk(0, bad_width as i64, is_64).is_err(),
                "width={} (imms overflow) should be rejected, got {:?}",
                bad_width, mk(0, bad_width as i64, is_64));
            // width == 0 -> imms = lsb - 1 underflow; must be rejected.
            prop_assert!(mk(0, 0, is_64).is_err(),
                "width=0 (imms underflow) should be rejected, got {:?}",
                mk(0, 0, is_64));
            // negative immediates must be rejected (cast `as u32` wraps today).
            prop_assert!(mk(neg_imm, 1, is_64).is_err(),
                "lsb={} (<0) should be rejected, got {:?}", neg_imm, mk(neg_imm, 1, is_64));
            prop_assert!(mk(1, neg_imm, is_64).is_err(),
                "width={} (<0) should be rejected, got {:?}", neg_imm, mk(1, neg_imm, is_64));
        }
    }
}

#[cfg(test)]
mod prop_encode_ubfm_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Field layout of the UBFM word (ARM ARM, Bitfield encoding) ──────
    //   sf [31] | opc=10 [30:29] | 100110 [28:23] | N [22]
    //   | immr [21:16] | imms [15:10] | Rn [9:5] | Rd [4:0]
    //
    // immr/imms are architecturally 6-bit fields (0..63); N must equal sf
    // (ARM ARM: "CONSTRAINED: N == sf").
    const MASK_SF: u32 = 0x8000_0000;
    const FIXED_30_29: u32 = 0b10 << 29; // 0x4000_0000
    const MASK_30_29: u32 = 0b11 << 29; // 0x6000_0000
    const FIXED_28_23: u32 = 0b100110 << 23; // 0x1300_0000
    const MASK_28_23: u32 = 0b111111 << 23; // 0x1F80_0000
    const MASK_N: u32 = 1 << 22; // 0x0040_0000
    const MASK_IMMR: u32 = 0x003F_0000; // bits [21:16]
    const MASK_IMMS: u32 = 0x0000_FC00; // bits [15:10]
    const MASK_RN: u32 = 0x0000_03E0; // bits [9:5]
    const MASK_RD: u32 = 0x0000_001F; // bits [4:0]

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word(encode_ubfm(ops))
    }

    fn reg_name(num: u32, is_64: bool) -> String {
        if is_64 {
            format!("x{}", num)
        } else {
            format!("w{}", num)
        }
    }

    /// (rd_name, rd_num, rn_name, rn_num, immr, imms, is_64) with immr/imms
    /// constrained to the architecturally-valid 6-bit range.
    fn arb_valid_case() -> impl Strategy<Value = (String, u32, String, u32, u32, u32, bool)> {
        (any::<bool>(), 0u32..=30u32, 0u32..=30u32, 0u32..=63u32, 0u32..=63u32).prop_map(
            |(is_64, rd, rn, immr, imms)| {
                (reg_name(rd, is_64), rd, reg_name(rn, is_64), rn, immr, imms, is_64)
            },
        )
    }

    proptest! {
        // Property A — structural / field-placement oracle.
        // Every fixed opcode bit and every variable field lands exactly
        // where the UBFM encoding mandates; immr/imms reconstruct to inputs.
        #[test]
        fn prop_ubfm_field_placement(c in arb_valid_case()) {
            let (rd_name, rd, rn_name, rn, immr, imms, is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(immr as i64),
                Operand::Imm(imms as i64),
            ];
            let w = enc(&ops);

            prop_assert_eq!(w & MASK_30_29, FIXED_30_29);
            prop_assert_eq!(w & MASK_28_23, FIXED_28_23);
            prop_assert_eq!(w & MASK_SF, if is_64 { MASK_SF } else { 0 });
            prop_assert_eq!(w & MASK_N, if is_64 { MASK_N } else { 0 });
            prop_assert_eq!((w & MASK_IMMR) >> 16, immr);
            prop_assert_eq!((w & MASK_IMMS) >> 10, imms);
            prop_assert_eq!((w & MASK_RN) >> 5, rn);
            prop_assert_eq!(w & MASK_RD, rd);
        }

        // Property B — N == sf invariant (ARM ARM CONSTRAINT for UBFM).
        // Both bits derive solely from the destination register width.
        #[test]
        fn prop_n_equals_sf(c in arb_valid_case()) {
            let (rd_name, _rd, rn_name, _rn, immr, imms, _is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(immr as i64),
                Operand::Imm(imms as i64),
            ];
            let w = enc(&ops);
            prop_assert_eq!((w >> 31) & 1, (w >> 22) & 1);
        }

        // Property C — register-width differential. Encoding x{N} vs w{N}
        // (same reg number, same immr/imms) differs only in sf[31] and N[22].
        #[test]
        fn prop_width_changes_only_sf_and_n(
            num in 0u32..=30u32,
            immr in 0u32..=63u32,
            imms in 0u32..=63u32,
        ) {
            let ops64 = vec![
                Operand::Reg(format!("x{}", num)),
                Operand::Reg("x0".into()),
                Operand::Imm(immr as i64),
                Operand::Imm(imms as i64),
            ];
            let ops32 = vec![
                Operand::Reg(format!("w{}", num)),
                Operand::Reg("w0".into()),
                Operand::Imm(immr as i64),
                Operand::Imm(imms as i64),
            ];
            let diff = enc(&ops64) ^ enc(&ops32);
            prop_assert_eq!(diff, MASK_SF | MASK_N);
        }

        // Property D — determinism. The same operand list always yields the
        // same 32-bit word (encoder is pure).
        #[test]
        fn prop_deterministic(c in arb_valid_case()) {
            let (rd_name, _rd, rn_name, _rn, immr, imms, _is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(immr as i64),
                Operand::Imm(imms as i64),
            ];
            prop_assert_eq!(enc(&ops), enc(&ops));
        }

        // Property E — NEGATIVE CONTRACT (the finding).
        // immr and imms are 6-bit fields ([21:16] / [15:10]); the AArch64
        // UBFM encoding (ARM ARM §C4.1.65 Bitfield) requires 0 <= immr,imms
        // <= 63. An assembler MUST reject out-of-range immediates rather
        // than silently OR-ing garbage into the opcode/N bits. The current
        // `as u32` cast performs NO range validation, so this property is
        // expected to FAIL and documents the bug.
        #[test]
        fn prop_rejects_out_of_range_immediates(
            bad_immr in 64u32..=4095u32,
            bad_imms in 64u32..=4095u32,
            neg_imm in (-4096i64)..(-1i64),
        ) {
            let mk = |immr: i64, imms: i64| {
                encode_ubfm(&[
                    Operand::Reg("x0".into()),
                    Operand::Reg("x1".into()),
                    Operand::Imm(immr),
                    Operand::Imm(imms),
                ])
            };
            prop_assert!(mk(bad_immr as i64, 0).is_err(),
                "immr={} (>63) should be rejected, got {:?}", bad_immr, mk(bad_immr as i64, 0));
            prop_assert!(mk(0, bad_imms as i64).is_err(),
                "imms={} (>63) should be rejected, got {:?}", bad_imms, mk(0, bad_imms as i64));
            prop_assert!(mk(neg_imm, 0).is_err(),
                "immr={} (<0) should be rejected, got {:?}", neg_imm, mk(neg_imm, 0));
            prop_assert!(mk(0, neg_imm).is_err(),
                "imms={} (<0) should be rejected, got {:?}", neg_imm, mk(0, neg_imm));
        }

        // Property F — malformed-operands negative contract (should pass).
        // Missing operands / wrong types in fixed slots must yield Err.
        #[test]
        fn prop_rejects_malformed_operands(
            bad in prop_oneof![Just(0u8), Just(1u8), Just(2u8), Just(3u8), Just(4u8), Just(5u8)],
            n in 0u32..=30u32,
            v in -16i64..=16i64,
        ) {
            let r = match bad {
                0 => encode_ubfm(&[]),
                1 => encode_ubfm(&[
                    Operand::Reg(reg_name(n, true)),
                    Operand::Reg("x1".into()),
                    Operand::Imm(v),
                ]),
                2 => encode_ubfm(&[
                    Operand::Imm(v),
                    Operand::Reg("x1".into()),
                    Operand::Imm(0),
                    Operand::Imm(0),
                ]),
                3 => encode_ubfm(&[
                    Operand::Reg("x0".into()),
                    Operand::Imm(v),
                    Operand::Imm(0),
                    Operand::Imm(0),
                ]),
                4 => encode_ubfm(&[
                    Operand::Reg("x0".into()),
                    Operand::Reg("x1".into()),
                    Operand::Reg(reg_name(n, true)),
                    Operand::Imm(0),
                ]),
                _ => encode_ubfm(&[
                    Operand::Reg("x0".into()),
                    Operand::Reg("x1".into()),
                    Operand::Imm(0),
                    Operand::Reg(reg_name(n, true)),
                ]),
            };
            prop_assert!(r.is_err(), "expected Err, got {:?}", r);
        }
    }
}

#[cfg(test)]
mod prop_encode_sbfm_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Field layout of the SBFM word (ARM ARM, Bitfield encoding) ──────
    //   sf [31] | opc=00 [30:29] | 100110 [28:23] | N [22]
    //   | immr [21:16] | imms [15:10] | Rn [9:5] | Rd [4:0]
    //
    // SBFM is the same shape as UBFM but with opc[30:29]=00 instead of 10
    // (and BFM uses 01). immr/imms are architecturally 6-bit fields (0..63);
    // the ARM ARM constrains N == sf.
    const MASK_SF: u32 = 0x8000_0000;
    const FIXED_30_29: u32 = 0b00 << 29; // SBFM uses opc=00 here
    const MASK_30_29: u32 = 0b11 << 29; // 0x6000_0000
    const FIXED_28_23: u32 = 0b100110 << 23; // 0x1300_0000
    const MASK_28_23: u32 = 0b111111 << 23; // 0x1F80_0000
    const MASK_N: u32 = 1 << 22; // 0x0040_0000
    const MASK_IMMR: u32 = 0x003F_0000; // bits [21:16]
    const MASK_IMMS: u32 = 0x0000_FC00; // bits [15:10]
    const MASK_RN: u32 = 0x0000_03E0; // bits [9:5]
    const MASK_RD: u32 = 0x0000_001F; // bits [4:0]

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word(encode_sbfm(ops))
    }

    fn reg_name(num: u32, is_64: bool) -> String {
        if is_64 {
            format!("x{}", num)
        } else {
            format!("w{}", num)
        }
    }

    /// (rd_name, rd_num, rn_name, rn_num, immr, imms, is_64) with immr/imms
    /// constrained to the architecturally-valid 6-bit range.
    fn arb_valid_case() -> impl Strategy<Value = (String, u32, String, u32, u32, u32, bool)> {
        (any::<bool>(), 0u32..=30u32, 0u32..=30u32, 0u32..=63u32, 0u32..=63u32).prop_map(
            |(is_64, rd, rn, immr, imms)| {
                (reg_name(rd, is_64), rd, reg_name(rn, is_64), rn, immr, imms, is_64)
            },
        )
    }

    /// SBFX/SBFIZ-free valid case where imms >= immr so the SBFX alias maps
    /// cleanly (SBFX lsb=immr, width=imms-immr+1, which needs imms>=immr and
    /// width>=1 i.e. imms>=immr).
    fn arb_sbfx_case() -> impl Strategy<Value = (String, u32, String, u32, u32, u32, bool)> {
        (any::<bool>(), 0u32..=30u32, 0u32..=30u32, 0u32..=62u32).prop_map(
            |(is_64, rd, rn, immr)| {
                let imms = immr + 1; // width = 1, always valid
                (reg_name(rd, is_64), rd, reg_name(rn, is_64), rn, immr, imms, is_64)
            },
        )
    }

    proptest! {
        // Property A — structural / field-placement oracle.
        // Every fixed opcode bit and every variable field lands exactly
        // where the SBFM encoding mandates; immr/imms reconstruct to inputs;
        // opc[30:29] is 00 (distinguishing SBFM from UBFM=10 / BFM=01);
        // and N == sf (ARM ARM constraint).
        #[test]
        fn prop_sbfm_field_placement(c in arb_valid_case()) {
            let (rd_name, rd, rn_name, rn, immr, imms, is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(immr as i64),
                Operand::Imm(imms as i64),
            ];
            let w = enc(&ops);

            prop_assert_eq!(w & MASK_30_29, FIXED_30_29, "SBFM opc[30:29] must be 00");
            prop_assert_eq!(w & MASK_28_23, FIXED_28_23);
            prop_assert_eq!(w & MASK_SF, if is_64 { MASK_SF } else { 0 });
            prop_assert_eq!(w & MASK_N, if is_64 { MASK_N } else { 0 });
            prop_assert_eq!((w & MASK_IMMR) >> 16, immr);
            prop_assert_eq!((w & MASK_IMMS) >> 10, imms);
            prop_assert_eq!((w & MASK_RN) >> 5, rn);
            prop_assert_eq!(w & MASK_RD, rd);
            // N == sf invariant (ARM ARM: constrained N == sf for SBFM).
            prop_assert_eq!((w >> 31) & 1, (w >> 22) & 1);
        }

        // Property B — differential oracle vs the SBFX alias.
        // SBFX Rd, Rn, #lsb, #width is defined as the alias
        //   SBFM Rd, Rn, #lsb, #(lsb+width-1)
        // So for imms >= immr, SBFM(immr, imms) must be bit-identical to
        // SBFX with lsb=immr and width=(imms-immr+1).
        #[test]
        fn prop_sbfm_equals_sbfx_alias(c in arb_sbfx_case()) {
            let (rd_name, _rd, rn_name, _rn, immr, imms, _is_64) = c;
            let width = imms - immr + 1;
            let sbfm = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Reg(rn_name.clone()),
                Operand::Imm(immr as i64),
                Operand::Imm(imms as i64),
            ];
            let sbfx = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(immr as i64),
                Operand::Imm(width as i64),
            ];
            prop_assert_eq!(enc(&sbfm), word(encode_sbfx(&sbfx)));
        }

        // Property C — differential oracle vs sibling UBFM.
        // SBFM and UBFM share an identical encoding template and differ ONLY
        // in opc[30:29]: SBFM=00, UBFM=10. Feeding identical operands must
        // therefore produce words that differ in exactly bit 30.
        #[test]
        fn prop_sbfm_xor_ubfm_is_only_bit_30(c in arb_valid_case()) {
            let (rd_name, _rd, rn_name, _rn, immr, imms, is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(immr as i64),
                Operand::Imm(imms as i64),
            ];
            let _ = is_64;
            let diff = enc(&ops) ^ word(encode_ubfm(&ops));
            prop_assert_eq!(diff, 0x4000_0000, "SBFM ^ UBFM must be exactly bit 30");
        }

        // Property D — register-width differential. Encoding x{N} vs w{N}
        // (same reg number, same immr/imms) differs only in sf[31] and N[22].
        #[test]
        fn prop_width_changes_only_sf_and_n(
            num in 0u32..=30u32,
            immr in 0u32..=63u32,
            imms in 0u32..=63u32,
        ) {
            let ops64 = vec![
                Operand::Reg(format!("x{}", num)),
                Operand::Reg("x0".into()),
                Operand::Imm(immr as i64),
                Operand::Imm(imms as i64),
            ];
            let ops32 = vec![
                Operand::Reg(format!("w{}", num)),
                Operand::Reg("w0".into()),
                Operand::Imm(immr as i64),
                Operand::Imm(imms as i64),
            ];
            let diff = enc(&ops64) ^ enc(&ops32);
            prop_assert_eq!(diff, MASK_SF | MASK_N);
        }

        // Property E — NEGATIVE CONTRACT (the finding).
        // immr and imms are 6-bit fields ([21:16] / [15:10]); the AArch64
        // SBFM encoding (ARM ARM Bitfield) requires 0 <= immr,imms <= 63.
        // An assembler MUST reject out-of-range immediates rather than
        // silently OR-ing the overflow into the N bit (immr=64) or the Rn
        // field (imms=64). The current `as u32` cast performs NO range
        // validation, so this property is EXPECTED TO FAIL and documents
        // the bug shared with encode_ubfm.
        #[test]
        fn prop_rejects_out_of_range_immediates(
            bad_immr in 64u32..=4095u32,
            bad_imms in 64u32..=4095u32,
            neg_imm in (-4096i64)..(-1i64),
        ) {
            let mk = |immr: i64, imms: i64| {
                encode_sbfm(&[
                    Operand::Reg("x0".into()),
                    Operand::Reg("x1".into()),
                    Operand::Imm(immr),
                    Operand::Imm(imms),
                ])
            };
            prop_assert!(mk(bad_immr as i64, 0).is_err(),
                "immr={} (>63) should be rejected, got {:?}", bad_immr, mk(bad_immr as i64, 0));
            prop_assert!(mk(0, bad_imms as i64).is_err(),
                "imms={} (>63) should be rejected, got {:?}", bad_imms, mk(0, bad_imms as i64));
            prop_assert!(mk(neg_imm, 0).is_err(),
                "immr={} (<0) should be rejected, got {:?}", neg_imm, mk(neg_imm, 0));
            prop_assert!(mk(0, neg_imm).is_err(),
                "imms={} (<0) should be rejected, got {:?}", neg_imm, mk(0, neg_imm));
        }

        // Property F — malformed-operands negative contract (should pass).
        // Missing operands / wrong types in fixed slots must yield Err.
        #[test]
        fn prop_rejects_malformed_operands(
            bad in prop_oneof![Just(0u8), Just(1u8), Just(2u8), Just(3u8), Just(4u8), Just(5u8)],
            n in 0u32..=30u32,
            v in -16i64..=16i64,
        ) {
            let r = match bad {
                0 => encode_sbfm(&[]),
                1 => encode_sbfm(&[
                    Operand::Reg(reg_name(n, true)),
                    Operand::Reg("x1".into()),
                    Operand::Imm(v),
                ]),
                2 => encode_sbfm(&[
                    Operand::Imm(v),
                    Operand::Reg("x1".into()),
                    Operand::Imm(0),
                    Operand::Imm(0),
                ]),
                3 => encode_sbfm(&[
                    Operand::Reg("x0".into()),
                    Operand::Imm(v),
                    Operand::Imm(0),
                    Operand::Imm(0),
                ]),
                4 => encode_sbfm(&[
                    Operand::Reg("x0".into()),
                    Operand::Reg("x1".into()),
                    Operand::Reg(reg_name(n, true)),
                    Operand::Imm(0),
                ]),
                _ => encode_sbfm(&[
                    Operand::Reg("x0".into()),
                    Operand::Reg("x1".into()),
                    Operand::Imm(0),
                    Operand::Reg(reg_name(n, true)),
                ]),
            };
            prop_assert!(r.is_err(), "expected Err, got {:?}", r);
        }
    }
}

#[cfg(test)]
mod prop_encode_bfm_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Field layout of the BFM word (ARM ARM, Bitfield encoding) ──────
    //   sf [31] | opc=01 [30:29] | 100110 [28:23] | N [22]
    //   | immr [21:16] | imms [15:10] | Rn [9:5] | Rd [4:0]
    //
    // BFM shares the UBFM/SBFM template but with opc[30:29]=01
    // (SBFM=00, BFM=01, UBFM=10). immr/imms are architecturally 6-bit
    // fields (0..63); the ARM ARM constrains N == sf.
    const MASK_SF: u32 = 0x8000_0000;
    const FIXED_30_29: u32 = 0b01 << 29; // BFM opc=01 → 0x2000_0000
    const MASK_30_29: u32 = 0b11 << 29; // 0x6000_0000
    const FIXED_28_23: u32 = 0b100110 << 23; // 0x1300_0000
    const MASK_28_23: u32 = 0b111111 << 23; // 0x1F80_0000
    const MASK_N: u32 = 1 << 22; // 0x0040_0000
    const MASK_IMMR: u32 = 0x003F_0000; // bits [21:16]
    const MASK_IMMS: u32 = 0x0000_FC00; // bits [15:10]
    const MASK_RN: u32 = 0x0000_03E0; // bits [9:5]
    const MASK_RD: u32 = 0x0000_001F; // bits [4:0]

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word(encode_bfm(ops))
    }

    fn reg_name(num: u32, is_64: bool) -> String {
        if is_64 {
            format!("x{}", num)
        } else {
            format!("w{}", num)
        }
    }

    /// (rd_name, rd_num, rn_name, rn_num, immr, imms, is_64) with immr/imms
    /// constrained to the architecturally-valid 6-bit range.
    fn arb_valid_case() -> impl Strategy<Value = (String, u32, String, u32, u32, u32, bool)> {
        (any::<bool>(), 0u32..=30u32, 0u32..=30u32, 0u32..=63u32, 0u32..=63u32).prop_map(
            |(is_64, rd, rn, immr, imms)| {
                (reg_name(rd, is_64), rd, reg_name(rn, is_64), rn, immr, imms, is_64)
            },
        )
    }

    proptest! {
        // Property A — structural / field-placement oracle.
        // Every fixed opcode bit and every variable field lands exactly
        // where the BFM encoding mandates; opc[30:29] is 01 (distinguishing
        // BFM from SBFM=00 / UBFM=10); immr/imms reconstruct to inputs;
        // and N == sf (ARM ARM constraint).
        #[test]
        fn prop_bfm_field_placement(c in arb_valid_case()) {
            let (rd_name, rd, rn_name, rn, immr, imms, is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(immr as i64),
                Operand::Imm(imms as i64),
            ];
            let w = enc(&ops);

            prop_assert_eq!(w & MASK_30_29, FIXED_30_29, "BFM opc[30:29] must be 01");
            prop_assert_eq!(w & MASK_28_23, FIXED_28_23);
            prop_assert_eq!(w & MASK_SF, if is_64 { MASK_SF } else { 0 });
            prop_assert_eq!(w & MASK_N, if is_64 { MASK_N } else { 0 });
            prop_assert_eq!((w & MASK_IMMR) >> 16, immr);
            prop_assert_eq!((w & MASK_IMMS) >> 10, imms);
            prop_assert_eq!((w & MASK_RN) >> 5, rn);
            prop_assert_eq!(w & MASK_RD, rd);
            prop_assert_eq!((w >> 31) & 1, (w >> 22) & 1);
        }

        // Property B — differential oracle vs sibling encoders.
        // BFM, SBFM and UBFM share an identical encoding template and differ
        // ONLY in opc[30:29]: BFM=01, SBFM=00, UBFM=10. Feeding identical
        // operands must therefore produce words whose XOR is exactly the
        // opc field: BFM ^ UBFM = 0b11<<29 = 0x6000_0000; BFM ^ SBFM = 0b01<<29.
        #[test]
        fn prop_bfm_xor_siblings(c in arb_valid_case()) {
            let (rd_name, _rd, rn_name, _rn, immr, imms, _is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(immr as i64),
                Operand::Imm(imms as i64),
            ];
            let bfm = enc(&ops);
            let ubfm = word(encode_ubfm(&ops));
            let sbfm = word(encode_sbfm(&ops));
            prop_assert_eq!(bfm ^ ubfm, 0x6000_0000, "BFM ^ UBFM must be exactly bits [30:29]");
            prop_assert_eq!(bfm ^ sbfm, 0x2000_0000, "BFM ^ SBFM must be exactly bit 29");
        }

        // Property C — differential oracle vs the BFXIL alias.
        // BFXIL Rd, Rn, #lsb, #width is defined as the alias
        //   BFM Rd, Rn, #lsb, #(lsb+width-1)
        // So for valid lsb/width, BFM(immr=lsb, imms=lsb+width-1) must be
        // bit-identical to BFXIL(lsb, width).
        #[test]
        fn prop_bfm_equals_bfxil_alias(
            rd in 0u32..=30u32,
            rn in 0u32..=30u32,
            is_64 in any::<bool>(),
            lsb in 0u32..=62u32,
            width in 1u32..=63u32,
        ) {
            let max = if is_64 { 64 } else { 32 };
            prop_assume!(lsb < max && lsb + width <= max, "within regsize");
            let imms = lsb + width - 1;
            let bfm = vec![
                Operand::Reg(reg_name(rd, is_64)),
                Operand::Reg(reg_name(rn, is_64)),
                Operand::Imm(lsb as i64),
                Operand::Imm(imms as i64),
            ];
            let bfxil = vec![
                Operand::Reg(reg_name(rd, is_64)),
                Operand::Reg(reg_name(rn, is_64)),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            prop_assert_eq!(enc(&bfm), word(encode_bfxil(&bfxil)));
        }

        // Property D — register-width differential. Encoding x{N} vs w{N}
        // (same reg number, same immr/imms) differs only in sf[31] and N[22].
        #[test]
        fn prop_width_changes_only_sf_and_n(
            num in 0u32..=30u32,
            immr in 0u32..=63u32,
            imms in 0u32..=63u32,
        ) {
            let ops64 = vec![
                Operand::Reg(format!("x{}", num)),
                Operand::Reg("x0".into()),
                Operand::Imm(immr as i64),
                Operand::Imm(imms as i64),
            ];
            let ops32 = vec![
                Operand::Reg(format!("w{}", num)),
                Operand::Reg("w0".into()),
                Operand::Imm(immr as i64),
                Operand::Imm(imms as i64),
            ];
            let diff = enc(&ops64) ^ enc(&ops32);
            prop_assert_eq!(diff, MASK_SF | MASK_N);
        }

        // Property E — NEGATIVE CONTRACT (the finding).
        // immr and imms are 6-bit fields ([21:16] / [15:10]); the AArch64
        // BFM encoding (ARM ARM Bitfield) requires 0 <= immr,imms <= 63.
        // An assembler MUST reject out-of-range immediates rather than
        // silently OR-ing the overflow into the N bit (immr=64) or the Rn
        // field (imms=64), and must reject negatives rather than letting
        // the `as u32` cast wrap into the upper opcode bits. The current
        // cast performs NO range validation, so this property is EXPECTED
        // TO FAIL and documents the bug shared with encode_ubfm/sbfm.
        #[test]
        fn prop_rejects_out_of_range_immediates(
            bad_immr in 64u32..=4095u32,
            bad_imms in 64u32..=4095u32,
            neg_imm in (-4096i64)..(-1i64),
        ) {
            let mk = |immr: i64, imms: i64| {
                encode_bfm(&[
                    Operand::Reg("x0".into()),
                    Operand::Reg("x1".into()),
                    Operand::Imm(immr),
                    Operand::Imm(imms),
                ])
            };
            prop_assert!(mk(bad_immr as i64, 0).is_err(),
                "immr={} (>63) should be rejected, got {:?}", bad_immr, mk(bad_immr as i64, 0));
            prop_assert!(mk(0, bad_imms as i64).is_err(),
                "imms={} (>63) should be rejected, got {:?}", bad_imms, mk(0, bad_imms as i64));
            prop_assert!(mk(neg_imm, 0).is_err(),
                "immr={} (<0) should be rejected, got {:?}", neg_imm, mk(neg_imm, 0));
            prop_assert!(mk(0, neg_imm).is_err(),
                "imms={} (<0) should be rejected, got {:?}", neg_imm, mk(0, neg_imm));
        }

        // Property F — malformed-operands negative contract (should pass).
        // Missing operands / wrong types in fixed slots must yield Err.
        #[test]
        fn prop_rejects_malformed_operands(
            bad in prop_oneof![Just(0u8), Just(1u8), Just(2u8), Just(3u8), Just(4u8), Just(5u8)],
            n in 0u32..=30u32,
            v in -16i64..=16i64,
        ) {
            let r = match bad {
                0 => encode_bfm(&[]),
                1 => encode_bfm(&[
                    Operand::Reg(reg_name(n, true)),
                    Operand::Reg("x1".into()),
                    Operand::Imm(v),
                ]),
                2 => encode_bfm(&[
                    Operand::Imm(v),
                    Operand::Reg("x1".into()),
                    Operand::Imm(0),
                    Operand::Imm(0),
                ]),
                3 => encode_bfm(&[
                    Operand::Reg("x0".into()),
                    Operand::Imm(v),
                    Operand::Imm(0),
                    Operand::Imm(0),
                ]),
                4 => encode_bfm(&[
                    Operand::Reg("x0".into()),
                    Operand::Reg("x1".into()),
                    Operand::Reg(reg_name(n, true)),
                    Operand::Imm(0),
                ]),
                _ => encode_bfm(&[
                    Operand::Reg("x0".into()),
                    Operand::Reg("x1".into()),
                    Operand::Imm(0),
                    Operand::Reg(reg_name(n, true)),
                ]),
            };
            prop_assert!(r.is_err(), "expected Err, got {:?}", r);
        }
    }
}

#[cfg(test)]
mod prop_encode_sbfx_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Field layout of the SBFM (SBFX alias) word (ARM ARM §C4.1.69) ────
    //   sf [31] | opc=00 [30:29] | 100110 [28:23] | N [22]
    //   | immr [21:16] | imms [15:10] | Rn [9:5] | Rd [4:0]
    //
    // SBFX Rd, Rn, #lsb, #width is the alias  SBFM Rd, Rn, #lsb, #(lsb+width-1)
    //   so immr == lsb and imms == lsb + width - 1.
    // ARM ARM operand constraints:
    //   64-bit: 0 <= lsb <= 63, 1 <= width <= 64 - lsb
    //   32-bit: 0 <= lsb <= 31, 1 <= width <= 32 - lsb
    // and the encoding constrains N == sf.
    const MASK_SF: u32 = 0x8000_0000;
    const FIXED_30_29: u32 = 0b00 << 29; // SBFM opc=00 -> 0x0000_0000
    const MASK_30_29: u32 = 0b11 << 29; // 0x6000_0000
    const FIXED_28_23: u32 = 0b100110 << 23; // 0x1300_0000
    const MASK_28_23: u32 = 0b111111 << 23; // 0x1F80_0000
    const MASK_N: u32 = 1 << 22; // 0x0040_0000
    const MASK_IMMR: u32 = 0x003F_0000; // bits [21:16]
    const MASK_IMMS: u32 = 0x0000_FC00; // bits [15:10]
    const MASK_RN: u32 = 0x0000_03E0; // bits [9:5]
    const MASK_RD: u32 = 0x0000_001F; // bits [4:0]

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word(encode_sbfx(ops))
    }

    fn reg_name(num: u32, is_64: bool) -> String {
        if is_64 {
            format!("x{}", num)
        } else {
            format!("w{}", num)
        }
    }

    /// (rd_name, rd_num, rn_name, rn_num, lsb, width, is_64) with lsb/width
    /// constrained to the architecturally-valid ranges so immr/imms fit their
    /// 6-bit fields without wrapping.
    fn arb_valid_case() -> impl Strategy<Value = (String, u32, String, u32, u32, u32, bool)> {
        (any::<bool>(), 0u32..=30u32, 0u32..=30u32, 0u32..=63u32, 1u32..=64u32)
            .prop_filter(
                "lsb+width must fit regsize",
                |&(is_64, _rd, _rn, lsb, width)| {
                    let max = if is_64 { 64 } else { 32 };
                    lsb < max && width >= 1 && lsb + width <= max
                },
            )
            .prop_map(|(is_64, rd, rn, lsb, width)| {
                (reg_name(rd, is_64), rd, reg_name(rn, is_64), rn, lsb, width, is_64)
            })
    }

    /// Broad (possibly out-of-range) lsb/width for differential tests where
    /// identical wrapping behaviour is what we compare.
    fn arb_broad_case() -> impl Strategy<Value = (String, u32, String, u32, u32, u32, bool)> {
        (any::<bool>(), 0u32..=30u32, 0u32..=30u32, 0u32..=63u32, 1u32..=64u32).prop_map(
            |(is_64, rd, rn, lsb, width)| {
                (reg_name(rd, is_64), rd, reg_name(rn, is_64), rn, lsb, width, is_64)
            },
        )
    }

    proptest! {
        // Property A — structural / field-placement oracle.
        // Every fixed opcode bit and every variable field lands in its
        // mandated position; opc[30:29]=00 (the SBFM opcode that distinguishes
        // SBFX from UBFX=10 / BFXIL=01); immr==lsb and imms==lsb+width-1;
        // N == sf (ARM ARM constraint).
        #[test]
        fn prop_sbfx_field_placement(c in arb_valid_case()) {
            let (rd_name, rd, rn_name, rn, lsb, width, is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let w = enc(&ops);

            prop_assert_eq!(w & MASK_30_29, FIXED_30_29, "SBFX opc[30:29] must be 00");
            prop_assert_eq!(w & MASK_28_23, FIXED_28_23);
            prop_assert_eq!(w & MASK_SF, if is_64 { MASK_SF } else { 0 });
            prop_assert_eq!(w & MASK_N, if is_64 { MASK_N } else { 0 });
            prop_assert_eq!((w & MASK_IMMR) >> 16, lsb);
            prop_assert_eq!((w & MASK_IMMS) >> 10, lsb + width - 1);
            prop_assert_eq!((w & MASK_RN) >> 5, rn);
            prop_assert_eq!(w & MASK_RD, rd);
            // N == sf invariant (ARM ARM: constrained N == sf for SBFM).
            prop_assert_eq!((w >> 31) & 1, (w >> 22) & 1);
        }

        // Property B — differential oracle vs the raw SBFM encoder.
        // SBFX Rd, Rn, #lsb, #width is defined as the alias
        //   SBFM Rd, Rn, #lsb, #(lsb+width-1).
        // Feeding both encoders the alias-equivalent operands must yield a
        // bit-identical word. Holds even for out-of-range lsb/width because
        // both encoders wrap identically (same `as u32` + same arithmetic).
        #[test]
        fn prop_sbfx_equals_sbfm_alias(c in arb_broad_case()) {
            let (rd_name, _rd, rn_name, _rn, lsb, width, _is_64) = c;
            let sbfx = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Reg(rn_name.clone()),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let sbfm = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(lsb as i64),                 // immr = lsb
                Operand::Imm((lsb + width - 1) as i64),   // imms = lsb+width-1
            ];
            prop_assert_eq!(enc(&sbfx), word(encode_sbfm(&sbfm)));
        }

        // Property C — register-width differential. Encoding with x{N} vs w{N}
        // (same numeric register, same lsb/width) must differ in exactly the
        // sf bit [31] and the N bit [22], and nowhere else.
        #[test]
        fn prop_width_changes_only_sf_and_n(
            num in 0u32..=30u32,
            lsb in 0u32..=31u32,
            width in 1u32..=32u32,
        ) {
            let ops64 = vec![
                Operand::Reg(format!("x{}", num)),
                Operand::Reg("x0".into()),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let ops32 = vec![
                Operand::Reg(format!("w{}", num)),
                Operand::Reg("w0".into()),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let diff = enc(&ops64) ^ enc(&ops32);
            prop_assert_eq!(diff, MASK_SF | MASK_N);
        }

        // Property D — determinism. The encoder is pure: the same operand
        // list always yields the same 32-bit word.
        #[test]
        fn prop_deterministic(c in arb_valid_case()) {
            let (rd_name, _rd, rn_name, _rn, lsb, width, _is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            prop_assert_eq!(enc(&ops), enc(&ops));
        }

        // Property E — NEGATIVE CONTRACT (the finding).
        // SBFX <Xd>,<Xn>,#<lsb>,#<width>: ARM ARM §C4.1.69 constrains
        //   64-bit: 0 <= lsb <= 63, 1 <= width <= 64 - lsb
        //   32-bit: 0 <= lsb <= 31, 1 <= width <= 32 - lsb
        // An assembler MUST reject out-of-range lsb/width rather than
        // silently OR-ing overflow into the N/Rn/Rd fields — and notably the
        // `imms = lsb + width - 1` underflows (wraps to ~0) when width == 0.
        // The current `as u32` cast performs NO range validation, so this
        // property is EXPECTED TO FAIL and documents the missing contract.
        #[test]
        fn prop_rejects_out_of_range_lsb_width(
            is_64 in any::<bool>(),
            big_lsb in 64u32..=4095u32,
            big_width in 65u32..=4095u32,
            neg in (-4096i64)..(-1i64),
        ) {
            let max = if is_64 { 64 } else { 32 };
            let over_lsb = max as u32;          // lsb == regsize (out of range)
            let over_width = (max + 1) as u32;  // width > regsize
            let mk = |lsb: i64, width: i64| {
                encode_sbfx(&[
                    Operand::Reg(reg_name(0, is_64)),
                    Operand::Reg(reg_name(1, is_64)),
                    Operand::Imm(lsb),
                    Operand::Imm(width),
                ])
            };
            // width == 0 -> imms underflow
            prop_assert!(mk(0, 0).is_err(), "width=0 must be rejected, got {:?}", mk(0, 0));
            // lsb == regsize
            prop_assert!(mk(over_lsb as i64, 1).is_err(),
                "lsb={} must be rejected, got {:?}", over_lsb, mk(over_lsb as i64, 1));
            // width > regsize
            prop_assert!(mk(0, over_width as i64).is_err(),
                "width={} must be rejected, got {:?}", over_width, mk(0, over_width as i64));
            // large lsb / width
            prop_assert!(mk(big_lsb as i64, 1).is_err(),
                "lsb={} must be rejected, got {:?}", big_lsb, mk(big_lsb as i64, 1));
            prop_assert!(mk(0, big_width as i64).is_err(),
                "width={} must be rejected, got {:?}", big_width, mk(0, big_width as i64));
            // negative lsb / width
            prop_assert!(mk(neg, 1).is_err(), "lsb={} must be rejected, got {:?}", neg, mk(neg, 1));
            prop_assert!(mk(0, neg).is_err(), "width={} must be rejected, got {:?}", neg, mk(0, neg));
        }
    }
}

#[cfg(test)]
mod prop_encode_bfi_tests {
    use super::*;
    use proptest::prelude::*;
    use std::panic;

    // ── Field layout of the BFM (BFI alias) word (ARM ARM, Bitfield) ────
    //   sf [31] | opc=01 [30:29] | 100110 [28:23] | N [22]
    //   | immr [21:16] | imms [15:10] | Rn [9:5] | Rd [4:0]
    //
    // BFI Rd, Rn, #lsb, #width is the alias
    //   BFM Rd, Rn, #(-lsb MOD regsize), #(width-1)
    // so immr == (regsize - lsb) % regsize and imms == width - 1.
    const MASK_SF: u32 = 0x8000_0000;
    const FIXED_30_29: u32 = 0b01 << 29; // BFM opc=01 -> 0x2000_0000
    const MASK_30_29: u32 = 0b11 << 29; // 0x6000_0000
    const FIXED_28_23: u32 = 0b100110 << 23; // 0x1300_0000
    const MASK_28_23: u32 = 0b111111 << 23; // 0x1F80_0000
    const MASK_N: u32 = 1 << 22; // 0x0040_0000
    const MASK_IMMR: u32 = 0x003F_0000; // bits [21:16]
    const MASK_IMMS: u32 = 0x0000_FC00; // bits [15:10]
    const MASK_RN: u32 = 0x0000_03E0; // bits [9:5]
    const MASK_RD: u32 = 0x0000_001F; // bits [4:0]

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word(encode_bfi(ops))
    }

    fn reg_name(num: u32, is_64: bool) -> String {
        if is_64 {
            format!("x{}", num)
        } else {
            format!("w{}", num)
        }
    }

    /// (rd_name, rd_num, rn_name, rn_num, lsb, width, is_64) with lsb/width
    /// constrained to the architecturally-valid ranges (ARM ARM BFI):
    ///   64-bit: 0 <= lsb <= 63, 1 <= width <= 64 - lsb
    ///   32-bit: 0 <= lsb <= 31, 1 <= width <= 32 - lsb
    fn arb_valid_case() -> impl Strategy<Value = (String, u32, String, u32, u32, u32, bool)> {
        (any::<bool>(), 0u32..=30u32, 0u32..=30u32, 0u32..=63u32, 1u32..=64u32)
            .prop_filter(
                "lsb+width must fit regsize",
                |&(is_64, _rd, _rn, lsb, width)| {
                    let max = if is_64 { 64 } else { 32 };
                    lsb < max && width >= 1 && lsb + width <= max
                },
            )
            .prop_map(|(is_64, rd, rn, lsb, width)| {
                (reg_name(rd, is_64), rd, reg_name(rn, is_64), rn, lsb, width, is_64)
            })
    }

    proptest! {
        // Property A — structural / field-placement oracle.
        // Every fixed opcode bit and variable field lands in its mandated
        // position; opc[30:29]=01 (the BFM opcode shared by BFI/BFXIL);
        // immr/imms reconstruct to the alias-computed values; N == sf.
        #[test]
        fn prop_bfi_field_placement(c in arb_valid_case()) {
            let (rd_name, rd, rn_name, rn, lsb, width, is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let w = enc(&ops);
            let reg_width: u32 = if is_64 { 64 } else { 32 };
            let expected_immr = (reg_width - lsb) % reg_width;
            let expected_imms = width - 1;

            prop_assert_eq!(w & MASK_30_29, FIXED_30_29, "BFI/BFM opc[30:29] must be 01");
            prop_assert_eq!(w & MASK_28_23, FIXED_28_23);
            prop_assert_eq!(w & MASK_SF, if is_64 { MASK_SF } else { 0 });
            prop_assert_eq!(w & MASK_N, if is_64 { MASK_N } else { 0 });
            prop_assert_eq!((w & MASK_IMMR) >> 16, expected_immr);
            prop_assert_eq!((w & MASK_IMMS) >> 10, expected_imms);
            prop_assert_eq!((w & MASK_RN) >> 5, rn);
            prop_assert_eq!(w & MASK_RD, rd);
            // N == sf invariant (ARM ARM: constrained N == sf for BFM).
            prop_assert_eq!((w >> 31) & 1, (w >> 22) & 1);
        }

        // Property B — differential oracle vs the raw BFM encoder.
        // BFI Rd, Rn, #lsb, #width is the alias
        //   BFM Rd, Rn, #(-lsb MOD regsize), #(width-1).
        // Feeding both encoders the alias-equivalent operands must yield a
        // bit-identical word.
        #[test]
        fn prop_bfi_equals_bfm_alias(c in arb_valid_case()) {
            let (rd_name, _rd, rn_name, _rn, lsb, width, is_64) = c;
            let reg_width: u32 = if is_64 { 64 } else { 32 };
            let immr = (reg_width - lsb) % reg_width;
            let imms = width - 1;
            let bfi = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Reg(rn_name.clone()),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let bfm = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(immr as i64),
                Operand::Imm(imms as i64),
            ];
            prop_assert_eq!(enc(&bfi), word(encode_bfm(&bfm)));
        }

        // Property C — register-width differential. Encoding x{N} vs w{N}
        // (same numeric register, same lsb/width) must differ ONLY in the
        // sf bit [31], the N bit [22], and the immr field [21:16] — because
        // immr = (-lsb) MOD regsize is regsize-dependent (unlike raw BFM
        // where immr is passed through verbatim). imms, Rn and Rd are
        // identical. Constrained to lsb/width valid in BOTH widths.
        #[test]
        fn prop_width_changes_sf_n_and_immr_only(
            num in 0u32..=30u32,
            lsb in 0u32..=31u32,
            width in 1u32..=32u32,
        ) {
            let ops64 = vec![
                Operand::Reg(format!("x{}", num)),
                Operand::Reg("x0".into()),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let ops32 = vec![
                Operand::Reg(format!("w{}", num)),
                Operand::Reg("w0".into()),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let diff = enc(&ops64) ^ enc(&ops32);
            let immr64 = (64u32 - lsb) % 64;
            let immr32 = (32u32 - lsb) % 32;
            let expected = MASK_SF | MASK_N | (((immr64 ^ immr32) << 16) & MASK_IMMR);
            prop_assert_eq!(diff, expected);
            // And the fields that must NOT change:
            prop_assert_eq!(diff & MASK_IMMS, 0, "imms must be identical");
            prop_assert_eq!(diff & MASK_RN, 0, "Rn must be identical");
            prop_assert_eq!(diff & MASK_RD, 0, "Rd must be identical");
        }

        // Property D — determinism. The encoder is pure: the same operand
        // list always yields the same 32-bit word.
        #[test]
        fn prop_deterministic(c in arb_valid_case()) {
            let (rd_name, _rd, rn_name, _rn, lsb, width, _is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            prop_assert_eq!(enc(&ops), enc(&ops));
        }

        // Property E — NEGATIVE CONTRACT / PANIC RISK (the finding).
        // BFI <Xd>,<Xn>,#<lsb>,#<width> (ARM ARM BFI) constrains
        //   64-bit: 0 <= lsb <= 63, 1 <= width <= 64 - lsb
        //   32-bit: 0 <= lsb <= 31, 1 <= width <= 32 - lsb
        // An assembler MUST reject out-of-range operands with a clean Err
        // rather than (a) silently encoding garbage or (b) PANICKING. The
        // current encode_bfi computes `imms = width - 1` (panics in debug
        // when width == 0) and `immr = (reg_width - lsb) % reg_width`
        // (panics on subtraction underflow when lsb > reg_width), and
        // performs NO range validation of its own. This property is
        // EXPECTED TO FAIL and documents both the missing validation and
        // the panic risk shared with the sibling BFM-family encoders.
        #[test]
        fn prop_rejects_out_of_range_operands(
            is_64 in any::<bool>(),
            over_lsb in 64u32..=1023u32,
            over_width in 65u32..=1023u32,
            neg in (-1024i64)..(-1i64),
        ) {
            let reg_width: u32 = if is_64 { 64 } else { 32 };
            // Each (lsb, width, label) is an out-of-range operand the encoder
            // MUST reject with a clean Err (not a panic, not a silent Ok word).
            let cases: &[(i64, i64, &str)] = &[
                (0, 0, "width=0 (imms underflow)"),
                (reg_width as i64, 1, "lsb==regsize"),
                (over_lsb as i64, 1, "lsb>regsize (immr underflow)"),
                ((reg_width - 1) as i64, 2, "lsb+width>regsize"),
                (0, over_width as i64, "width>regsize"),
                (neg, 1, "negative lsb"),
                (0, neg, "negative width"),
            ];
            for &(lsb, width, label) in cases {
                let ops = vec![
                    Operand::Reg(reg_name(0, is_64)),
                    Operand::Reg(reg_name(1, is_64)),
                    Operand::Imm(lsb),
                    Operand::Imm(width),
                ];
                // Catch panics so a panic is reported as a contract failure
                // instead of aborting the proptest run.
                // (Note: the default panic hook still prints the debug underflow
                // message to stderr; this is harmless noise from proptest
                // shrinking the failing input.)
                let got = panic::catch_unwind(panic::AssertUnwindSafe(|| encode_bfi(&ops)));
                match got {
                    Ok(Ok(w)) => prop_assert!(false,
                        "{}: lsb={} width={} should be Err, got Ok({:?})",
                        label, lsb, width, w),
                    Ok(Err(_)) => {} // clean rejection: good
                    Err(_) => prop_assert!(false,
                        "{}: lsb={} width={} should be Err but PANICKED",
                        label, lsb, width),
                }
            }
        }

        // Property F — malformed-operands negative contract (should pass).
        // Missing operands / wrong types in fixed slots must yield Err.
        #[test]
        fn prop_rejects_malformed_operands(
            bad in prop_oneof![Just(0u8), Just(1u8), Just(2u8), Just(3u8), Just(4u8), Just(5u8)],
            n in 0u32..=30u32,
            v in -16i64..=16i64,
        ) {
            let r = match bad {
                0 => encode_bfi(&[]),
                1 => encode_bfi(&[
                    Operand::Reg(reg_name(n, true)),
                    Operand::Reg("x1".into()),
                    Operand::Imm(v),
                ]),
                2 => encode_bfi(&[
                    Operand::Imm(v),
                    Operand::Reg("x1".into()),
                    Operand::Imm(0),
                    Operand::Imm(1),
                ]),
                3 => encode_bfi(&[
                    Operand::Reg("x0".into()),
                    Operand::Imm(v),
                    Operand::Imm(0),
                    Operand::Imm(1),
                ]),
                4 => encode_bfi(&[
                    Operand::Reg("x0".into()),
                    Operand::Reg("x1".into()),
                    Operand::Reg(reg_name(n, true)),
                    Operand::Imm(1),
                ]),
                _ => encode_bfi(&[
                    Operand::Reg("x0".into()),
                    Operand::Reg("x1".into()),
                    Operand::Imm(0),
                    Operand::Reg(reg_name(n, true)),
                ]),
            };
            prop_assert!(r.is_err(), "expected Err, got {:?}", r);
        }
    }
}

#[cfg(test)]
mod prop_encode_bfxil_tests {
    use super::*;
    use proptest::prelude::*;
    use std::panic;

    // ── Field layout of the BFM (BFXIL alias) word (ARM ARM, Bitfield) ───
    //   sf [31] | opc=01 [30:29] | 100110 [28:23] | N [22]
    //   | immr [21:16] | imms [15:10] | Rn [9:5] | Rd [4:0]
    //
    // BFXIL Rd, Rn, #lsb, #width is the alias
    //   BFM Rd, Rn, #lsb, #(lsb+width-1)
    // so immr == lsb and imms == lsb + width - 1.
    // ARM ARM operand constraints (BFXIL):
    //   64-bit: 0 <= lsb <= 63, 1 <= width <= 64 - lsb
    //   32-bit: 0 <= lsb <= 31, 1 <= width <= 32 - lsb
    // and the encoding constrains N == sf.
    const MASK_SF: u32 = 0x8000_0000;
    const FIXED_30_29: u32 = 0b01 << 29; // BFM opc=01 -> 0x2000_0000
    const MASK_30_29: u32 = 0b11 << 29; // 0x6000_0000
    const FIXED_28_23: u32 = 0b100110 << 23; // 0x1300_0000
    const MASK_28_23: u32 = 0b111111 << 23; // 0x1F80_0000
    const MASK_N: u32 = 1 << 22; // 0x0040_0000
    const MASK_IMMR: u32 = 0x003F_0000; // bits [21:16]
    const MASK_IMMS: u32 = 0x0000_FC00; // bits [15:10]
    const MASK_RN: u32 = 0x0000_03E0; // bits [9:5]
    const MASK_RD: u32 = 0x0000_001F; // bits [4:0]

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word(encode_bfxil(ops))
    }

    fn reg_name(num: u32, is_64: bool) -> String {
        if is_64 {
            format!("x{}", num)
        } else {
            format!("w{}", num)
        }
    }

    /// (rd_name, rd_num, rn_name, rn_num, lsb, width, is_64) with lsb/width
    /// constrained to the architecturally-valid ranges (ARM ARM BFXIL) so
    /// that immr (=lsb) and imms (=lsb+width-1) each fit their 6-bit fields
    /// without wrapping or underflowing.
    fn arb_valid_case() -> impl Strategy<Value = (String, u32, String, u32, u32, u32, bool)> {
        (any::<bool>(), 0u32..=30u32, 0u32..=30u32, 0u32..=63u32, 1u32..=64u32)
            .prop_filter(
                "lsb+width must fit regsize",
                |&(is_64, _rd, _rn, lsb, width)| {
                    let max = if is_64 { 64 } else { 32 };
                    lsb < max && width >= 1 && lsb + width <= max
                },
            )
            .prop_map(|(is_64, rd, rn, lsb, width)| {
                (reg_name(rd, is_64), rd, reg_name(rn, is_64), rn, lsb, width, is_64)
            })
    }

    proptest! {
        // Property A — structural / field-placement oracle.
        // Every fixed opcode bit and every variable field lands in its
        // mandated position; opc[30:29]=01 (the BFM opcode shared by
        // BFI/BFXIL, distinguishing it from SBFX=00 / UBFX=10);
        // immr reconstructs to lsb, imms reconstructs to lsb+width-1;
        // and N == sf (ARM ARM constraint).
        #[test]
        fn prop_bfxil_field_placement(c in arb_valid_case()) {
            let (rd_name, rd, rn_name, rn, lsb, width, is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let w = enc(&ops);

            prop_assert_eq!(w & MASK_30_29, FIXED_30_29, "BFXIL/BFM opc[30:29] must be 01");
            prop_assert_eq!(w & MASK_28_23, FIXED_28_23);
            prop_assert_eq!(w & MASK_SF, if is_64 { MASK_SF } else { 0 });
            prop_assert_eq!(w & MASK_N, if is_64 { MASK_N } else { 0 });
            prop_assert_eq!((w & MASK_IMMR) >> 16, lsb);
            prop_assert_eq!((w & MASK_IMMS) >> 10, lsb + width - 1);
            prop_assert_eq!((w & MASK_RN) >> 5, rn);
            prop_assert_eq!(w & MASK_RD, rd);
            // N == sf invariant (ARM ARM: constrained N == sf for BFM).
            prop_assert_eq!((w >> 31) & 1, (w >> 22) & 1);
        }

        // Property B — differential oracle vs the raw BFM encoder.
        // BFXIL Rd, Rn, #lsb, #width is defined as the alias
        //   BFM Rd, Rn, #lsb, #(lsb+width-1).
        // Feeding BFXIL(lsb, width) and BFM(lsb, lsb+width-1) the same
        // Rd/Rn must yield a bit-identical word.
        #[test]
        fn prop_bfxil_equals_bfm_alias(c in arb_valid_case()) {
            let (rd_name, _rd, rn_name, _rn, lsb, width, _is_64) = c;
            let imms = lsb + width - 1;
            let bfxil = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Reg(rn_name.clone()),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let bfm = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(lsb as i64),
                Operand::Imm(imms as i64),
            ];
            prop_assert_eq!(enc(&bfxil), word(encode_bfm(&bfm)));
        }

        // Property C — register-width differential. Encoding x{N} vs w{N}
        // (same numeric register, same lsb/width) must differ ONLY in the
        // sf bit [31] and the N bit [22], and nowhere else — because for
        // BFXIL immr == lsb and imms == lsb+width-1 are passed through
        // verbatim and do NOT depend on regsize (unlike BFI/BFIZ where
        // immr = -lsb MOD regsize). Constrained to lsb/width valid in both
        // widths so neither width wraps its 6-bit field.
        #[test]
        fn prop_width_changes_only_sf_and_n(
            num in 0u32..=30u32,
            lsb in 0u32..=31u32,
            width in 1u32..=32u32,
        ) {
            let ops64 = vec![
                Operand::Reg(format!("x{}", num)),
                Operand::Reg("x0".into()),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let ops32 = vec![
                Operand::Reg(format!("w{}", num)),
                Operand::Reg("w0".into()),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let diff = enc(&ops64) ^ enc(&ops32);
            prop_assert_eq!(diff, MASK_SF | MASK_N);
        }

        // Property D — differential oracle vs sibling extract aliases.
        // BFXIL (BFM, opc=01), SBFX (SBFM, opc=00) and UBFX (UBFM, opc=10)
        // share an identical encoding template and differ ONLY in opc[30:29].
        // Feeding the three encoders the SAME lsb/width operands must yield
        // words whose pairwise XOR is exactly the opc field:
        //   BFXIL ^ UBFX = 0b11<<29 = 0x6000_0000
        //   BFXIL ^ SBFX = 0b01<<29 = 0x2000_0000
        #[test]
        fn prop_bfxil_xor_siblings(c in arb_valid_case()) {
            let (rd_name, _rd, rn_name, _rn, lsb, width, _is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let bfxil = enc(&ops);
            let ubfx = word(encode_ubfx(&ops));
            let sbfx = word(encode_sbfx(&ops));
            prop_assert_eq!(bfxil ^ ubfx, 0x6000_0000, "BFXIL ^ UBFX must be exactly bits [30:29]");
            prop_assert_eq!(bfxil ^ sbfx, 0x2000_0000, "BFXIL ^ SBFX must be exactly bit 29");
        }

        // Property E — NEGATIVE CONTRACT / PANIC RISK (the finding).
        // BFXIL <Xd>,<Xn>,#<lsb>,#<width> (ARM ARM BFXIL) constrains
        //   64-bit: 0 <= lsb <= 63, 1 <= width <= 64 - lsb
        //   32-bit: 0 <= lsb <= 31, 1 <= width <= 32 - lsb
        // An assembler MUST reject out-of-range operands with a clean Err
        // rather than (a) silently encoding garbage or (b) PANICKING. The
        // current encode_bfxil computes `imms = lsb + width - 1`, which
        // underflows and PANICS in debug builds when lsb == 0 && width == 0
        // (and `lsb + width` can overflow u32 for huge immediates), and it
        // performs NO range validation of its own — so out-of-range lsb/width
        // get silently OR-ed into the N bit (immr=lsb>=64) or the Rn field
        // (imms>=64), and negatives wrap via the `as u32` cast. This
        // property is EXPECTED TO FAIL and documents both the missing
        // validation and the panic risk shared with the BFM-family encoders.
        #[test]
        fn prop_rejects_out_of_range_operands(
            is_64 in any::<bool>(),
            over_lsb in 64u32..=1023u32,
            over_width in 65u32..=1023u32,
            neg in (-1024i64)..(-1i64),
        ) {
            let reg_width: u32 = if is_64 { 64 } else { 32 };
            // Each (lsb, width, label) is an out-of-range operand the encoder
            // MUST reject with a clean Err (not a panic, not a silent Ok word).
            let cases: &[(i64, i64, &str)] = &[
                (0, 0, "lsb=0,width=0 (imms underflow PANIC)"),
                (reg_width as i64, 1, "lsb==regsize"),
                (over_lsb as i64, 1, "lsb>regsize (immr overflow into N bit)"),
                ((reg_width - 1) as i64, 2, "lsb+width>regsize (imms overflow)"),
                (0, over_width as i64, "width>regsize"),
                (neg, 1, "negative lsb"),
                (0, neg, "negative width"),
            ];
            for &(lsb, width, label) in cases {
                let ops = vec![
                    Operand::Reg(reg_name(0, is_64)),
                    Operand::Reg(reg_name(1, is_64)),
                    Operand::Imm(lsb),
                    Operand::Imm(width),
                ];
                // Catch panics so a panic (the lsb=0,width=0 underflow) is
                // reported as a contract failure instead of aborting the run.
                // (The default panic hook still prints the debug underflow
                // message to stderr; this is harmless proptest-shrink noise.)
                let got = panic::catch_unwind(panic::AssertUnwindSafe(|| encode_bfxil(&ops)));
                match got {
                    Ok(Ok(w)) => prop_assert!(false,
                        "{}: lsb={} width={} should be Err, got Ok({:?})",
                        label, lsb, width, w),
                    Ok(Err(_)) => {} // clean rejection: good
                    Err(_) => prop_assert!(false,
                        "{}: lsb={} width={} should be Err but PANICKED",
                        label, lsb, width),
                }
            }
        }
    }
}

#[cfg(test)]
mod prop_encode_ubfiz_tests {
    use super::*;
    use proptest::prelude::*;
    use std::panic;

    // ── Field layout of the UBFM (UBFIZ alias) word (ARM ARM §C4.1.66) ────
    //   sf [31] | opc=10 [30:29] | 100110 [28:23] | N [22]
    //   | immr [21:16] | imms [15:10] | Rn [9:5] | Rd [4:0]
    //
    // UBFIZ Rd, Rn, #lsb, #width is the alias
    //   UBFM Rd, Rn, #(-lsb MOD regsize), #(width-1)
    // so immr == (regsize - lsb) & (regsize - 1)  and  imms == width - 1.
    // ARM ARM operand constraints:
    //   64-bit: 0 <= lsb <= 63, 1 <= width <= 64 - lsb
    //   32-bit: 0 <= lsb <= 31, 1 <= width <= 32 - lsb
    // and the encoding constrains N == sf.
    const MASK_SF: u32 = 0x8000_0000;
    const FIXED_30_29: u32 = 0b10 << 29; // 0x4000_0000
    const MASK_30_29: u32 = 0b11 << 29; // 0x6000_0000
    const FIXED_28_23: u32 = 0b100110 << 23; // 0x1300_0000
    const MASK_28_23: u32 = 0b111111 << 23; // 0x1F80_0000
    const MASK_N: u32 = 1 << 22; // 0x0040_0000
    const MASK_IMMR: u32 = 0x003F_0000; // bits [21:16]
    const MASK_IMMS: u32 = 0x0000_FC00; // bits [15:10]
    const MASK_RN: u32 = 0x0000_03E0; // bits [9:5]
    const MASK_RD: u32 = 0x0000_001F; // bits [4:0]

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word(encode_ubfiz(ops))
    }

    fn reg_name(num: u32, is_64: bool) -> String {
        if is_64 {
            format!("x{}", num)
        } else {
            format!("w{}", num)
        }
    }

    /// (rd_name, rd_num, rn_name, rn_num, lsb, width, is_64) with lsb/width
    /// constrained to architecturally-valid ranges so that immr = (regsize-lsb)
    /// and imms = width-1 each fit their 6-bit field without wrap/underflow.
    fn arb_valid_case() -> impl Strategy<Value = (String, u32, String, u32, u32, u32, bool)> {
        (any::<bool>(), 0u32..=30u32, 0u32..=30u32, 0u32..63u32, 1u32..=64u32)
            .prop_filter(
                "lsb+width must fit regsize",
                |&(is_64, _rd, _rn, lsb, width)| {
                    let max = if is_64 { 64 } else { 32 };
                    lsb < max && lsb + width <= max
                },
            )
            .prop_map(|(is_64, rd, rn, lsb, width)| {
                (reg_name(rd, is_64), rd, reg_name(rn, is_64), rn, lsb, width, is_64)
            })
    }

    proptest! {
        // Property A — structural / field-placement oracle.
        // Every fixed opcode bit and every variable field lands in its
        // mandated position. imms reconstructs exactly to width-1, and the
        // UBFIZ-specific immr = (regsize - lsb) & (regsize - 1).
        #[test]
        fn prop_ubfiz_field_placement(c in arb_valid_case()) {
            let (rd_name, rd, rn_name, rn, lsb, width, is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let w = enc(&ops);
            let regsize: u32 = if is_64 { 64 } else { 32 };
            let exp_immr = (regsize.wrapping_sub(lsb)) & (regsize - 1);
            let exp_imms = width - 1;

            prop_assert_eq!(w & MASK_30_29, FIXED_30_29);
            prop_assert_eq!(w & MASK_28_23, FIXED_28_23);
            prop_assert_eq!(w & MASK_SF, if is_64 { MASK_SF } else { 0 });
            prop_assert_eq!(w & MASK_N, if is_64 { MASK_N } else { 0 });
            prop_assert_eq!((w & MASK_IMMR) >> 16, exp_immr);
            prop_assert_eq!((w & MASK_IMMS) >> 10, exp_imms);
            prop_assert_eq!((w & MASK_RN) >> 5, rn);
            prop_assert_eq!(w & MASK_RD, rd);
        }

        // Property B — the UBFIZ alias formula: immr == -lsb MOD regsize.
        // For every valid lsb in [0, regsize), immr must equal (regsize-lsb)
        // mod regsize (so lsb=0 -> immr=0; lsb=1 -> immr=regsize-1; ...).
        // This is the UBFIZ-specific invariant that distinguishes it from UBFX.
        #[test]
        fn prop_immr_is_neg_lsb_mod_regsize(
            is_64 in any::<bool>(),
            lsb in 0u32..63u32,
            width in 1u32..=64u32,
        ) {
            let regsize: u32 = if is_64 { 64 } else { 32 };
            prop_assume!(lsb < regsize && lsb + width <= regsize);
            let ops = vec![
                Operand::Reg(reg_name(0, is_64)),
                Operand::Reg("x1".into()),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let w = enc(&ops);
            let exp_immr = (regsize - lsb) % regsize;
            prop_assert_eq!((w & MASK_IMMR) >> 16, exp_immr);
        }

        // Property C — differential oracle against the raw UBFM encoder.
        // UBFIZ Rd, Rn, #lsb, #width is defined as the alias
        //   UBFM Rd, Rn, #(-lsb MOD regsize), #(width-1)
        // so feeding UBFM the converted immediates must yield a bit-identical
        // word. This verifies the UBFIZ->UBFM alias formula is consistent with
        // the raw UBFM path.
        #[test]
        fn prop_ubfiz_equals_ubfm_with_converted_immediates(c in arb_valid_case()) {
            let (rd_name, _rd, rn_name, _rn, lsb, width, is_64) = c;
            let regsize: u32 = if is_64 { 64 } else { 32 };
            let immr = ((regsize.wrapping_sub(lsb)) & (regsize - 1)) as i64;
            let imms = (width - 1) as i64;
            let ubfiz = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Reg(rn_name.clone()),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let ubfm = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(immr),
                Operand::Imm(imms),
            ];
            prop_assert_eq!(enc(&ubfiz), word(encode_ubfm(&ubfm)));
        }

        // Property D — register-width differential. Switching x{N} -> w{N}
        // (same numeric register, same lsb/width) must leave every fixed
        // opcode bit, the imms field, Rn and Rd unchanged. Only sf[31], N[22],
        // and the regsize-dependent immr[21:16] may differ: immr = -lsb mod
        // regsize depends on regsize, so it legitimately changes with the
        // register width, whereas imms = width-1 does not.
        #[test]
        fn prop_width_changes_only_sf_n_immr(
            num in 0u32..=30u32,
            lsb in 0u32..63u32,
            width in 1u32..=64u32,
        ) {
            let ops64 = vec![
                Operand::Reg(format!("x{}", num)),
                Operand::Reg("x0".into()),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let ops32 = vec![
                Operand::Reg(format!("w{}", num)),
                Operand::Reg("w0".into()),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let diff = enc(&ops64) ^ enc(&ops32);
            let allowed = MASK_SF | MASK_N | MASK_IMMR;
            prop_assert_eq!(diff & !allowed, 0,
                "diff outside sf/N/immr fields: {:#010x}", diff);
            // sf and N both derive solely from register width, so both must flip.
            prop_assert_eq!(diff & MASK_SF, MASK_SF);
            prop_assert_eq!(diff & MASK_N, MASK_N);
        }

        // Property E — NEGATIVE CONTRACT (the finding).
        // UBFIZ Rd, Rn, #lsb, #width maps to UBFM with immr = -lsb mod regsize
        // and imms = width-1. ARM ARM operand constraints (Bitfield, UBFIZ):
        //   64-bit: 0 <= lsb <= 63, 1 <= width <= 64 - lsb
        //   32-bit: 0 <= lsb <= 31, 1 <= width <= 32 - lsb
        // so that immr/imms each fit their 6-bit fields ([21:16]/[15:10]).
        // An assembler MUST reject out-of-range immediates rather than silently
        // truncating: today the `as u32` cast wraps negatives into the upper
        // opcode bits, an out-of-range lsb silently maps immr to garbage via
        // wrapping_sub, an oversized width overflows imms into the Rn field,
        // and width==0 underflows imms to u32::MAX (a debug-mode panic). The
        // current encoder performs NO range validation, so this property is
        // EXPECTED TO FAIL and documents the bug shared with the sibling
        // UBFIZ/UBFX/UBFM/SBFM/BFM family.
        #[test]
        fn prop_rejects_out_of_range_immediates(
            is_64 in any::<bool>(),
            over_lsb in 64u32..=1023u32,
            over_width in 65u32..=1023u32,
            neg in (-1024i64)..(-1i64),
        ) {
            // width == 0 underflows imms = width-1 -> panic in debug, so wrap
            // it in catch_unwind to treat a panic as a contract failure too.
            let mk_panic = |lsb: i64, width: i64, w64: bool| {
                let ops = vec![
                    Operand::Reg(reg_name(0, w64)),
                    Operand::Reg(reg_name(1, w64)),
                    Operand::Imm(lsb),
                    Operand::Imm(width),
                ];
                panic::catch_unwind(panic::AssertUnwindSafe(|| encode_ubfiz(&ops)))
            };
            // width == 0 (imms underflow) must be rejected.
            match mk_panic(0, 0, is_64) {
                Ok(Ok(w)) => prop_assert!(false,
                    "width=0 (imms underflow) should be Err, got Ok({:?})", w),
                Ok(Err(_)) => {}
                Err(_) => prop_assert!(false,
                    "width=0 (imms underflow) should be Err but PANICKED"),
            }
            // The remaining cases don't panic, so check them directly.
            let mk = |lsb: i64, width: i64, w64: bool| {
                encode_ubfiz(&[
                    Operand::Reg(reg_name(0, w64)),
                    Operand::Reg(reg_name(1, w64)),
                    Operand::Imm(lsb),
                    Operand::Imm(width),
                ])
            };
            // lsb beyond the 6-bit / register-width field must be rejected.
            prop_assert!(mk(over_lsb as i64, 1, is_64).is_err(),
                "lsb={} (>{}) should be rejected, got {:?}",
                over_lsb, if is_64 { 63 } else { 31 }, mk(over_lsb as i64, 1, is_64));
            // width that pushes imms = width-1 out of range must be rejected.
            prop_assert!(mk(0, over_width as i64, is_64).is_err(),
                "width={} (imms overflow) should be rejected, got {:?}",
                over_width, mk(0, over_width as i64, is_64));
            // negative immediates must be rejected (cast `as u32` wraps today).
            prop_assert!(mk(neg, 1, is_64).is_err(),
                "lsb={} (<0) should be rejected, got {:?}", neg, mk(neg, 1, is_64));
            prop_assert!(mk(1, neg, is_64).is_err(),
                "width={} (<0) should be rejected, got {:?}", neg, mk(1, neg, is_64));
        }
    }
}

#[cfg(test)]
mod prop_encode_sbfiz_tests {
    use super::*;
    use proptest::prelude::*;
    use std::panic;

    // ── Field layout of the SBFM (SBFIZ alias) word (ARM ARM §C4.1.67) ────
    //   sf [31] | opc=00 [30:29] | 100110 [28:23] | N [22]
    //   | immr [21:16] | imms [15:10] | Rn [9:5] | Rd [4:0]
    //
    // SBFIZ Rd, Rn, #lsb, #width is the alias
    //   SBFM Rd, Rn, #(-lsb MOD regsize), #(width-1)
    // so immr == (regsize - lsb) & (regsize - 1)  and  imms == width - 1.
    // It differs from the sibling UBFIZ alias ONLY in opc[30:29] (00 vs 10),
    // i.e. in bit 30. ARM ARM operand constraints:
    //   64-bit: 0 <= lsb <= 63, 1 <= width <= 64 - lsb
    //   32-bit: 0 <= lsb <= 31, 1 <= width <= 32 - lsb
    // and the encoding constrains N == sf.
    const MASK_SF: u32 = 0x8000_0000;
    const FIXED_30_29: u32 = 0b00 << 29; // SBFM opc=00 -> 0x0000_0000
    const MASK_30_29: u32 = 0b11 << 29; // 0x6000_0000
    const FIXED_28_23: u32 = 0b100110 << 23; // 0x1300_0000
    const MASK_28_23: u32 = 0b111111 << 23; // 0x1F80_0000
    const MASK_N: u32 = 1 << 22; // 0x0040_0000
    const MASK_IMMR: u32 = 0x003F_0000; // bits [21:16]
    const MASK_IMMS: u32 = 0x0000_FC00; // bits [15:10]
    const MASK_RN: u32 = 0x0000_03E0; // bits [9:5]
    const MASK_RD: u32 = 0x0000_001F; // bits [4:0]

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word(encode_sbfiz(ops))
    }

    fn reg_name(num: u32, is_64: bool) -> String {
        if is_64 {
            format!("x{}", num)
        } else {
            format!("w{}", num)
        }
    }

    /// (rd_name, rd_num, rn_name, rn_num, lsb, width, is_64) with lsb/width
    /// constrained to architecturally-valid ranges so that immr = (regsize-lsb)
    /// and imms = width-1 each fit their 6-bit field without wrap/underflow.
    fn arb_valid_case() -> impl Strategy<Value = (String, u32, String, u32, u32, u32, bool)> {
        (any::<bool>(), 0u32..=30u32, 0u32..=30u32, 0u32..63u32, 1u32..=64u32)
            .prop_filter(
                "lsb+width must fit regsize",
                |&(is_64, _rd, _rn, lsb, width)| {
                    let max = if is_64 { 64 } else { 32 };
                    lsb < max && lsb + width <= max
                },
            )
            .prop_map(|(is_64, rd, rn, lsb, width)| {
                (reg_name(rd, is_64), rd, reg_name(rn, is_64), rn, lsb, width, is_64)
            })
    }

    proptest! {
        // Property A — structural / field-placement oracle.
        // Every fixed opcode bit and every variable field lands in its
        // mandated position. SBFIZ maps to SBFM, so opc[30:29] must be 00
        // (this is the bit that distinguishes SBFIZ from UBFIZ=10). imms
        // reconstructs exactly to width-1, the SBFIZ-specific immr equals
        // (regsize - lsb) & (regsize - 1), and N == sf (ARM ARM constraint).
        #[test]
        fn prop_sbfiz_field_placement(c in arb_valid_case()) {
            let (rd_name, rd, rn_name, rn, lsb, width, is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let w = enc(&ops);
            let regsize: u32 = if is_64 { 64 } else { 32 };
            let exp_immr = (regsize.wrapping_sub(lsb)) & (regsize - 1);
            let exp_imms = width - 1;

            prop_assert_eq!(w & MASK_30_29, FIXED_30_29, "SBFIZ opc[30:29] must be 00 (SBFM)");
            prop_assert_eq!(w & MASK_28_23, FIXED_28_23);
            prop_assert_eq!(w & MASK_SF, if is_64 { MASK_SF } else { 0 });
            prop_assert_eq!(w & MASK_N, if is_64 { MASK_N } else { 0 });
            prop_assert_eq!((w & MASK_IMMR) >> 16, exp_immr);
            prop_assert_eq!((w & MASK_IMMS) >> 10, exp_imms);
            prop_assert_eq!((w & MASK_RN) >> 5, rn);
            prop_assert_eq!(w & MASK_RD, rd);
            // N == sf invariant (ARM ARM: constrained N == sf for SBFM).
            prop_assert_eq!((w >> 31) & 1, (w >> 22) & 1);
        }

        // Property B — the SBFIZ alias formula: immr == -lsb MOD regsize.
        // For every valid lsb in [0, regsize), immr must equal (regsize-lsb)
        // mod regsize (so lsb=0 -> immr=0; lsb=1 -> immr=regsize-1; ...).
        // This is the SBFIZ-specific invariant that distinguishes it from SBFX.
        #[test]
        fn prop_immr_is_neg_lsb_mod_regsize(
            is_64 in any::<bool>(),
            lsb in 0u32..63u32,
            width in 1u32..=64u32,
        ) {
            let regsize: u32 = if is_64 { 64 } else { 32 };
            prop_assume!(lsb < regsize && lsb + width <= regsize);
            let ops = vec![
                Operand::Reg(reg_name(0, is_64)),
                Operand::Reg("x1".into()),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let w = enc(&ops);
            let exp_immr = (regsize - lsb) % regsize;
            prop_assert_eq!((w & MASK_IMMR) >> 16, exp_immr);
        }

        // Property C — differential oracle against the raw SBFM encoder.
        // SBFIZ Rd, Rn, #lsb, #width is defined as the alias
        //   SBFM Rd, Rn, #(-lsb MOD regsize), #(width-1)
        // so feeding SBFM the converted immediates must yield a bit-identical
        // word. This verifies the SBFIZ->SBFM alias formula is consistent with
        // the raw SBFM path (the canonical reference for SBFIZ).
        #[test]
        fn prop_sbfiz_equals_sbfm_with_converted_immediates(c in arb_valid_case()) {
            let (rd_name, _rd, rn_name, _rn, lsb, width, is_64) = c;
            let regsize: u32 = if is_64 { 64 } else { 32 };
            let immr = ((regsize.wrapping_sub(lsb)) & (regsize - 1)) as i64;
            let imms = (width - 1) as i64;
            let sbfiz = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Reg(rn_name.clone()),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let sbfm = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(immr),
                Operand::Imm(imms),
            ];
            prop_assert_eq!(enc(&sbfiz), word(encode_sbfm(&sbfm)));
        }

        // Property D — differential oracle vs the sibling UBFIZ encoder.
        // SBFIZ and UBFIZ share an identical encoding template and differ ONLY
        // in opc[30:29]: SBFIZ=00, UBFIZ=10. Feeding identical operands must
        // therefore produce words that differ in exactly bit 30.
        #[test]
        fn prop_sbfiz_xor_ubfiz_is_only_bit_30(c in arb_valid_case()) {
            let (rd_name, _rd, rn_name, _rn, lsb, width, _is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Imm(lsb as i64),
                Operand::Imm(width as i64),
            ];
            let diff = enc(&ops) ^ word(encode_ubfiz(&ops));
            prop_assert_eq!(diff, 0x4000_0000, "SBFIZ ^ UBFIZ must be exactly bit 30");
        }

        // Property E — NEGATIVE CONTRACT (the finding).
        // SBFIZ Rd, Rn, #lsb, #width maps to SBFM with immr = -lsb mod regsize
        // and imms = width-1. ARM ARM operand constraints (Bitfield, SBFIZ):
        //   64-bit: 0 <= lsb <= 63, 1 <= width <= 64 - lsb
        //   32-bit: 0 <= lsb <= 31, 1 <= width <= 32 - lsb
        // so that immr/imms each fit their 6-bit fields ([21:16]/[15:10]).
        // An assembler MUST reject out-of-range immediates rather than silently
        // truncating: today the `as u32` cast wraps negatives into the upper
        // opcode bits, an out-of-range lsb silently maps immr to garbage via
        // wrapping_sub, an oversized width overflows imms into the Rn field,
        // and width==0 underflows imms to u32::MAX (a debug-mode panic). The
        // current encoder performs NO range validation, so this property is
        // EXPECTED TO FAIL and documents the bug shared with the sibling
        // UBFIZ/UBFX/UBFM/SBFM/BFM family.
        #[test]
        fn prop_rejects_out_of_range_immediates(
            is_64 in any::<bool>(),
            over_lsb in 64u32..=1023u32,
            over_width in 65u32..=1023u32,
            neg in (-1024i64)..(-1i64),
        ) {
            // width == 0 underflows imms = width-1 -> panic in debug, so wrap
            // it in catch_unwind to treat a panic as a contract failure too.
            let mk_panic = |lsb: i64, width: i64, w64: bool| {
                let ops = vec![
                    Operand::Reg(reg_name(0, w64)),
                    Operand::Reg(reg_name(1, w64)),
                    Operand::Imm(lsb),
                    Operand::Imm(width),
                ];
                panic::catch_unwind(panic::AssertUnwindSafe(|| encode_sbfiz(&ops)))
            };
            // width == 0 (imms underflow) must be rejected.
            match mk_panic(0, 0, is_64) {
                Ok(Ok(w)) => prop_assert!(false,
                    "width=0 (imms underflow) should be Err, got Ok({:?})", w),
                Ok(Err(_)) => {}
                Err(_) => prop_assert!(false,
                    "width=0 (imms underflow) should be Err but PANICKED"),
            }
            // The remaining cases don't panic, so check them directly.
            let mk = |lsb: i64, width: i64, w64: bool| {
                encode_sbfiz(&[
                    Operand::Reg(reg_name(0, w64)),
                    Operand::Reg(reg_name(1, w64)),
                    Operand::Imm(lsb),
                    Operand::Imm(width),
                ])
            };
            // lsb beyond the 6-bit / register-width field must be rejected.
            prop_assert!(mk(over_lsb as i64, 1, is_64).is_err(),
                "lsb={} (>{}) should be rejected, got {:?}",
                over_lsb, if is_64 { 63 } else { 31 }, mk(over_lsb as i64, 1, is_64));
            // width that pushes imms = width-1 out of range must be rejected.
            prop_assert!(mk(0, over_width as i64, is_64).is_err(),
                "width={} (imms overflow) should be rejected, got {:?}",
                over_width, mk(0, over_width as i64, is_64));
            // negative immediates must be rejected (cast `as u32` wraps today).
            prop_assert!(mk(neg, 1, is_64).is_err(),
                "lsb={} (<0) should be rejected, got {:?}", neg, mk(neg, 1, is_64));
            prop_assert!(mk(1, neg, is_64).is_err(),
                "width={} (<0) should be rejected, got {:?}", neg, mk(1, neg, is_64));
        }

        // Property F — malformed-operands negative contract (should pass).
        // Missing operands / wrong types in fixed slots must yield Err.
        #[test]
        fn prop_rejects_malformed_operands(
            bad in prop_oneof![Just(0u8), Just(1u8), Just(2u8), Just(3u8), Just(4u8), Just(5u8)],
            n in 0u32..=30u32,
            v in -16i64..=16i64,
        ) {
            let r = match bad {
                0 => encode_sbfiz(&[]),
                1 => encode_sbfiz(&[
                    Operand::Reg(reg_name(n, true)),
                    Operand::Reg("x1".into()),
                    Operand::Imm(v),
                ]),
                2 => encode_sbfiz(&[
                    Operand::Imm(v),
                    Operand::Reg("x1".into()),
                    Operand::Imm(0),
                    Operand::Imm(1),
                ]),
                3 => encode_sbfiz(&[
                    Operand::Reg("x0".into()),
                    Operand::Imm(v),
                    Operand::Imm(0),
                    Operand::Imm(1),
                ]),
                4 => encode_sbfiz(&[
                    Operand::Reg("x0".into()),
                    Operand::Reg("x1".into()),
                    Operand::Reg(reg_name(n, true)),
                    Operand::Imm(1),
                ]),
                _ => encode_sbfiz(&[
                    Operand::Reg("x0".into()),
                    Operand::Reg("x1".into()),
                    Operand::Imm(0),
                    Operand::Reg(reg_name(n, true)),
                ]),
            };
            prop_assert!(r.is_err(), "expected Err, got {:?}", r);
        }
    }
}

#[cfg(test)]
mod prop_encode_extr_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Field layout of the EXTR word (ARM ARM, Extract register) ────────
    //   EXTR <Rd>, <Rn>, <Rm>, #<lsb>
    //   sf [31] | opc=00 [30:29] | 100111 [28:23] | N [22] | o0=0 [21]
    //   | Rm [20:16] | imms [15:10] (= lsb) | Rn [9:5] | Rd [4:0]
    //
    // Architectural constraints (ARM ARM, Extract register):
    //   64-bit: sf=1, N=1, 0 <= lsb <= 63
    //   32-bit: sf=0, N=0, 0 <= lsb <= 31   (imms[5] must be 0)
    //   o0 (bit 21) is fixed 0; N must equal sf (CONSTRAINED).
    //   imms is a 6-bit field ([15:10]).
    const MASK_SF: u32 = 0x8000_0000;
    const FIXED_30_29: u32 = 0b00 << 29; // 0x0000_0000
    const MASK_30_29: u32 = 0b11 << 29; // 0x6000_0000
    const FIXED_28_23: u32 = 0b100111 << 23; // 0x1380_0000
    const MASK_28_23: u32 = 0b111111 << 23; // 0x1F80_0000
    const MASK_N: u32 = 1 << 22; // 0x0040_0000
    const MASK_O0: u32 = 1 << 21; // 0x0020_0000 — must be 0
    const MASK_RM: u32 = 0x001F_0000; // bits [20:16]
    const MASK_IMMS: u32 = 0x0000_FC00; // bits [15:10]
    const MASK_RN: u32 = 0x0000_03E0; // bits [9:5]
    const MASK_RD: u32 = 0x0000_001F; // bits [4:0]

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word(encode_extr(ops))
    }

    fn reg_name(num: u32, is_64: bool) -> String {
        if is_64 {
            format!("x{}", num)
        } else {
            format!("w{}", num)
        }
    }

    /// (rd_name, rd_num, rn_name, rn_num, rm_name, rm_num, lsb, is_64) with
    /// lsb constrained to the architecturally-valid range for the register
    /// width (64-bit: 0..=63, 32-bit: 0..=31).
    fn arb_valid_case() -> impl Strategy<Value = (String, u32, String, u32, String, u32, u32, bool)> {
        (
            any::<bool>(),
            0u32..=30u32,
            0u32..=30u32,
            0u32..=30u32,
            0u32..=63u32,
        )
            .prop_filter("lsb within regsize", |&(is_64, _, _, _, lsb)| {
                let max = if is_64 { 63 } else { 31 };
                lsb <= max
            })
            .prop_map(|(is_64, rd, rn, rm, lsb)| {
                (
                    reg_name(rd, is_64),
                    rd,
                    reg_name(rn, is_64),
                    rn,
                    reg_name(rm, is_64),
                    rm,
                    lsb,
                    is_64,
                )
            })
    }

    // Property B — known-encoding spot check (reference oracle).
    // EXTR x0, x1, x2, #5 is the canonical reference: sf=1, opc=00,
    // 100111, N=1, o0=0, Rm=2, imms=5, Rn=1, Rd=0 => 0x93C21420.
    // (Standalone #[test]: proptest! requires >=1 generated argument.)
    #[test]
    fn prop_extr_canonical_encoding() {
        let ops = vec![
            Operand::Reg("x0".into()),
            Operand::Reg("x1".into()),
            Operand::Reg("x2".into()),
            Operand::Imm(5),
        ];
        assert_eq!(enc(&ops), 0x93C21420u32);
    }

    proptest! {
        // Property A — structural / field-placement oracle.
        // Every fixed opcode bit and every variable field lands exactly
        // where the EXTR encoding mandates; imms reconstructs to lsb; o0[21]
        // is always 0; opc[30:29] is 00; and the [28:23] opcode is 100111.
        #[test]
        fn prop_extr_field_placement(c in arb_valid_case()) {
            let (rd_name, rd, rn_name, rn, rm_name, rm, lsb, is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Reg(rm_name),
                Operand::Imm(lsb as i64),
            ];
            let w = enc(&ops);

            prop_assert_eq!(w & MASK_30_29, FIXED_30_29, "EXTR opc[30:29] must be 00");
            prop_assert_eq!(w & MASK_28_23, FIXED_28_23, "EXTR [28:23] must be 100111");
            prop_assert_eq!(w & MASK_SF, if is_64 { MASK_SF } else { 0 });
            prop_assert_eq!(w & MASK_N, if is_64 { MASK_N } else { 0 });
            prop_assert_eq!(w & MASK_O0, 0, "o0 (bit 21) must be 0");
            prop_assert_eq!((w & MASK_RM) >> 16, rm);
            prop_assert_eq!((w & MASK_IMMS) >> 10, lsb);
            prop_assert_eq!((w & MASK_RN) >> 5, rn);
            prop_assert_eq!(w & MASK_RD, rd);
            // N == sf invariant (ARM ARM: constrained N == sf for EXTR).
            prop_assert_eq!((w >> 31) & 1, (w >> 22) & 1);
        }

        // Property C — register-width differential. Encoding x{N} vs w{N}
        // (same reg number for all three registers, same lsb within the
        // 32-bit-valid range) differs only in sf[31] and N[22].
        #[test]
        fn prop_width_changes_only_sf_and_n(
            num in 0u32..=30u32,
            lsb in 0u32..=31u32, // valid for both 32- and 64-bit
        ) {
            let ops64 = vec![
                Operand::Reg(format!("x{}", num)),
                Operand::Reg("x1".into()),
                Operand::Reg("x2".into()),
                Operand::Imm(lsb as i64),
            ];
            let ops32 = vec![
                Operand::Reg(format!("w{}", num)),
                Operand::Reg("w1".into()),
                Operand::Reg("w2".into()),
                Operand::Imm(lsb as i64),
            ];
            let diff = enc(&ops64) ^ enc(&ops32);
            prop_assert_eq!(diff, MASK_SF | MASK_N);
        }

        // Property D — determinism. The same operand list always yields the
        // same 32-bit word (encoder is pure).
        #[test]
        fn prop_deterministic(c in arb_valid_case()) {
            let (rd_name, _rd, rn_name, _rn, rm_name, _rm, lsb, _is_64) = c;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Reg(rm_name),
                Operand::Imm(lsb as i64),
            ];
            prop_assert_eq!(enc(&ops), enc(&ops));
        }

        // Property E — malformed-operands negative contract (should pass).
        // EXTR takes 4 operands: Reg, Reg, Reg, Imm. Missing operands or a
        // wrong-typed operand in any fixed slot must yield Err.
        #[test]
        fn prop_rejects_malformed_operands(
            bad in prop_oneof![
                Just(0u8), Just(1u8), Just(2u8), Just(3u8), Just(4u8), Just(5u8),
            ],
            n in 0u32..=30u32,
            v in -16i64..=16i64,
        ) {
            let r = match bad {
                0 => encode_extr(&[]),
                // too few operands (missing the immediate)
                1 => encode_extr(&[
                    Operand::Reg(reg_name(n, true)),
                    Operand::Reg("x1".into()),
                    Operand::Reg("x2".into()),
                ]),
                // slot 0 not a register
                2 => encode_extr(&[
                    Operand::Imm(v),
                    Operand::Reg("x1".into()),
                    Operand::Reg("x2".into()),
                    Operand::Imm(0),
                ]),
                // slot 1 not a register
                3 => encode_extr(&[
                    Operand::Reg("x0".into()),
                    Operand::Imm(v),
                    Operand::Reg("x2".into()),
                    Operand::Imm(0),
                ]),
                // slot 2 not a register
                4 => encode_extr(&[
                    Operand::Reg("x0".into()),
                    Operand::Reg("x1".into()),
                    Operand::Imm(v),
                    Operand::Imm(0),
                ]),
                // slot 3 not an immediate
                _ => encode_extr(&[
                    Operand::Reg("x0".into()),
                    Operand::Reg("x1".into()),
                    Operand::Reg("x2".into()),
                    Operand::Reg(reg_name(n, true)),
                ]),
            };
            prop_assert!(r.is_err(), "expected Err, got {:?}", r);
        }

        // Property F — NEGATIVE CONTRACT (the finding).
        // EXTR's imms field is 6 bits ([15:10]). The ARM ARM constrains the
        // lsb (encoded in imms) to: 64-bit 0..=63, 32-bit 0..=31 (for which
        // imms[5] must be 0). An assembler MUST reject:
        //   * lsb >= 64  — overflows the 6-bit imms field, OR-ing garbage into
        //     the Rn field ([9:5]) and beyond;
        //   * lsb in 32..=63 with a 32-bit (W) register — imms[5] set, which is
        //     an architecturally UNDEFINED / unpreferrable encoding;
        //   * negative lsb — the `as u32` cast wraps to ~0, overflowing every
        //     upper field.
        // The current encoder performs NO range validation (only an `as u32`
        // cast), so this property is EXPECTED TO FAIL and documents the same
        // bug class as encode_ubfm/encode_sbfm/encode_bfm, plus the EXTR-
        // specific 32-bit imms[5] constraint that lets w-form EXTR silently
        // emit an invalid instruction for 32 <= lsb <= 63.
        #[test]
        fn prop_rejects_out_of_range_lsb(
            is_64 in any::<bool>(),
            big_lsb in 64u32..=4095u32,
            mid_lsb in 32u32..=63u32,
            neg_imm in (-4096i64)..(-1i64),
        ) {
            let mk = |lsb: i64, w64: bool| {
                encode_extr(&[
                    Operand::Reg(reg_name(0, w64)),
                    Operand::Reg(reg_name(1, w64)),
                    Operand::Reg(reg_name(2, w64)),
                    Operand::Imm(lsb),
                ])
            };
            // lsb beyond the 6-bit field must be rejected (both widths).
            prop_assert!(mk(big_lsb as i64, is_64).is_err(),
                "lsb={} (>63) should be rejected, got {:?}",
                big_lsb, mk(big_lsb as i64, is_64));
            // 32-bit form: 32 <= lsb <= 63 sets imms[5] -> must be rejected.
            prop_assert!(mk(mid_lsb as i64, false).is_err(),
                "w-form lsb={} (32..=63, sets imms[5]) should be rejected, got {:?}",
                mid_lsb, mk(mid_lsb as i64, false));
            // negative lsb must be rejected (cast `as u32` wraps today).
            prop_assert!(mk(neg_imm, is_64).is_err(),
                "lsb={} (<0) should be rejected, got {:?}", neg_imm, mk(neg_imm, is_64));
        }
    }
}

#[cfg(test)]
mod prop_encode_rev32_tests {
    use super::*;
    use proptest::prelude::*;

    // ── REV32 encoding (ARM ARM, Data-processing (1 source) / vector) ────
    //
    // SCALAR form: REV32 <Rd>, <Rn>   (Rd/Rn are W or X)
    //   sf 1 0 11010110 00000 opc[15:10] Rn Rd
    //   The (sf, opc) decode is a bijection; REV and REV32 share opc in
    //   {000010, 000011} and are MIRRORED in sf:
    //     REV   32-bit: sf=0 opc=000010   |   REV32 64-bit: sf=1 opc=000010
    //     REV   64-bit: sf=1 opc=000011   |   REV32 32-bit: sf=0 opc=000011
    //   (encode_rev in this file uses opc = if is_64 {0b000011} else {0b000010}
    //    and is correct; REV32 must mirror it.)
    //
    // VECTOR form: REV32 <Vd>.<T>, <Vn>.<T>
    //   0 Q 1 01110 size 1 00000 0000 10 Rn Rd
    //   size in {00 (bytes), 01 (halfwords)} only; size=10/11 is UNALLOCATED.
    //
    // Reference base words (excluding Rn/Rd):
    //   scalar 64-bit (X): 0xDAC00800      scalar 32-bit (W): 0x5AC00C00
    const MASK_SF: u32 = 0x8000_0000;
    const MASK_30_29: u32 = 0b11 << 29; // 0x6000_0000
    const MASK_28_21: u32 = 0xFF << 21; // 0x1FE0_0000
    const MASK_20_16: u32 = 0x1F << 16; // 0x001F_0000
    const MASK_OPC: u32 = 0x3F << 10; // bits [15:10]
    const MASK_RN: u32 = 0x1F << 5; // bits [9:5]
    const MASK_RD: u32 = 0x1F; // bits [4:0]

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word(encode_rev32(ops))
    }

    fn reg_name(num: u32, is_64: bool) -> String {
        if is_64 { format!("x{}", num) } else { format!("w{}", num) }
    }

    /// ARM ARM reference word for the scalar form, both widths.
    fn ref_scalar(is_64: bool, rn: u32, rd: u32) -> u32 {
        let base = if is_64 { 0xDAC0_0800 } else { 0x5AC0_0C00 };
        base | (rn << 5) | rd
    }

    proptest! {
        // Property A — structural / field-placement oracle for the 64-bit
        // scalar form (the only width the current encoder happens to get
        // right). Every fixed bit and the opc[15:10]=000010 field land
        // exactly where the ARM ARM mandates; Rn/Rd reconstruct to inputs.
        #[test]
        fn prop_scalar_field_placement_64bit(rd in 0u32..=30u32, rn in 0u32..=30u32) {
            let ops = vec![Operand::Reg(reg_name(rd, true)), Operand::Reg(reg_name(rn, true))];
            let w = enc(&ops);
            prop_assert_eq!(w & MASK_SF, MASK_SF, "sf must be 1 for X registers");
            prop_assert_eq!(w & MASK_30_29, 0b10 << 29);
            prop_assert_eq!(w & MASK_28_21, 0xD6 << 21, "bits[28:21] must be 11010110");
            prop_assert_eq!(w & MASK_20_16, 0, "bits[20:16] must be 0");
            prop_assert_eq!((w & MASK_OPC) >> 10, 0b000010, "opc must be 000010 for 64-bit REV32");
            prop_assert_eq!((w & MASK_RN) >> 5, rn);
            prop_assert_eq!(w & MASK_RD, rd);
        }

        // Property B — reference oracle for BOTH widths (THE FINDING).
        // The ARM ARM decode for scalar REV32 is:
        //   64-bit (X): sf=1, opc=000010  -> base 0xDAC00800
        //   32-bit (W): sf=0, opc=000011  -> base 0x5AC00C00
        // The current encoder discards the register width
        // (`let (rd, _) = get_reg(...)`) and hardcodes sf=1 with opc=000010,
        // i.e. it ALWAYS emits the 64-bit encoding regardless of input.
        // For W registers the output is therefore doubly wrong: sf should be
        // 0 and opc should be 000011. This property PASSES for X registers
        // and FAILS for W registers.
        #[test]
        fn prop_scalar_matches_arm_reference(
            is_64 in any::<bool>(),
            rd in 0u32..=30u32,
            rn in 0u32..=30u32,
        ) {
            let ops = vec![
                Operand::Reg(reg_name(rd, is_64)),
                Operand::Reg(reg_name(rn, is_64)),
            ];
            let got = enc(&ops);
            let want = ref_scalar(is_64, rn, rd);
            let w = if is_64 { 'x' } else { 'w' };
            prop_assert_eq!(got, want,
                "REV32 {}{}, {}{} (is_64={}): expected {:#010X}, got {:#010X}",
                w, rd, w, rn, is_64, want, got);
        }

        // Property C — structural / field-placement oracle for the NEON
        // (vector) form. Verifies bit31=0, Q from arrangement, bit29=1,
        // bits[28:24]=01110, size from arrangement, bit21=1, bits[20:16]=0,
        // bits[15:12]=0, bits[11:10]=10, and Rn/Rd reconstruct. Architecturally
        // REV32 vector is only valid for size in {00, 01}; UNALLOCATED sizes
        // (2s/4s/1d/2d) are skipped here via prop_assume.
        #[test]
        fn prop_neon_field_placement(
            rd in 0u32..=31u32,
            rn in 0u32..=31u32,
            arr in prop_oneof![
                Just("8b"), Just("16b"), Just("4h"), Just("8h"),
                Just("2s"), Just("4s"), Just("1d"), Just("2d"),
            ],
        ) {
            let (q, size): (u32, u32) = match arr {
                "8b" => (0, 0b00), "16b" => (1, 0b00),
                "4h" => (0, 0b01), "8h" => (1, 0b01),
                "2s" => (0, 0b10), "4s" => (1, 0b10),
                "1d" => (0, 0b11), "2d" => (1, 0b11),
                _ => unreachable!(),
            };
            prop_assume!(size <= 0b01, "REV32 vector valid only for size<=01");

            let ops = vec![
                Operand::RegArrangement { reg: format!("v{}", rd), arrangement: arr.into() },
                Operand::RegArrangement { reg: format!("v{}", rn), arrangement: arr.into() },
            ];
            let w = enc(&ops);
            prop_assert_eq!(w & MASK_SF, 0, "bit31 must be 0 for NEON form");
            prop_assert_eq!((w >> 30) & 1, q, "Q must match arrangement");
            prop_assert_eq!((w >> 29) & 1, 1);
            prop_assert_eq!((w >> 24) & 0x1F, 0b01110, "bits[28:24] must be 01110");
            prop_assert_eq!((w >> 22) & 0x3, size, "size must match arrangement");
            prop_assert_eq!((w >> 21) & 1, 1, "bit21 must be 1");
            prop_assert_eq!(w & MASK_20_16, 0, "bits[20:16] must be 0");
            prop_assert_eq!((w >> 12) & 0xF, 0, "bits[15:12] must be 0");
            prop_assert_eq!((w >> 10) & 0x3, 0b10, "bits[11:10] must be 10");
            prop_assert_eq!((w & MASK_RN) >> 5, rn);
            prop_assert_eq!(w & MASK_RD, rd);
        }

        // Property D — malformed-operands negative contract (should pass).
        // Missing operands or a non-register in a fixed slot must yield Err.
        #[test]
        fn prop_rejects_malformed_operands(
            kind in prop_oneof![Just(0u8), Just(1u8), Just(2u8), Just(3u8)],
            n in 0u32..=30u32,
        ) {
            let r = match kind {
                0 => encode_rev32(&[]),
                1 => encode_rev32(&[Operand::Reg(reg_name(n, true))]),
                2 => encode_rev32(&[Operand::Imm(n as i64), Operand::Reg("x1".into())]),
                _ => encode_rev32(&[Operand::Reg(reg_name(n, true)), Operand::Imm(0)]),
            };
            prop_assert!(r.is_err(), "expected Err, got {:?}", r);
        }

        // Property E — determinism. The same operand list always yields the
        // same 32-bit word (the encoder is a pure function).
        #[test]
        fn prop_deterministic(
            rd in 0u32..=30u32,
            rn in 0u32..=30u32,
            is_64 in any::<bool>(),
        ) {
            let ops = vec![Operand::Reg(reg_name(rd, is_64)), Operand::Reg(reg_name(rn, is_64))];
            prop_assert_eq!(enc(&ops), enc(&ops));
        }
    }
}

#[cfg(test)]
mod prop_encode_rev16_tests {
    use super::*;
    use proptest::prelude::*;

    // ── REV16 encoding (ARM ARM, Data-processing (1 source)) ──────────────
    //
    // REV16 <Rd>, <Rn>   (Rd/Rn are W or X — available in BOTH widths)
    //   sf 1 0 11010110 00000 opc[15:10] Rn Rd
    //
    // Unlike REV / REV32 (whose opc SWAPS with sf: REV uses 000010/000011
    // and REV32 mirrors it), REV16 uses the SAME opc = 000001 for both the
    // 32-bit and 64-bit forms. The width therefore changes ONLY bit sf[31].
    //
    // Reference base words (Rn=0, Rd=0), built from the field layout above:
    //   64-bit (X), sf=1: 0xDAC0_0400
    //   32-bit (W), sf=0: 0x5AC0_0400
    const BASE_64: u32 = 0xDAC0_0400;
    const BASE_32: u32 = 0x5AC0_0400;

    const MASK_SF: u32 = 0x8000_0000;
    const MASK_30_29: u32 = 0b11 << 29; // 0x6000_0000
    const MASK_28_21: u32 = 0xFF << 21; // 0x1FE0_0000
    const MASK_20_16: u32 = 0x1F << 16; // 0x001F_0000
    const MASK_OPC: u32 = 0x3F << 10; // bits [15:10]
    const MASK_RN: u32 = 0x1F << 5; // bits [9:5]
    const MASK_RD: u32 = 0x1F; // bits [4:0]

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word(encode_rev16(ops))
    }

    fn reg_name(num: u32, is_64: bool) -> String {
        if is_64 { format!("x{}", num) } else { format!("w{}", num) }
    }

    /// ARM ARM reference word for REV16, both widths. Only bit sf differs.
    fn ref_rev16(is_64: bool, rn: u32, rd: u32) -> u32 {
        let base = if is_64 { BASE_64 } else { BASE_32 };
        base | (rn << 5) | rd
    }

    proptest! {
        // Property A — structural / field-placement oracle.
        // Every fixed bit lands where the ARM ARM mandates: bit30=1,
        // bit29=0, bits[28:21]=11010110 (0xD6), bits[20:16]=0; the opc field
        // [15:10] is 000001 for BOTH widths (the defining correctness point
        // for REV16); sf tracks the destination register width; and Rn/Rd
        // reconstruct exactly to the inputs.
        #[test]
        fn prop_field_placement(
            is_64 in any::<bool>(),
            rd in 0u32..=30u32,
            rn in 0u32..=30u32,
        ) {
            let ops = vec![Operand::Reg(reg_name(rd, is_64)), Operand::Reg(reg_name(rn, is_64))];
            let w = enc(&ops);
            prop_assert_eq!(w & MASK_SF, if is_64 { MASK_SF } else { 0 }, "sf must track width");
            prop_assert_eq!(w & MASK_30_29, 0b10 << 29, "bit30=1, bit29=0");
            prop_assert_eq!(w & MASK_28_21, 0xD6 << 21, "bits[28:21] must be 11010110");
            prop_assert_eq!(w & MASK_20_16, 0, "bits[20:16] must be 0");
            prop_assert_eq!((w & MASK_OPC) >> 10, 0b000001, "opc must be 000001 (both widths)");
            prop_assert_eq!((w & MASK_RN) >> 5, rn, "Rn reconstruct");
            prop_assert_eq!(w & MASK_RD, rd, "Rd reconstruct");
        }

        // Property B — reference oracle against the full ARM ARM word.
        // The emitted word must equal the hand-derived base for the given
        // width OR'd with (Rn<<5)|Rd. This pins every bit and confirms the
        // encoder varies only sf with width (the correct REV16 behavior —
        // contrast REV32, which incorrectly hardcodes a single width).
        #[test]
        fn prop_matches_arm_reference(
            is_64 in any::<bool>(),
            rd in 0u32..=30u32,
            rn in 0u32..=30u32,
        ) {
            let ops = vec![Operand::Reg(reg_name(rd, is_64)), Operand::Reg(reg_name(rn, is_64))];
            let got = enc(&ops);
            let want = ref_rev16(is_64, rn, rd);
            let w = if is_64 { 'x' } else { 'w' };
            prop_assert_eq!(got, want,
                "REV16 {}{}, {}{} (is_64={}): expected {:#010X}, got {:#010X}",
                w, rd, w, rn, is_64, want, got);
        }

        // Property C — register-width differential.
        // REV16 must change ONLY bit sf[31] when switching between X and W
        // registers of the same number, because opc is identical for both
        // widths. (This invariant would FAIL for REV/REV32, whose opc swaps
        // with width — it passing here is precisely what makes REV16 correct.)
        #[test]
        fn prop_width_changes_only_sf(
            num in 0u32..=30u32,
            rn in 0u32..=30u32,
        ) {
            let ops64 = vec![Operand::Reg(format!("x{}", num)), Operand::Reg(format!("x{}", rn))];
            let ops32 = vec![Operand::Reg(format!("w{}", num)), Operand::Reg(format!("w{}", rn))];
            let diff = enc(&ops64) ^ enc(&ops32);
            prop_assert_eq!(diff, MASK_SF, "X vs W must differ only in bit sf[31]");
        }

        // Property D — differential oracle vs the sibling REV encoder.
        // REV16 and REV share the Data-processing(1 source) encoding template
        // and differ ONLY in the opc[15:10] field: REV uses 000011 (64-bit) /
        // 000010 (32-bit), REV16 uses 000001 (both). Feeding identical operands
        // must therefore produce words whose XOR is confined to bits[15:10]
        // and equal to (rev_opc ^ 0b000001) << 10. sf, Rn, Rd are all unchanged.
        #[test]
        fn prop_xor_rev_confined_to_opc(
            is_64 in any::<bool>(),
            rd in 0u32..=30u32,
            rn in 0u32..=30u32,
        ) {
            let ops = vec![Operand::Reg(reg_name(rd, is_64)), Operand::Reg(reg_name(rn, is_64))];
            let diff = enc(&ops) ^ word(encode_rev(&ops));
            let rev_opc: u32 = if is_64 { 0b000011 } else { 0b000010 };
            let expected = ((rev_opc ^ 0b000001) & 0x3F) << 10;
            prop_assert_eq!(diff & !MASK_OPC, 0, "REV16 ^ REV must touch only bits[15:10]");
            prop_assert_eq!(diff, expected, "opc XOR must equal (rev_opc ^ 000001) << 10");
        }

        // Property E — malformed-operands negative contract (should pass).
        // REV16 requires two register operands (Rd, Rn) and reads exactly
        // operands[0..2]; a missing operand or a non-register in either fixed
        // slot must yield Err rather than a silently-wrong word. (A trailing
        // *extra* operand is deliberately not asserted here: the encoder only
        // indexes the first two slots, so surplus operands are ignored.)
        #[test]
        fn prop_rejects_malformed_operands(
            kind in prop_oneof![Just(0u8), Just(1u8), Just(2u8), Just(3u8)],
            n in 0u32..=30u32,
            v in -16i64..=16i64,
        ) {
            let r = match kind {
                0 => encode_rev16(&[]),
                1 => encode_rev16(&[Operand::Reg(reg_name(n, true))]),
                2 => encode_rev16(&[Operand::Imm(v), Operand::Reg("x1".into())]),
                _ => encode_rev16(&[Operand::Reg(reg_name(n, true)), Operand::Imm(v)]),
            };
            prop_assert!(r.is_err(), "expected Err, got {:?}", r);
        }
    }
}

#[cfg(test)]
mod prop_encode_rbit_tests {
    use super::*;
    use proptest::prelude::*;

    // ── RBIT encoding (ARM ARM) ───────────────────────────────────────────
    //
    // SCALAR form: RBIT <Rd>, <Rn>   (Rd/Rn are W or X — available in BOTH widths)
    //   sf 1 0 11010110 00000 000000 Rn Rd
    //   31 30 29 28:21       20:16 15:10 9:5 4:0
    //   bit30=1, bit29=0, bits[28:21]=11010110 (0xD6), bits[20:16]=0,
    //   bits[15:10]=000000. sf tracks the register width.
    //
    // VECTOR form: RBIT <Vd>.<T>, <Vn>.<T>   (T is 8B or 16B ONLY)
    //   0 Q 1 01110 01 10000 00101 10 Rn Rd
    //   bit31=0, Q[30], bit29=1, bits[28:24]=01110, bits[23:22]=01,
    //   bits[21:17]=10000, bits[16:12]=00101, bits[11:10]=10.
    //   The ARM ARM CONSTRAINS T ∈ {8B, 16B}: RBIT reverses bits within each
    //   byte, so there is no size field and no other arrangement is allocated.
    //
    // Reference base words (Rn=0, Rd=0):
    //   scalar 64-bit (X): 0xDAC00000      scalar 32-bit (W): 0x5AC00000
    //   vector 16B (Q=1): 0x6E605800       vector  8B (Q=0): 0x2E605800
    const SCALAR_BASE_64: u32 = 0xDAC0_0000;
    const SCALAR_BASE_32: u32 = 0x5AC0_0000;
    const NEON_BASE_16B: u32 = 0x6E60_5800;
    const NEON_BASE_8B: u32 = 0x2E60_5800;

    const MASK_SF: u32 = 0x8000_0000;
    const MASK_30_29: u32 = 0b11 << 29; // 0x6000_0000
    const MASK_28_21: u32 = 0xFF << 21; // 0x1FE0_0000
    const MASK_20_16: u32 = 0x1F << 16; // 0x001F_0000
    const MASK_15_10: u32 = 0x3F << 10; // 0x0000_FC00
    const MASK_Q: u32 = 1 << 30; // 0x4000_0000
    const MASK_RN: u32 = 0x1F << 5; // bits [9:5]
    const MASK_RD: u32 = 0x1F; // bits [4:0]

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word(encode_rbit(ops))
    }

    fn reg_name(num: u32, is_64: bool) -> String {
        if is_64 { format!("x{}", num) } else { format!("w{}", num) }
    }

    /// ARM ARM reference word for the scalar form, both widths.
    fn ref_scalar(is_64: bool, rn: u32, rd: u32) -> u32 {
        let base = if is_64 { SCALAR_BASE_64 } else { SCALAR_BASE_32 };
        base | (rn << 5) | rd
    }

    /// ARM ARM reference word for the vector form, both valid arrangements.
    fn ref_neon(arr: &str, rn: u32, rd: u32) -> u32 {
        let base = if arr == "16b" { NEON_BASE_16B } else { NEON_BASE_8B };
        base | (rn << 5) | rd
    }

    proptest! {
        // Property A — scalar structural / field-placement oracle.
        // Every fixed bit lands where the ARM ARM mandates: bit30=1,
        // bit29=0, bits[28:21]=11010110 (0xD6), bits[20:16]=0,
        // bits[15:10]=000000; sf tracks the destination register width;
        // Rn/Rd reconstruct exactly to the inputs.
        #[test]
        fn prop_scalar_field_placement(
            is_64 in any::<bool>(),
            rd in 0u32..=30u32,
            rn in 0u32..=30u32,
        ) {
            let ops = vec![Operand::Reg(reg_name(rd, is_64)), Operand::Reg(reg_name(rn, is_64))];
            let w = enc(&ops);
            prop_assert_eq!(w & MASK_SF, if is_64 { MASK_SF } else { 0 }, "sf must track width");
            prop_assert_eq!(w & MASK_30_29, 0b10 << 29, "bit30=1, bit29=0");
            prop_assert_eq!(w & MASK_28_21, 0xD6 << 21, "bits[28:21] must be 11010110");
            prop_assert_eq!(w & MASK_20_16, 0, "bits[20:16] must be 0");
            prop_assert_eq!(w & MASK_15_10, 0, "bits[15:10] must be 000000");
            prop_assert_eq!((w & MASK_RN) >> 5, rn, "Rn reconstruct");
            prop_assert_eq!(w & MASK_RD, rd, "Rd reconstruct");
        }

        // Property B — scalar reference oracle (full word).
        // The emitted word must equal the hand-derived ARM ARM base for the
        // given width OR'd with (Rn<<5)|Rd, for BOTH widths. This pins every
        // bit and confirms RBIT varies only sf with width (correct behavior,
        // contrast REV32 which hardcodes a single width).
        #[test]
        fn prop_scalar_matches_arm_reference(
            is_64 in any::<bool>(),
            rd in 0u32..=30u32,
            rn in 0u32..=30u32,
        ) {
            let ops = vec![Operand::Reg(reg_name(rd, is_64)), Operand::Reg(reg_name(rn, is_64))];
            let got = enc(&ops);
            let want = ref_scalar(is_64, rn, rd);
            let c = if is_64 { 'x' } else { 'w' };
            prop_assert_eq!(got, want,
                "RBIT {}{}, {}{} (is_64={}): expected {:#010X}, got {:#010X}",
                c, rd, c, rn, is_64, want, got);
        }

        // Property C — NEON vector reference + field-placement oracle.
        // For the two VALID arrangements (8B, 16B) the emitted word must equal
        // the ARM ARM base OR'd with (Rn<<5)|Rd, and every fixed bit (bit31=0,
        // bit29=1, bits[28:24]=01110, bits[23:22]=01, bits[21:17]=10000,
        // bits[16:12]=00101, bits[11:10]=10) must land in place.
        #[test]
        fn prop_neon_matches_arm_reference(
            arr in prop_oneof![Just("8b"), Just("16b")],
            rd in 0u32..=31u32,
            rn in 0u32..=31u32,
        ) {
            let ops = vec![
                Operand::RegArrangement { reg: format!("v{}", rd), arrangement: arr.into() },
                Operand::RegArrangement { reg: format!("v{}", rn), arrangement: arr.into() },
            ];
            let w = enc(&ops);
            let want = ref_neon(arr, rn, rd);
            prop_assert_eq!(w, want,
                "RBIT v{}.{}, v{}.{}: expected {:#010X}, got {:#010X}",
                rd, arr, rn, arr, want, w);
            // Field placement of the fixed bits.
            prop_assert_eq!(w & MASK_SF, 0, "bit31 must be 0 for vector form");
            prop_assert_eq!((w >> 30) & 1, if arr == "16b" { 1 } else { 0 }, "Q must match arrangement");
            prop_assert_eq!((w >> 29) & 1, 1);
            prop_assert_eq!((w >> 24) & 0x1F, 0b01110, "bits[28:24] must be 01110");
            prop_assert_eq!((w >> 22) & 0x3, 0b01, "bits[23:22] must be 01");
            prop_assert_eq!((w >> 17) & 0x1F, 0b10000, "bits[21:17] must be 10000");
            prop_assert_eq!((w >> 12) & 0x1F, 0b00101, "bits[16:12] must be 00101");
            prop_assert_eq!((w >> 10) & 0x3, 0b10, "bits[11:10] must be 10");
        }

        // Property D — differential oracles.
        // (1) Scalar: switching x{N} -> w{N} (same number, same Rn width) must
        //     change ONLY bit sf[31] — the fixed/opc fields are identical for
        //     both widths (contrast REV/REV32 whose opc swaps with width).
        // (2) Vector: switching .8b -> .16b (same registers) must change ONLY
        //     bit Q[30]; bit31 stays 0.
        #[test]
        fn prop_differentials(
            num in 0u32..=30u32,
            rn in 0u32..=30u32,
            vnum in 0u32..=31u32,
            vrn in 0u32..=31u32,
        ) {
            // (1) scalar width differential
            let s64 = vec![Operand::Reg(format!("x{}", num)), Operand::Reg(format!("x{}", rn))];
            let s32 = vec![Operand::Reg(format!("w{}", num)), Operand::Reg(format!("w{}", rn))];
            prop_assert_eq!(enc(&s64) ^ enc(&s32), MASK_SF,
                "scalar X vs W must differ only in bit sf[31]");

            // (2) vector arrangement differential
            let v8 = vec![
                Operand::RegArrangement { reg: format!("v{}", vnum), arrangement: "8b".into() },
                Operand::RegArrangement { reg: format!("v{}", vrn), arrangement: "8b".into() },
            ];
            let v16 = vec![
                Operand::RegArrangement { reg: format!("v{}", vnum), arrangement: "16b".into() },
                Operand::RegArrangement { reg: format!("v{}", vrn), arrangement: "16b".into() },
            ];
            prop_assert_eq!(enc(&v8) ^ enc(&v16), MASK_Q,
                "vector 8b vs 16b must differ only in bit Q[30]");
        }

        // Property E — malformed-operands negative contract (should pass).
        // Scalar RBIT takes two register operands; the vector form takes two
        // RegArrangement operands and dispatches on operands[0]. A missing
        // operand, a non-register in a fixed slot, or the wrong dispatch type
        // must yield Err rather than a silently-wrong word.
        #[test]
        fn prop_rejects_malformed_operands(
            kind in prop_oneof![
                Just(0u8), Just(1u8), Just(2u8), Just(3u8), Just(4u8), Just(5u8),
            ],
            n in 0u32..=30u32,
            v in -16i64..=16i64,
        ) {
            let r = match kind {
                // scalar: empty
                0 => encode_rbit(&[]),
                // scalar: one operand
                1 => encode_rbit(&[Operand::Reg(reg_name(n, true))]),
                // scalar: slot 0 not a register
                2 => encode_rbit(&[Operand::Imm(v), Operand::Reg("x1".into())]),
                // scalar: slot 1 not a register
                3 => encode_rbit(&[Operand::Reg(reg_name(n, true)), Operand::Imm(v)]),
                // vector: only one arrangement operand
                4 => encode_rbit(&[Operand::RegArrangement {
                    reg: format!("v{}", n), arrangement: "8b".into() }]),
                // vector: slot 1 not an arrangement register
                _ => encode_rbit(&[
                    Operand::RegArrangement { reg: format!("v{}", n), arrangement: "8b".into() },
                    Operand::Imm(v),
                ]),
            };
            prop_assert!(r.is_err(), "expected Err, got {:?}", r);
        }

        // Property F — NEGATIVE CONTRACT (the finding).
        // RBIT (vector) is CONSTRAINED by the ARM ARM to <T> ∈ {8B, 16B} only
        // (it reverses bits within each byte; there is no size field and no
        // other arrangement is allocated). An assembler MUST therefore reject
        // every other arrangement (.4h/.8h/.2s/.4s/.1d/.2d) with a clean Err.
        //
        // The current encoder performs NO arrangement validation: it sets
        // Q = if arr_d == "16b" { 1 } else { 0 } and otherwise uses the byte
        // op word verbatim, so:
        //   * a halfword/word/doubleword arrangement is SILENTLY accepted and
        //     encoded as the 8B RBIT instruction (Q=0) — a different
        //     instruction than the mnemonic+arrangement denote;
        //   * the SOURCE arrangement (operands[1]) is ignored entirely, so a
        //     mismatched pair like `RBIT v0.16b, v1.8b` is also silently taken.
        // This property is EXPECTED TO FAIL and documents the missing
        // arrangement validation in the NEON branch of encode_rbit.
        #[test]
        fn prop_rejects_invalid_vector_arrangements(
            bad in prop_oneof![
                Just("4h"), Just("8h"), Just("2s"), Just("4s"), Just("1d"), Just("2d"),
            ],
            rd in 0u32..=31u32,
            rn in 0u32..=31u32,
        ) {
            // An invalid arrangement must be rejected (Ok here is the bug).
            let same = vec![
                Operand::RegArrangement { reg: format!("v{}", rd), arrangement: bad.into() },
                Operand::RegArrangement { reg: format!("v{}", rn), arrangement: bad.into() },
            ];
            match encode_rbit(&same) {
                Ok(EncodeResult::Word(w)) => prop_assert!(false,
                    "RBIT v{}.{}, v{}.{} should be Err (invalid arrangement), got Ok({:#010X})",
                    rd, bad, rn, bad, w),
                Ok(other) => prop_assert!(false,
                    "RBIT v{}.{}, v{}.{} should be Err (invalid arrangement), got Ok({:?})",
                    rd, bad, rn, bad, other),
                Err(_) => {}
            }
            // Mismatched source arrangement must also be rejected; the
            // current code ignores it and returns the dest-arrangement word.
            let mismatch = vec![
                Operand::RegArrangement { reg: format!("v{}", rd), arrangement: "16b".into() },
                Operand::RegArrangement { reg: format!("v{}", rn), arrangement: "8b".into() },
            ];
            match encode_rbit(&mismatch) {
                Ok(EncodeResult::Word(w)) => prop_assert!(false,
                    "RBIT v{}.16b, v{}.8b should be Err (mismatched arrangement), got Ok({:#010X})",
                    rd, rn, w),
                Ok(other) => prop_assert!(false,
                    "RBIT v{}.16b, v{}.8b should be Err (mismatched arrangement), got Ok({:?})",
                    rd, rn, other),
                Err(_) => {}
            }
        }
    }
}
