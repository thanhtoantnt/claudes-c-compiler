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
    }
}
