use super::*;
use crate::backend::arm::assembler::parser::Operand;

// ── Compare ──────────────────────────────────────────────────────────────

pub(crate) fn encode_cmp(operands: &[Operand]) -> Result<EncodeResult, String> {
    // CMP Rn, op -> SUBS XZR, Rn, op
    let mut new_ops = vec![Operand::Reg("xzr".to_string())];
    new_ops.extend(operands.iter().cloned());
    // Determine if 32-bit or 64-bit from the first operand
    let is_32 = if let Some(Operand::Reg(r)) = operands.first() {
        is_32bit_reg(r)
    } else {
        false
    };
    if is_32 {
        new_ops[0] = Operand::Reg("wzr".to_string());
    }
    encode_add_sub(&new_ops, true, true)
}

pub(crate) fn encode_cmn(operands: &[Operand]) -> Result<EncodeResult, String> {
    // CMN Rn, op -> ADDS XZR, Rn, op
    let mut new_ops = vec![Operand::Reg("xzr".to_string())];
    new_ops.extend(operands.iter().cloned());
    let is_32 = if let Some(Operand::Reg(r)) = operands.first() {
        is_32bit_reg(r)
    } else {
        false
    };
    if is_32 {
        new_ops[0] = Operand::Reg("wzr".to_string());
    }
    encode_add_sub(&new_ops, false, true)
}

pub(crate) fn encode_tst(operands: &[Operand]) -> Result<EncodeResult, String> {
    // TST Rn, op -> ANDS XZR, Rn, op
    let mut new_ops = vec![Operand::Reg("xzr".to_string())];
    new_ops.extend(operands.iter().cloned());
    let is_32 = if let Some(Operand::Reg(r)) = operands.first() {
        is_32bit_reg(r)
    } else {
        false
    };
    if is_32 {
        new_ops[0] = Operand::Reg("wzr".to_string());
    }
    encode_logical(&new_ops, 0b11)
}

pub(crate) fn encode_ccmp_ccmn(operands: &[Operand], is_ccmp: bool) -> Result<EncodeResult, String> {
    // CCMP/CCMN Rn, #imm5, #nzcv, cond
    // The only difference: CCMP has bit 30 = 1, CCMN has bit 30 = 0
    let (rn, is_64) = get_reg(operands, 0)?;
    let sf = sf_bit(is_64);
    let op = if is_ccmp { 1u32 << 30 } else { 0u32 };

    if let (Some(Operand::Imm(imm5)), Some(Operand::Imm(nzcv)), Some(Operand::Cond(cond))) =
        (operands.get(1), operands.get(2), operands.get(3))
    {
        let cond_val = encode_cond(cond).ok_or("invalid condition")?;
        let word = (sf << 31) | op | (1 << 29) | (0b11010010 << 21)
            | ((*imm5 as u32 & 0x1F) << 16) | (cond_val << 12) | (1 << 11) | (rn << 5) | (*nzcv as u32 & 0xF);
        return Ok(EncodeResult::Word(word));
    }

    // CCMP/CCMN Rn, Rm, #nzcv, cond
    if let (Some(Operand::Reg(rm_name)), Some(Operand::Imm(nzcv)), Some(Operand::Cond(cond))) =
        (operands.get(1), operands.get(2), operands.get(3))
    {
        let rm = parse_reg_num(rm_name).ok_or("invalid rm")?;
        let cond_val = encode_cond(cond).ok_or("invalid condition")?;
        let word = (sf << 31) | op | (1 << 29) | (0b11010010 << 21)
            | (rm << 16) | (cond_val << 12) | (rn << 5) | (*nzcv as u32 & 0xF);
        return Ok(EncodeResult::Word(word));
    }

    let name = if is_ccmp { "ccmp" } else { "ccmn" };
    Err(format!("unsupported {} operands", name))
}

// ── Conditional select ───────────────────────────────────────────────────

pub(crate) fn encode_csel(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let cond = match operands.get(3) {
        Some(Operand::Cond(c)) => encode_cond(c).ok_or("invalid cond")?,
        _ => return Err("csel requires condition".to_string()),
    };
    let sf = sf_bit(is_64);
    let word = ((sf << 31) | (0b11010100 << 21)
        | (rm << 16) | (cond << 12)) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_csinc(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let cond = match operands.get(3) {
        Some(Operand::Cond(c)) => encode_cond(c).ok_or("invalid cond")?,
        _ => return Err("csinc requires condition".to_string()),
    };
    let sf = sf_bit(is_64);
    let word = (sf << 31) | (0b11010100 << 21)
        | (rm << 16) | (cond << 12) | (0b01 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_csinv(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let cond = match operands.get(3) {
        Some(Operand::Cond(c)) => encode_cond(c).ok_or("invalid cond")?,
        _ => return Err("csinv requires condition".to_string()),
    };
    let sf = sf_bit(is_64);
    let word = (((sf << 31) | (1 << 30)) | (0b11010100 << 21)
        | (rm << 16) | (cond << 12)) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_csneg(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let cond = match operands.get(3) {
        Some(Operand::Cond(c)) => encode_cond(c).ok_or("invalid cond")?,
        _ => return Err("csneg requires condition".to_string()),
    };
    let sf = sf_bit(is_64);
    let word = ((sf << 31) | (1 << 30)) | (0b11010100 << 21)
        | (rm << 16) | (cond << 12) | (0b01 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_cset(operands: &[Operand]) -> Result<EncodeResult, String> {
    // CSET Rd, cond -> CSINC Rd, XZR, XZR, invert(cond)
    let (rd, is_64) = get_reg(operands, 0)?;
    let cond = match operands.get(1) {
        Some(Operand::Cond(c)) => encode_cond(c).ok_or("invalid cond")?,
        _ => return Err("cset requires condition".to_string()),
    };
    let sf = sf_bit(is_64);
    let inv_cond = cond ^ 1; // invert least significant bit
    let word = (sf << 31) | (0b11010100 << 21)
        | (0b11111 << 16) | (inv_cond << 12) | (0b01 << 10) | (0b11111 << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_csetm(operands: &[Operand]) -> Result<EncodeResult, String> {
    // CSETM Rd, cond -> CSINV Rd, XZR, XZR, invert(cond)
    let (rd, is_64) = get_reg(operands, 0)?;
    let cond = match operands.get(1) {
        Some(Operand::Cond(c)) => encode_cond(c).ok_or("invalid cond")?,
        _ => return Err("csetm requires condition".to_string()),
    };
    let sf = sf_bit(is_64);
    let inv_cond = cond ^ 1;
    let word = (((sf << 31) | (1 << 30)) | (0b11010100 << 21)
        | (0b11111 << 16) | (inv_cond << 12)) | (0b11111 << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── Branches ─────────────────────────────────────────────────────────────

pub(crate) fn encode_branch(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (sym, addend) = get_symbol(operands, 0)?;
    // B: 000101 imm26 (filled by linker/assembler)
    Ok(EncodeResult::WordWithReloc {
        word: 0b000101 << 26,
        reloc: Relocation {
            reloc_type: RelocType::Jump26,
            symbol: sym,
            addend,
        },
    })
}

pub(crate) fn encode_bl(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (sym, addend) = get_symbol(operands, 0)?;
    // BL: 100101 imm26
    Ok(EncodeResult::WordWithReloc {
        word: 0b100101 << 26,
        reloc: Relocation {
            reloc_type: RelocType::Call26,
            symbol: sym,
            addend,
        },
    })
}

pub(crate) fn encode_cond_branch(cond: &str, operands: &[Operand]) -> Result<EncodeResult, String> {
    let cond_val = encode_cond(cond).ok_or_else(|| format!("unknown condition: {}", cond))?;
    let (sym, addend) = get_symbol(operands, 0)?;
    // B.cond: 01010100 imm19 0 cond
    let word = (0b01010100 << 24) | cond_val;
    Ok(EncodeResult::WordWithReloc {
        word,
        reloc: Relocation {
            reloc_type: RelocType::CondBr19,
            symbol: sym,
            addend,
        },
    })
}

pub(crate) fn encode_br(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rn, _) = get_reg(operands, 0)?;
    // BR: 1101011 0000 11111 000000 Rn 00000
    let word = 0xd61f0000 | (rn << 5);
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_blr(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rn, _) = get_reg(operands, 0)?;
    // BLR: 1101011 0001 11111 000000 Rn 00000
    let word = 0xd63f0000 | (rn << 5);
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_ret(operands: &[Operand]) -> Result<EncodeResult, String> {
    let rn = if operands.is_empty() {
        30 // default to x30 (LR)
    } else {
        get_reg(operands, 0)?.0
    };
    // RET: 1101011 0010 11111 000000 Rn 00000
    let word = 0xd65f0000 | (rn << 5);
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_cbz(operands: &[Operand], is_nz: bool) -> Result<EncodeResult, String> {
    let (rt, is_64) = get_reg(operands, 0)?;
    let (sym, addend) = get_symbol(operands, 1)?;
    let sf = sf_bit(is_64);
    let op = if is_nz { 1u32 } else { 0u32 };
    // CBZ/CBNZ: sf 011010 op imm19 Rt
    let word = (sf << 31) | (0b011010 << 25) | (op << 24) | rt;
    Ok(EncodeResult::WordWithReloc {
        word,
        reloc: Relocation {
            reloc_type: RelocType::CondBr19,
            symbol: sym,
            addend,
        },
    })
}

pub(crate) fn encode_tbz(operands: &[Operand], is_nz: bool) -> Result<EncodeResult, String> {
    let (rt, _) = get_reg(operands, 0)?;
    let bit = get_imm(operands, 1)?;
    let (sym, addend) = get_symbol(operands, 2)?;
    let b5 = ((bit as u32) >> 5) & 1;
    let b40 = (bit as u32) & 0x1F;
    let op = if is_nz { 1u32 } else { 0u32 };
    // TBZ/TBNZ: b5 011011 op b40 imm14 Rt
    let word = (b5 << 31) | (0b011011 << 25) | (op << 24) | (b40 << 19) | rt;
    Ok(EncodeResult::WordWithReloc {
        word,
        reloc: Relocation {
            reloc_type: RelocType::TstBr14,
            symbol: sym,
            addend,
        },
    })
}

// ── Additional conditional operations ────────────────────────────────────

/// Encode CNEG Rd, Rn, cond -> CSNEG Rd, Rn, Rn, invert(cond)
pub(crate) fn encode_cneg(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let cond = match operands.get(2) {
        Some(Operand::Cond(c)) => encode_cond(c).ok_or_else(|| format!("unknown condition: {}", c))?,
        _ => return Err("cneg: expected condition code as third operand".to_string()),
    };
    let sf = sf_bit(is_64);
    // Invert the condition (flip bit 0)
    let inv_cond = cond ^ 1;
    // CSNEG: sf 1 0 11010100 Rm cond 0 1 Rn Rd (with Rm = Rn)
    let word = (sf << 31) | (1 << 30) | (0b011010100 << 21) | (rn << 16)
        | (inv_cond << 12) | (0b01 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode CINC Rd, Rn, cond -> CSINC Rd, Rn, Rn, invert(cond)
pub(crate) fn encode_cinc(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let cond = match operands.get(2) {
        Some(Operand::Cond(c)) => encode_cond(c).ok_or_else(|| format!("unknown condition: {}", c))?,
        _ => return Err("cinc: expected condition code as third operand".to_string()),
    };
    let sf = sf_bit(is_64);
    let inv_cond = cond ^ 1;
    // CSINC: sf 0 0 11010100 Rm cond 0 1 Rn Rd (with Rm = Rn)
    let word = (sf << 31) | (0b011010100 << 21) | (rn << 16)
        | (inv_cond << 12) | (0b01 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode CINV Rd, Rn, cond -> CSINV Rd, Rn, Rn, invert(cond)
pub(crate) fn encode_cinv(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let cond = match operands.get(2) {
        Some(Operand::Cond(c)) => encode_cond(c).ok_or_else(|| format!("unknown condition: {}", c))?,
        _ => return Err("cinv: expected condition code as third operand".to_string()),
    };
    let sf = sf_bit(is_64);
    let inv_cond = cond ^ 1;
    // CSINV: sf 1 0 11010100 Rm cond 0 0 Rn Rd (with Rm = Rn)
    let word = (sf << 31) | (1 << 30) | (0b011010100 << 21) | (rn << 16)
        | (inv_cond << 12) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

#[cfg(test)]
mod prop_ccmp_ccmn_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- Opcode constants for the CCMP/CCMN instruction class ----
    // Bits always 1: bit 29 (S) + bits 28,27,25,22 (opcode 11010010 @ [28:21]).
    const FIXED_SET: u32 = (1u32 << 29) | (0b11010010u32 << 21); // == 0x3A400000
    // Bits always 0: 26,24,23,21 (opcode tail) + 10,4 (gaps between fields).
    const FIXED_ZERO: u32 = 0x05A0_0410;

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand], is_ccmp: bool) -> u32 {
        word_of(encode_ccmp_ccmn(ops, is_ccmp))
    }

    /// Canonical ARM condition-code table mirroring `encode_cond`: (name, 4-bit value).
    const COND_TABLE: &[(&str, u32)] = &[
        ("eq", 0), ("ne", 1), ("cs", 2), ("hs", 2), ("cc", 3), ("lo", 3),
        ("mi", 4), ("pl", 5), ("vs", 6), ("vc", 7), ("hi", 8), ("ls", 9),
        ("ge", 10), ("lt", 11), ("gt", 12), ("le", 13), ("al", 14), ("nv", 15),
    ];

    prop_compose! {
        fn arb_reg()(n in 0u32..=30u32, is_64 in any::<bool>()) -> (String, u32) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            (name, n)
        }
    }

    proptest! {
        // Property A — full structural / field-placement oracle.
        // Verifies the fixed opcode bits and the position+mask of every field.
        #[test]
        fn prop_opcode_structure_and_fields(
            (rn_name, rn_num) in arb_reg(),
            (rm_name, rm_num) in arb_reg(),
            imm5 in 0i64..=255i64,
            nzcv in 0i64..=255i64,
            cond_idx in 0usize..COND_TABLE.len(),
            is_ccmp in any::<bool>(),
            is_imm in any::<bool>(),
        ) {
            let (cond_name, cond_val) = COND_TABLE[cond_idx];
            let ops: Vec<Operand> = if is_imm {
                vec![Operand::Reg(rn_name.clone()), Operand::Imm(imm5),
                     Operand::Imm(nzcv), Operand::Cond(cond_name.to_string())]
            } else {
                vec![Operand::Reg(rn_name.clone()), Operand::Reg(rm_name.clone()),
                     Operand::Imm(nzcv), Operand::Cond(cond_name.to_string())]
            };
            let word = enc(&ops, is_ccmp);

            // Fixed opcode bits.
            prop_assert_eq!(word & FIXED_SET, FIXED_SET);
            prop_assert_eq!(word & FIXED_ZERO, 0u32);
            // sf bit [31] tracks the width of Rn.
            let expected_sf = if rn_name.starts_with('x') { 1u32 } else { 0u32 };
            prop_assert_eq!((word >> 31) & 1, expected_sf);
            // op bit [30]: CCMP => 1, CCMN => 0.
            prop_assert_eq!((word >> 30) & 1, if is_ccmp { 1 } else { 0 });
            // cond field [15:12].
            prop_assert_eq!((word >> 12) & 0xF, cond_val);
            // Rn field [9:5].
            prop_assert_eq!((word >> 5) & 0x1F, rn_num);
            // nzcv field [3:0] is masked to a nibble.
            prop_assert_eq!(word & 0xF, (nzcv as u32) & 0xF);
            // o3 bit [11]: 1 for immediate form, 0 for register form.
            prop_assert_eq!((word >> 11) & 1, if is_imm { 1 } else { 0 });
            // imm5 / Rm field [20:16].
            if is_imm {
                prop_assert_eq!((word >> 16) & 0x1F, (imm5 as u32) & 0x1F);
            } else {
                prop_assert_eq!((word >> 16) & 0x1F, rm_num);
            }
        }

        // Property B — differential: CCMP and CCMN differ ONLY in bit 30.
        #[test]
        fn prop_ccmp_xor_ccmn_is_bit30(
            (rn_name, _) in arb_reg(),
            (rm_name, _) in arb_reg(),
            imm5 in 0i64..=255i64,
            nzcv in 0i64..=255i64,
            cond_idx in 0usize..COND_TABLE.len(),
            is_imm in any::<bool>(),
        ) {
            let (cond_name, _) = COND_TABLE[cond_idx];
            let ops: Vec<Operand> = if is_imm {
                vec![Operand::Reg(rn_name), Operand::Imm(imm5),
                     Operand::Imm(nzcv), Operand::Cond(cond_name.to_string())]
            } else {
                vec![Operand::Reg(rn_name), Operand::Reg(rm_name),
                     Operand::Imm(nzcv), Operand::Cond(cond_name.to_string())]
            };
            let ccmp = enc(&ops, true);
            let ccmn = enc(&ops, false);
            prop_assert_eq!(ccmp ^ ccmn, 1u32 << 30);
        }

        // Property C — differential: 64- vs 32-bit register differ ONLY in bit 31 (sf).
        #[test]
        fn prop_sf_bit_is_bit31(
            rn_num in 0u32..=30u32,
            imm5 in 0i64..=255i64,
            nzcv in 0i64..=255i64,
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, _) = COND_TABLE[cond_idx];
            let ops64 = vec![Operand::Reg(format!("x{}", rn_num)), Operand::Imm(imm5),
                             Operand::Imm(nzcv), Operand::Cond(cond_name.to_string())];
            let ops32 = vec![Operand::Reg(format!("w{}", rn_num)), Operand::Imm(imm5),
                             Operand::Imm(nzcv), Operand::Cond(cond_name.to_string())];
            let w64 = enc(&ops64, true);
            let w32 = enc(&ops32, true);
            prop_assert_eq!(w64 ^ w32, 1u32 << 31);
        }

        // Property D — nzcv is masked to 4 bits (idempotent under & 0xF).
        #[test]
        fn prop_nzcv_masked_to_nibble(
            (rn_name, _) in arb_reg(),
            imm5 in 0i64..=255i64,
            nzcv in 0i64..=65535i64,
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, _) = COND_TABLE[cond_idx];
            let mk = |n: i64| vec![Operand::Reg(rn_name.clone()), Operand::Imm(imm5),
                                   Operand::Imm(n), Operand::Cond(cond_name.to_string())];
            let full = enc(&mk(nzcv), true);
            let masked = enc(&mk(nzcv & 0xF), true);
            // Low nibble equals nzcv & 0xF ...
            prop_assert_eq!(full & 0xF, (nzcv as u32) & 0xF);
            // ... and the rest of the word is independent of nzcv's high bits.
            prop_assert_eq!(full & !0xFu32, masked & !0xFu32);
        }

        // Property E — differential: immediate vs register forms differ ONLY in
        // bit 11 (o3) when imm5 equals the Rm register number.
        #[test]
        fn prop_imm_vs_reg_differ_only_bit11(
            (rn_name, _) in arb_reg(),
            field_val in 0u32..=30u32, // used both as imm5 and as the Rm number
            nzcv in 0i64..=255i64,
            cond_idx in 0usize..COND_TABLE.len(),
            is_ccmp in any::<bool>(),
        ) {
            let (cond_name, _) = COND_TABLE[cond_idx];
            let imm_ops = vec![Operand::Reg(rn_name.clone()), Operand::Imm(field_val as i64),
                               Operand::Imm(nzcv), Operand::Cond(cond_name.to_string())];
            let reg_ops = vec![Operand::Reg(rn_name), Operand::Reg(format!("x{}", field_val)),
                               Operand::Imm(nzcv), Operand::Cond(cond_name.to_string())];
            let imm_word = enc(&imm_ops, is_ccmp);
            let reg_word = enc(&reg_ops, is_ccmp);
            prop_assert_eq!(imm_word ^ reg_word, 1u32 << 11);
        }

        // Property F — negative contract (immediate-range validation).
        // Per ARM ARM (CCMP/CCMN, immediate form): `imm5` is a 5-bit UNSIGNED
        // immediate (valid 0..=31) and `nzcv` is a 4-bit field (valid 0..=15).
        // Any operand outside these ranges is architecturally invalid and the
        // encoder MUST reject it with Err rather than silently truncating with
        // `& 0x1F` / `& 0xF`. No cited spec permits wrapping for these fields.
        #[test]
        fn prop_rejects_out_of_range_immediates(
            (rn_name, _) in arb_reg(),
            out_imm5 in 32i64..=4096i64,
            out_nzcv in 16i64..=255i64,
            cond_idx in 0usize..COND_TABLE.len(),
            is_ccmp in any::<bool>(),
        ) {
            let (cond_name, _) = COND_TABLE[cond_idx];

            // Out-of-range imm5 (too large) must be rejected, not truncated.
            let big_imm = vec![Operand::Reg(rn_name.clone()), Operand::Imm(out_imm5),
                               Operand::Imm(0), Operand::Cond(cond_name.to_string())];
            prop_assert!(
                encode_ccmp_ccmn(&big_imm, is_ccmp).is_err(),
                "imm5={} should be rejected (valid range 0..=31), got {:?}",
                out_imm5, encode_ccmp_ccmn(&big_imm, is_ccmp)
            );
            // Negative imm5 must be rejected (the field is unsigned).
            let neg_imm = vec![Operand::Reg(rn_name.clone()), Operand::Imm(-1),
                               Operand::Imm(0), Operand::Cond(cond_name.to_string())];
            prop_assert!(
                encode_ccmp_ccmn(&neg_imm, is_ccmp).is_err(),
                "negative imm5=-1 should be rejected"
            );
            // Out-of-range nzcv must be rejected, not truncated.
            let bad_nzcv = vec![Operand::Reg(rn_name), Operand::Imm(0),
                                Operand::Imm(out_nzcv), Operand::Cond(cond_name.to_string())];
            prop_assert!(
                encode_ccmp_ccmn(&bad_nzcv, is_ccmp).is_err(),
                "nzcv={} should be rejected (valid range 0..=15)", out_nzcv
            );
        }
    }
}

#[cfg(test)]
mod prop_encode_cmn_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- CMN semantics ----
    // CMN Rn, op -> ADDS XZR/WZR, Rn, op, i.e. encode_add_sub(.., is_sub=false, set_flags=true).
    //   op bit [30] = 0   (ADD, not SUB)
    //   S  bit [29] = 1   (set flags -> ADDS)
    //   sf bit [31]       tracks the width of the FIRST operand (Rn)
    //   Rd field [4:0] = 31 (xzr / wzr)
    // Two operand shapes:
    //   * immediate:   sf 0 1 100010 sh imm12 Rn Rd      (bits [28:24]=10001, [23]=0)
    //   * shifted-reg: sf 0 1 01011 shift 0 Rm imm6 Rn Rd (bits [28:24]=01011, [21]=0)
    const OP_BIT: u32 = 1u32 << 30;  // [30]
    const S_BIT: u32 = 1u32 << 29;   // [29]
    const SF_BIT: u32 = 1u32 << 31;  // [31]
    const RD_MASK: u32 = 0x1F;       // [4:0]

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word_of(encode_cmn(ops))
    }

    prop_compose! {
        fn arb_reg()(n in 0u32..=30u32, is_64 in any::<bool>()) -> (String, u32) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            (name, n)
        }
    }

    proptest! {
        // Property A — immediate-form structural / field-placement oracle.
        // CMN Rn, #imm encodes as ADDS (immediate): the fixed opcode bits and
        // the exact position+mask of every populated field (sh, imm12, Rn, Rd).
        #[test]
        fn prop_imm_form_structure(
            (rn_name, rn_num) in arb_reg(),
            imm in 0i64..=0xFFF, // fits unshifted in the 12-bit immediate
        ) {
            let ops = vec![Operand::Reg(rn_name.clone()), Operand::Imm(imm)];
            let word = enc(&ops);

            // sf bit [31] tracks Rn width (xN -> 1, wN -> 0).
            let expected_sf = u32::from(rn_name.starts_with('x'));
            prop_assert_eq!(word & SF_BIT, expected_sf << 31);
            // op bit [30] = 0 (ADD, not SUB).
            prop_assert_eq!(word & OP_BIT, 0u32);
            // S bit [29] = 1 (set flags -> ADDS).
            prop_assert_eq!(word & S_BIT, S_BIT);
            // bits [28:24] = 10001 and bit [23] = 0 (immediate add/sub opcode 100010).
            prop_assert_eq!((word >> 24) & 0x1F, 0b10001u32);
            prop_assert_eq!((word >> 23) & 1, 0u32);
            // sh bit [22] = 0 (no shift for an unshifted small immediate).
            prop_assert_eq!((word >> 22) & 1, 0u32);
            // imm12 [21:10].
            prop_assert_eq!((word >> 10) & 0xFFF, imm as u32);
            // Rn field [9:5].
            prop_assert_eq!((word >> 5) & 0x1F, rn_num);
            // Rd field [4:0] = 31 (xzr / wzr).
            prop_assert_eq!(word & RD_MASK, 31u32);
        }

        // Property B — the defining CMN invariant: the destination register is
        // ALWAYS the zero register (Rd == 31), for BOTH forms and BOTH widths.
        #[test]
        fn prop_rd_always_zr(
            (rn_name, _) in arb_reg(),
            (rm_name, _) in arb_reg(),
            imm in 0i64..=0xFFF,
        ) {
            let imm_ops = vec![Operand::Reg(rn_name.clone()), Operand::Imm(imm)];
            let reg_ops = vec![Operand::Reg(rn_name), Operand::Reg(rm_name)];
            prop_assert_eq!(enc(&imm_ops) & RD_MASK, 31u32);
            prop_assert_eq!(enc(&reg_ops) & RD_MASK, 31u32);
        }

        // Property C — differential vs CMP. CMN (ADDS) and CMP (SUBS) differ
        // ONLY in the op bit [30] (ADD vs SUB); every other field is identical.
        // Restricted to positive immediates so the add/sub sign-negation logic
        // in encode_add_sub does not engage.
        #[test]
        fn prop_cmn_vs_cmp_differs_only_op_bit(
            (rn_name, _) in arb_reg(),
            (rm_name, _) in arb_reg(),
            imm in 1i64..=0xFFF,
        ) {
            // Immediate form.
            let imm_ops = vec![Operand::Reg(rn_name.clone()), Operand::Imm(imm)];
            prop_assert_eq!(
                word_of(encode_cmn(&imm_ops)) ^ word_of(encode_cmp(&imm_ops)),
                1u32 << 30
            );
            // Register form.
            let reg_ops = vec![Operand::Reg(rn_name), Operand::Reg(rm_name)];
            prop_assert_eq!(
                word_of(encode_cmn(&reg_ops)) ^ word_of(encode_cmp(&reg_ops)),
                1u32 << 30
            );
        }

        // Property D — differential: 64- vs 32-bit Rn differ ONLY in bit 31 (sf).
        #[test]
        fn prop_sf_bit_is_bit31(
            rn_num in 0u32..=30u32,
            imm in 0i64..=0xFFF,
        ) {
            let ops64 = vec![Operand::Reg(format!("x{}", rn_num)), Operand::Imm(imm)];
            let ops32 = vec![Operand::Reg(format!("w{}", rn_num)), Operand::Imm(imm)];
            prop_assert_eq!(enc(&ops64) ^ enc(&ops32), 1u32 << 31);
        }

        // Property E — shifted-register-form structural / field-placement oracle.
        // CMN Rn, Rm encodes as ADDS (shifted register).
        #[test]
        fn prop_reg_form_structure(
            (rn_name, rn_num) in arb_reg(),
            (rm_name, rm_num) in arb_reg(),
        ) {
            let ops = vec![Operand::Reg(rn_name.clone()), Operand::Reg(rm_name.clone())];
            let word = enc(&ops);

            let expected_sf = u32::from(rn_name.starts_with('x'));
            prop_assert_eq!(word & SF_BIT, expected_sf << 31);
            prop_assert_eq!(word & OP_BIT, 0u32);
            prop_assert_eq!(word & S_BIT, S_BIT);
            // bits [28:24] = 01011 (shifted-register add/sub opcode).
            prop_assert_eq!((word >> 24) & 0x1F, 0b01011u32);
            // shift type [23:22] = 0 (no explicit shift).
            prop_assert_eq!((word >> 22) & 0x3, 0u32);
            // bit [21] = 0 (fixed).
            prop_assert_eq!((word >> 21) & 1, 0u32);
            // Rm field [20:16].
            prop_assert_eq!((word >> 16) & 0x1F, rm_num);
            // imm6 [15:10] = 0 (no shift amount).
            prop_assert_eq!((word >> 10) & 0x3F, 0u32);
            // Rn field [9:5].
            prop_assert_eq!((word >> 5) & 0x1F, rn_num);
            // Rd field [4:0] = 31.
            prop_assert_eq!(word & RD_MASK, 31u32);
        }

        // Property F — negative contract (immediate-range validation).
        // Per ARM ARM, ADDS (immediate) encodes a 12-bit unsigned immediate,
        // optionally left-shifted by 12. A value that fits NEITHER an unshifted
        // imm12 (0..=0xFFF) NOR a shifted one (multiple of 0x1000, <=0xFFF000)
        // is architecturally invalid and the encoder MUST reject it with Err
        // rather than silently truncating into the imm12 field via `& 0xFFF`.
        // No cited spec permits wrapping for this field.
        #[test]
        fn prop_rejects_unrepresentable_immediate(
            (rn_name, _) in arb_reg(),
            bad in 0x1001i64..=0x1FFFi64,
        ) {
            // Filter out multiples of 0x1000, which would be auto-shifted and
            // legitimately accepted by encode_add_sub.
            prop_assume!(bad & 0xFFF != 0);
            let ops = vec![Operand::Reg(rn_name), Operand::Imm(bad)];
            prop_assert!(
                encode_cmn(&ops).is_err(),
                "imm=0x{:x} is not representable in imm12, must be rejected, got {:?}",
                bad, encode_cmn(&ops)
            );
        }

        // Property G — negative contract (explicit `lsl #12` immediate range).
        // CMN Rn, #imm, lsl #12 forwards verbatim to encode_add_sub's
        // explicit-shift branch. Per ARM ARM the imm12 field is a 12-bit
        // UNSIGNED value, so an immediate > 0xFFF is unrepresentable and MUST
        // be rejected with Err. The implementation instead masks with
        // `& 0xFFF`, silently truncating e.g. `cmn x1, #4097, lsl #12` into
        // `cmn x1, #1, lsl #12`. This is the same silent-truncation bug that
        // encode_cmp inherits from the same encode_add_sub path. GAS/LLVM-MC
        // reject this input. No cited spec permits wrapping.
        #[test]
        fn prop_rejects_oversized_lsl12_immediate(
            (rn_name, _) in arb_reg(),
            bad in 0x1001i64..=0x1FFFi64,
        ) {
            let ops = vec![
                Operand::Reg(rn_name),
                Operand::Imm(bad),
                Operand::Shift { kind: "lsl".to_string(), amount: 12 },
            ];
            prop_assert!(
                encode_cmn(&ops).is_err(),
                "cmn Rn, #0x{:x}, lsl #12 (imm > 0xFFF) must be rejected, not truncated, got {:?}",
                bad, encode_cmn(&ops)
            );
        }
    }
}

#[cfg(test)]
mod prop_encode_tbz_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- Opcode constants for the TBZ / TBNZ instruction class ----
    // Fixed-1 bits [30:25] = 0b011011.
    const OPCODE: u32 = 0b011011u32 << 25; // == 0x3600_0000
    const OPCODE_MASK: u32 = 0x7E00_0000;  // bits [30:25]
    // The imm14 branch-offset field [18:5] is filled in by the linker, so the
    // encoder must leave it zero. This is the only always-zero region.
    const FIXED_ZERO: u32 = 0x0007_FFE0;   // bits [18:5]

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::WordWithReloc { word, .. }) => word,
            other => panic!("expected WordWithReloc, got {:?}", other),
        }
    }

    fn reloc_of(r: Result<EncodeResult, String>) -> Relocation {
        match r {
            Ok(EncodeResult::WordWithReloc { reloc, .. }) => reloc,
            other => panic!("expected WordWithReloc, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand], is_nz: bool) -> u32 {
        word_of(encode_tbz(ops, is_nz))
    }

    prop_compose! {
        fn arb_reg()(n in 0u32..=30u32, is_64 in any::<bool>()) -> (String, u32) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            (name, n)
        }
    }

    /// Generate a symbol-like operand together with the (symbol, addend) the
    /// encoder is expected to forward into the relocation.
    prop_compose! {
        fn arb_sym_operand()(
            sym in "[a-z][a-z0-9_]{0,7}",
            has_off in any::<bool>(),
            off in -4096i64..=4096i64,
        ) -> (Operand, String, i64) {
            if has_off {
                (Operand::SymbolOffset(sym.clone(), off), sym, off)
            } else {
                (Operand::Symbol(sym.clone()), sym.clone(), 0)
            }
        }
    }

    proptest! {
        // Property A — full structural / field-placement oracle.
        // Verifies the fixed opcode bits, the always-zero imm14 gap, and the
        // position+mask of every populated field (b5, op, b40, Rt).
        #[test]
        fn prop_opcode_structure_and_fields(
            (rt_name, rt_num) in arb_reg(),
            bit in 0i64..=63i64, // valid AArch64 bit position
            (sym_op, _sym, _off) in arb_sym_operand(),
            is_nz in any::<bool>(),
        ) {
            let ops = vec![Operand::Reg(rt_name), Operand::Imm(bit), sym_op];
            let word = enc(&ops, is_nz);

            // Fixed opcode bits [30:25] = 0b011011.
            prop_assert_eq!(word & OPCODE_MASK, OPCODE);
            // Linker-reserved imm14 field must be zero in the encoder output.
            prop_assert_eq!(word & FIXED_ZERO, 0u32);
            // op bit [24]: TBNZ => 1, TBZ => 0.
            prop_assert_eq!((word >> 24) & 1, if is_nz { 1 } else { 0 });
            // b5 [31] is bit 5 of the immediate.
            prop_assert_eq!((word >> 31) & 1, ((bit as u32) >> 5) & 1);
            // b40 [23:19] is the low 5 bits of the immediate.
            prop_assert_eq!((word >> 19) & 0x1F, (bit as u32) & 0x1F);
            // Rt field [4:0].
            prop_assert_eq!(word & 0x1F, rt_num);
        }

        // Property B — differential: TBZ and TBNZ differ ONLY in bit 24 (op).
        #[test]
        fn prop_tbz_xor_tbnz_is_bit24(
            (rt_name, _) in arb_reg(),
            bit in 0i64..=63i64,
            sym in "[a-z][a-z0-9_]{0,7}",
        ) {
            let ops = vec![Operand::Reg(rt_name), Operand::Imm(bit),
                           Operand::Symbol(sym)];
            prop_assert_eq!(enc(&ops, false) ^ enc(&ops, true), 1u32 << 24);
        }

        // Property C — differential: register width is irrelevant. The TBZ
        // format has no sf bit (bit 31 is reused for b5), so x{N} and w{N}
        // must encode to identical words.
        #[test]
        fn prop_width_independent(
            n in 0u32..=30u32,
            bit in 0i64..=63i64,
            sym in "[a-z][a-z0-9_]{0,7}",
            is_nz in any::<bool>(),
        ) {
            let ops64 = vec![Operand::Reg(format!("x{}", n)), Operand::Imm(bit),
                             Operand::Symbol(sym.clone())];
            let ops32 = vec![Operand::Reg(format!("w{}", n)), Operand::Imm(bit),
                             Operand::Symbol(sym)];
            prop_assert_eq!(enc(&ops64, is_nz), enc(&ops32, is_nz));
        }

        // Property D — round-trip: for a valid bit position the split (b5,b40)
        // reconstructs the original bit number: (b5<<5) | b40 == bit.
        #[test]
        fn prop_bit_round_trips(
            (rt_name, _) in arb_reg(),
            bit in 0i64..=63i64,
            sym in "[a-z][a-z0-9_]{0,7}",
            is_nz in any::<bool>(),
        ) {
            let ops = vec![Operand::Reg(rt_name), Operand::Imm(bit),
                           Operand::Symbol(sym)];
            let word = enc(&ops, is_nz);
            let b5 = (word >> 31) & 1;
            let b40 = (word >> 19) & 0x1F;
            prop_assert_eq!((b5 << 5) | b40, bit as u32);
        }

        // Property E — relocation contract: the result carries a TstBr14
        // relocation whose symbol and addend exactly mirror the input operand.
        #[test]
        fn prop_reloc_is_tstbr14_with_symbol(
            (rt_name, _) in arb_reg(),
            bit in 0i64..=63i64,
            (sym_op, sym, off) in arb_sym_operand(),
            is_nz in any::<bool>(),
        ) {
            let ops = vec![Operand::Reg(rt_name), Operand::Imm(bit), sym_op];
            let reloc = reloc_of(encode_tbz(&ops, is_nz));
            prop_assert!(matches!(reloc.reloc_type, RelocType::TstBr14));
            prop_assert_eq!(reloc.symbol, sym);
            prop_assert_eq!(reloc.addend, off);
        }

        // Property F — negative contract (immediate-range validation).
        // Per ARM ARM (TBZ/TBNZ, C5.6.27/28): the test bit is the 6-bit field
        // b5:b40, an UNSIGNED value valid only in 0..=63. No spec permits
        // wrapping/truncation. An immediate above 63 MUST be rejected with Err,
        // not silently masked into the 6-bit field via `& 0x1F` / `>> 5 & 1`.
        #[test]
        fn prop_rejects_bit_above_63(
            (rt_name, _) in arb_reg(),
            bit in 64i64..=4096i64,
            is_nz in any::<bool>(),
        ) {
            let ops = vec![Operand::Reg(rt_name), Operand::Imm(bit),
                           Operand::Symbol("target".into())];
            prop_assert!(
                encode_tbz(&ops, is_nz).is_err(),
                "bit={} (valid range 0..=63) must be rejected, got {:?}",
                bit, encode_tbz(&ops, is_nz)
            );
        }

        // Property G — negative contract: the test-bit field is unsigned, so a
        // negative immediate MUST be rejected. The implementation does
        // `bit as u32`, which silently truncates a negative i64 to its low 32
        // bits before masking to 6 bits — that is not a valid encoding.
        #[test]
        fn prop_rejects_negative_bit(
            (rt_name, _) in arb_reg(),
            bit in -4096i64..=-1i64,
            is_nz in any::<bool>(),
        ) {
            let ops = vec![Operand::Reg(rt_name), Operand::Imm(bit),
                           Operand::Symbol("target".into())];
            prop_assert!(
                encode_tbz(&ops, is_nz).is_err(),
                "negative bit={} must be rejected (field is unsigned 0..=63)",
                bit
            );
        }
    }
}

#[cfg(test)]
mod prop_encode_branch_tests {
    use super::*;
    use proptest::prelude::*;

    // B instruction (unconditional branch): opcode 0b000101 occupies bits
    // [31:26]; the imm26 offset field [25:0] is left zero for the linker to
    // fill via a Jump26 relocation.
    const B_OPCODE: u32 = 0b000101u32 << 26; // == 0x1400_0000
    const OPCODE_MASK: u32 = 0xFC00_0000;    // bits [31:26]
    const IMM26_MASK: u32 = 0x03FF_FFFF;     // bits [25:0]

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::WordWithReloc { word, .. }) => word,
            other => panic!("expected WordWithReloc, got {:?}", other),
        }
    }

    fn reloc_of(r: Result<EncodeResult, String>) -> Relocation {
        match r {
            Ok(EncodeResult::WordWithReloc { reloc, .. }) => reloc,
            other => panic!("expected WordWithReloc, got {:?}", other),
        }
    }

    // Build every operand kind that `get_symbol` accepts, paired with the
    /// (symbol, addend) the encoder is expected to forward into the relocation.
    /// Mirrors `get_symbol`'s documented forwarding table.
    fn accepted_operand_and_expected(
        sym: String,
        off: i64,
        kind_idx: usize,
    ) -> (Operand, String, i64) {
        let cases: Vec<(Operand, String, i64)> = vec![
            (Operand::Symbol(sym.clone()), sym.clone(), 0),
            (Operand::Label(sym.clone()), sym.clone(), 0),
            (Operand::SymbolOffset(sym.clone(), off), sym.clone(), off),
            (Operand::Modifier { kind: "lo12".into(), symbol: sym.clone() }, sym.clone(), 0),
            (Operand::ModifierOffset {
                kind: "lo12".into(), symbol: sym.clone(), offset: off,
            }, sym.clone(), off),
            (Operand::Reg(sym.clone()), sym.clone(), 0),
            (Operand::Cond(sym.clone()), sym.clone(), 0),
            (Operand::Barrier(sym.clone()), sym.clone(), 0),
        ];
        cases[kind_idx].clone()
    }

    prop_compose! {
        fn arb_accepted_symbol()(
            s in "[a-z][a-z0-9_]{0,7}",
            off in -8192i64..=8192i64,
            kind_idx in 0usize..8usize,
        ) -> (Operand, String, i64) {
            accepted_operand_and_expected(s, off, kind_idx)
        }
    }

    proptest! {
        // Property A — opcode structure oracle. The encoded word is fully
        // determined: opcode 0b000101 in bits [31:26] and the imm26 offset
        // field [25:0] is left zero for the linker to fill.
        #[test]
        fn prop_opcode_structure_and_imm26_zero(
            sym in "[a-z][a-z0-9_]{0,7}",
        ) {
            let ops = vec![Operand::Symbol(sym)];
            let word = word_of(encode_branch(&ops));
            prop_assert_eq!(word & OPCODE_MASK, B_OPCODE);
            prop_assert_eq!(word & IMM26_MASK, 0u32);
            // Equivalently: the word is exactly the fixed base, independent of operand.
            prop_assert_eq!(word, B_OPCODE);
        }

        // Property B — differential: B (encode_branch) and BL (encode_bl)
        // share the 100101/000101 layout and differ ONLY in bit 31 (the
        // link bit).
        #[test]
        fn prop_branch_vs_bl_differ_only_bit31(
            sym in "[a-z][a-z0-9_]{0,7}",
        ) {
            let ops = vec![Operand::Symbol(sym)];
            let b_word = word_of(encode_branch(&ops));
            let bl_word = word_of(encode_bl(&ops));
            prop_assert_eq!(b_word ^ bl_word, 1u32 << 31);
        }

        // Property C — relocation contract for the primary operand forms: the
        // result always carries a Jump26 relocation whose symbol & addend
        // exactly mirror the input operand.
        #[test]
        fn prop_reloc_is_jump26_primary_forms(
            sym in "[a-z][a-z0-9_]{0,7}",
            off in -8192i64..=8192i64,
            is_offset in any::<bool>(),
        ) {
            let (op, exp_off) = if is_offset {
                (Operand::SymbolOffset(sym.clone(), off), off)
            } else {
                (Operand::Symbol(sym.clone()), 0)
            };
            let reloc = reloc_of(encode_branch(&[op]));
            prop_assert!(matches!(reloc.reloc_type, RelocType::Jump26));
            prop_assert_eq!(reloc.symbol, sym);
            prop_assert_eq!(reloc.addend, exp_off);
        }

        // Property D — symbol forwarding across every operand kind that
        // `get_symbol` accepts (incl. parser-misclassified Reg/Cond/Barrier).
        #[test]
        fn prop_symbol_forwarding_all_accepted_kinds(
            (op, exp_sym, exp_off) in arb_accepted_symbol(),
        ) {
            let reloc = reloc_of(encode_branch(&[op]));
            prop_assert!(matches!(reloc.reloc_type, RelocType::Jump26));
            prop_assert_eq!(reloc.symbol, exp_sym);
            prop_assert_eq!(reloc.addend, exp_off);
        }

        // Property E — negative contract. Operand kinds that `get_symbol`
        // does NOT accept must make encode_branch return Err; no silent
        // encoding of an invalid branch target.
        #[test]
        fn prop_rejects_non_symbol_operands(
            idx in 0usize..11usize,
        ) {
            let rejected: Vec<Operand> = vec![
                Operand::Imm(42),
                Operand::Mem { base: "x0".into(), offset: 0 },
                Operand::MemExpr {
                    base: "x0".into(), expr: "foo".into(), writeback: false,
                },
                Operand::MemPreIndex { base: "x0".into(), offset: 8 },
                Operand::MemPostIndex { base: "x0".into(), offset: 8 },
                Operand::MemRegOffset {
                    base: "x0".into(), index: "x1".into(), extend: None, shift: None,
                },
                Operand::Shift { kind: "lsl".into(), amount: 2 },
                Operand::Extend { kind: "sxtw".into(), amount: 0 },
                Operand::Expr("x + y".into()),
                Operand::RegArrangement { reg: "v0".into(), arrangement: "16b".into() },
                Operand::RegLane { reg: "v0".into(), elem_size: "s".into(), index: 2 },
            ];
            let op = rejected[idx].clone();
            prop_assert!(
                encode_branch(&[op]).is_err(),
                "encode_branch should reject this operand"
            );
        }

        // Property F — word invariance across ALL accepted operand kinds.
        // Property A only proves the constant word 0x1400_0000 for the
        // `Symbol` form; Property D only checks the *relocation* across
        // kinds. This closes the gap: the instruction word must be the
        // fixed base for EVERY kind `get_symbol` accepts, because the
        // operand influences ONLY the relocation, never the word.
        #[test]
        fn prop_word_is_operand_independent(
            (op, _sym, _off) in arb_accepted_symbol(),
        ) {
            let ops = vec![op];
            prop_assert_eq!(word_of(encode_branch(&ops)), B_OPCODE);
        }

        // Property G — arity / negative contract. Property E covers the wrong
        // *type* of operand; this covers the *missing* operand. A branch
        // with no target must return Err — `get_symbol` reads operands[0]
        // unconditionally, so an empty vector cannot yield a relocation.
        #[test]
        fn prop_empty_operands_rejected(_dummy in 0u32..=0u32) {
            let empty: Vec<Operand> = vec![];
            prop_assert!(encode_branch(&empty).is_err());
            // A single non-symbol operand was already covered by Property E;
            // here the operand vector itself is empty.
        }

        // Property H — determinism / purity. Encoding the same operand
        // repeatedly yields bit-identical word AND relocation (symbol and
        // addend). encode_branch is a pure function of its operands.
        #[test]
        fn prop_encoding_is_deterministic(
            (op, exp_sym, exp_off) in arb_accepted_symbol(),
        ) {
            let ops = vec![op];
            prop_assert_eq!(
                word_of(encode_branch(&ops)),
                word_of(encode_branch(&ops))
            );
            let rel1 = reloc_of(encode_branch(&ops));
            let rel2 = reloc_of(encode_branch(&ops));
            prop_assert_eq!(&rel1.symbol, &rel2.symbol);
            prop_assert_eq!(&rel1.symbol, &exp_sym);
            prop_assert_eq!(rel1.addend, rel2.addend);
            prop_assert_eq!(rel1.addend, exp_off);
        }
    }
}

#[cfg(test)]
mod prop_encode_cond_branch_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- Opcode constants for the B.cond instruction class (ARM ARM C5.6.6) ----
    // B.cond = 0101 0100 | imm19[23:5] | 0[4] | cond[3:0].
    // The encoder hard-codes the top byte and relies on the linker (CondBr19)
    // to fill the imm19 branch-offset field, which it must leave at zero.
    const OPCODE: u32 = 0b0101_0100u32 << 24; // == 0x5400_0000
    const OPCODE_MASK: u32 = 0xFF00_0000;     // bits [31:24]
    const IMM19_MASK: u32 = 0x00FF_FFE0;      // bits [23:5] (linker-filled, must be zero)
    const O0_BIT: u32 = 1u32 << 4;            // bit [4] (must be zero)

    /// Canonical ARM condition-code table mirroring `encode_cond`.
    const COND_TABLE: &[(&str, u32)] = &[
        ("eq", 0), ("ne", 1), ("cs", 2), ("hs", 2), ("cc", 3), ("lo", 3),
        ("mi", 4), ("pl", 5), ("vs", 6), ("vc", 7), ("hi", 8), ("ls", 9),
        ("ge", 10), ("lt", 11), ("gt", 12), ("le", 13), ("al", 14), ("nv", 15),
    ];

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::WordWithReloc { word, .. }) => word,
            other => panic!("expected WordWithReloc, got {:?}", other),
        }
    }

    fn reloc_of(r: Result<EncodeResult, String>) -> Relocation {
        match r {
            Ok(EncodeResult::WordWithReloc { reloc, .. }) => reloc,
            other => panic!("expected WordWithReloc, got {:?}", other),
        }
    }

    fn enc(cond: &str, op: &Operand) -> u32 {
        word_of(encode_cond_branch(cond, &[op.clone()]))
    }

    /// Re-spell a (lowercase) condition code in a chosen ASCII case.
    ///   mode 0 → all upper ("EQ"), mode 1 → identity ("eq"), mode ≥2 → title ("Eq").
    fn cased(s: &str, mode: usize) -> String {
        s.chars().enumerate().map(|(i, c)| match mode {
            0 => c.to_ascii_uppercase(),
            1 => c,
            _ => if i == 0 { c.to_ascii_uppercase() } else { c },
        }).collect()
    }

    /// Mirrors `get_symbol`'s forwarding table: every operand kind it accepts
    /// and the (symbol, addend) the encoder must forward into the relocation.
    fn accepted_operand_and_expected(
        sym: String,
        off: i64,
        kind_idx: usize,
    ) -> (Operand, String, i64) {
        let cases: Vec<(Operand, String, i64)> = vec![
            (Operand::Symbol(sym.clone()), sym.clone(), 0),
            (Operand::Label(sym.clone()), sym.clone(), 0),
            (Operand::SymbolOffset(sym.clone(), off), sym.clone(), off),
            (Operand::Modifier { kind: "lo12".into(), symbol: sym.clone() }, sym.clone(), 0),
            (Operand::ModifierOffset {
                kind: "lo12".into(), symbol: sym.clone(), offset: off,
            }, sym.clone(), off),
            // The parser misclassifies symbol names colliding with register /
            // condition / barrier names; `get_symbol` accepts them as symbols.
            (Operand::Reg(sym.clone()), sym.clone(), 0),
            (Operand::Cond(sym.clone()), sym.clone(), 0),
            (Operand::Barrier(sym.clone()), sym.clone(), 0),
        ];
        cases[kind_idx].clone()
    }

    prop_compose! {
        fn arb_accepted_symbol()(
            s in "[a-z][a-z0-9_]{0,7}",
            off in -8192i64..=8192i64,
            kind_idx in 0usize..8usize,
        ) -> (Operand, String, i64) {
            accepted_operand_and_expected(s, off, kind_idx)
        }
    }

    proptest! {
        // Property A — full structural / field-placement oracle.
        // The encoded word is fully determined: opcode byte 0x54 in [31:24],
        // the imm19 branch offset [23:5] is left zero for the linker, o0 bit
        // [4] is zero, and cond occupies [3:0].
        #[test]
        fn prop_opcode_structure_and_fields(
            cond_idx in 0usize..COND_TABLE.len(),
            (sym_op, _sym, _off) in arb_accepted_symbol(),
        ) {
            let (cond_name, cond_val) = COND_TABLE[cond_idx];
            let word = enc(cond_name, &sym_op);

            prop_assert_eq!(word & OPCODE_MASK, OPCODE);
            prop_assert_eq!(word & IMM19_MASK, 0u32);
            prop_assert_eq!(word & O0_BIT, 0u32);
            prop_assert_eq!(word & 0xF, cond_val);
            // The whole word is exactly opcode | cond — nothing else is set.
            prop_assert_eq!(word, OPCODE | cond_val);
        }

        // Property B — condition-code mapping round-trips for every name in
        // the canonical table (incl. aliases cs/hs, cc/lo and the nv/al edge).
        #[test]
        fn prop_cond_field_matches_table(
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, cond_val) = COND_TABLE[cond_idx];
            let op = Operand::Symbol("target".into());
            let word = enc(cond_name, &op);
            prop_assert_eq!(word & 0xF, cond_val);
        }

        // Property C — alias equivalence (differential). The two spellings of
        // carry-set (cs/hs) and carry-clear (cc/lo) must produce bit-identical
        // words, since they encode the same condition.
        #[test]
        fn prop_aliases_encode_identically(
            sym in "[a-z][a-z0-9_]{0,7}",
        ) {
            let op = Operand::Symbol(sym);
            prop_assert_eq!(enc("cs", &op), enc("hs", &op));
            prop_assert_eq!(enc("cc", &op), enc("lo", &op));
        }

        // Property D — relocation contract across every operand kind that
        // `get_symbol` accepts: the result carries a CondBr19 relocation whose
        // symbol & addend exactly mirror the input operand.
        #[test]
        fn prop_reloc_is_condbr19_with_symbol(
            cond_idx in 0usize..COND_TABLE.len(),
            (sym_op, exp_sym, exp_off) in arb_accepted_symbol(),
        ) {
            let cond_name = COND_TABLE[cond_idx].0;
            let reloc = reloc_of(encode_cond_branch(cond_name, &[sym_op]));
            prop_assert!(matches!(reloc.reloc_type, RelocType::CondBr19));
            prop_assert_eq!(reloc.symbol, exp_sym);
            prop_assert_eq!(reloc.addend, exp_off);
        }

        // Property E — classifier / negative contract. For an arbitrary
        // lower-case token: if it is a known condition the encoder succeeds
        // and yields cond == table value; otherwise it MUST return Err. No
        // silent encoding of an unknown condition, and no rejection of a
        // valid one (incl. case-folding done by encode_cond).
        #[test]
        fn prop_unknown_condition_rejected(
            token in "[a-z]{0,4}",
        ) {
            let op = Operand::Symbol("target".into());
            let res = encode_cond_branch(&token, &[op.clone()]);
            let known = COND_TABLE.iter().find(|(n, _)| *n == token).map(|(_, v)| *v);
            match (known, res) {
                (Some(v), Ok(r)) => {
                    prop_assert_eq!(word_of(Ok(r)) & 0xF, v);
                }
                (Some(_), Err(e)) => panic!("known cond '{}' rejected: {}", token, e),
                (None, Ok(_)) => panic!("unknown cond '{}' accepted", token),
                (None, Err(_)) => {}
            }
        }

        // Property F — negative contract on the branch target: operand kinds
        // that `get_symbol` does NOT accept must make encode_cond_branch return
        // Err even when the condition itself is valid. No silent encoding of
        // an invalid branch target.
        #[test]
        fn prop_rejects_non_symbol_operands(
            idx in 0usize..13usize,
        ) {
            let rejected: Vec<Operand> = vec![
                Operand::Imm(42),
                Operand::Mem { base: "x0".into(), offset: 0 },
                Operand::MemExpr {
                    base: "x0".into(), expr: "foo".into(), writeback: false,
                },
                Operand::MemPreIndex { base: "x0".into(), offset: 8 },
                Operand::MemPostIndex { base: "x0".into(), offset: 8 },
                Operand::MemRegOffset {
                    base: "x0".into(), index: "x1".into(), extend: None, shift: None,
                },
                Operand::Shift { kind: "lsl".into(), amount: 2 },
                Operand::Extend { kind: "sxtw".into(), amount: 0 },
                Operand::Expr("x + y".into()),
                Operand::RegArrangement { reg: "v0".into(), arrangement: "16b".into() },
                Operand::RegLane { reg: "v0".into(), elem_size: "s".into(), index: 2 },
                Operand::RegList(vec![Operand::Reg("v0".into())]),
                Operand::RegListIndexed { regs: vec![Operand::Reg("v0".into())], index: 0 },
            ];
            let op = rejected[idx].clone();
            prop_assert!(
                encode_cond_branch("eq", &[op]).is_err(),
                "encode_cond_branch should reject this operand as a branch target"
            );
        }

        // Property G — case-insensitivity contract. `encode_cond` lowercases its
        // input, and `encode_instruction` lowercases the whole mnemonic before
        // dispatching `b.<cond>`, so a condition code is matched regardless of
        // ASCII case and must yield a bit-identical word for every spelling.
        // This locks behavior the lowercase-only generators above cannot reach.
        #[test]
        fn prop_condition_is_case_insensitive(
            cond_idx in 0usize..COND_TABLE.len(),
            sym in "[a-z][a-z0-9_]{0,7}",
            case_mode in 0usize..3usize,
        ) {
            let (cond_name, _) = COND_TABLE[cond_idx];
            let op = Operand::Symbol(sym);
            let lower_word = enc(cond_name, &op);
            let cased_word = enc(&cased(cond_name, case_mode), &op);
            prop_assert_eq!(
                cased_word, lower_word,
                "cond '{}' must encode identically to '{}'",
                cased(cond_name, case_mode), cond_name
            );
        }

        // Property H — determinism / purity. Encoding the same (cond, operand)
        // pair repeatedly yields bit-identical word AND relocation (symbol and
        // addend). encode_cond_branch is a pure function of its inputs.
        #[test]
        fn prop_encoding_is_deterministic(
            cond_idx in 0usize..COND_TABLE.len(),
            (op, exp_sym, exp_off) in arb_accepted_symbol(),
        ) {
            let cond_name = COND_TABLE[cond_idx].0;
            let ops = vec![op];
            prop_assert_eq!(
                word_of(encode_cond_branch(cond_name, &ops)),
                word_of(encode_cond_branch(cond_name, &ops))
            );
            let rel1 = reloc_of(encode_cond_branch(cond_name, &ops));
            let rel2 = reloc_of(encode_cond_branch(cond_name, &ops));
            prop_assert_eq!(&rel1.symbol, &rel2.symbol);
            prop_assert_eq!(&rel1.symbol, &exp_sym);
            prop_assert_eq!(rel1.addend, rel2.addend);
            prop_assert_eq!(rel1.addend, exp_off);
        }

        // Property I — arity / negative contract. A conditional branch with no
        // target operand must return Err. `get_symbol` reads operands[0]
        // unconditionally, so an empty vector cannot yield a relocation.
        #[test]
        fn prop_empty_operands_rejected(
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let cond_name = COND_TABLE[cond_idx].0;
            let empty: Vec<Operand> = vec![];
            prop_assert!(
                encode_cond_branch(cond_name, &empty).is_err(),
                "encode_cond_branch must reject an empty operand list"
            );
        }

        // Property J — word invariance across ALL accepted operand kinds.
        // Property A only proves the fixed word for the `Symbol` form; Property
        // D only checks the *relocation* across kinds. This closes the gap: the
        // instruction word must be exactly `OPCODE | cond_val` for EVERY kind
        // `get_symbol` accepts, because the operand influences ONLY the
        // relocation, never the word.
        #[test]
        fn prop_word_is_operand_independent(
            cond_idx in 0usize..COND_TABLE.len(),
            (op, _sym, _off) in arb_accepted_symbol(),
        ) {
            let (cond_name, cond_val) = COND_TABLE[cond_idx];
            let ops = vec![op];
            prop_assert_eq!(
                word_of(encode_cond_branch(cond_name, &ops)),
                OPCODE | cond_val
            );
        }

        // Property K — injectivity modulo aliases. Property B checks each known
        // condition name maps to its table value; this complements it by
        // asserting two DISTINCT condition values yield DISTINCT words. The cond
        // field is the only varying part of the word and occupies the low 4
        // bits, so distinct values cannot collide. Aliases (cs/hs, cc/lo) map
        // to the same value and are covered by Property C.
        #[test]
        fn prop_distinct_cond_values_distinct_words(
            i in 0usize..COND_TABLE.len(),
            j in 0usize..COND_TABLE.len(),
        ) {
            let (_, vi) = COND_TABLE[i];
            let (_, vj) = COND_TABLE[j];
            let op = Operand::Symbol("target".into());
            let wi = enc(COND_TABLE[i].0, &op);
            let wj = enc(COND_TABLE[j].0, &op);
            prop_assume!(vi != vj);
            prop_assert_ne!(wi, wj);
        }
    }
}

#[cfg(test)]
mod prop_encode_cbz_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- Opcode constants for the CBZ / CBNZ instruction class (ARM ARM C5.6.21/22) ----
    // CBZ/CBNZ = sf 011010 op imm19 Rt.
    //   [31]    sf   — 1 = 64-bit (X), 0 = 32-bit (W)
    //   [30:25] 011010 — fixed opcode
    //   [24]    op   — 0 = CBZ, 1 = CBNZ
    //   [23:5]  imm19 — linker-filled branch offset (encoder must leave zero)
    //   [4:0]   Rt   — register
    const OPCODE: u32 = 0b011010u32 << 25; // == 0x3400_0000
    const OPCODE_MASK: u32 = 0x7E00_0000;  // bits [30:25]
    const OP_BIT: u32 = 1u32 << 24;        // bit [24]
    const SF_BIT: u32 = 1u32 << 31;        // bit [31]
    const IMM19_MASK: u32 = 0x00FF_FFE0;   // bits [23:5] (linker-filled, must be zero)
    const RT_MASK: u32 = 0x1F;             // bits [4:0]

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::WordWithReloc { word, .. }) => word,
            other => panic!("expected WordWithReloc, got {:?}", other),
        }
    }

    fn reloc_of(r: Result<EncodeResult, String>) -> Relocation {
        match r {
            Ok(EncodeResult::WordWithReloc { reloc, .. }) => reloc,
            other => panic!("expected WordWithReloc, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand], is_nz: bool) -> u32 {
        word_of(encode_cbz(ops, is_nz))
    }

    prop_compose! {
        fn arb_reg()(n in 0u32..=31u32, is_64 in any::<bool>()) -> (String, u32) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            (name, n)
        }
    }

    /// Mirrors `get_symbol`'s forwarding table: every operand kind it accepts
    /// and the (symbol, addend) the encoder must forward into the relocation.
    fn accepted_operand_and_expected(
        sym: String,
        off: i64,
        kind_idx: usize,
    ) -> (Operand, String, i64) {
        let cases: Vec<(Operand, String, i64)> = vec![
            (Operand::Symbol(sym.clone()), sym.clone(), 0),
            (Operand::Label(sym.clone()), sym.clone(), 0),
            (Operand::SymbolOffset(sym.clone(), off), sym.clone(), off),
            (Operand::Modifier { kind: "lo12".into(), symbol: sym.clone() }, sym.clone(), 0),
            (Operand::ModifierOffset {
                kind: "lo12".into(), symbol: sym.clone(), offset: off,
            }, sym.clone(), off),
            // The parser misclassifies symbol names colliding with register /
            // condition / barrier names; `get_symbol` accepts them as symbols.
            (Operand::Reg(sym.clone()), sym.clone(), 0),
            (Operand::Cond(sym.clone()), sym.clone(), 0),
            (Operand::Barrier(sym.clone()), sym.clone(), 0),
        ];
        cases[kind_idx].clone()
    }

    prop_compose! {
        fn arb_accepted_symbol()(
            s in "[a-z][a-z0-9_]{0,7}",
            off in -8192i64..=8192i64,
            kind_idx in 0usize..8usize,
        ) -> (Operand, String, i64) {
            accepted_operand_and_expected(s, off, kind_idx)
        }
    }

    proptest! {
        // Property A — full structural / field-placement oracle.
        // The encoded word is fully determined: opcode 011010 in [30:25],
        // the imm19 branch-offset field [23:5] is left zero for the linker,
        // sf occupies [31], op occupies [24], and Rt occupies [4:0].
        #[test]
        fn prop_opcode_structure_and_fields(
            (rt_name, rt_num) in arb_reg(),
            (sym_op, _sym, _off) in arb_accepted_symbol(),
            is_nz in any::<bool>(),
        ) {
            let ops = vec![Operand::Reg(rt_name.clone()), sym_op];
            let word = enc(&ops, is_nz);

            // Fixed opcode bits.
            prop_assert_eq!(word & OPCODE_MASK, OPCODE);
            // Linker-reserved imm19 field must be zero in the encoder output.
            prop_assert_eq!(word & IMM19_MASK, 0u32);
            // sf bit [31] tracks the register width.
            let expected_sf = if rt_name.starts_with('x') { 1u32 } else { 0u32 };
            prop_assert_eq!((word >> 31) & 1, expected_sf);
            // op bit [24]: CBNZ => 1, CBZ => 0.
            prop_assert_eq!((word >> 24) & 1, if is_nz { 1 } else { 0 });
            // Rt field [4:0].
            prop_assert_eq!(word & RT_MASK, rt_num);
            // Reconstruct the whole word from its fields — nothing else is set.
            prop_assert_eq!(word, (expected_sf << 31) | OPCODE | ((is_nz as u32) << 24) | rt_num);
        }

        // Property B — differential: CBZ and CBNZ differ ONLY in bit 24 (op).
        #[test]
        fn prop_cbz_xor_cbnz_is_bit24(
            (rt_name, _) in arb_reg(),
            sym in "[a-z][a-z0-9_]{0,7}",
        ) {
            let ops = vec![Operand::Reg(rt_name), Operand::Symbol(sym)];
            prop_assert_eq!(enc(&ops, false) ^ enc(&ops, true), OP_BIT);
        }

        // Property C — differential: 64- vs 32-bit register differ ONLY in
        // bit 31 (sf). Same register number, same opcode, same target.
        #[test]
        fn prop_sf_bit_is_bit31(
            n in 0u32..=30u32,
            sym in "[a-z][a-z0-9_]{0,7}",
            is_nz in any::<bool>(),
        ) {
            let ops64 = vec![Operand::Reg(format!("x{}", n)), Operand::Symbol(sym.clone())];
            let ops32 = vec![Operand::Reg(format!("w{}", n)), Operand::Symbol(sym)];
            prop_assert_eq!(enc(&ops64, is_nz) ^ enc(&ops32, is_nz), SF_BIT);
        }

        // Property D — relocation contract across every operand kind that
        // `get_symbol` accepts: the result always carries a CondBr19
        // relocation whose symbol & addend exactly mirror the input operand.
        #[test]
        fn prop_reloc_is_condbr19_with_symbol(
            (rt_name, _) in arb_reg(),
            (sym_op, exp_sym, exp_off) in arb_accepted_symbol(),
            is_nz in any::<bool>(),
        ) {
            let ops = vec![Operand::Reg(rt_name), sym_op];
            let reloc = reloc_of(encode_cbz(&ops, is_nz));
            prop_assert!(matches!(reloc.reloc_type, RelocType::CondBr19));
            prop_assert_eq!(reloc.symbol, exp_sym);
            prop_assert_eq!(reloc.addend, exp_off);
        }

        // Property E — negative contract. Operand forms that `get_reg` /
        // `get_symbol` do NOT accept — and operand lists that are too short —
        // must make encode_cbz return Err. No silent encoding of an invalid
        // register or branch target, and no panic on missing operands.
        #[test]
        fn prop_rejects_invalid_operands(
            case in 0usize..14usize,
        ) {
            // A valid symbol for the target slot when we want to exercise an
            // invalid register (and vice-versa).
            let good_sym = Operand::Symbol("tgt".into());
            let good_reg = Operand::Reg("x0".into());
            let result = match case {
                0  => encode_cbz(&[], false),                                   // no operands
                1  => encode_cbz(&[good_reg.clone()], false),                   // only register, no target
                2  => encode_cbz(&[Operand::Imm(0)], false),                    // Imm as register
                3  => encode_cbz(&[Operand::Symbol("r".into())], false),        // Symbol as register, no target
                4  => encode_cbz(&[Operand::Mem { base: "x0".into(), offset: 0 }], false),
                5  => encode_cbz(&[Operand::Reg("xyz".into()), good_sym.clone()], false), // malformed register name (valid target present)
                6  => encode_cbz(&[good_reg.clone(), Operand::Imm(7)], false),  // Imm as target
                7  => encode_cbz(&[good_reg.clone(), Operand::Expr("a+b".into())], false),
                8  => encode_cbz(&[good_reg.clone(), Operand::Mem { base: "x0".into(), offset: 0 }], false),
                9  => encode_cbz(&[good_reg.clone(), Operand::Shift { kind: "lsl".into(), amount: 2 }], false),
                10 => encode_cbz(&[good_reg.clone(), Operand::Extend { kind: "sxtw".into(), amount: 0 }], false),
                11 => encode_cbz(&[good_reg.clone(), Operand::RegArrangement { reg: "v0".into(), arrangement: "16b".into() }], false),
                12 => encode_cbz(&[good_reg.clone(), Operand::RegList(vec![good_reg.clone()])], false),
                _  => encode_cbz(&[good_reg.clone(), Operand::MemPreIndex { base: "x0".into(), offset: 8 }], false),
            };
            prop_assert!(result.is_err(), "encode_cbz should reject case {} (got {:?})", case, result);
        }

        // Property F — KNOWN-BUG characterization (differential). Per the ARM ARM
        // (C5.6.21/22), CBZ/CBNZ's `<Rt>` operand is a *general-purpose* register;
        // the Rt field value 31 denotes **XZR/WZR**, and there is NO SP-using form.
        // Therefore `cbz sp, <target>` and `cbz wsp, <target>` are UNPREDICTABLE /
        // unallocated encodings that a conforming assembler must reject.
        //
        // This encoder accepts them because the shared `get_reg`->`parse_reg_num`
        // helper maps `sp`/`wsp` -> 31 (correct for SP-aware ADD/SUB/LDR, wrong for
        // every XZR-only instruction), silently producing a branch on the ZERO
        // register. The assertions below pin that buggy, bit-identical aliasing so
        // the regression is caught the moment it is fixed (then flip them to `is_err`).
        //
        // Sibling encoders suffer the identical defect — see
        // `encode_mul_sp_operand_silently_accepted_as_xzr`, `encode_div_sp_...`, etc.
        #[test]
        fn prop_sp_silently_aliased_to_xzr(
            sym in "[a-z][a-z0-9_]{0,7}",
            is_nz in any::<bool>(),
        ) {
            let mk = |r: &str| vec![Operand::Reg(r.to_string()), Operand::Symbol(sym.clone())];

            // 64-bit: `sp` and `xzr` both encode as Rt=31, sf=1 — bit-identical.
            let sp_word = enc(&mk("sp"), is_nz);
            let xzr_word = enc(&mk("xzr"), is_nz);
            prop_assert_eq!(sp_word, xzr_word,
                "cbz/cbnz sp == cbz/cbnz xzr (SP silently aliased to XZR)");
            // Rt field == 31 (XZR) and sf == 1 (64-bit).
            prop_assert_eq!(sp_word & RT_MASK, 31);
            prop_assert_eq!((sp_word >> 31) & 1, 1);

            // 32-bit: `wsp` and `wzr` both encode as Rt=31, sf=0 — bit-identical.
            let wsp_word = enc(&mk("wsp"), is_nz);
            let wzr_word = enc(&mk("wzr"), is_nz);
            prop_assert_eq!(wsp_word, wzr_word,
                "cbz/cbnz wsp == cbz/cbnz wzr (WSP silently aliased to WZR)");
            prop_assert_eq!(wsp_word & RT_MASK, 31);
            prop_assert_eq!((wsp_word >> 31) & 1, 0);

            // The encoder currently returns Ok for these (the bug); a fixed
            // assembler must return Err. Pinning the acceptance here.
            prop_assert!(encode_cbz(&mk("sp"), is_nz).is_ok(),
                "BUG: cbz sp is accepted, not rejected");
            prop_assert!(encode_cbz(&mk("wsp"), is_nz).is_ok(),
                "BUG: cbz wsp is accepted, not rejected");
        }
    }
}

#[cfg(test)]
mod prop_encode_csel_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- CSEL opcode constants (ARM ARM C4.1.64, "Conditional Select") ----
    // CSEL = sf 0 0 11010100 Rm cond 0 0 Rn Rd
    //   [31]    sf        — 1 = 64-bit (X), 0 = 32-bit (W); taken from Rd ONLY
    //   [30:29] 00        — op=0, S=0 (CSEL is the non-S variant)
    //   [28:21] 11010100  — fixed opcode for the conditional-select group
    //   [20:16] Rm
    //   [15:12] cond
    //   [11:10] 00        — o2=0, o1=0 (selects CSEL within the group)
    //   [9:5]   Rn
    //   [4:0]   Rd
    const OPCODE: u32 = 0b11010100u32 << 21; // == 0x1A80_0000, bits [28:21]
    // Bits that must be ZERO for CSEL: [30, 29, 11, 10].
    const FIXED_ZERO: u32 =
        (1u32 << 30) | (1u32 << 29) | (1u32 << 11) | (1u32 << 10); // == 0x6000_0C00

    /// Canonical ARM condition-code table mirroring `encode_cond`.
    const COND_TABLE: &[(&str, u32)] = &[
        ("eq", 0), ("ne", 1), ("cs", 2), ("hs", 2), ("cc", 3), ("lo", 3),
        ("mi", 4), ("pl", 5), ("vs", 6), ("vc", 7), ("hi", 8), ("ls", 9),
        ("ge", 10), ("lt", 11), ("gt", 12), ("le", 13), ("al", 14), ("nv", 15),
    ];

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word_of(encode_csel(ops))
    }

    prop_compose! {
        fn arb_gp_reg()(n in 0u32..=30u32, is_64 in any::<bool>()) -> (String, u32) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            (name, n)
        }
    }

    proptest! {
        // Property A — full structural / field-placement oracle.
        // The encoded word is fully determined: opcode 11010100 in [28:21],
        // bits [30,29,11,10] are zero, sf tracks Rd's width, and Rm/cond/Rn/Rd
        // occupy exactly their spec fields. Reconstructing from the fields
        // reproduces the whole word — nothing else is set.
        #[test]
        fn prop_opcode_structure_and_fields(
            (rd_name, rd_num) in arb_gp_reg(),
            rn_num in 0u32..=30u32,
            rm_num in 0u32..=30u32,
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, cond_val) = COND_TABLE[cond_idx];
            let ops = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Reg(format!("x{}", rn_num)),
                Operand::Reg(format!("x{}", rm_num)),
                Operand::Cond(cond_name.to_string()),
            ];
            let word = enc(&ops);

            // Fixed opcode bits [28:21].
            prop_assert_eq!(word & OPCODE, OPCODE);
            // Bits that must be zero for CSEL.
            prop_assert_eq!(word & FIXED_ZERO, 0u32);
            // sf bit [31] tracks Rd's width.
            let expected_sf = if rd_name.starts_with('x') { 1u32 } else { 0u32 };
            prop_assert_eq!((word >> 31) & 1, expected_sf);
            // Rm [20:16], cond [15:12], Rn [9:5], Rd [4:0].
            prop_assert_eq!((word >> 16) & 0x1F, rm_num);
            prop_assert_eq!((word >> 12) & 0xF, cond_val);
            prop_assert_eq!((word >> 5) & 0x1F, rn_num);
            prop_assert_eq!(word & 0x1F, rd_num);
            // Full reconstruction — the word is exactly the OR of its fields.
            prop_assert_eq!(
                word,
                (expected_sf << 31) | OPCODE | (rm_num << 16)
                    | (cond_val << 12) | (rn_num << 5) | rd_num
            );
        }

        // Property B — differential: 64- vs 32-bit Rd differ ONLY in bit 31.
        // sf is derived solely from Rd (operand 0), so flipping x<->w on Rd
        // changes exactly one bit and leaves every other field untouched.
        #[test]
        fn prop_sf_bit_is_bit31(
            n in 0u32..=30u32,
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let cond_name = COND_TABLE[cond_idx].0;
            let mk = |rd: String| vec![
                Operand::Reg(rd),
                Operand::Reg("x1".into()),
                Operand::Reg("x2".into()),
                Operand::Cond(cond_name.to_string()),
            ];
            let w64 = enc(&mk(format!("x{}", n)));
            let w32 = enc(&mk(format!("w{}", n)));
            prop_assert_eq!(w64 ^ w32, 1u32 << 31);
        }

        // Property C — condition-code mapping round-trips for every name in the
        // canonical table, and the cs/hs & cc/lo aliases encode bit-identically.
        #[test]
        fn prop_cond_round_trips_and_aliases(i in 0usize..COND_TABLE.len()) {
            let (name_i, val_i) = COND_TABLE[i];
            let ops = vec![
                Operand::Reg("x0".into()),
                Operand::Reg("x1".into()),
                Operand::Reg("x2".into()),
                Operand::Cond(name_i.to_string()),
            ];
            prop_assert_eq!((enc(&ops) >> 12) & 0xF, val_i);

            let base = |c: &str| enc(&[
                Operand::Reg("x0".into()),
                Operand::Reg("x1".into()),
                Operand::Reg("x2".into()),
                Operand::Cond(c.to_string()),
            ]);
            prop_assert_eq!(base("cs"), base("hs"));
            prop_assert_eq!(base("cc"), base("lo"));
        }

        // Property D — negative contract. CSEL needs exactly four operands:
        // three registers followed by a condition. Lists that are too short,
        // that lack a trailing condition, or that place a non-register in the
        // Rd/Rn/Rm slots must make encode_csel return Err — no silent encoding.
        #[test]
        fn prop_rejects_invalid_operands(case in 0usize..10usize) {
            let r = Operand::Reg("x0".into());
            let c = Operand::Cond("eq".into());
            let result = match case {
                0 => encode_csel(&[]),                                            // no operands
                1 => encode_csel(&[r.clone()]),                                   // only Rd
                2 => encode_csel(&[r.clone(), r.clone()]),                        // Rd, Rn
                3 => encode_csel(&[r.clone(), r.clone(), r.clone()]),             // no condition
                4 => encode_csel(&[r.clone(), r.clone(), r.clone(), r.clone()]), // 4th not a Cond
                5 => encode_csel(&[Operand::Imm(0), r.clone(), r.clone(), c.clone()]),          // Rd not a reg
                6 => encode_csel(&[r.clone(), Operand::Imm(1), r.clone(), c.clone()]),          // Rn not a reg
                7 => encode_csel(&[r.clone(), r.clone(), Operand::Symbol("s".into()), c.clone()]), // Rm not a reg
                8 => encode_csel(&[r.clone(), r.clone(), r.clone(), Operand::Imm(4)]),          // cond is Imm
                _ => encode_csel(&[r.clone(), r.clone(), r.clone(), Operand::Symbol("notcond".into())]), // cond is Symbol
            };
            prop_assert!(result.is_err(), "encode_csel should reject case {} (got {:?})", case, result);
        }

        // Property E — register-class negative contract. CSEL is defined ONLY on
        // general-purpose (X/W) registers (ARM ARM C4.1.64). FP/SIMD register
        // names (d/s/q/v/h/b) must therefore be rejected rather than silently
        // re-encoded with their numeric index as if they were GP registers.
        #[test]
        fn prop_rejects_fp_simd_registers(
            prefix in "[dsvhbq]",
            n in 0u32..=31u32,
            slot in 0usize..3,
        ) {
            let bad = format!("{}{}", prefix, n);
            let mut ops = vec![
                Operand::Reg("x0".into()),
                Operand::Reg("x1".into()),
                Operand::Reg("x2".into()),
                Operand::Cond("eq".into()),
            ];
            ops[slot] = Operand::Reg(bad);
            let result = encode_csel(&ops);
            prop_assert!(
                result.is_err(),
                "encode_csel should reject FP/SIMD register in slot {} (got {:?})",
                slot, result
            );
        }
        // Property F — width-coherence negative contract. CSEL has one sf bit
        // for all three GP operands; Rd/Rn/Rm must all be W or all be X.
        #[test]
        fn prop_rejects_mixed_width_operands(n in 0u32..=30u32) {
            let cases = [
                vec![Operand::Reg(format!("x{}", n)), Operand::Reg(format!("w{}", n)), Operand::Reg(format!("x{}", n)), Operand::Cond("eq".into())],
                vec![Operand::Reg(format!("x{}", n)), Operand::Reg(format!("x{}", n)), Operand::Reg(format!("w{}", n)), Operand::Cond("eq".into())],
                vec![Operand::Reg(format!("w{}", n)), Operand::Reg(format!("x{}", n)), Operand::Reg(format!("w{}", n)), Operand::Cond("eq".into())],
                vec![Operand::Reg(format!("w{}", n)), Operand::Reg(format!("w{}", n)), Operand::Reg(format!("x{}", n)), Operand::Cond("eq".into())],
            ];
            for ops in cases {
                prop_assert!(encode_csel(&ops).is_err(), "mixed-width CSEL operands must be Err, got {:?}", encode_csel(&ops));
            }
        }
    }
}

#[cfg(test)]
mod prop_encode_csinc_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- CSINC opcode constants (ARM ARM C4.1.66, "Conditional Select (increment)") ----
    // CSINC = sf 0 0 11010100 Rm cond 0 1 Rn Rd
    //   [31]    sf        — 1 = 64-bit (X), 0 = 32-bit (W); taken from Rd ONLY
    //   [30:29] 00        — op=0, S=0
    //   [28:21] 11010100  — fixed opcode for the conditional-select group
    //   [20:16] Rm
    //   [15:12] cond
    //   [11:10] 01        — o2=0, o1=1 (selects CSINC within the group)
    //   [9:5]   Rn
    //   [4:0]   Rd
    const OPCODE: u32 = 0b11010100u32 << 21; // == 0x1A80_0000, bits [28:21]
    // Bits that must be ZERO for CSINC: [30, 29, 11] (bit 10 is the CSINC marker = 1).
    const FIXED_ZERO: u32 = (1u32 << 30) | (1u32 << 29) | (1u32 << 11); // == 0x6000_0800
    // o1 bit [10] must be ONE: this is what distinguishes CSINC from CSEL.
    const O1_BIT: u32 = 1u32 << 10; // == 0x400

    /// Canonical ARM condition-code table mirroring `encode_cond`.
    const COND_TABLE: &[(&str, u32)] = &[
        ("eq", 0), ("ne", 1), ("cs", 2), ("hs", 2), ("cc", 3), ("lo", 3),
        ("mi", 4), ("pl", 5), ("vs", 6), ("vc", 7), ("hi", 8), ("ls", 9),
        ("ge", 10), ("lt", 11), ("gt", 12), ("le", 13), ("al", 14), ("nv", 15),
    ];

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word_of(encode_csinc(ops))
    }

    prop_compose! {
        fn arb_gp_reg()(n in 0u32..=30u32, is_64 in any::<bool>()) -> (String, u32) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            (name, n)
        }
    }

    proptest! {
        // Property A — full structural / field-placement oracle.
        // The encoded word is fully determined: opcode 11010100 in [28:21],
        // bits [30,29,11] are zero, bit [10] is one (the CSINC marker), sf
        // tracks Rd's width, and Rm/cond/Rn/Rd occupy exactly their spec
        // fields. Reconstructing from the fields reproduces the whole word.
        #[test]
        fn prop_opcode_structure_and_fields(
            (rd_name, rd_num) in arb_gp_reg(),
            (rn_name, rn_num) in arb_gp_reg(),
            (rm_name, rm_num) in arb_gp_reg(),
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, cond_val) = COND_TABLE[cond_idx];
            let ops = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Reg(rn_name),
                Operand::Reg(rm_name),
                Operand::Cond(cond_name.to_string()),
            ];
            let word = enc(&ops);

            // Fixed opcode bits [28:21].
            prop_assert_eq!(word & OPCODE, OPCODE);
            // Bits that must be zero for CSINC.
            prop_assert_eq!(word & FIXED_ZERO, 0u32);
            // o1 bit [10] must be one — the CSINC distinguishing bit.
            prop_assert_eq!(word & O1_BIT, O1_BIT);
            // sf bit [31] tracks Rd's width.
            let expected_sf = if rd_name.starts_with('x') { 1u32 } else { 0u32 };
            prop_assert_eq!((word >> 31) & 1, expected_sf);
            // Rm [20:16], cond [15:12], Rn [9:5], Rd [4:0].
            prop_assert_eq!((word >> 16) & 0x1F, rm_num);
            prop_assert_eq!((word >> 12) & 0xF, cond_val);
            prop_assert_eq!((word >> 5) & 0x1F, rn_num);
            prop_assert_eq!(word & 0x1F, rd_num);
            // Full reconstruction — the word is exactly the OR of its fields.
            prop_assert_eq!(
                word,
                (expected_sf << 31) | OPCODE | O1_BIT | (rm_num << 16)
                    | (cond_val << 12) | (rn_num << 5) | rd_num
            );
        }

        // Property B — differential: CSINC and CSEL are the same instruction
        // group and differ ONLY in bit 10 (o1=1 for CSINC, o1=0 for CSEL) when
        // given identical operands. This is the defining differentiator.
        #[test]
        fn prop_csinc_xor_csel_is_bit10(
            (rd_name, _) in arb_gp_reg(),
            (rn_name, _) in arb_gp_reg(),
            (rm_name, _) in arb_gp_reg(),
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, _) = COND_TABLE[cond_idx];
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Reg(rm_name),
                Operand::Cond(cond_name.to_string()),
            ];
            let csinc = enc(&ops);
            let csel = word_of(encode_csel(&ops));
            prop_assert_eq!(csinc ^ csel, O1_BIT);
        }

        // Property C — differential: 64- vs 32-bit Rd differ ONLY in bit 31.
        // sf is derived solely from Rd (operand 0), so flipping x<->w on Rd
        // changes exactly one bit and leaves every other field untouched.
        #[test]
        fn prop_sf_bit_is_bit31(
            n in 0u32..=30u32,
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let cond_name = COND_TABLE[cond_idx].0;
            let mk = |rd: String| vec![
                Operand::Reg(rd),
                Operand::Reg("x1".into()),
                Operand::Reg("x2".into()),
                Operand::Cond(cond_name.to_string()),
            ];
            let w64 = enc(&mk(format!("x{}", n)));
            let w32 = enc(&mk(format!("w{}", n)));
            prop_assert_eq!(w64 ^ w32, 1u32 << 31);
        }

        // Property D — condition-code mapping round-trips for every name in
        // the canonical table, and the cs/hs & cc/lo aliases encode bit-identically.
        #[test]
        fn prop_cond_round_trips_and_aliases(i in 0usize..COND_TABLE.len()) {
            let (name_i, val_i) = COND_TABLE[i];
            let ops = vec![
                Operand::Reg("x0".into()),
                Operand::Reg("x1".into()),
                Operand::Reg("x2".into()),
                Operand::Cond(name_i.to_string()),
            ];
            prop_assert_eq!((enc(&ops) >> 12) & 0xF, val_i);

            let base = |c: &str| enc(&[
                Operand::Reg("x0".into()),
                Operand::Reg("x1".into()),
                Operand::Reg("x2".into()),
                Operand::Cond(c.to_string()),
            ]);
            prop_assert_eq!(base("cs"), base("hs"));
            prop_assert_eq!(base("cc"), base("lo"));
        }

        // Property E — negative contract. CSINC needs exactly four operands:
        // three registers followed by a condition. Operand lists that are too
        // short, that lack a trailing condition, that place a non-register in
        // the Rd/Rn/Rm slots — OR that use FP/SIMD register names, which CSINC
        // is NOT defined on (ARM ARM C4.1.66: GP registers only) — must make
        // encode_csinc return Err rather than silently re-encode them.
        #[test]
        fn prop_rejects_invalid_operands(case in 0usize..16usize) {
            let r = Operand::Reg("x0".into());
            let c = Operand::Cond("eq".into());
            let result = match case {
                0  => encode_csinc(&[]),                                            // no operands
                1  => encode_csinc(&[r.clone()]),                                   // only Rd
                2  => encode_csinc(&[r.clone(), r.clone()]),                        // Rd, Rn
                3  => encode_csinc(&[r.clone(), r.clone(), r.clone()]),             // no condition
                4  => encode_csinc(&[r.clone(), r.clone(), r.clone(), r.clone()]), // 4th not a Cond
                5  => encode_csinc(&[Operand::Imm(0), r.clone(), r.clone(), c.clone()]),          // Rd not a reg
                6  => encode_csinc(&[r.clone(), Operand::Imm(1), r.clone(), c.clone()]),          // Rn not a reg
                7  => encode_csinc(&[r.clone(), r.clone(), Operand::Symbol("s".into()), c.clone()]), // Rm not a reg
                8  => encode_csinc(&[r.clone(), r.clone(), r.clone(), Operand::Imm(4)]),          // cond is Imm
                9  => encode_csinc(&[r.clone(), r.clone(), r.clone(), Operand::Symbol("notcond".into())]), // cond is Symbol
                // CSINC is defined ONLY on GP (X/W) registers (ARM ARM C4.1.66).
                // FP/SIMD register names must be rejected, not silently re-encoded
                // with their numeric index as if they were GP registers.
                10 => encode_csinc(&[Operand::Reg("d0".into()), r.clone(), r.clone(), c.clone()]), // Rd is FP
                11 => encode_csinc(&[r.clone(), Operand::Reg("s1".into()), r.clone(), c.clone()]), // Rn is FP
                12 => encode_csinc(&[r.clone(), r.clone(), Operand::Reg("v2".into()), c.clone()]), // Rm is SIMD
                13 => encode_csinc(&[Operand::Reg("q3".into()), r.clone(), r.clone(), c.clone()]),
                14 => encode_csinc(&[Operand::Reg("h4".into()), r.clone(), r.clone(), c.clone()]),
                _  => encode_csinc(&[r.clone(), r.clone(), r.clone(), Operand::Extend { kind: "sxtw".into(), amount: 0 }]),
            };
            prop_assert!(
                result.is_err(),
                "encode_csinc should reject case {} (got {:?})", case, result
            );
        }

        // Property F — negative contract: MIXED-WIDTH validation (the focus).
        // CSINC's single sf bit applies to the WHOLE instruction, so Rd, Rn,
        // and Rm MUST all share one register width (ARM ARM C4.1.66,
        // "Conditional select (increment)"). GAS and LLVM-MC reject e.g.
        // `csinc w0, x1, x2, eq` with "operand size mismatch". The encoder
        // derives sf from Rd (operand 0) and DISCARDS the widths of Rn/Rm
        // (`let (rn, _) = get_reg(...)` / `let (rm, _) = get_reg(...)`), so it
        // silently coerces a mixed-width input instead of returning Err.
        // No cited spec permits this silent coercion.
        #[test]
        fn prop_rejects_mixed_width_registers(
            n in 0u32..=30u32,
            rd64 in any::<bool>(),
            rn64 in any::<bool>(),
            rm64 in any::<bool>(),
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            // Skip the all-uniform case (covered by Property G): we want a
            // genuine mix of widths across Rd/Rn/Rm.
            prop_assume!(!(rd64 == rn64 && rn64 == rm64));
            let cond_name = COND_TABLE[cond_idx].0;
            let mk = |is64: bool| if is64 { format!("x{}", n) } else { format!("w{}", n) };
            let ops = vec![
                Operand::Reg(mk(rd64)),
                Operand::Reg(mk(rn64)),
                Operand::Reg(mk(rm64)),
                Operand::Cond(cond_name.to_string()),
            ];
            prop_assert!(
                encode_csinc(&ops).is_err(),
                "mixed-width CSINC {:?} must be rejected: Rd/Rn/Rm MUST share \
                 one width (ARM ARM C4.1.66); GAS/LLVM-MC reject this with \
                 'operand size mismatch'. Got {:?}",
                ops, encode_csinc(&ops)
            );
        }

        // Property G — positive control for the width contract.
        // When all three registers share ONE width, encode_csinc MUST succeed
        // and the sf bit MUST equal that shared width. This anchors the
        // invariant that Property F exercises negatively.
        #[test]
        fn prop_uniform_widths_always_succeed(
            n in 0u32..=30u32,
            is64 in any::<bool>(),
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let cond_name = COND_TABLE[cond_idx].0;
            let reg = if is64 { format!("x{}", n) } else { format!("w{}", n) };
            let ops = vec![
                Operand::Reg(reg.clone()),
                Operand::Reg(reg.clone()),
                Operand::Reg(reg),
                Operand::Cond(cond_name.to_string()),
            ];
            let r = encode_csinc(&ops);
            prop_assert!(r.is_ok(), "uniform-width CSINC must encode, got {:?}", r);
            let word = word_of(r);
            let expected_sf = if is64 { 1u32 } else { 0u32 };
            prop_assert_eq!((word >> 31) & 1, expected_sf);
        }

        // Property H — negative contract: mixed-width validation extends to
        // the zero register aliases. XZR (64-bit) and WZR (32-bit) are valid
        // CSINC operands, but they still carry a width, so pairing an XZR with
        // a W-register (or WZR with an X-register) is a width mismatch and
        // MUST be rejected, not silently coerced to Rd's width.
        #[test]
        fn prop_rejects_zr_width_mismatch(
            n in 0u32..=30u32,
            zr_first in any::<bool>(),
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let cond_name = COND_TABLE[cond_idx].0;
            // Rd = XZR (64-bit) but Rn = W<n> (32-bit) -> mismatch; or
            // Rd = WZR (32-bit) but Rn = X<n> (64-bit) -> mismatch.
            let (rd, rn) = if zr_first {
                ("xzr".to_string(), format!("w{}", n))
            } else {
                ("wzr".to_string(), format!("x{}", n))
            };
            let ops = vec![
                Operand::Reg(rd),
                Operand::Reg(rn),
                Operand::Reg("x2".into()),
                Operand::Cond(cond_name.to_string()),
            ];
            prop_assert!(
                encode_csinc(&ops).is_err(),
                "mixed-width CSINC with zr {:?} must be rejected (width applies \
                 to the whole instruction). Got {:?}",
                ops, encode_csinc(&ops)
            );
        }
    }
}

#[cfg(test)]
mod prop_encode_csinv_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- CSINV opcode constants (ARM ARM C4.1.67, "Conditional Select (invert)") ----
    // CSINV = sf 1 0 11010100 Rm cond 0 0 Rn Rd
    //   [31]    sf        — 1 = 64-bit (X), 0 = 32-bit (W); taken from Rd ONLY
    //   [30]    1         — op=1  (distinguishes CSINV/CSNEG from CSEL/CSINC)
    //   [29]    0         — S=0
    //   [28:21] 11010100  — fixed opcode for the conditional-select group
    //   [20:16] Rm
    //   [15:12] cond
    //   [11:10] 00        — o2=0, o1=0 (selects CSINV within the group; o1=1 → CSNEG)
    //   [9:5]   Rn
    //   [4:0]   Rd
    const OPCODE: u32 = 0b11010100u32 << 21; // == 0x1A80_0000, bits [28:21]
    // Bit [30] (op) must be ONE: this is what distinguishes CSINV from CSEL/CSINC.
    const OP_BIT: u32 = 1u32 << 30; // == 0x4000_0000
    // Bits that must be ZERO for CSINV: [29] (S=0), [11] (o2=0), [10] (o1=0).
    //   [11:10] == 00 distinguishes CSINV from CSNEG (which has o1=1, bit 10 set).
    const FIXED_ZERO: u32 = (1u32 << 29) | (1u32 << 11) | (1u32 << 10); // == 0x2000_0C00

    /// Canonical ARM condition-code table mirroring `encode_cond`.
    const COND_TABLE: &[(&str, u32)] = &[
        ("eq", 0), ("ne", 1), ("cs", 2), ("hs", 2), ("cc", 3), ("lo", 3),
        ("mi", 4), ("pl", 5), ("vs", 6), ("vc", 7), ("hi", 8), ("ls", 9),
        ("ge", 10), ("lt", 11), ("gt", 12), ("le", 13), ("al", 14), ("nv", 15),
    ];

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word_of(encode_csinv(ops))
    }

    prop_compose! {
        fn arb_gp_reg()(n in 0u32..=30u32, is_64 in any::<bool>()) -> (String, u32) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            (name, n)
        }
    }

    proptest! {
        // Property A — full structural / field-placement oracle.
        // The encoded word is fully determined: opcode 11010100 in [28:21],
        // bit [30]=1 (op, the CSINV marker), bits [29,11,10] are zero, sf
        // tracks Rd's width, and Rm/cond/Rn/Rd occupy exactly their spec
        // fields. Reconstructing from the fields reproduces the whole word —
        // nothing else is set.
        #[test]
        fn prop_opcode_structure_and_fields(
            (rd_name, rd_num) in arb_gp_reg(),
            (rn_name, rn_num) in arb_gp_reg(),
            (rm_name, rm_num) in arb_gp_reg(),
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, cond_val) = COND_TABLE[cond_idx];
            let ops = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Reg(rn_name),
                Operand::Reg(rm_name),
                Operand::Cond(cond_name.to_string()),
            ];
            let word = enc(&ops);

            // Fixed opcode bits [28:21].
            prop_assert_eq!(word & OPCODE, OPCODE);
            // Bits that must be zero for CSINV.
            prop_assert_eq!(word & FIXED_ZERO, 0u32);
            // op bit [30] must be one — the CSINV/CSNEG distinguishing bit.
            prop_assert_eq!(word & OP_BIT, OP_BIT);
            // sf bit [31] tracks Rd's width.
            let expected_sf = if rd_name.starts_with('x') { 1u32 } else { 0u32 };
            prop_assert_eq!((word >> 31) & 1, expected_sf);
            // Rm [20:16], cond [15:12], Rn [9:5], Rd [4:0].
            prop_assert_eq!((word >> 16) & 0x1F, rm_num);
            prop_assert_eq!((word >> 12) & 0xF, cond_val);
            prop_assert_eq!((word >> 5) & 0x1F, rn_num);
            prop_assert_eq!(word & 0x1F, rd_num);
            // Full reconstruction — the word is exactly the OR of its fields.
            prop_assert_eq!(
                word,
                (expected_sf << 31) | OP_BIT | OPCODE | (rm_num << 16)
                    | (cond_val << 12) | (rn_num << 5) | rd_num
            );
        }

        // Property B — differential: CSINV and CSEL are the same instruction
        // group with identical operand layout and identical o2/o1 bits (00);
        // they differ ONLY in bit 30 (op=1 for CSINV, op=0 for CSEL). This is
        // the defining differentiator between invert-select and plain-select.
        #[test]
        fn prop_csinv_xor_csel_is_bit30(
            (rd_name, _) in arb_gp_reg(),
            (rn_name, _) in arb_gp_reg(),
            (rm_name, _) in arb_gp_reg(),
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, _) = COND_TABLE[cond_idx];
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Reg(rm_name),
                Operand::Cond(cond_name.to_string()),
            ];
            let csinv = enc(&ops);
            let csel = word_of(encode_csel(&ops));
            prop_assert_eq!(csinv ^ csel, OP_BIT);
        }

        // Property C — differential: 64- vs 32-bit Rd differ ONLY in bit 31.
        // sf is derived solely from Rd (operand 0), so flipping x<->w on Rd
        // changes exactly one bit and leaves every other field untouched.
        #[test]
        fn prop_sf_bit_is_bit31(
            n in 0u32..=30u32,
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let cond_name = COND_TABLE[cond_idx].0;
            let mk = |rd: String| vec![
                Operand::Reg(rd),
                Operand::Reg("x1".into()),
                Operand::Reg("x2".into()),
                Operand::Cond(cond_name.to_string()),
            ];
            let w64 = enc(&mk(format!("x{}", n)));
            let w32 = enc(&mk(format!("w{}", n)));
            prop_assert_eq!(w64 ^ w32, 1u32 << 31);
        }

        // Property D — condition-code mapping round-trips for every name in
        // the canonical table, and the cs/hs & cc/lo aliases encode bit-identically.
        #[test]
        fn prop_cond_round_trips_and_aliases(i in 0usize..COND_TABLE.len()) {
            let (name_i, val_i) = COND_TABLE[i];
            let ops = vec![
                Operand::Reg("x0".into()),
                Operand::Reg("x1".into()),
                Operand::Reg("x2".into()),
                Operand::Cond(name_i.to_string()),
            ];
            prop_assert_eq!((enc(&ops) >> 12) & 0xF, val_i);

            let base = |c: &str| enc(&[
                Operand::Reg("x0".into()),
                Operand::Reg("x1".into()),
                Operand::Reg("x2".into()),
                Operand::Cond(c.to_string()),
            ]);
            prop_assert_eq!(base("cs"), base("hs"));
            prop_assert_eq!(base("cc"), base("lo"));
        }

        // Property E — structural negative contract. CSINV needs exactly four
        // operands: three registers followed by a condition. Lists that are too
        // short, that lack a trailing condition, or that place a non-register /
        // non-Cond in the wrong slot must make encode_csinv return Err — no
        // silent encoding. (Register-class validation is covered separately in
        // Property F.)
        #[test]
        fn prop_rejects_invalid_operands(case in 0usize..10usize) {
            let r = Operand::Reg("x0".into());
            let c = Operand::Cond("eq".into());
            let result = match case {
                0 => encode_csinv(&[]),                                            // no operands
                1 => encode_csinv(&[r.clone()]),                                   // only Rd
                2 => encode_csinv(&[r.clone(), r.clone()]),                        // Rd, Rn
                3 => encode_csinv(&[r.clone(), r.clone(), r.clone()]),             // no condition
                4 => encode_csinv(&[r.clone(), r.clone(), r.clone(), r.clone()]), // 4th not a Cond
                5 => encode_csinv(&[Operand::Imm(0), r.clone(), r.clone(), c.clone()]),          // Rd not a reg
                6 => encode_csinv(&[r.clone(), Operand::Imm(1), r.clone(), c.clone()]),          // Rn not a reg
                7 => encode_csinv(&[r.clone(), r.clone(), Operand::Symbol("s".into()), c.clone()]), // Rm not a reg
                8 => encode_csinv(&[r.clone(), r.clone(), r.clone(), Operand::Imm(4)]),          // cond is Imm
                _ => encode_csinv(&[r.clone(), r.clone(), r.clone(), Operand::Symbol("notcond".into())]), // cond is Symbol
            };
            prop_assert!(
                result.is_err(),
                "encode_csinv should reject case {} (got {:?})", case, result
            );
        }

        // Property F — register-class negative contract (EXPECTED TO FAIL — see
        // BUG-report). CSINV is defined ONLY on general-purpose (X/W) registers
        // (ARM ARM C4.1.67: "Conditional Select (invert)", GP register operands).
        // FP/SIMD register names (d/s/q/v/h/b) must therefore be rejected rather
        // than silently re-encoded with their numeric index as if they were GP
        // registers, which would emit a malformed instruction. `parse_reg_num`
        // (encoder/mod.rs:131) accepts every FP/SIMD prefix, so this property
        // currently fails — surfacing the latent validation gap.
        #[test]
        fn prop_rejects_fp_simd_registers(
            prefix in "[dsvhbq]",
            n in 0u32..=31u32,
            slot in 0usize..3,
        ) {
            let bad = format!("{}{}", prefix, n);
            let mut ops = vec![
                Operand::Reg("x0".into()),
                Operand::Reg("x1".into()),
                Operand::Reg("x2".into()),
                Operand::Cond("eq".into()),
            ];
            ops[slot] = Operand::Reg(bad);
            let result = encode_csinv(&ops);
            prop_assert!(
                result.is_err(),
                "encode_csinv should reject FP/SIMD register in slot {} (got {:?})",
                slot, result
            );
        }

        // Property G1 — MIXED-WIDTH negative contract (EXPECTED TO FAIL — see
        // BUG-report). Per ARM ARM (C4.1.67, conditional-select group) the `sf`
        // bit governs the width of Rd, Rn AND Rm COLLECTIVELY: all three
        // registers MUST be the same size, otherwise the encoding is
        // UNALLOCATED. A mixed-width tuple such as `csinv x0, w1, x2, eq` must
        // therefore be rejected. The encoder, however, derives `sf` solely from
        // Rd (operand 0) and silently discards the widths of Rn (operand 1)
        // and Rm (operand 2), emitting a malformed instruction. A correct
        // assembler MUST return Err for any non-uniform-width triple. No cited
        // spec permits mixing register widths within one instruction.
        #[test]
        fn prop_rejects_mixed_width_operands(
            rd_is64 in any::<bool>(),
            rn_is64 in any::<bool>(),
            rm_is64 in any::<bool>(),
            rd in 0u32..=30u32,
            rn in 0u32..=30u32,
            rm in 0u32..=30u32,
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            // Only exercise genuinely MIXED-width operand triples.
            prop_assume!(!(rd_is64 == rn_is64 && rn_is64 == rm_is64));
            let cond_name = COND_TABLE[cond_idx].0;
            let name = |is_64: bool, n: u32| {
                if is_64 { format!("x{}", n) } else { format!("w{}", n) }
            };
            let ops = vec![
                Operand::Reg(name(rd_is64, rd)),
                Operand::Reg(name(rn_is64, rn)),
                Operand::Reg(name(rm_is64, rm)),
                Operand::Cond(cond_name.to_string()),
            ];
            prop_assert!(
                encode_csinv(&ops).is_err(),
                "mixed-width CSINV {:?} must be rejected as UNALLOCATED; got {:?}",
                ops, encode_csinv(&ops)
            );
        }

        // Property G2 — mixed-width ROOT-CAUSE characterization (PASSING).
        // Property G1 fails because `sf` is taken from Rd alone: the widths of
        // Rn and Rm have ZERO effect on the encoded word. Holding Rd fixed and
        // independently flipping Rn's and Rm's x<->w width leaves the word
        // bit-identical — proving those widths are silently discarded, which is
        // precisely the gap G1 exposes.
        #[test]
        fn prop_rn_and_rm_widths_do_not_affect_word(
            rd_n in 0u32..=30u32,
            rd_is64 in any::<bool>(),
            rn_n in 0u32..=30u32,
            rm_n in 0u32..=30u32,
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let cond_name = COND_TABLE[cond_idx].0;
            let rd = if rd_is64 { format!("x{}", rd_n) } else { format!("w{}", rd_n) };
            let mk = |rn_64: bool, rm_64: bool| vec![
                Operand::Reg(rd.clone()),
                Operand::Reg(if rn_64 { format!("x{}", rn_n) } else { format!("w{}", rn_n) }),
                Operand::Reg(if rm_64 { format!("x{}", rm_n) } else { format!("w{}", rm_n) }),
                Operand::Cond(cond_name.to_string()),
            ];
            let base = enc(&mk(false, false));
            // Flipping Rn width, Rm width, or both must not change the word.
            prop_assert_eq!(enc(&mk(true, false)), base);
            prop_assert_eq!(enc(&mk(false, true)), base);
            prop_assert_eq!(enc(&mk(true, true)), base);
        }
    }
}

#[cfg(test)]
mod prop_encode_csneg_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- CSNEG opcode constants (ARM ARM C4.1.68, "Conditional Select (negate)") ----
    // CSNEG = sf 1 0 11010100 Rm cond 0 1 Rn Rd
    //   [31]    sf        — 1 = 64-bit (X), 0 = 32-bit (W); taken from Rd ONLY
    //   [30]    1         — op=1  (distinguishes CSINV/CSNEG from CSEL/CSINC)
    //   [29]    0         — S=0
    //   [28:21] 11010100  — fixed opcode for the conditional-select group
    //   [20:16] Rm
    //   [15:12] cond
    //   [11:10] 01        — o2=0, o1=1 (selects CSNEG within the group; o1=0 → CSINV)
    //   [9:5]   Rn
    //   [4:0]   Rd
    const OPCODE: u32 = 0b11010100u32 << 21; // == 0x1A80_0000, bits [28:21]
    // Bit [30] (op) must be ONE: shared with CSINV; distinguishes from CSEL/CSINC.
    const OP_BIT: u32 = 1u32 << 30; // == 0x4000_0000
    // Bit [10] (o1) must be ONE: this is what distinguishes CSNEG from CSINV.
    const O1_BIT: u32 = 1u32 << 10; // == 0x400
    // Bits that must be ZERO for CSNEG: [29] (S=0), [11] (o2=0).
    //   [11] == 0 + [10] == 1 distinguishes CSNEG from CSINV ([11:10]==00).
    const FIXED_ZERO: u32 = (1u32 << 29) | (1u32 << 11); // == 0x2000_0800

    /// Canonical ARM condition-code table mirroring `encode_cond`.
    const COND_TABLE: &[(&str, u32)] = &[
        ("eq", 0), ("ne", 1), ("cs", 2), ("hs", 2), ("cc", 3), ("lo", 3),
        ("mi", 4), ("pl", 5), ("vs", 6), ("vc", 7), ("hi", 8), ("ls", 9),
        ("ge", 10), ("lt", 11), ("gt", 12), ("le", 13), ("al", 14), ("nv", 15),
    ];

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word_of(encode_csneg(ops))
    }

    prop_compose! {
        fn arb_gp_reg()(n in 0u32..=30u32, is_64 in any::<bool>()) -> (String, u32) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            (name, n)
        }
    }

    proptest! {
        // Property A — full structural / field-placement oracle.
        // The encoded word is fully determined: opcode 11010100 in [28:21],
        // bit [30]=1 (op), bit [10]=1 (o1, the CSNEG marker), bits [29,11]
        // are zero, sf tracks Rd's width, and Rm/cond/Rn/Rd occupy exactly
        // their spec fields. Reconstructing from the fields reproduces the
        // whole word — nothing else is set.
        #[test]
        fn prop_opcode_structure_and_fields(
            (rd_name, rd_num) in arb_gp_reg(),
            (rn_name, rn_num) in arb_gp_reg(),
            (rm_name, rm_num) in arb_gp_reg(),
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, cond_val) = COND_TABLE[cond_idx];
            let ops = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Reg(rn_name),
                Operand::Reg(rm_name),
                Operand::Cond(cond_name.to_string()),
            ];
            let word = enc(&ops);

            // Fixed opcode bits [28:21].
            prop_assert_eq!(word & OPCODE, OPCODE);
            // Bits that must be zero for CSNEG.
            prop_assert_eq!(word & FIXED_ZERO, 0u32);
            // op bit [30] must be one — shared with CSINV; distinguishes from CSEL/CSINC.
            prop_assert_eq!(word & OP_BIT, OP_BIT);
            // o1 bit [10] must be one — the CSNEG distinguishing bit.
            prop_assert_eq!(word & O1_BIT, O1_BIT);
            // sf bit [31] tracks Rd's width.
            let expected_sf = if rd_name.starts_with('x') { 1u32 } else { 0u32 };
            prop_assert_eq!((word >> 31) & 1, expected_sf);
            // Rm [20:16], cond [15:12], Rn [9:5], Rd [4:0].
            prop_assert_eq!((word >> 16) & 0x1F, rm_num);
            prop_assert_eq!((word >> 12) & 0xF, cond_val);
            prop_assert_eq!((word >> 5) & 0x1F, rn_num);
            prop_assert_eq!(word & 0x1F, rd_num);
            // Full reconstruction — the word is exactly the OR of its fields.
            prop_assert_eq!(
                word,
                (expected_sf << 31) | OP_BIT | OPCODE | O1_BIT | (rm_num << 16)
                    | (cond_val << 12) | (rn_num << 5) | rd_num
            );
        }

        // Property B — differential: CSNEG and CSINV are the same instruction
        // group with identical op=1 and identical operand layout; they differ
        // ONLY in bit 10 (o1=1 for CSNEG, o1=0 for CSINV). This is the defining
        // differentiator between negate-select and invert-select.
        #[test]
        fn prop_csneg_xor_csinv_is_bit10(
            (rd_name, _) in arb_gp_reg(),
            (rn_name, _) in arb_gp_reg(),
            (rm_name, _) in arb_gp_reg(),
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, _) = COND_TABLE[cond_idx];
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Reg(rm_name),
                Operand::Cond(cond_name.to_string()),
            ];
            let csneg = enc(&ops);
            let csinv = word_of(encode_csinv(&ops));
            prop_assert_eq!(csneg ^ csinv, O1_BIT);
        }

        // Property C — differential: 64- vs 32-bit Rd differ ONLY in bit 31.
        // sf is derived solely from Rd (operand 0), so flipping x<->w on Rd
        // changes exactly one bit and leaves every other field untouched.
        #[test]
        fn prop_sf_bit_is_bit31(
            n in 0u32..=30u32,
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let cond_name = COND_TABLE[cond_idx].0;
            let mk = |rd: String| vec![
                Operand::Reg(rd),
                Operand::Reg("x1".into()),
                Operand::Reg("x2".into()),
                Operand::Cond(cond_name.to_string()),
            ];
            let w64 = enc(&mk(format!("x{}", n)));
            let w32 = enc(&mk(format!("w{}", n)));
            prop_assert_eq!(w64 ^ w32, 1u32 << 31);
        }

        // Property D — condition-code mapping round-trips for every name in
        // the canonical table, and the cs/hs & cc/lo aliases encode
        // bit-identically (encode_cond maps both spellings to the same value).
        #[test]
        fn prop_cond_round_trips_and_aliases(i in 0usize..COND_TABLE.len()) {
            let (name_i, val_i) = COND_TABLE[i];
            let ops = vec![
                Operand::Reg("x0".into()),
                Operand::Reg("x1".into()),
                Operand::Reg("x2".into()),
                Operand::Cond(name_i.to_string()),
            ];
            prop_assert_eq!((enc(&ops) >> 12) & 0xF, val_i);

            let base = |c: &str| enc(&[
                Operand::Reg("x0".into()),
                Operand::Reg("x1".into()),
                Operand::Reg("x2".into()),
                Operand::Cond(c.to_string()),
            ]);
            prop_assert_eq!(base("cs"), base("hs"));
            prop_assert_eq!(base("cc"), base("lo"));
        }

        // Property E — structural negative contract. CSNEG needs exactly four
        // operands: three registers followed by a condition. Lists that are too
        // short, that lack a trailing condition, or that place a non-register /
        // non-Cond in the wrong slot must make encode_csneg return Err — no
        // silent encoding and no panic. (Register-class validation is covered
        // separately in Property F.)
        #[test]
        fn prop_rejects_invalid_operands(case in 0usize..10usize) {
            let r = Operand::Reg("x0".into());
            let c = Operand::Cond("eq".into());
            let result = match case {
                0 => encode_csneg(&[]),                                            // no operands
                1 => encode_csneg(&[r.clone()]),                                   // only Rd
                2 => encode_csneg(&[r.clone(), r.clone()]),                        // Rd, Rn
                3 => encode_csneg(&[r.clone(), r.clone(), r.clone()]),             // no condition
                4 => encode_csneg(&[r.clone(), r.clone(), r.clone(), r.clone()]), // 4th not a Cond
                5 => encode_csneg(&[Operand::Imm(0), r.clone(), r.clone(), c.clone()]),          // Rd not a reg
                6 => encode_csneg(&[r.clone(), Operand::Imm(1), r.clone(), c.clone()]),          // Rn not a reg
                7 => encode_csneg(&[r.clone(), r.clone(), Operand::Symbol("s".into()), c.clone()]), // Rm not a reg
                8 => encode_csneg(&[r.clone(), r.clone(), r.clone(), Operand::Imm(4)]),          // cond is Imm
                _ => encode_csneg(&[r.clone(), r.clone(), r.clone(), Operand::Symbol("notcond".into())]), // cond is Symbol
            };
            prop_assert!(
                result.is_err(),
                "encode_csneg should reject case {} (got {:?})", case, result
            );
        }

        // Property F — register-class negative contract (EXPECTED TO FAIL — see
        // BUG report). CSNEG is defined ONLY on general-purpose (X/W) registers
        // (ARM ARM C4.1.68: "Conditional Select (negate)", GP register operands).
        // FP/SIMD register names (d/s/q/v/h/b) must therefore be rejected rather
        // than silently re-encoded with their numeric index as if they were GP
        // registers, which would emit a malformed instruction. `parse_reg_num`
        // (encoder/mod.rs:131) accepts every FP/SIMD prefix, and `get_reg` derives
        // sf only from is_64bit_reg (so an FP reg silently gets sf=0), so this
        // property currently fails — surfacing the latent validation gap shared
        // with CSEL/CSINC/CSINV.
        #[test]
        fn prop_rejects_fp_simd_registers(
            prefix in "[dsvhbq]",
            n in 0u32..=31u32,
            slot in 0usize..3,
        ) {
            let bad = format!("{}{}", prefix, n);
            let mut ops = vec![
                Operand::Reg("x0".into()),
                Operand::Reg("x1".into()),
                Operand::Reg("x2".into()),
                Operand::Cond("eq".into()),
            ];
            ops[slot] = Operand::Reg(bad);
            let result = encode_csneg(&ops);
            prop_assert!(
                result.is_err(),
                "encode_csneg should reject FP/SIMD register in slot {} (got {:?})",
                slot, result
            );
        }

        // Property G — MIXED-WIDTH negative contract (EXPECTED TO FAIL — see
        // BUG report). This is the focus of this suite.
        //
        // Per ARM ARM (Conditional Select / CSNEG, C4.1.68), the `sf` field
        // applies UNIFORMLY to Rd, Rn AND Rm: all three registers MUST share
        // one width. An instruction that mixes a 64-bit (xN) register with a
        // 32-bit (wN) register is UNPREDICTABLE / UNDEFINED and the encoder
        // MUST reject it with Err. GAS and LLVM-MC both reject such input
        // with "operand size mismatch".
        //
        // The implementation derives `sf` from Rd ONLY via
        // `get_reg(operands, 0)` and DISCARDS the widths returned for Rn and
        // Rm (`let (rn, _) = ...`, `let (rm, _) = ...`), so it silently
        // encodes malformed mixed-width instructions. No cited spec permits
        // mixed-width conditional-select encodings.
        #[test]
        fn prop_rejects_mixed_width_operands(
            rd_is64 in any::<bool>(),
            rn_is64 in any::<bool>(),
            rm_is64 in any::<bool>(),
            n in 0u32..=30u32,
            m in 0u32..=30u32,
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            // Only interesting when the three widths are NOT all equal.
            prop_assume!(!(rd_is64 == rn_is64 && rn_is64 == rm_is64));
            let rd = if rd_is64 { format!("x{}", n) } else { format!("w{}", n) };
            let rn = if rn_is64 { format!("x{}", n) } else { format!("w{}", n) };
            let rm = if rm_is64 { format!("x{}", m) } else { format!("w{}", m) };
            let (cond_name, _) = COND_TABLE[cond_idx];
            let ops = vec![
                Operand::Reg(rd),
                Operand::Reg(rn),
                Operand::Reg(rm),
                Operand::Cond(cond_name.to_string()),
            ];
            let res = encode_csneg(&ops);
            prop_assert!(
                res.is_err(),
                "mixed-width CSNEG ({:?}/{:?}/{:?}, {:?}) MUST be rejected — all of \
                 Rd/Rn/Rm must share the sf width; got {:?}",
                ops[0], ops[1], ops[2], ops[3], res
            );
        }
    }
}

#[cfg(test)]
mod prop_encode_cset_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- CSET opcode constants (ARM ARM C6.2.43, alias of CSINC) ----
    // CSET Rd, cond  ==  CSINC Rd, XZR, XZR, invert(cond)
    //   sf 0 0 11010100 11111 inv_cond 0 1 11111 Rd
    //   [31]    sf        — 1 = 64-bit (X), 0 = 32-bit (W); taken from Rd ONLY
    //   [30:29] 00        — op=0, S=0
    //   [28:21] 11010100  — conditional-select group opcode
    //   [20:16] 11111     — Rm = XZR (31)   [CSET-defining: must be all ones]
    //   [15:12] inv_cond  — condition ^ 1
    //   [11:10] 01        — o2=0, o1=1 (CSINC marker)
    //   [9:5]   11111     — Rn = XZR (31)   [CSET-defining: must be all ones]
    //   [4:0]   Rd
    const OPCODE: u32 = 0b11010100u32 << 21; // bits [28:21], == 0x1A80_0000
    const O1_BIT: u32 = 1u32 << 10;          // bit [10], the CSINC marker
    // Bits that must be ZERO for CSET: [30] (op), [29] (S), [11] (o2).
    const FIXED_ZERO: u32 = (1u32 << 30) | (1u32 << 29) | (1u32 << 11); // == 0x6000_0800
    // CSET-defining: Rm [20:16] and Rn [9:5] are BOTH the all-ones XZR field.
    // This is the structural signature that distinguishes CSET from CSINC.
    const RM_FIELD: u32 = 0b11111u32 << 16; // == 0x001F_0000
    const RN_FIELD: u32 = 0b11111u32 << 5;  // == 0x0000_03E0

    /// Canonical ARM condition-code table mirroring `encode_cond`.
    const COND_TABLE: &[(&str, u32)] = &[
        ("eq", 0), ("ne", 1), ("cs", 2), ("hs", 2), ("cc", 3), ("lo", 3),
        ("mi", 4), ("pl", 5), ("vs", 6), ("vc", 7), ("hi", 8), ("ls", 9),
        ("ge", 10), ("lt", 11), ("gt", 12), ("le", 13), ("al", 14), ("nv", 15),
    ];

    /// Condition-inversion table mirroring `inv_cond = cond ^ 1` (flip LSB).
    /// Each entry is (cond, name-of-condition-whose-encode_cond == cond ^ 1).
    /// Inversion pairs each condition with its logical opposite and swaps AL<->NV.
    const COND_INVERSION: &[(&str, &str)] = &[
        ("eq", "ne"), ("ne", "eq"),
        ("cs", "cc"), ("hs", "lo"),
        ("cc", "cs"), ("lo", "hs"),
        ("mi", "pl"), ("pl", "mi"),
        ("vs", "vc"), ("vc", "vs"),
        ("hi", "ls"), ("ls", "hi"),
        ("ge", "lt"), ("lt", "ge"),
        ("gt", "le"), ("le", "gt"),
        ("al", "nv"), ("nv", "al"),
    ];

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word_of(encode_cset(ops))
    }

    prop_compose! {
        fn arb_gp_reg()(n in 0u32..=30u32, is_64 in any::<bool>()) -> (String, u32) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            (name, n)
        }
    }

    proptest! {
        // Property A — full structural / field-placement oracle.
        // The encoded word is fully determined: opcode 11010100 in [28:21],
        // bit [10]=1 (o1, CSINC marker), bits [30,29,11] zero, BOTH Rm [20:16]
        // and Rn [9:5] are the all-ones XZR field (this is what makes it CSET
        // rather than a generic CSINC), cond holds the INVERTED condition, and
        // sf tracks Rd's width. Reconstructing from the fields reproduces the
        // whole word — nothing else is set.
        #[test]
        fn prop_opcode_structure_and_fields(
            (rd_name, rd_num) in arb_gp_reg(),
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, cond_val) = COND_TABLE[cond_idx];
            let ops = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Cond(cond_name.to_string()),
            ];
            let word = enc(&ops);

            // Fixed opcode bits [28:21].
            prop_assert_eq!(word & OPCODE, OPCODE);
            // Bits that must be zero for CSET.
            prop_assert_eq!(word & FIXED_ZERO, 0u32);
            // o1 bit [10] must be one — the CSINC marker.
            prop_assert_eq!(word & O1_BIT, O1_BIT);
            // Rm [20:16] and Rn [9:5] must BOTH be all-ones (XZR = 31). This is
            // the defining structural difference between CSET and CSINC.
            prop_assert_eq!(word & RM_FIELD, RM_FIELD);
            prop_assert_eq!(word & RN_FIELD, RN_FIELD);
            // sf bit [31] tracks Rd's width.
            let expected_sf = if rd_name.starts_with('x') { 1u32 } else { 0u32 };
            prop_assert_eq!((word >> 31) & 1, expected_sf);
            // cond [15:12] holds the INVERTED condition (cond ^ 1).
            prop_assert_eq!((word >> 12) & 0xF, cond_val ^ 1);
            // Rd [4:0].
            prop_assert_eq!(word & 0x1F, rd_num);
            // Full reconstruction — the word is exactly the OR of its fields.
            prop_assert_eq!(
                word,
                (expected_sf << 31) | OPCODE | O1_BIT | RM_FIELD
                    | ((cond_val ^ 1) << 12) | RN_FIELD | rd_num
            );
        }

        // Property B — differential: CSET is a pure alias of CSINC with
        // Rn = Rm = XZR and the inverted condition. CSET(Rd, cond) MUST equal
        // CSINC(Rd, XZR, XZR, invert(cond)) bit-for-bit for every register and
        // condition (ARM ARM C6.2.43). This is the defining alias relationship.
        #[test]
        fn prop_cset_equals_csinc_xzr_alias(
            (rd_name, _rd_num) in arb_gp_reg(),
            inv_idx in 0usize..COND_INVERSION.len(),
        ) {
            let (cond_name, inv_name) = COND_INVERSION[inv_idx];
            let cset_word = enc(&[
                Operand::Reg(rd_name.clone()),
                Operand::Cond(cond_name.to_string()),
            ]);
            // XZR parses to register number 31 in both Rn and Rm slots; its
            // width flag is discarded by encode_csinc for those operands.
            let csinc_word = word_of(encode_csinc(&[
                Operand::Reg(rd_name),
                Operand::Reg("xzr".into()),
                Operand::Reg("xzr".into()),
                Operand::Cond(inv_name.to_string()),
            ]));
            prop_assert_eq!(cset_word, csinc_word);
        }

        // Property C — condition inversion round-trips for every name in the
        // canonical table (cond field == encode_cond(name) ^ 1), and the cs/hs
        // & cc/lo aliases — which encode_cond maps to the same value — produce
        // bit-identical CSET words.
        #[test]
        fn prop_cond_inverted_and_aliases(cond_idx in 0usize..COND_TABLE.len()) {
            let (name, val) = COND_TABLE[cond_idx];
            let word = enc(&[
                Operand::Reg("x0".into()),
                Operand::Cond(name.to_string()),
            ]);
            prop_assert_eq!((word >> 12) & 0xF, val ^ 1);

            let base = |c: &str| enc(&[
                Operand::Reg("x0".into()),
                Operand::Cond(c.to_string()),
            ]);
            prop_assert_eq!(base("cs"), base("hs"));
            prop_assert_eq!(base("cc"), base("lo"));
        }

        // Property D — structural negative contract. CSET takes exactly two
        // operands: a register followed by a condition. Lists that are too
        // short, that lack a trailing condition, or that place a non-register /
        // non-Cond in the wrong slot must make encode_cset return Err — no
        // silent encoding and no panic.
        #[test]
        fn prop_rejects_invalid_operands(case in 0usize..9usize) {
            let r = Operand::Reg("x0".into());
            let c = Operand::Cond("eq".into());
            let result = match case {
                0 => encode_cset(&[]),                                            // no operands
                1 => encode_cset(&[r.clone()]),                                   // only Rd, no cond
                2 => encode_cset(&[Operand::Imm(0), c.clone()]),                  // Rd not a reg
                3 => encode_cset(&[Operand::Symbol("s".into()), c.clone()]),     // Rd is Symbol
                4 => encode_cset(&[r.clone(), r.clone()]),                        // 2nd not a Cond
                5 => encode_cset(&[r.clone(), Operand::Imm(4)]),                  // cond is Imm
                6 => encode_cset(&[r.clone(), Operand::Symbol("notcond".into())]),// cond is Symbol
                7 => encode_cset(&[r.clone(), Operand::Reg("x1".into())]),        // cond is Reg
                _ => encode_cset(&[r.clone(), Operand::Extend { kind: "sxtw".into(), amount: 0 }]),
            };
            prop_assert!(
                result.is_err(),
                "encode_cset should reject case {} (got {:?})", case, result
            );
        }

        // Property E — register-class negative contract (EXPECTED TO FAIL — see
        // BUG report). CSET is defined ONLY on general-purpose (X/W) registers
        // (ARM ARM C6.2.43: GP register destination). FP/SIMD register names
        // (d/s/q/v/h/b) must therefore be rejected rather than silently
        // re-encoded with their numeric index and sf=0, which would emit a
        // malformed instruction. `parse_reg_num` (encoder/mod.rs:131) accepts
        // every FP/SIMD prefix, and `get_reg` derives sf only from
        // is_64bit_reg, so this property currently fails — surfacing the latent
        // validation gap shared with CSEL/CSINC/CSINV/CSNEG.
        #[test]
        fn prop_rejects_fp_simd_registers(
            prefix in "[dsvhbq]",
            n in 0u32..=31u32,
        ) {
            let bad = format!("{}{}", prefix, n);
            let ops = vec![Operand::Reg(bad), Operand::Cond("eq".into())];
            let result = encode_cset(&ops);
            prop_assert!(
                result.is_err(),
                "encode_cset should reject FP/SIMD destination (got {:?})", result
            );
        }

        // Property F — condition-code negative contract (EXPECTED TO FAIL — see
        // BUG report, CSET-SPECIFIC). Per the ARM ARM, CSET is an alias of CSINC
        // and the aliased CSINC condition must NOT be AL or NV (those are
        // reserved / UNDEFINED encodings that reference assemblers — GAS and
        // LLVM llvm-mc — reject). Since invert(cond) swaps AL<->NV, a
        // user-facing `cset Rd, al` / `cset Rd, nv` produces a CSINC whose
        // condition is NV / AL: a reserved word. encode_cset performs no such
        // check and currently emits the reserved encoding.
        #[test]
        fn prop_rejects_al_nv_conditions(case in 0usize..2usize) {
            let cond_name = ["al", "nv"][case];
            let ops = vec![Operand::Reg("x0".into()), Operand::Cond(cond_name.to_string())];
            let result = encode_cset(&ops);
            prop_assert!(
                result.is_err(),
                "encode_cset should reject reserved condition '{}' (got {:?})",
                cond_name, result
            );
        }
    }
}

#[cfg(test)]
mod prop_encode_csetm_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- CSETM opcode constants (ARM ARM C6.2.44, alias of CSINV) ----
    // CSETM Rd, cond  ==  CSINV Rd, XZR, XZR, invert(cond)
    //   sf 1 0 11010100 11111 inv_cond 0 0 11111 Rd
    //   [31]    sf        — 1 = 64-bit (X), 0 = 32-bit (W); taken from Rd ONLY
    //   [30]    1         — op=1  (CSETM is the invert-select family; distinguishes
    //                              it from CSEL/CSINC/CSET which all have op=0)
    //   [29]    0         — S=0
    //   [28:21] 11010100  — conditional-select group opcode
    //   [20:16] 11111     — Rm = XZR (31)   [CSETM-defining: must be all ones]
    //   [15:12] inv_cond  — condition ^ 1
    //   [11:10] 00        — o2=0, o1=0 (CSINV marker; o1=1 would be CSNEG)
    //   [9:5]   11111     — Rn = XZR (31)   [CSETM-defining: must be all ones]
    //   [4:0]   Rd
    const OPCODE: u32 = 0b11010100u32 << 21; // bits [28:21], == 0x1A80_0000
    // Bit [30] (op) must be ONE: this is what places CSETM in the CSINV/CSNEG
    // (invert/negate) family rather than the CSEL/CSINC (select/increment) family.
    const OP_BIT: u32 = 1u32 << 30; // == 0x4000_0000
    // Bits that must be ZERO for CSETM: [29] (S=0), [11] (o2=0), [10] (o1=0).
    //   [11:10] == 00 is the CSINV marker; o1=1 (bit 10) would select CSNEG.
    const FIXED_ZERO: u32 = (1u32 << 29) | (1u32 << 11) | (1u32 << 10); // == 0x2000_0C00
    // CSETM-defining: Rm [20:16] and Rn [9:5] are BOTH the all-ones XZR field.
    // This is the structural signature that distinguishes CSETM from generic CSINV.
    const RM_FIELD: u32 = 0b11111u32 << 16; // == 0x001F_0000
    const RN_FIELD: u32 = 0b11111u32 << 5;  // == 0x0000_03E0
    // The bits that differ between CSETM (CSINV alias: op=1,o1=0) and CSET
    // (CSINC alias: op=0,o1=1): bit 30 (op) and bit 10 (o1) both flip.
    const DIFF_VS_CSET: u32 = (1u32 << 30) | (1u32 << 10); // == 0x4000_0400

    /// Canonical ARM condition-code table mirroring `encode_cond`.
    const COND_TABLE: &[(&str, u32)] = &[
        ("eq", 0), ("ne", 1), ("cs", 2), ("hs", 2), ("cc", 3), ("lo", 3),
        ("mi", 4), ("pl", 5), ("vs", 6), ("vc", 7), ("hi", 8), ("ls", 9),
        ("ge", 10), ("lt", 11), ("gt", 12), ("le", 13), ("al", 14), ("nv", 15),
    ];

    /// Condition-inversion table mirroring `inv_cond = cond ^ 1` (flip LSB).
    /// Each entry is (cond, name-of-condition-whose-encode_cond == cond ^ 1).
    /// Inversion pairs each condition with its logical opposite and swaps AL<->NV.
    const COND_INVERSION: &[(&str, &str)] = &[
        ("eq", "ne"), ("ne", "eq"),
        ("cs", "cc"), ("hs", "lo"),
        ("cc", "cs"), ("lo", "hs"),
        ("mi", "pl"), ("pl", "mi"),
        ("vs", "vc"), ("vc", "vs"),
        ("hi", "ls"), ("ls", "hi"),
        ("ge", "lt"), ("lt", "ge"),
        ("gt", "le"), ("le", "gt"),
        ("al", "nv"), ("nv", "al"),
    ];

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word_of(encode_csetm(ops))
    }

    prop_compose! {
        fn arb_gp_reg()(n in 0u32..=30u32, is_64 in any::<bool>()) -> (String, u32) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            (name, n)
        }
    }

    proptest! {
        // Property A — full structural / field-placement oracle.
        // The encoded word is fully determined: opcode 11010100 in [28:21],
        // bit [30]=1 (op), bits [29,11,10] zero, BOTH Rm [20:16] and Rn [9:5]
        // are the all-ones XZR field (this is what makes it CSETM rather than a
        // generic CSINV), cond holds the INVERTED condition, and sf tracks Rd's
        // width. Reconstructing from the fields reproduces the whole word —
        // nothing else is set.
        #[test]
        fn prop_opcode_structure_and_fields(
            (rd_name, rd_num) in arb_gp_reg(),
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, cond_val) = COND_TABLE[cond_idx];
            let ops = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Cond(cond_name.to_string()),
            ];
            let word = enc(&ops);

            // Fixed opcode bits [28:21].
            prop_assert_eq!(word & OPCODE, OPCODE);
            // Bits that must be zero for CSETM.
            prop_assert_eq!(word & FIXED_ZERO, 0u32);
            // op bit [30] must be one — the CSINV/CSETM family marker.
            prop_assert_eq!(word & OP_BIT, OP_BIT);
            // Rm [20:16] and Rn [9:5] must BOTH be all-ones (XZR = 31). This is
            // the defining structural difference between CSETM and CSINV.
            prop_assert_eq!(word & RM_FIELD, RM_FIELD);
            prop_assert_eq!(word & RN_FIELD, RN_FIELD);
            // sf bit [31] tracks Rd's width.
            let expected_sf = if rd_name.starts_with('x') { 1u32 } else { 0u32 };
            prop_assert_eq!((word >> 31) & 1, expected_sf);
            // cond [15:12] holds the INVERTED condition (cond ^ 1).
            prop_assert_eq!((word >> 12) & 0xF, cond_val ^ 1);
            // Rd [4:0].
            prop_assert_eq!(word & 0x1F, rd_num);
            // Full reconstruction — the word is exactly the OR of its fields.
            prop_assert_eq!(
                word,
                (expected_sf << 31) | OP_BIT | OPCODE | RM_FIELD
                    | ((cond_val ^ 1) << 12) | RN_FIELD | rd_num
            );
        }

        // Property B — differential: CSETM is a pure alias of CSINV with
        // Rn = Rm = XZR and the inverted condition. CSETM(Rd, cond) MUST equal
        // CSINV(Rd, XZR, XZR, invert(cond)) bit-for-bit for every register and
        // condition (ARM ARM C6.2.44). This is the defining alias relationship.
        #[test]
        fn prop_csetm_equals_csinv_xzr_alias(
            (rd_name, _rd_num) in arb_gp_reg(),
            inv_idx in 0usize..COND_INVERSION.len(),
        ) {
            let (cond_name, inv_name) = COND_INVERSION[inv_idx];
            let csetm_word = enc(&[
                Operand::Reg(rd_name.clone()),
                Operand::Cond(cond_name.to_string()),
            ]);
            // XZR parses to register number 31 in both Rn and Rm slots; its
            // width flag is discarded by encode_csinv for those operands.
            let csinv_word = word_of(encode_csinv(&[
                Operand::Reg(rd_name),
                Operand::Reg("xzr".into()),
                Operand::Reg("xzr".into()),
                Operand::Cond(inv_name.to_string()),
            ]));
            prop_assert_eq!(csetm_word, csinv_word);
        }

        // Property C — condition inversion round-trips for every name in the
        // canonical table (cond field == encode_cond(name) ^ 1), and the cs/hs
        // & cc/lo aliases — which encode_cond maps to the same value — produce
        // bit-identical CSETM words.
        #[test]
        fn prop_cond_inverted_and_aliases(cond_idx in 0usize..COND_TABLE.len()) {
            let (name, val) = COND_TABLE[cond_idx];
            let word = enc(&[
                Operand::Reg("x0".into()),
                Operand::Cond(name.to_string()),
            ]);
            prop_assert_eq!((word >> 12) & 0xF, val ^ 1);

            let base = |c: &str| enc(&[
                Operand::Reg("x0".into()),
                Operand::Cond(c.to_string()),
            ]);
            prop_assert_eq!(base("cs"), base("hs"));
            prop_assert_eq!(base("cc"), base("lo"));
        }

        // Property D — differential: CSETM (CSINV alias: op=1, o1=0) and CSET
        // (CSINC alias: op=0, o1=1) share the identical XZR-pinned Rm/Rn fields
        // and inverted-condition layout; they differ ONLY in bit 30 (op) and
        // bit 10 (o1), which both flip. For identical (Rd, sf, cond) the two
        // instructions' XOR is exactly these two bits — no more, no less.
        #[test]
        fn prop_csetm_xor_cset_is_bits30_and_10(
            (rd_name, _rd_num) in arb_gp_reg(),
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, _) = COND_TABLE[cond_idx];
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Cond(cond_name.to_string()),
            ];
            let csetm_word = enc(&ops);
            let cset_word = word_of(encode_cset(&ops));
            prop_assert_eq!(csetm_word ^ cset_word, DIFF_VS_CSET);
        }

        // Property E — structural negative contract. CSETM takes exactly two
        // operands: a register followed by a condition. Lists that are too
        // short, that lack a trailing condition, or that place a non-register /
        // non-Cond in the wrong slot must make encode_csetm return Err — no
        // silent encoding and no panic.
        #[test]
        fn prop_rejects_invalid_operands(case in 0usize..9usize) {
            let r = Operand::Reg("x0".into());
            let c = Operand::Cond("eq".into());
            let result = match case {
                0 => encode_csetm(&[]),                                            // no operands
                1 => encode_csetm(&[r.clone()]),                                   // only Rd, no cond
                2 => encode_csetm(&[Operand::Imm(0), c.clone()]),                  // Rd not a reg
                3 => encode_csetm(&[Operand::Symbol("s".into()), c.clone()]),     // Rd is Symbol
                4 => encode_csetm(&[r.clone(), r.clone()]),                        // 2nd not a Cond
                5 => encode_csetm(&[r.clone(), Operand::Imm(4)]),                  // cond is Imm
                6 => encode_csetm(&[r.clone(), Operand::Symbol("notcond".into())]),// cond is Symbol
                7 => encode_csetm(&[r.clone(), Operand::Reg("x1".into())]),        // cond is Reg
                _ => encode_csetm(&[r.clone(), Operand::Extend { kind: "sxtw".into(), amount: 0 }]),
            };
            prop_assert!(
                result.is_err(),
                "encode_csetm should reject case {} (got {:?})", case, result
            );
        }

        // Property F — register-class negative contract (EXPECTED TO FAIL — see
        // BUG report). CSETM is defined ONLY on general-purpose (X/W) registers
        // (ARM ARM C6.2.44: GP register destination). FP/SIMD register names
        // (d/s/q/v/h/b) must therefore be rejected rather than silently
        // re-encoded with their numeric index and sf=0, which would emit a
        // malformed instruction. `parse_reg_num` (encoder/mod.rs:131) accepts
        // every FP/SIMD prefix, and `get_reg` derives sf only from
        // is_64bit_reg, so this property currently fails — surfacing the latent
        // validation gap shared with CSEL/CSINC/CSINV/CSNEG/CSET.
        #[test]
        fn prop_rejects_fp_simd_registers(
            prefix in "[dsvhbq]",
            n in 0u32..=31u32,
        ) {
            let bad = format!("{}{}", prefix, n);
            let ops = vec![Operand::Reg(bad), Operand::Cond("eq".into())];
            let result = encode_csetm(&ops);
            prop_assert!(
                result.is_err(),
                "encode_csetm should reject FP/SIMD destination (got {:?})", result
            );
        }

        // Property G — condition-code negative contract (EXPECTED TO FAIL — see
        // BUG report, CSETM-SPECIFIC). Per the ARM ARM, CSETM is an alias of
        // CSINV and the aliased CSINV condition must NOT be AL or NV (those are
        // reserved / UNDEFINED encodings that reference assemblers — GAS and
        // LLVM llvm-mc — reject). Since invert(cond) swaps AL<->NV, a
        // user-facing `csetm Rd, al` / `csetm Rd, nv` produces a CSINV whose
        // condition is NV / AL: a reserved word. encode_csetm performs no such
        // check and currently emits the reserved encoding.
        #[test]
        fn prop_rejects_al_nv_conditions(case in 0usize..2usize) {
            let cond_name = ["al", "nv"][case];
            let ops = vec![Operand::Reg("x0".into()), Operand::Cond(cond_name.to_string())];
            let result = encode_csetm(&ops);
            prop_assert!(
                result.is_err(),
                "encode_csetm should reject reserved condition '{}' (got {:?})",
                cond_name, result
            );
        }
    }
}

#[cfg(test)]
mod prop_encode_cinc_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- CINC opcode constants (ARM ARM C4.1.66 CSINC; alias C6.2.45 CINC) ----
    // CINC is an alias: `CINC Rd, Rn, cond` -> `CSINC Rd, Rn, Rn, invert(cond)`.
    // CSINC = sf 0 0 11010100 Rm cond 0 1 Rn Rd, with Rm fixed equal to Rn.
    //   [31]    sf        - 1 = 64-bit (X), 0 = 32-bit (W); taken from Rd ONLY
    //   [30:29] 00        - op=0, S=0
    //   [28:21] 11010100  - fixed opcode for the conditional-select group
    //   [20:16] Rm        - == Rn (the alias sets Rm = Rn)
    //   [15:12] cond      - the INVERTED condition
    //   [11:10] 01        - o2=0, o1=1 (selects CSINC within the group)
    //   [9:5]   Rn
    //   [4:0]   Rd
    const OPCODE: u32 = 0b11010100u32 << 21; // == 0x1A80_0000, bits [28:21]
    // Bits that must be ZERO for CINC: [30, 29, 11] (bit 10 is the CSINC marker = 1).
    const FIXED_ZERO: u32 = (1u32 << 30) | (1u32 << 29) | (1u32 << 11); // == 0x6000_0800
    // o1 bit [10] must be ONE: this is what distinguishes CSINC from CSEL.
    const O1_BIT: u32 = 1u32 << 10; // == 0x400

    /// Canonical ARM condition-code table mirroring `encode_cond`.
    const COND_TABLE: &[(&str, u32)] = &[
        ("eq", 0), ("ne", 1), ("cs", 2), ("hs", 2), ("cc", 3), ("lo", 3),
        ("mi", 4), ("pl", 5), ("vs", 6), ("vc", 7), ("hi", 8), ("ls", 9),
        ("ge", 10), ("lt", 11), ("gt", 12), ("le", 13), ("al", 14), ("nv", 15),
    ];
    /// Reverse map: cond value -> canonical name (first spelling). Used to name
    /// the inverted condition when building the CSINC differential oracle.
    const COND_NAMES_BY_VAL: [&str; 16] = [
        "eq", "ne", "cs", "cc", "mi", "pl", "vs", "vc",
        "hi", "ls", "ge", "lt", "gt", "le", "al", "nv",
    ];

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word_of(encode_cinc(ops))
    }

    prop_compose! {
        fn arb_gp_reg()(n in 0u32..=30u32, is_64 in any::<bool>()) -> (String, u32) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            (name, n)
        }
    }

    proptest! {
        // Property A - full structural / field-placement oracle.
        // The encoded word is fully determined: opcode 11010100 in [28:21],
        // bits [30,29,11] are zero, bit [10] is one (the CSINC marker), sf
        // tracks Rd's width, Rm==Rn occupy [20:16] and [9:5], the INVERTED
        // condition occupies [15:12], and Rd occupies [4:0]. Reconstructing
        // from the fields reproduces the whole word - nothing else is set.
        #[test]
        fn prop_opcode_structure_and_fields(
            (rd_name, rd_num) in arb_gp_reg(),
            (rn_name, rn_num) in arb_gp_reg(),
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, cond_val) = COND_TABLE[cond_idx];
            let ops = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Reg(rn_name),
                Operand::Cond(cond_name.to_string()),
            ];
            let word = enc(&ops);
            let inv_cond = cond_val ^ 1;

            // Fixed opcode bits [28:21].
            prop_assert_eq!(word & OPCODE, OPCODE);
            // Bits that must be zero for CINC/CSINC.
            prop_assert_eq!(word & FIXED_ZERO, 0u32);
            // o1 bit [10] must be one - the CSINC distinguishing bit.
            prop_assert_eq!(word & O1_BIT, O1_BIT);
            // sf bit [31] tracks Rd's width.
            let expected_sf = if rd_name.starts_with('x') { 1u32 } else { 0u32 };
            prop_assert_eq!((word >> 31) & 1, expected_sf);
            // Rm [20:16] == Rn [9:5] == Rn register number (alias sets Rm = Rn).
            prop_assert_eq!((word >> 16) & 0x1F, rn_num);
            prop_assert_eq!((word >> 5) & 0x1F, rn_num);
            // Inverted condition [15:12].
            prop_assert_eq!((word >> 12) & 0xF, inv_cond);
            // Rd [4:0].
            prop_assert_eq!(word & 0x1F, rd_num);
            // Full reconstruction - the word is exactly the OR of its fields.
            prop_assert_eq!(
                word,
                (expected_sf << 31) | OPCODE | O1_BIT | (rn_num << 16)
                    | (inv_cond << 12) | (rn_num << 5) | rd_num
            );
        }

        // Property B - differential: CINC is an alias of CSINC.
        // `cinc Rd, Rn, cond` MUST encode identically to
        // `csinc Rd, Rn, Rn, invert(cond)`. This is the defining alias
        // relationship (ARM ARM C6.2.45) and the strongest available oracle.
        #[test]
        fn prop_cinc_equals_csinc_alias(
            (rd_name, _) in arb_gp_reg(),
            (rn_name, rn_num) in arb_gp_reg(),
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, cond_val) = COND_TABLE[cond_idx];
            let inv_name = COND_NAMES_BY_VAL[(cond_val ^ 1) as usize];

            let cinc_ops = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Reg(rn_name.clone()),
                Operand::Cond(cond_name.to_string()),
            ];
            let csinc_ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name.clone()),
                Operand::Reg(rn_name), // Rm == Rn
                Operand::Cond(inv_name.to_string()),
            ];
            // Sanity: the csinc oracle itself is internally consistent
            // (its Rm field really does hold rn_num).
            let csinc_word = word_of(encode_csinc(&csinc_ops));
            prop_assert_eq!((csinc_word >> 16) & 0x1F, rn_num);
            // The alias relationship.
            prop_assert_eq!(enc(&cinc_ops), csinc_word);
        }

        // Property C - Rm==Rn invariant (CINC-specific).
        // Unlike general CSINC (which takes a distinct Rm), CINC always sets
        // Rm = Rn. Therefore the [20:16] and [9:5] fields must be equal for
        // EVERY input, and both must equal the Rn register number.
        #[test]
        fn prop_rm_equals_rn(
            (rd_name, _) in arb_gp_reg(),
            (rn_name, rn_num) in arb_gp_reg(),
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let cond_name = COND_TABLE[cond_idx].0;
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(rn_name),
                Operand::Cond(cond_name.to_string()),
            ];
            let word = enc(&ops);
            let rm = (word >> 16) & 0x1F;
            let rn = (word >> 5) & 0x1F;
            prop_assert_eq!(rm, rn);
            prop_assert_eq!(rm, rn_num);
        }

        // Property D - condition inversion + complementary pairs.
        // (1) The cond field always equals encode_cond(c) ^ 1 (LSB-flipped).
        // (2) Complementary condition pairs (eq/ne, cs/cc, ... al/nv) differ
        //     ONLY in bit 12: inverting a complement flips just the cond LSB,
        //     leaving every other field identical.
        #[test]
        fn prop_condition_is_inverted(
            (rd_name, _) in arb_gp_reg(),
            (rn_name, _) in arb_gp_reg(),
            k in 0usize..8usize, // pairs (2k, 2k+1)
        ) {
            let lo = COND_NAMES_BY_VAL[2 * k];      // value 2k
            let hi = COND_NAMES_BY_VAL[2 * k + 1];   // value 2k+1
            let mk = |c: &str| vec![
                Operand::Reg(rd_name.clone()),
                Operand::Reg(rn_name.clone()),
                Operand::Cond(c.to_string()),
            ];
            let w_lo = enc(&mk(lo));
            let w_hi = enc(&mk(hi));
            // cond field == value ^ 1.
            prop_assert_eq!((w_lo >> 12) & 0xF, (2 * k as u32) ^ 1);
            prop_assert_eq!((w_hi >> 12) & 0xF, ((2 * k + 1) as u32) ^ 1);
            // Complementary inputs differ only in bit 12.
            prop_assert_eq!(w_lo ^ w_hi, 1u32 << 12);
        }

        // Property E - negative contract (deterministic enumeration).
        // CINC is defined ONLY on general-purpose (X/W) registers (ARM ARM
        // C6.2.45), and the aliased CSINC must NOT carry condition AL or NV
        // (ARM ARM C4.1.66: cond 1110/1111 is constrained-UNPREDICTABLE; GAS
        // and llvm-mc reject `cinc Rd, Rn, al|nv`). The encoder must therefore
        // reject: FP/SIMD register names, reserved al/nv conditions, out-of-
        // range register numbers, and malformed operand lists. We enumerate
        // EVERY violation and assert each is rejected so all defects surface.
        #[test]
        fn prop_rejects_invalid_operands(case in 0usize..22usize) {
            let r = Operand::Reg("x0".into());
            let c = Operand::Cond("eq".into());
            let result = match case {
                // --- malformed operand structure (must be rejected) ---
                0  => encode_cinc(&[]),                                  // no operands
                1  => encode_cinc(&[r.clone()]),                         // only Rd
                2  => encode_cinc(&[r.clone(), r.clone()]),              // Rd, Rn, no cond
                3  => encode_cinc(&[r.clone(), r.clone(), r.clone()]),   // 3rd not a Cond
                4  => encode_cinc(&[Operand::Imm(0), r.clone(), c.clone()]),        // Rd not a reg
                5  => encode_cinc(&[r.clone(), Operand::Imm(1), c.clone()]),        // Rn not a reg
                6  => encode_cinc(&[r.clone(), r.clone(), Operand::Imm(4)]),        // cond is Imm
                7  => encode_cinc(&[r.clone(), r.clone(), Operand::Symbol("s".into())]), // cond is Symbol
                // --- out-of-range register numbers (must be rejected) ---
                8  => encode_cinc(&[Operand::Reg("x32".into()), r.clone(), c.clone()]),
                9  => encode_cinc(&[r.clone(), Operand::Reg("w99".into()), c.clone()]),
                // --- FP/SIMD register names in a GP-only instruction ---
                // (CINC is GP-only; d/s/q/v/h/b must NOT be silently
                // re-encoded with their numeric index as a GP register.)
                10 => encode_cinc(&[Operand::Reg("d0".into()), r.clone(), c.clone()]), // Rd is FP
                11 => encode_cinc(&[Operand::Reg("s1".into()), r.clone(), c.clone()]),
                12 => encode_cinc(&[r.clone(), Operand::Reg("v2".into()), c.clone()]), // Rn is SIMD
                13 => encode_cinc(&[r.clone(), Operand::Reg("q3".into()), c.clone()]),
                14 => encode_cinc(&[Operand::Reg("h4".into()), r.clone(), c.clone()]),
                15 => encode_cinc(&[r.clone(), Operand::Reg("b5".into()), c.clone()]),
                // --- reserved conditions al/nv (UNPREDICTABLE aliased CSINC) ---
                16 => encode_cinc(&[r.clone(), r.clone(), Operand::Cond("al".into())]),
                17 => encode_cinc(&[r.clone(), r.clone(), Operand::Cond("nv".into())]),
                // --- reserved condition via case-folded spelling ---
                18 => encode_cinc(&[r.clone(), r.clone(), Operand::Cond("AL".into())]),
                19 => encode_cinc(&[r.clone(), r.clone(), Operand::Cond("Nv".into())]),
                // --- FP reg combined with reserved cond ---
                20 => encode_cinc(&[Operand::Reg("d0".into()), r.clone(), Operand::Cond("al".into())]),
                _  => encode_cinc(&[Operand::Reg("v7".into()), Operand::Reg("s9".into()), Operand::Cond("nv".into())]),
            };
            prop_assert!(
                result.is_err(),
                "encode_cinc should reject case {} (got {:?})",
                case, result
            );
        }
    }
}

#[cfg(test)]
mod prop_encode_cinv_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- CINV opcode constants (ARM ARM C4.1.66, "Conditional Select (inverted)") ----
    // CINV Rd, Rn, cond is an alias of CSINV Rd, Rn, Rn, invert(cond):
    //   CSINV = sf 1 0 11010100 Rm cond 0 0 Rn Rd   with Rm == Rn, cond inverted.
    //   [31]    sf        — 1 = 64-bit (X), 0 = 32-bit (W); taken from Rd ONLY
    //   [30]    1         — op (selects the CSINV/CSNEG inverted-select family)
    //   [29]    0         — S = 0
    //   [28:21] 11010100  — fixed opcode for the conditional-select group
    //   [20:16] Rm        — aliased onto Rn
    //   [15:12] cond      — invert(input condition)
    //   [11:10] 00        — o2=0, o1=0 (selects CSINV within the group)
    //   [9:5]   Rn
    //   [4:0]   Rd
    const OP_BIT: u32 = 1u32 << 30;            // bit [30] must be ONE
    const OPCODE: u32 = 0b11010100u32 << 21;   // bits [28:21] == 0x1A80_0000
    const OPCODE_MASK: u32 = 0x1FE0_0000;      // bits [28:21]
    // Bits that must be ZERO for CINV: [29] (S), [11] (o2), [10] (o1).
    const FIXED_ZERO: u32 = (1u32 << 29) | (1u32 << 11) | (1u32 << 10); // == 0x2000_0800

    /// Canonical ARM condition-code table mirroring `encode_cond`.
    const COND_TABLE: &[(&str, u32)] = &[
        ("eq", 0), ("ne", 1), ("cs", 2), ("hs", 2), ("cc", 3), ("lo", 3),
        ("mi", 4), ("pl", 5), ("vs", 6), ("vc", 7), ("hi", 8), ("ls", 9),
        ("ge", 10), ("lt", 11), ("gt", 12), ("le", 13), ("al", 14), ("nv", 15),
    ];

    /// Map a condition value back to one canonical name (for building the
    /// inverted-condition operand that `encode_cinv` must match).
    fn name_of_cond(v: u32) -> &'static str {
        match v {
            0 => "eq", 1 => "ne", 2 => "cs", 3 => "cc", 4 => "mi", 5 => "pl",
            6 => "vs", 7 => "vc", 8 => "hi", 9 => "ls", 10 => "ge", 11 => "lt",
            12 => "gt", 13 => "le", 14 => "al", _ => "nv",
        }
    }

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word_of(encode_cinv(ops))
    }

    prop_compose! {
        fn arb_gp_reg()(n in 0u32..=30u32, is_64 in any::<bool>()) -> (String, u32) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            (name, n)
        }
    }

    proptest! {
        // Property A — full structural / field-placement oracle.
        // The encoded word is fully determined: bit 30 set, opcode 11010100 in
        // [28:21], bits [29,11,10] zero, sf tracks Rd's width, cond is the
        // INVERTED input condition, and the CINV alias collapses Rm onto Rn
        // (Rm field == Rn field == rn). Reconstructing from the fields
        // reproduces the whole word — nothing else is set.
        #[test]
        fn prop_opcode_structure_and_fields(
            (rd_name, rd_num) in arb_gp_reg(),
            rn_num in 0u32..=30u32,
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, cond_val) = COND_TABLE[cond_idx];
            let inv_cond = cond_val ^ 1;
            let ops = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Reg(format!("x{}", rn_num)),
                Operand::Cond(cond_name.to_string()),
            ];
            let word = enc(&ops);

            // Fixed opcode bits.
            prop_assert_eq!(word & OP_BIT, OP_BIT);
            prop_assert_eq!(word & OPCODE_MASK, OPCODE);
            prop_assert_eq!(word & FIXED_ZERO, 0u32);
            // sf bit [31] tracks Rd's width.
            let expected_sf = if rd_name.starts_with('x') { 1u32 } else { 0u32 };
            prop_assert_eq!((word >> 31) & 1, expected_sf);
            // Rm [20:16] is aliased onto Rn: both equal rn_num.
            prop_assert_eq!((word >> 16) & 0x1F, rn_num);
            // cond [15:12] is the INVERTED input condition.
            prop_assert_eq!((word >> 12) & 0xF, inv_cond);
            // Rn [9:5] and Rd [4:0].
            prop_assert_eq!((word >> 5) & 0x1F, rn_num);
            prop_assert_eq!(word & 0x1F, rd_num);
            // CINV-distinct invariant: Rm field must equal Rn field.
            prop_assert_eq!((word >> 16) & 0x1F, (word >> 5) & 0x1F);
            // Full reconstruction — the word is exactly the OR of its fields.
            prop_assert_eq!(
                word,
                (expected_sf << 31) | OP_BIT | OPCODE | (rn_num << 16)
                    | (inv_cond << 12) | (rn_num << 5) | rd_num
            );
        }

        // Property B — defining semantic oracle (differential).
        // CINV Rd, Rn, cond  ==  CSINV Rd, Rn, Rn, invert(cond).
        // The alias must produce the bit-identical encoding of its base form
        // for every register pair and every condition code (incl. al<->nv).
        #[test]
        fn prop_cinv_equals_csinv_rn_rn_inverted(
            (rd_name, rd_num) in arb_gp_reg(),
            rn_num in 0u32..=30u32,
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, cond_val) = COND_TABLE[cond_idx];
            let inv_name = name_of_cond(cond_val ^ 1);
            let cinv_ops = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Reg(format!("x{}", rn_num)),
                Operand::Cond(cond_name.to_string()),
            ];
            let csinv_ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(format!("x{}", rn_num)),
                Operand::Reg(format!("x{}", rn_num)),   // Rm == Rn
                Operand::Cond(inv_name.to_string()),     // inverted cond
            ];
            prop_assert_eq!(enc(&cinv_ops), word_of(encode_csinv(&csinv_ops)));
        }

        // Property C — differential: 64- vs 32-bit Rd differ ONLY in bit 31.
        // sf is derived solely from Rd (operand 0); the Rn source register's
        // own width is ignored (only its number is read), so flipping x<->w on
        // Rd changes exactly one bit and leaves every other field untouched.
        #[test]
        fn prop_sf_bit_is_bit31(
            n in 0u32..=30u32,
            rn_num in 0u32..=30u32,
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let cond_name = COND_TABLE[cond_idx].0;
            let mk = |rd: String| vec![
                Operand::Reg(rd),
                Operand::Reg(format!("x{}", rn_num)),
                Operand::Cond(cond_name.to_string()),
            ];
            let w64 = enc(&mk(format!("x{}", n)));
            let w32 = enc(&mk(format!("w{}", n)));
            prop_assert_eq!(w64 ^ w32, 1u32 << 31);
        }

        // Property D — negative contract on operand shape. CINV takes exactly
        // three operands: Rd, Rn, cond. Lists that are too short, that lack a
        // trailing condition, or that place a non-register in the Rd/Rn slots
        // must make encode_cinv return Err — no silent encoding, no panic.
        #[test]
        fn prop_rejects_invalid_operands(case in 0usize..10usize) {
            let r = Operand::Reg("x0".into());
            let c = Operand::Cond("eq".into());
            let result = match case {
                0 => encode_cinv(&[]),                                            // no operands
                1 => encode_cinv(&[r.clone()]),                                   // only Rd
                2 => encode_cinv(&[r.clone(), r.clone()]),                        // Rd, Rn (no cond)
                3 => encode_cinv(&[r.clone(), r.clone(), r.clone()]),             // 3rd not a Cond
                4 => encode_cinv(&[Operand::Imm(0), r.clone(), c.clone()]),       // Rd not a reg
                5 => encode_cinv(&[r.clone(), Operand::Imm(1), c.clone()]),       // Rn not a reg
                6 => encode_cinv(&[r.clone(), r.clone(), Operand::Imm(4)]),       // cond is Imm
                7 => encode_cinv(&[r.clone(), r.clone(), Operand::Symbol("s".into())]), // cond is Symbol
                8 => encode_cinv(&[Operand::Reg("xyz".into()), r.clone(), c.clone()]), // malformed Rd
                _ => encode_cinv(&[r.clone(), Operand::Reg("zz".into()), c.clone()]),  // malformed Rn
            };
            prop_assert!(result.is_err(), "encode_cinv should reject case {} (got {:?})", case, result);
        }

        // Property E — register-class negative contract. CINV/CSINV are defined
        // ONLY on general-purpose (X/W) registers (ARM ARM C4.1.66). FP/SIMD
        // register names (d/s/q/v/h/b) must therefore be rejected rather than
        // silently re-encoded with their numeric index as if they were GP
        // registers.
        #[test]
        fn prop_rejects_fp_simd_registers(
            prefix in "[dsvhbq]",
            n in 0u32..=31u32,
            slot in 0usize..2,   // Rd or Rn slot
        ) {
            let bad = format!("{}{}", prefix, n);
            let mut ops = vec![
                Operand::Reg("x0".into()),
                Operand::Reg("x1".into()),
                Operand::Cond("eq".into()),
            ];
            ops[slot] = Operand::Reg(bad);
            let result = encode_cinv(&ops);
            prop_assert!(
                result.is_err(),
                "encode_cinv should reject FP/SIMD register in slot {} (got {:?})",
                slot, result
            );
        }
    }
}

#[cfg(test)]
mod prop_encode_cneg_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- CNEG opcode constants (ARM ARM C4.1.67, "Conditional Select (negate)") ----
    // CNEG Rd, Rn, cond is an alias of CSNEG Rd, Rn, Rn, invert(cond):
    //   CSNEG = sf 1 0 11010100 Rm cond 0 1 Rn Rd   with Rm == Rn, cond inverted.
    //   [31]    sf        — 1 = 64-bit (X), 0 = 32-bit (W); taken from Rd ONLY
    //   [30]    1         — op (selects the CSINV/CSNEG inverted-select family)
    //   [29]    0         — S = 0
    //   [28:21] 11010100  — fixed opcode for the conditional-select group
    //   [20:16] Rm        — aliased onto Rn
    //   [15:12] cond      — invert(input condition)
    //   [11:10] 01        — o2=0, o1=1 (selects CSNEG within the group; o1=0 → CSINV)
    //   [9:5]   Rn
    //   [4:0]   Rd
    const OP_BIT: u32 = 1u32 << 30;            // bit [30] must be ONE
    const O1_BIT: u32 = 1u32 << 10;            // bit [10] must be ONE (the CSNEG/CNEG marker)
    const OPCODE: u32 = 0b11010100u32 << 21;   // bits [28:21] == 0x1A80_0000
    const OPCODE_MASK: u32 = 0x1FE0_0000;      // bits [28:21]
    // Bits that must be ZERO for CNEG: [29] (S), [11] (o2).
    //   [11] == 0 + [10] == 1 distinguishes CNEG from CINV ([11:10] == 00).
    const FIXED_ZERO: u32 = (1u32 << 29) | (1u32 << 11); // == 0x2000_0800

    /// Canonical ARM condition-code table mirroring `encode_cond`.
    const COND_TABLE: &[(&str, u32)] = &[
        ("eq", 0), ("ne", 1), ("cs", 2), ("hs", 2), ("cc", 3), ("lo", 3),
        ("mi", 4), ("pl", 5), ("vs", 6), ("vc", 7), ("hi", 8), ("ls", 9),
        ("ge", 10), ("lt", 11), ("gt", 12), ("le", 13), ("al", 14), ("nv", 15),
    ];

    /// Map a condition value back to one canonical name (for building the
    /// inverted-condition operand that the base CSNEG form must match).
    fn name_of_cond(v: u32) -> &'static str {
        match v {
            0 => "eq", 1 => "ne", 2 => "cs", 3 => "cc", 4 => "mi", 5 => "pl",
            6 => "vs", 7 => "vc", 8 => "hi", 9 => "ls", 10 => "ge", 11 => "lt",
            12 => "gt", 13 => "le", 14 => "al", _ => "nv",
        }
    }

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word_of(encode_cneg(ops))
    }

    prop_compose! {
        fn arb_gp_reg()(n in 0u32..=30u32, is_64 in any::<bool>()) -> (String, u32) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            (name, n)
        }
    }

    proptest! {
        // Property A — full structural / field-placement oracle.
        // The encoded word is fully determined: bit 30 set, opcode 11010100 in
        // [28:21], bit [10] set with bit [11] clear (o2:o1 == 01, the CSNEG
        // marker that distinguishes CNEG from CINV), bit [29] zero, sf tracks
        // Rd's width, cond is the INVERTED input condition, and the CNEG alias
        // collapses Rm onto Rn (Rm field == Rn field == rn). Reconstructing
        // from the fields reproduces the whole word — nothing else is set.
        #[test]
        fn prop_opcode_structure_and_fields(
            (rd_name, rd_num) in arb_gp_reg(),
            rn_num in 0u32..=30u32,
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, cond_val) = COND_TABLE[cond_idx];
            let inv_cond = cond_val ^ 1;
            let ops = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Reg(format!("x{}", rn_num)),
                Operand::Cond(cond_name.to_string()),
            ];
            let word = enc(&ops);

            // Fixed opcode bits.
            prop_assert_eq!(word & OP_BIT, OP_BIT);
            prop_assert_eq!(word & O1_BIT, O1_BIT);
            prop_assert_eq!(word & OPCODE_MASK, OPCODE);
            prop_assert_eq!(word & FIXED_ZERO, 0u32);
            // sf bit [31] tracks Rd's width.
            let expected_sf = if rd_name.starts_with('x') { 1u32 } else { 0u32 };
            prop_assert_eq!((word >> 31) & 1, expected_sf);
            // Rm [20:16] is aliased onto Rn: both equal rn_num.
            prop_assert_eq!((word >> 16) & 0x1F, rn_num);
            // cond [15:12] is the INVERTED input condition.
            prop_assert_eq!((word >> 12) & 0xF, inv_cond);
            // Rn [9:5] and Rd [4:0].
            prop_assert_eq!((word >> 5) & 0x1F, rn_num);
            prop_assert_eq!(word & 0x1F, rd_num);
            // CNEG-distinct invariant: Rm field must equal Rn field.
            prop_assert_eq!((word >> 16) & 0x1F, (word >> 5) & 0x1F);
            // Full reconstruction — the word is exactly the OR of its fields.
            prop_assert_eq!(
                word,
                (expected_sf << 31) | OP_BIT | OPCODE | O1_BIT | (rn_num << 16)
                    | (inv_cond << 12) | (rn_num << 5) | rd_num
            );
        }

        // Property B — defining semantic oracle (differential).
        // CNEG Rd, Rn, cond  ==  CSNEG Rd, Rn, Rn, invert(cond).
        // The alias must produce the bit-identical encoding of its base form
        // for every register pair and every condition code (incl. al<->nv).
        #[test]
        fn prop_cneg_equals_csneg_rn_rn_inverted(
            (rd_name, rd_num) in arb_gp_reg(),
            rn_num in 0u32..=30u32,
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, cond_val) = COND_TABLE[cond_idx];
            let inv_name = name_of_cond(cond_val ^ 1);
            let cneg_ops = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Reg(format!("x{}", rn_num)),
                Operand::Cond(cond_name.to_string()),
            ];
            let csneg_ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(format!("x{}", rn_num)),
                Operand::Reg(format!("x{}", rn_num)),   // Rm == Rn
                Operand::Cond(inv_name.to_string()),     // inverted cond
            ];
            prop_assert_eq!(enc(&cneg_ops), word_of(encode_csneg(&csneg_ops)));
        }

        // Property C — differential: CNEG and CINV are the same alias shape
        // (both collapse Rm onto Rn and invert the condition) and share op=1;
        // they differ ONLY in bit 10 (o1=1 for CNEG/CSNEG, o1=0 for CINV/CSINV).
        // This is the defining differentiator between negate-select and
        // invert-select.
        #[test]
        fn prop_cneg_xor_cinv_is_bit10(
            (rd_name, _) in arb_gp_reg(),
            rn_num in 0u32..=30u32,
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let (cond_name, _) = COND_TABLE[cond_idx];
            let ops = vec![
                Operand::Reg(rd_name),
                Operand::Reg(format!("x{}", rn_num)),
                Operand::Cond(cond_name.to_string()),
            ];
            let cneg = enc(&ops);
            let cinv = word_of(encode_cinv(&ops));
            prop_assert_eq!(cneg ^ cinv, O1_BIT);
        }

        // Property D — differential: 64- vs 32-bit Rd differ ONLY in bit 31.
        // sf is derived solely from Rd (operand 0); the Rn source register's
        // own width is ignored (only its number is read), so flipping x<->w on
        // Rd changes exactly one bit and leaves every other field untouched.
        #[test]
        fn prop_sf_bit_is_bit31(
            n in 0u32..=30u32,
            rn_num in 0u32..=30u32,
            cond_idx in 0usize..COND_TABLE.len(),
        ) {
            let cond_name = COND_TABLE[cond_idx].0;
            let mk = |rd: String| vec![
                Operand::Reg(rd),
                Operand::Reg(format!("x{}", rn_num)),
                Operand::Cond(cond_name.to_string()),
            ];
            let w64 = enc(&mk(format!("x{}", n)));
            let w32 = enc(&mk(format!("w{}", n)));
            prop_assert_eq!(w64 ^ w32, 1u32 << 31);
        }

        // Property E — negative contract on operand shape. CNEG takes exactly
        // three operands: Rd, Rn, cond. Lists that are too short, that lack a
        // trailing condition, or that place a non-register in the Rd/Rn slots
        // must make encode_cneg return Err — no silent encoding, no panic.
        #[test]
        fn prop_rejects_invalid_operands(case in 0usize..10usize) {
            let r = Operand::Reg("x0".into());
            let c = Operand::Cond("eq".into());
            let result = match case {
                0 => encode_cneg(&[]),                                            // no operands
                1 => encode_cneg(&[r.clone()]),                                   // only Rd
                2 => encode_cneg(&[r.clone(), r.clone()]),                        // Rd, Rn (no cond)
                3 => encode_cneg(&[r.clone(), r.clone(), r.clone()]),             // 3rd not a Cond
                4 => encode_cneg(&[Operand::Imm(0), r.clone(), c.clone()]),       // Rd not a reg
                5 => encode_cneg(&[r.clone(), Operand::Imm(1), c.clone()]),       // Rn not a reg
                6 => encode_cneg(&[r.clone(), r.clone(), Operand::Imm(4)]),       // cond is Imm
                7 => encode_cneg(&[r.clone(), r.clone(), Operand::Symbol("s".into())]), // cond is Symbol
                8 => encode_cneg(&[Operand::Reg("xyz".into()), r.clone(), c.clone()]), // malformed Rd
                _ => encode_cneg(&[r.clone(), Operand::Reg("zz".into()), c.clone()]),  // malformed Rn
            };
            prop_assert!(result.is_err(), "encode_cneg should reject case {} (got {:?})", case, result);
        }

        // Property F — register-class negative contract (EXPECTED TO FAIL — see
        // BUG report). CNEG/CSNEG are defined ONLY on general-purpose (X/W)
        // registers (ARM ARM C4.1.67: "Conditional Select (negate)", GP
        // register operands). FP/SIMD register names (d/s/q/v/h/b) must
        // therefore be rejected rather than silently re-encoded with their
        // numeric index as if they were GP registers, which would emit a
        // malformed instruction. `parse_reg_num` (encoder/mod.rs:131) accepts
        // every FP/SIMD prefix, and `get_reg` derives sf only from
        // is_32bit_reg (so an FP reg silently gets sf=0), so this property
        // currently fails — surfacing the latent validation gap shared with
        // CSEL/CSINC/CSINV/CINC/CINV.
        #[test]
        fn prop_rejects_fp_simd_registers(
            prefix in "[dsvhbq]",
            n in 0u32..=31u32,
            slot in 0usize..2,   // Rd or Rn slot
        ) {
            let bad = format!("{}{}", prefix, n);
            let mut ops = vec![
                Operand::Reg("x0".into()),
                Operand::Reg("x1".into()),
                Operand::Cond("eq".into()),
            ];
            ops[slot] = Operand::Reg(bad);
            let result = encode_cneg(&ops);
            prop_assert!(
                result.is_err(),
                "encode_cneg should reject FP/SIMD register in slot {} (got {:?})",
                slot, result
            );
        }
    }
}

#[cfg(test)]
mod prop_encode_bl_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- Opcode constants for the BL instruction class (ARM ARM C5.6.21/22) ----
    // BL = 1001 01 imm26. Opcode 0b100101 occupies bits [31:26]; the imm26
    // branch-offset field [25:0] is left zero for the linker to fill via a
    // Call26 (R_AARCH64_CALL26) relocation.
    const BL_OPCODE: u32 = 0b100101u32 << 26; // == 0x9400_0000
    const OPCODE_MASK: u32 = 0xFC00_0000;     // bits [31:26]
    const IMM26_MASK: u32 = 0x03FF_FFFF;      // bits [25:0] (linker-filled, must be zero)

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::WordWithReloc { word, .. }) => word,
            other => panic!("expected WordWithReloc, got {:?}", other),
        }
    }

    fn reloc_of(r: Result<EncodeResult, String>) -> Relocation {
        match r {
            Ok(EncodeResult::WordWithReloc { reloc, .. }) => reloc,
            other => panic!("expected WordWithReloc, got {:?}", other),
        }
    }

    /// Mirrors `get_symbol`'s forwarding table: every operand kind it accepts
    /// and the (symbol, addend) the encoder is expected to forward into the
    /// relocation.
    fn accepted_operand_and_expected(
        sym: String,
        off: i64,
        kind_idx: usize,
    ) -> (Operand, String, i64) {
        let cases: Vec<(Operand, String, i64)> = vec![
            (Operand::Symbol(sym.clone()), sym.clone(), 0),
            (Operand::Label(sym.clone()), sym.clone(), 0),
            (Operand::SymbolOffset(sym.clone(), off), sym.clone(), off),
            (Operand::Modifier { kind: "lo12".into(), symbol: sym.clone() }, sym.clone(), 0),
            (Operand::ModifierOffset {
                kind: "lo12".into(), symbol: sym.clone(), offset: off,
            }, sym.clone(), off),
            // The parser misclassifies symbol names colliding with register /
            // condition / barrier names; `get_symbol` accepts them as symbols.
            (Operand::Reg(sym.clone()), sym.clone(), 0),
            (Operand::Cond(sym.clone()), sym.clone(), 0),
            (Operand::Barrier(sym.clone()), sym.clone(), 0),
        ];
        cases[kind_idx].clone()
    }

    prop_compose! {
        fn arb_accepted_symbol()(
            s in "[a-z][a-z0-9_]{0,7}",
            off in -8192i64..=8192i64,
            kind_idx in 0usize..8usize,
        ) -> (Operand, String, i64) {
            accepted_operand_and_expected(s, off, kind_idx)
        }
    }

    proptest! {
        // Property A — opcode structure oracle. The encoded word is fully
        // determined: opcode 0b100101 in bits [31:26] and the imm26 offset
        // field [25:0] is left zero for the linker to fill.
        #[test]
        fn prop_opcode_structure_and_imm26_zero(
            sym in "[a-z][a-z0-9_]{0,7}",
        ) {
            let ops = vec![Operand::Symbol(sym)];
            let word = word_of(encode_bl(&ops));
            prop_assert_eq!(word & OPCODE_MASK, BL_OPCODE);
            prop_assert_eq!(word & IMM26_MASK, 0u32);
            // Equivalently: the word is exactly the fixed base, independent of operand.
            prop_assert_eq!(word, BL_OPCODE);
        }

        // Property B — differential: BL (encode_bl) and B (encode_branch)
        // share the 100101/000101 layout and differ ONLY in bit 31 (the link
        // bit). BL is the link variant (bit 31 = 1); B is not.
        #[test]
        fn prop_bl_vs_branch_differ_only_bit31(
            sym in "[a-z][a-z0-9_]{0,7}",
        ) {
            let ops = vec![Operand::Symbol(sym)];
            let bl_word = word_of(encode_bl(&ops));
            let b_word = word_of(encode_branch(&ops));
            prop_assert_eq!(bl_word ^ b_word, 1u32 << 31);
            // BL must be the one with the link bit set.
            prop_assert_eq!(bl_word & (1u32 << 31), 1u32 << 31);
            prop_assert_eq!(b_word & (1u32 << 31), 0u32);
        }

        // Property C — relocation contract across every operand kind that
        // `get_symbol` accepts: the result carries a Call26 relocation whose
        // symbol & addend exactly mirror the input operand.
        #[test]
        fn prop_reloc_is_call26_with_symbol(
            (op, exp_sym, exp_off) in arb_accepted_symbol(),
        ) {
            let reloc = reloc_of(encode_bl(&[op]));
            prop_assert!(matches!(reloc.reloc_type, RelocType::Call26));
            prop_assert_eq!(reloc.symbol, exp_sym);
            prop_assert_eq!(reloc.addend, exp_off);
        }

        // Property D — word invariance across ALL accepted operand kinds.
        // Property A only proves the constant word 0x9400_0000 for the
        // `Symbol` form; Property C only checks the *relocation* across
        // kinds. This closes the gap: the instruction word must be the
        // fixed base for EVERY kind `get_symbol` accepts, because the
        // operand influences ONLY the relocation, never the word.
        #[test]
        fn prop_word_is_operand_independent(
            (op, _sym, _off) in arb_accepted_symbol(),
        ) {
            let ops = vec![op];
            prop_assert_eq!(word_of(encode_bl(&ops)), BL_OPCODE);
        }

        // Property E — negative contract. Operand kinds that `get_symbol`
        // does NOT accept must make encode_bl return Err; no silent encoding
        // of an invalid call target. Also: an empty operand vector (no
        // target at all) must be rejected — `get_symbol` reads operands[0]
        // unconditionally.
        #[test]
        fn prop_rejects_non_symbol_and_empty_operands(
            idx in 0usize..11usize,
        ) {
            let rejected: Vec<Operand> = vec![
                Operand::Imm(42),
                Operand::Mem { base: "x0".into(), offset: 0 },
                Operand::MemExpr {
                    base: "x0".into(), expr: "foo".into(), writeback: false,
                },
                Operand::MemPreIndex { base: "x0".into(), offset: 8 },
                Operand::MemPostIndex { base: "x0".into(), offset: 8 },
                Operand::MemRegOffset {
                    base: "x0".into(), index: "x1".into(), extend: None, shift: None,
                },
                Operand::Shift { kind: "lsl".into(), amount: 2 },
                Operand::Extend { kind: "sxtw".into(), amount: 0 },
                Operand::Expr("x + y".into()),
                Operand::RegArrangement { reg: "v0".into(), arrangement: "16b".into() },
                Operand::RegLane { reg: "v0".into(), elem_size: "s".into(), index: 2 },
            ];
            let op = rejected[idx].clone();
            prop_assert!(
                encode_bl(&[op]).is_err(),
                "encode_bl should reject this operand as a call target"
            );
            // Missing operand: empty vector cannot yield a relocation.
            let empty: Vec<Operand> = vec![];
            prop_assert!(encode_bl(&empty).is_err());
        }

        // Property F — negative contract for parser-token collisions. BL takes
        // a branch target label/immediate, not a register, condition code, or
        // barrier mnemonic. These token kinds must be rejected at assembler
        // level instead of being forwarded as relocation symbols.
        #[test]
        fn prop_rejects_reg_cond_barrier_targets(
            idx in 0usize..6usize,
        ) {
            let invalid = [
                Operand::Reg("x0".into()),
                Operand::Reg("wzr".into()),
                Operand::Cond("eq".into()),
                Operand::Cond("nv".into()),
                Operand::Barrier("sy".into()),
                Operand::Barrier("ish".into()),
            ];
            let op = invalid[idx].clone();
            prop_assert!(
                encode_bl(&[op]).is_err(),
                "encode_bl should reject register/condition/barrier tokens as call targets"
            );
        }

        // Property G — determinism / purity. Encoding the same operand
        // repeatedly yields bit-identical word AND relocation (symbol and
        // addend). encode_bl is a pure function of its operands.
        #[test]
        fn prop_encoding_is_deterministic(
            (op, exp_sym, exp_off) in arb_accepted_symbol(),
        ) {
            let ops = vec![op];
            prop_assert_eq!(
                word_of(encode_bl(&ops)),
                word_of(encode_bl(&ops))
            );
            let rel1 = reloc_of(encode_bl(&ops));
            let rel2 = reloc_of(encode_bl(&ops));
            prop_assert_eq!(&rel1.symbol, &rel2.symbol);
            prop_assert_eq!(&rel1.symbol, &exp_sym);
            prop_assert_eq!(rel1.addend, rel2.addend);
            prop_assert_eq!(rel1.addend, exp_off);
        }
    }
}

#[cfg(test)]
mod prop_encode_br_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- BR opcode constants (ARM ARM C5.6.17, "Branch to register") ----
    // BR <Xn> = 1101 0110 0001 1111 0000 00 Rn 00000
    //   [31:10] fixed opcode  == 0xD61F_0000
    //   [9:5]   Rn            (5-bit register field)
    //   [4:0]   00000         (reserved, must be zero)
    const OPCODE: u32 = 0xD61F_0000;
    const RN_MASK: u32 = 0x1Fu32 << 5;   // bits [9:5]
    const LOW5: u32 = 0x1Fu32;           // bits [4:0] (reserved zero)

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word_of(encode_br(ops))
    }

    // General-purpose register: x{n}/w{n} for n in 0..=30 (avoid the
    // xzr/sp-encoded 31 so structural assertions are unambiguous).
    prop_compose! {
        fn arb_gp_reg()(n in 0u32..=30u32, is_64 in any::<bool>()) -> (String, u32) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            (name, n)
        }
    }




    proptest! {
        // Property A — full structural / field-placement oracle.
        // For a valid GP register the encoded word is fully determined: every
        // bit outside [9:5] is fixed to the BR opcode, the low 5 bits are
        // reserved-zero, and Rn occupies [9:5]. Reconstruction must match.
        #[test]
        fn prop_opcode_structure_and_fields(
            (rn_name, rn_num) in arb_gp_reg(),
        ) {
            let ops = vec![Operand::Reg(rn_name.clone())];
            let word = enc(&ops);

            // Every bit outside the Rn field is the fixed opcode.
            prop_assert_eq!(word & !RN_MASK, OPCODE);
            // Low 5 reserved bits are zero.
            prop_assert_eq!(word & LOW5, 0u32);
            // Rn field [9:5] carries the register number.
            prop_assert_eq!((word >> 5) & 0x1F, rn_num);
            // Whole word reconstructs exactly from opcode + shifted Rn.
            prop_assert_eq!(word, OPCODE | (rn_num << 5));
        }

        // Property B — register-number range validation (negative contract).
        // The Rn field is a 5-bit UNSIGNED value, valid only 0..=31. A
        // register number above 31 MUST be rejected with Err rather than
        // silently masked via `rn << 5` (which would wrap/truncate). The
        // shared parse_reg_num helper enforces `num <= 31`.
        #[test]
        fn prop_rejects_register_number_above_31(
            n in 32u32..=999u32,
            is_64 in any::<bool>(),
        ) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            let ops = vec![Operand::Reg(name.clone())];
            prop_assert!(
                encode_br(&ops).is_err(),
                "register {} (valid 0..=31) must be rejected, got {:?}",
                name, encode_br(&ops)
            );
        }

        // Property C — dead-parameter / width-independence characterization
        // (FINDING). encode_br binds `(rn, _) = get_reg(...)` and DISCARDS the
        // `is_64` flag, so `br x{n}` and `br w{n}` encode bit-identically. Per
        // the ARM ARM (C5.6.17) BR's sole operand is `<Xn>`: the 32-bit W form
        // is unallocated and a conforming assembler (e.g. GAS) rejects
        // `br w0`. This encoder accepts it. The differential below pins that
        // dead-`is_64` behavior; flip to `is_err` once width is validated.
        #[test]
        fn prop_width_independent_w_form_accepted(
            n in 0u32..=30u32,
        ) {
            let x = enc(&[Operand::Reg(format!("x{}", n))]);
            let w = enc(&[Operand::Reg(format!("w{}", n))]);
            // Currently bit-identical (is_64 discarded).
            prop_assert_eq!(x, w, "br x{} == br w{} (is_64 dead; W form wrongly accepted)", n, n);
            // The acceptance itself is the bug — currently Ok, should be Err.
            prop_assert!(encode_br(&[Operand::Reg(format!("w{}", n))]).is_ok(),
                "BUG: br w{} is accepted, not rejected", n);
        }

        // Property D — sp silently aliased to xzr (KNOWN-BUG characterization).
        // Per the ARM ARM, BR's Rn field value 31 denotes XZR (there is NO
        // SP-using form of BR). The shared parse_reg_num maps both `sp` and
        // `xzr` to 31, so `br sp` silently produces a branch to address 0
        // (== `br xzr`) instead of being rejected as an unallocated encoding.
        // Sibling encoders (CBZ/CBNZ, etc.) exhibit the identical defect.
        #[test]
        fn prop_sp_silently_aliased_to_xzr(
            _dummy in 0u32..=0u32,
        ) {
            let sp = enc(&[Operand::Reg("sp".into())]);
            let xzr = enc(&[Operand::Reg("xzr".into())]);
            prop_assert_eq!(sp, xzr, "br sp == br xzr (SP silently aliased to XZR)");
            prop_assert_eq!((sp >> 5) & 0x1F, 31);
            prop_assert_eq!(sp & !RN_MASK, OPCODE);
            // 64-bit accepted (bug) — a fixed assembler must reject `br sp`.
            prop_assert!(encode_br(&[Operand::Reg("sp".into())]).is_ok(),
                "BUG: br sp is accepted, not rejected");

            // 32-bit twin: wsp == wzr.
            let wsp = enc(&[Operand::Reg("wsp".into())]);
            let wzr = enc(&[Operand::Reg("wzr".into())]);
            prop_assert_eq!(wsp, wzr, "br wsp == br wzr (WSP silently aliased to WZR)");
            prop_assert_eq!((wsp >> 5) & 0x1F, 31);
        }

        // Property E — FP/SIMD register silently accepted (FINDING).
        // BR's `<Xn>` operand is a GENERAL-PURPOSE register; FP/SIMD operands
        // (`d/s/q/v/h/b`) are the wrong register class and `br d0`, `br v5`,
        // etc. are unallocated encodings a conforming assembler must reject.
        // The shared parse_reg_num accepts those prefixes, so they silently
        // encode as `br x{n}`. The acceptance below pins the bug.
        #[test]
        fn prop_fp_simd_registers_wrongly_accepted(
            prefix_idx in 0usize..6usize,
            n in 0u32..=31u32,
        ) {
            let prefixes = ["d", "s", "q", "v", "h", "b"];
            let prefix = prefixes[prefix_idx];
            let name = format!("{}{}", prefix, n);
            let ops = vec![Operand::Reg(name.clone())];

            // Currently accepted — the bug.
            prop_assert!(encode_br(&ops).is_ok(),
                "BUG: br {} is accepted (FP/SIMD register), should be rejected", name);
            // And it encodes identically to the GP register of the same number.
            let fp_word = enc(&ops);
            let gp_word = enc(&[Operand::Reg(format!("x{}", n))]);
            prop_assert_eq!(fp_word, gp_word,
                "br {} silently encodes as br x{} (wrong register class)", name, n);
        }

        // Property F — non-register operand negative contract.
        // Operand kinds that get_reg does NOT accept (and an empty operand
        // vector) must make encode_br return Err; no silent encoding of a
        // non-register branch target, and no panic on missing operands.
        #[test]
        fn prop_rejects_non_register_operands(
            case in 0usize..12usize,
        ) {
            let rejected: Vec<Operand> = vec![
                Operand::Imm(42),
                Operand::Symbol("tgt".into()),
                Operand::Label("tgt".into()),
                Operand::SymbolOffset("tgt".into(), 8),
                Operand::Mem { base: "x0".into(), offset: 0 },
                Operand::MemExpr {
                    base: "x0".into(), expr: "foo".into(), writeback: false,
                },
                Operand::MemPreIndex { base: "x0".into(), offset: 8 },
                Operand::MemPostIndex { base: "x0".into(), offset: 8 },
                Operand::Shift { kind: "lsl".into(), amount: 2 },
                Operand::Cond("eq".into()),
                Operand::Expr("a + b".into()),
                Operand::RegList(vec![Operand::Reg("x0".into())]),
            ];
            let result = match case {
                0 => encode_br(&[]),                       // empty vector
                _ => {
                    let op = rejected[case - 1].clone();
                    encode_br(&[op])
                }
            };
            prop_assert!(result.is_err(),
                "encode_br should reject case {} (got {:?})", case, result);
        }

        // Property G — determinism / purity. Encoding the same operand twice
        // yields a bit-identical word; encode_br is a pure function of its
        // operand (no relocation, no hidden state).
        #[test]
        fn prop_encoding_is_deterministic(
            (rn_name, _) in arb_gp_reg(),
        ) {
            let ops = vec![Operand::Reg(rn_name)];
            prop_assert_eq!(enc(&ops), enc(&ops));
            prop_assert_eq!(enc(&ops), enc(&ops));
        }
    }
}

#[cfg(test)]
mod prop_encode_blr_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- BLR opcode constants (ARM ARM C5.6.18, "Branch with Link to Register") ----
    // BLR <Xn> = 1101 0110 0011 1111 0000 00 Rn 00000
    //   [31:10] fixed opcode  == 0xD63F_0000
    //   [9:5]   Rn            (5-bit register field)
    //   [4:0]   00000         (reserved, must be zero)
    // BLR differs from BR (C5.6.17) ONLY in bit 21: BR opc=0001, BLR opc=0011.
    const OPCODE: u32 = 0xD63F_0000;
    const RN_MASK: u32 = 0x1Fu32 << 5;   // bits [9:5]
    const LOW5: u32 = 0x1Fu32;           // bits [4:0] (reserved zero)
    const BLR_VS_BR_BIT: u32 = 1u32 << 21; // the sole bit distinguishing BLR from BR

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word_of(encode_blr(ops))
    }

    fn enc_br(ops: &[Operand]) -> u32 {
        // BR shares the same operand contract; used only for the differential.
        match encode_br(ops) {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    // General-purpose register: x{n}/w{n} for n in 0..=30 (avoid the
    // xzr/sp-encoded 31 so structural assertions are unambiguous).
    prop_compose! {
        fn arb_gp_reg()(n in 0u32..=30u32, is_64 in any::<bool>()) -> (String, u32) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            (name, n)
        }
    }

    proptest! {
        // Property A — full structural / field-placement oracle.
        // For a valid GP register the encoded word is fully determined: every
        // bit outside [9:5] is fixed to the BLR opcode, the low 5 bits are
        // reserved-zero, and Rn occupies [9:5]. Reconstruction must match.
        #[test]
        fn prop_opcode_structure_and_fields(
            (rn_name, rn_num) in arb_gp_reg(),
        ) {
            let ops = vec![Operand::Reg(rn_name.clone())];
            let word = enc(&ops);

            // Every bit outside the Rn field is the fixed opcode.
            prop_assert_eq!(word & !RN_MASK, OPCODE);
            // Low 5 reserved bits are zero.
            prop_assert_eq!(word & LOW5, 0u32);
            // Rn field [9:5] carries the register number.
            prop_assert_eq!((word >> 5) & 0x1F, rn_num);
            // Whole word reconstructs exactly from opcode + shifted Rn.
            prop_assert_eq!(word, OPCODE | (rn_num << 5));
        }

        // Property B — differential: BLR and BR share the identical operand
        // contract and differ ONLY in bit 21 (the link bit). For any valid
        // register, blr{x} ^ br{x} == 1<<21.
        #[test]
        fn prop_blr_xor_br_is_bit21(
            (rn_name, _) in arb_gp_reg(),
        ) {
            let ops = vec![Operand::Reg(rn_name)];
            prop_assert_eq!(enc(&ops) ^ enc_br(&ops), BLR_VS_BR_BIT);
        }

        // Property C — register-number range validation (negative contract).
        // The Rn field is a 5-bit UNSIGNED value, valid only 0..=31. A
        // register number above 31 MUST be rejected with Err rather than
        // silently masked via `rn << 5` (which would wrap/truncate). The
        // shared parse_reg_num helper enforces `num <= 31`.
        #[test]
        fn prop_rejects_register_number_above_31(
            n in 32u32..=999u32,
            is_64 in any::<bool>(),
        ) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            let ops = vec![Operand::Reg(name.clone())];
            prop_assert!(
                encode_blr(&ops).is_err(),
                "register {} (valid 0..=31) must be rejected, got {:?}",
                name, encode_blr(&ops)
            );
        }

        // Property D — NEGATIVE CONTRACT (FAILING; bug confirmed).
        // Per ARM ARM C5.6.18, BLR's sole operand is <Xn>: a 64-bit GP register.
        // The 32-bit W form is UNALLOCATED and a conforming assembler (GAS,
        // llvm-mc) rejects `blr wN`. encode_blr binds `(rn, _)` and discards
        // is_64, so it currently accepts the W form and encodes it
        // bit-identically to `blr xN`. This property asserts the REQUIRED
        // behavior (Err) and therefore FAILS until validation is added.
        #[test]
        fn prop_rejects_32bit_w_form(
            n in 0u32..=30u32,
        ) {
            let res = encode_blr(&[Operand::Reg(format!("w{}", n))]);
            prop_assert!(
                res.is_err(),
                "blr w{} must be rejected (32-bit form is unallocated per ARM ARM C5.6.18), got {:?}",
                n, res
            );
        }

        // Property E — NEGATIVE CONTRACT (FAILING; bug confirmed).
        // Per ARM ARM C5.6.18, BLR's Rn field value 31 denotes XZR; there is
        // NO SP-using form of BLR. The shared parse_reg_num maps both `sp`
        // and `xzr` to 31, so `blr sp` currently succeeds and silently encodes
        // a branch-with-link to address 0 (== `blr xzr`) instead of erroring;
        // `blr wsp` behaves identically. This property asserts the REQUIRED
        // behavior (Err) and therefore FAILS until validation is added.
        #[test]
        fn prop_rejects_sp_wsp(
            which in 0usize..2usize,
        ) {
            let name = if which == 0 { "sp" } else { "wsp" };
            let res = encode_blr(&[Operand::Reg(name.into())]);
            prop_assert!(
                res.is_err(),
                "blr {} must be rejected (SP/WSP is not a valid BLR operand; field 31 == XZR), got {:?}",
                name, res
            );
        }

        // Property F — NEGATIVE CONTRACT (FAILING; bug confirmed).
        // Per ARM ARM C5.6.18, BLR's operand is a GENERAL-PURPOSE register
        // <Xn>. FP/SIMD operands (d/s/q/v/h/b) are the wrong register class
        // and `blr d0`, `blr v5`, etc. are unallocated encodings a conforming
        // assembler must reject. The shared parse_reg_num accepts those
        // prefixes, so they currently encode as `blr xN`. This property
        // asserts the REQUIRED behavior (Err) and therefore FAILS until
        // validation is added.
        #[test]
        fn prop_rejects_fp_simd_registers(
            prefix_idx in 0usize..6usize,
            n in 0u32..=31u32,
        ) {
            let prefixes = ["d", "s", "q", "v", "h", "b"];
            let prefix = prefixes[prefix_idx];
            let name = format!("{}{}", prefix, n);
            let res = encode_blr(&[Operand::Reg(name.clone())]);
            prop_assert!(
                res.is_err(),
                "blr {} must be rejected (FP/SIMD register; BLR requires a GP register), got {:?}",
                name, res
            );
        }

        // Property G — non-register operand negative contract.
        // Operand kinds that get_reg does NOT accept (and an empty operand
        // vector) must make encode_blr return Err; no silent encoding of a
        // non-register branch target, and no panic on missing operands.
        #[test]
        fn prop_rejects_non_register_operands(
            case in 0usize..12usize,
        ) {
            let rejected: Vec<Operand> = vec![
                Operand::Imm(42),
                Operand::Symbol("tgt".into()),
                Operand::Label("tgt".into()),
                Operand::SymbolOffset("tgt".into(), 8),
                Operand::Mem { base: "x0".into(), offset: 0 },
                Operand::MemExpr {
                    base: "x0".into(), expr: "foo".into(), writeback: false,
                },
                Operand::MemPreIndex { base: "x0".into(), offset: 8 },
                Operand::MemPostIndex { base: "x0".into(), offset: 8 },
                Operand::Shift { kind: "lsl".into(), amount: 2 },
                Operand::Cond("eq".into()),
                Operand::Expr("a + b".into()),
                Operand::RegList(vec![Operand::Reg("x0".into())]),
            ];
            let result = match case {
                0 => encode_blr(&[]),                       // empty vector
                _ => {
                    let op = rejected[case - 1].clone();
                    encode_blr(&[op])
                }
            };
            prop_assert!(result.is_err(),
                "encode_blr should reject case {} (got {:?})", case, result);
        }

        // Property H — determinism / purity. Encoding the same operand twice
        // yields a bit-identical word; encode_blr is a pure function of its
        // operand (no relocation, no hidden state).
        #[test]
        fn prop_encoding_is_deterministic(
            (rn_name, _) in arb_gp_reg(),
        ) {
            let ops = vec![Operand::Reg(rn_name)];
            prop_assert_eq!(enc(&ops), enc(&ops));
            prop_assert_eq!(enc(&ops), enc(&ops));
        }
    }
}

#[cfg(test)]
mod prop_encode_ret_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- RET opcode constants (ARM ARM C5.6.20, "Return"); identical layout
    // to BR/BLR with a different opc sub-field ----
    // RET <Xn> = 1101 0110 0101 1111 0000 00 Rn 00000
    //   [31:10] fixed opcode  == 0xD65F_0000
    //   [9:5]   Rn            (5-bit register field)
    //   [4:0]   00000         (reserved, must be zero)
    // Differential bit-masks derived from the fixed opcodes:
    //   ret ^ br  = 0xD65F0000 ^ 0xD61F0000 = 0x00400000  (bit 22 only)
    //   ret ^ blr = 0xD65F0000 ^ 0xD63F0000 = 0x00600000  (bits 22 AND 21)
    // (BR and BLR differ by a single bit 21; RET is opc=0101, sitting two
    // bits away from BLR's opc=0011 and one bit from BR's opc=0001.)
    const OPCODE: u32 = 0xD65F_0000;
    const RN_MASK: u32 = 0x1Fu32 << 5; // bits [9:5]
    const LOW5: u32 = 0x1Fu32;         // bits [4:0] (reserved zero)
    const RET_VS_BR_XOR: u32 = 1u32 << 22;
    const RET_VS_BLR_XOR: u32 = (1u32 << 22) | (1u32 << 21);

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word_of(encode_ret(ops))
    }

    fn enc_br(ops: &[Operand]) -> u32 {
        match encode_br(ops) {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc_blr(ops: &[Operand]) -> u32 {
        match encode_blr(ops) {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    // General-purpose register: x{n}/w{n} for n in 0..=30 (avoid the
    // xzr/sp-encoded 31 so structural assertions are unambiguous).
    prop_compose! {
        fn arb_gp_reg()(n in 0u32..=30u32, is_64 in any::<bool>()) -> (String, u32) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            (name, n)
        }
    }

    proptest! {
        // Property A — full structural / field-placement oracle.
        // For a valid GP register the encoded word is fully determined: every
        // bit outside [9:5] is fixed to the RET opcode, the low 5 bits are
        // reserved-zero, and Rn occupies [9:5]. Reconstruction must match.
        #[test]
        fn prop_opcode_structure_and_fields(
            (rn_name, rn_num) in arb_gp_reg(),
        ) {
            let ops = vec![Operand::Reg(rn_name.clone())];
            let word = enc(&ops);

            // Every bit outside the Rn field is the fixed opcode.
            prop_assert_eq!(word & !RN_MASK, OPCODE);
            // Low 5 reserved bits are zero.
            prop_assert_eq!(word & LOW5, 0u32);
            // Rn field [9:5] carries the register number.
            prop_assert_eq!((word >> 5) & 0x1F, rn_num);
            // Whole word reconstructs exactly from opcode + shifted Rn.
            prop_assert_eq!(word, OPCODE | (rn_num << 5));
        }

        // Property B — default-register contract (RET-specific).
        // ARM ARM defines RET with no operand as `RET` == `RET X30` (return
        // via the Link Register). So empty operands, an explicit `x30`, and
        // the `lr` alias must all encode to the identical word with Rn=30.
        #[test]
        fn prop_empty_defaults_to_lr(_dummy in 0u32..=0u32) {
            let empty: Vec<Operand> = vec![];
            let from_empty = enc(&empty);
            let from_x30 = enc(&[Operand::Reg("x30".into())]);
            let from_lr = enc(&[Operand::Reg("lr".into())]);
            prop_assert_eq!(from_empty, OPCODE | (30u32 << 5));
            prop_assert_eq!(from_empty, from_x30);
            prop_assert_eq!(from_empty, from_lr);
        }

        // Property C — differential: RET shares BR/BLR's operand contract and
        // differs only in the fixed opcode. ret^br is exactly 0x00400000 and
        // ret^blr is exactly 0x00600000, independent of the register.
        #[test]
        fn prop_differs_from_br_blr_by_fixed_mask(
            (rn_name, _) in arb_gp_reg(),
        ) {
            let ops = vec![Operand::Reg(rn_name)];
            prop_assert_eq!(enc(&ops) ^ enc_br(&ops), RET_VS_BR_XOR);
            prop_assert_eq!(enc(&ops) ^ enc_blr(&ops), RET_VS_BLR_XOR);
        }

        // Property D — register-number range validation (negative contract).
        // The Rn field is a 5-bit UNSIGNED value, valid only 0..=31. A
        // register number above 31 MUST be rejected with Err rather than
        // silently masked via `rn << 5` (which would wrap/truncate). The
        // shared parse_reg_num helper enforces `num <= 31`.
        #[test]
        fn prop_rejects_register_number_above_31(
            n in 32u32..=999u32,
            is_64 in any::<bool>(),
        ) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            let ops = vec![Operand::Reg(name.clone())];
            prop_assert!(
                encode_ret(&ops).is_err(),
                "register {} (valid 0..=31) must be rejected, got {:?}",
                name, encode_ret(&ops)
            );
        }

        // Property E — non-register operand negative contract.
        // Operand kinds that get_reg does NOT accept must make encode_ret
        // return Err. (Note: an EMPTY operand vector is NOT rejected — see
        // Property B; it defaults to X30.)
        #[test]
        fn prop_rejects_non_register_operands(
            case in 0usize..12usize,
        ) {
            let rejected: Vec<Operand> = vec![
                Operand::Imm(42),
                Operand::Symbol("tgt".into()),
                Operand::Label("tgt".into()),
                Operand::SymbolOffset("tgt".into(), 8),
                Operand::Mem { base: "x0".into(), offset: 0 },
                Operand::MemExpr {
                    base: "x0".into(), expr: "foo".into(), writeback: false,
                },
                Operand::MemPreIndex { base: "x0".into(), offset: 8 },
                Operand::MemPostIndex { base: "x0".into(), offset: 8 },
                Operand::Shift { kind: "lsl".into(), amount: 2 },
                Operand::Cond("eq".into()),
                Operand::Expr("a + b".into()),
                Operand::RegList(vec![Operand::Reg("x0".into())]),
            ];
            let op = rejected[case].clone();
            let result = encode_ret(&[op]);
            prop_assert!(result.is_err(),
                "encode_ret should reject non-register operand case {} (got {:?})", case, result);
        }

        // Property F — determinism / purity. Encoding the same operand twice
        // yields a bit-identical word; encode_ret is a pure function of its
        // operands (no relocation, no hidden state).
        #[test]
        fn prop_encoding_is_deterministic(
            (rn_name, _) in arb_gp_reg(),
        ) {
            let ops = vec![Operand::Reg(rn_name)];
            prop_assert_eq!(enc(&ops), enc(&ops));
        }

        // Property G — NEGATIVE CONTRACT (FAILING; bug confirmed).
        // Per ARM ARM C5.6.20, RET's operand is <Xn>: a 64-bit GP register.
        // The 32-bit W form is UNALLOCATED and a conforming assembler (GAS,
        // llvm-mc) rejects `ret wN`. encode_ret binds `(rn, _)` and discards
        // is_64, so it currently accepts the W form and encodes it
        // bit-identically to `ret xN`. This property asserts the REQUIRED
        // behavior (Err) and therefore FAILS until validation is added.
        #[test]
        fn prop_rejects_32bit_w_form(
            n in 0u32..=30u32,
        ) {
            let res = encode_ret(&[Operand::Reg(format!("w{}", n))]);
            prop_assert!(
                res.is_err(),
                "ret w{} must be rejected (32-bit form is unallocated per ARM ARM C5.6.20), got {:?}",
                n, res
            );
        }

        // Property H — NEGATIVE CONTRACT (FAILING; bug confirmed).
        // Per ARM ARM C5.6.20, RET's Rn field value 31 denotes XZR; there is
        // NO SP-using form of RET. The shared parse_reg_num maps both `sp`
        // and `xzr` to 31, so `ret sp` currently succeeds and silently encodes
        // a return to the address in XZR (== `ret xzr`) instead of erroring;
        // `ret wsp` behaves identically. This property asserts the REQUIRED
        // behavior (Err) and therefore FAILS until validation is added.
        #[test]
        fn prop_rejects_sp_wsp(
            which in 0usize..2usize,
        ) {
            let name = if which == 0 { "sp" } else { "wsp" };
            let res = encode_ret(&[Operand::Reg(name.into())]);
            prop_assert!(
                res.is_err(),
                "ret {} must be rejected (SP/WSP is not a valid RET operand; field 31 == XZR), got {:?}",
                name, res
            );
        }

        // Property I — NEGATIVE CONTRACT (FAILING; bug confirmed).
        // Per ARM ARM C5.6.20, RET's operand is a GENERAL-PURPOSE register
        // <Xn>. FP/SIMD operands (d/s/q/v/h/b) are the wrong register class
        // and `ret d0`, `ret v5`, etc. are unallocated encodings a conforming
        // assembler must reject. The shared parse_reg_num accepts those
        // prefixes, so they currently encode as `ret xN`. This property
        // asserts the REQUIRED behavior (Err) and therefore FAILS until
        // validation is added.
        #[test]
        fn prop_rejects_fp_simd_registers(
            prefix_idx in 0usize..6usize,
            n in 0u32..=31u32,
        ) {
            let prefixes = ["d", "s", "q", "v", "h", "b"];
            let prefix = prefixes[prefix_idx];
            let name = format!("{}{}", prefix, n);
            let res = encode_ret(&[Operand::Reg(name.clone())]);
            prop_assert!(
                res.is_err(),
                "ret {} must be rejected (FP/SIMD register; RET requires a GP register), got {:?}",
                name, res
            );
        }
    }
}

#[cfg(test)]
mod prop_encode_cmp_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- CMP Rn, op  ==  SUBS ZR, Rn, op  (ARM ARM C4.1.63, flags-setting subtract) ----
    // The function prepends the zero register (XZR or WZR) as Rd and delegates to
    // encode_add_sub with is_sub=true, set_flags=true. The defining invariants:
    //   op = 1 (subtract), S = 1 (set flags), Rd = 31 (the zero register).
    //
    // Immediate form:  sf 1 1 10001 sh imm12  Rn  11111
    //   [31] sf  [30] op=1  [29] S=1  [28:24]=10001  [23:22]=sh
    //   [21:10]=imm12  [9:5]=Rn  [4:0]=Rd
    // Register form:   sf 1 1 01011 00 Rm 000000 Rn 11111
    //   [28:24]=01011  [23:22]=shift  [21:16]=Rm  [15:10]=imm6  [9:5]=Rn  [4:0]=Rd

    const IMM_OPCODE: u32 = 0b10001u32 << 24; // add/sub immediate [28:24]
    const REG_OPCODE: u32 = 0b01011u32 << 24; // add/sub shifted register [28:24]
    const SF_BIT: u32 = 1u32 << 31;
    const OP_BIT: u32 = 1u32 << 30; // subtract
    const XZR_NUM: u32 = 31;

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word_of(encode_cmp(ops))
    }

    prop_compose! {
        fn arb_gp_reg()(n in 0u32..=30u32, is_64 in any::<bool>()) -> (String, u32, u32) {
            // (name, reg_number, sf_bit)
            if is_64 {
                (format!("x{}", n), n, 1)
            } else {
                (format!("w{}", n), n, 0)
            }
        }
    }

    proptest! {
        // Property A — structural oracle for the immediate form.
        // CMP Rn, #imm  ==  SUBS ZR, Rn, #imm with an unshifted 12-bit immediate.
        // Every field is pinned, including Rd = 31 (the zero register).
        #[test]
        fn prop_cmp_imm_structure(
            (rn_name, rn_num, sf) in arb_gp_reg(),
            imm in 0i64..=0xFFF, // valid unshifted add/sub immediate
        ) {
            let ops = vec![Operand::Reg(rn_name), Operand::Imm(imm)];
            let word = enc(&ops);

            let expected = (sf << 31) | OP_BIT | (1u32 << 29)
                | IMM_OPCODE | ((imm as u32) << 10) | (rn_num << 5) | XZR_NUM;
            prop_assert_eq!(word, expected);
            prop_assert_eq!((word >> 31) & 1, sf);             // sf tracks Rn width
            prop_assert_eq!((word >> 30) & 1, 1);              // op = subtract
            prop_assert_eq!((word >> 29) & 1, 1);              // S = set flags
            prop_assert_eq!((word >> 24) & 0x1F, 0b10001);     // immediate opcode
            prop_assert_eq!((word >> 22) & 0x3, 0);            // sh = 0 (unshifted)
            prop_assert_eq!((word >> 10) & 0xFFF, imm as u32); // imm12 round-trips
            prop_assert_eq!((word >> 5) & 0x1F, rn_num);       // Rn
            prop_assert_eq!(word & 0x1F, XZR_NUM);             // Rd = 31 (ZR)
        }

        // Property B — structural oracle for the register form.
        // CMP Rn, Rm  ==  SUBS ZR, Rn, Rm with no shift.
        #[test]
        fn prop_cmp_reg_structure(
            (rn_name, rn_num, sf) in arb_gp_reg(),
            (rm_name, rm_num, _) in arb_gp_reg(),
        ) {
            let ops = vec![Operand::Reg(rn_name), Operand::Reg(rm_name)];
            let word = enc(&ops);

            let expected = (sf << 31) | OP_BIT | (1u32 << 29)
                | REG_OPCODE | (rm_num << 16) | (rn_num << 5) | XZR_NUM;
            prop_assert_eq!(word, expected);
            prop_assert_eq!((word >> 31) & 1, sf);
            prop_assert_eq!((word >> 30) & 1, 1);              // op = subtract
            prop_assert_eq!((word >> 29) & 1, 1);              // S = set flags
            prop_assert_eq!((word >> 24) & 0x1F, 0b01011);     // shifted-register opcode
            prop_assert_eq!((word >> 22) & 0x3, 0);            // shift type = LSL
            prop_assert_eq!((word >> 16) & 0x1F, rm_num);      // Rm
            prop_assert_eq!((word >> 10) & 0x3F, 0);           // imm6 = 0
            prop_assert_eq!((word >> 5) & 0x1F, rn_num);       // Rn
            prop_assert_eq!(word & 0x1F, XZR_NUM);             // Rd = 31 (ZR)
        }

        // Property C — width differential. CMP Xn, op vs CMP Wn, op must differ
        // ONLY in bit 31 (sf), proving is_32bit_reg(operands[0]) drives the
        // destination width (XZR vs WZR) and touches nothing else.
        #[test]
        fn prop_width_differs_only_bit31(
            n in 0u32..=30u32,
            rm_num in 0u32..=30u32,
            imm in 0i64..=0xFFF,
            use_imm in any::<bool>(),
        ) {
            let ops64: Vec<Operand> = if use_imm {
                vec![Operand::Reg(format!("x{}", n)), Operand::Imm(imm)]
            } else {
                vec![Operand::Reg(format!("x{}", n)), Operand::Reg(format!("x{}", rm_num))]
            };
            let ops32: Vec<Operand> = if use_imm {
                vec![Operand::Reg(format!("w{}", n)), Operand::Imm(imm)]
            } else {
                vec![Operand::Reg(format!("w{}", n)), Operand::Reg(format!("x{}", rm_num))]
            };
            prop_assert_eq!(enc(&ops64) ^ enc(&ops32), SF_BIT);
        }

        // Property D — differential vs the sibling CMN path. encode_cmn is
        // "ADDS ZR, Rn, op" (op=0); encode_cmp is "SUBS ZR, Rn, op" (op=1).
        // Identical operands must produce words differing ONLY in bit 30.
        #[test]
        fn prop_cmp_xor_cmn_is_bit30(
            (rn_name, _, _) in arb_gp_reg(),
            (rm_name, _, _) in arb_gp_reg(),
            imm in 0i64..=0xFFF,
            use_imm in any::<bool>(),
        ) {
            let ops: Vec<Operand> = if use_imm {
                vec![Operand::Reg(rn_name), Operand::Imm(imm)]
            } else {
                vec![Operand::Reg(rn_name), Operand::Reg(rm_name)]
            };
            prop_assert_eq!(
                word_of(encode_cmp(&ops)) ^ word_of(encode_cmn(&ops)),
                OP_BIT
            );
        }

        // Property E — negative contract (plain immediate range validation).
        // Per ARM ARM (add/sub immediate): imm12 encodes ONLY 0..=4095 unshifted
        // or a multiple of 0x1000 (up to 0xFFF000) shifted. A value that is BOTH
        // > 0xFFF and not a multiple of 0x1000 cannot be encoded and MUST be
        // rejected with Err — not silently truncated via `& 0xFFF`. No spec
        // permits wrapping for this field.
        #[test]
        fn prop_rejects_unencodable_immediate(
            (rn_name, _, _) in arb_gp_reg(),
            // 0x1001..=0x1FFF: always > 0xFFF, low 12 bits always non-zero.
            bad_imm in 0x1001i64..=0x1FFFi64,
        ) {
            let ops = vec![Operand::Reg(rn_name), Operand::Imm(bad_imm)];
            let res = encode_cmp(&ops);
            prop_assert!(
                res.is_err(),
                "immediate {} (valid: 0..=4095 unshifted, or 0x1000-multiple shifted) \
                 must be rejected, got {:?}",
                bad_imm, res
            );
        }

        // Property F — negative contract (explicit `lsl #12` shift form).
        // `CMP Rn, #imm, lsl #12` selects the shifted-12-bit immediate variant;
        // imm12 is still constrained to 0..=4095. An immediate above 0xFFF in
        // this form is architecturally invalid and MUST be rejected. The
        // implementation instead does `imm & 0xFFF`, silently truncating the
        // value — e.g. #0x12345 encodes as if it were #0x345. This property is
        // expected to FAIL and documents that silent-truncation bug.
        #[test]
        fn prop_rejects_large_imm_with_explicit_shift(
            (rn_name, _, _) in arb_gp_reg(),
            // > 0xFFF with distinct high bits so truncation changes the value.
            bad_imm in 0x1001i64..=0x1FFFi64,
        ) {
            let ops = vec![
                Operand::Reg(rn_name),
                Operand::Imm(bad_imm),
                Operand::Shift { kind: "lsl".to_string(), amount: 12 },
            ];
            let res = encode_cmp(&ops);
            prop_assert!(
                res.is_err(),
                "`lsl #12` immediate {} (> 0xFFF) must be rejected, not silently \
                 truncated to 0x{:X}; got {:?}",
                bad_imm, (bad_imm as u32) & 0xFFF, res
            );
        }
    }
}

#[cfg(test)]
mod prop_encode_tst_tests {
    use super::*;
    use proptest::prelude::*;

    // ---- TST semantics (ARM ARM C6.2.x — "ANDS (immediate / shifted register)") ----
    // TST Rn, op -> ANDS XZR/WZR, Rn, op, i.e. encode_logical(.., opc = 0b11).
    //   opc field [30:29] = 0b11   (ANDS = AND-with-flags)
    //   S  bit  [29]      = 1      (sets condition flags — the whole point of TST)
    //   sf bit  [31]      tracks the width of Rn (via the prepended xzr/wzr)
    //   Rd field [4:0] = 31       (xzr / wzr: result discarded, only flags matter)
    //
    // Two operand shapes forwarded to encode_logical:
    //   * shifted register:  sf opc 01010 shift 0 Rm imm6 Rn Rd   (bits [28:24]=01010)
    //   * bitmask immediate: sf opc 100100 N immr imms Rn Rd      (bits [28:23]=100100)
    const OPC_BITS: u32 = 0b11u32 << 29;       // [30:29] = 0b11 (ANDS)
    const OPC_MASK: u32 = 0x6000_0000;         // [30:29]
    const SF_BIT: u32 = 1u32 << 31;            // [31]
    const S_BIT: u32 = 1u32 << 29;             // [29]
    const RD_MASK: u32 = 0x1F;                 // [4:0]
    const REG_OPCODE: u32 = 0b01010u32 << 24;  // == 0x0A00_0000, shifted-reg opcode [28:24]
    const REG_OPCODE_MASK: u32 = 0x1F00_0000;  // [28:24]
    const IMM_OPCODE: u32 = 0b100100u32 << 23; // == 0x1200_0000, immediate opcode [28:23]
    const IMM_OPCODE_MASK: u32 = 0x1F80_0000;  // [28:23]
    const BIT21: u32 = 1u32 << 21;             // must be 0 in shifted-register form

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn enc(ops: &[Operand]) -> u32 {
        word_of(encode_tst(ops))
    }

    prop_compose! {
        fn arb_reg()(n in 0u32..=30u32, is_64 in any::<bool>()) -> (String, u32) {
            let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
            (name, n)
        }
    }

    /// Curated (element_size, ones_width, rotation) triples. Each builds a
    /// repeating contiguous-1s pattern — the exact class `encode_bitmask_imm`
    /// accepts — so every value is a guaranteed-valid bitmask immediate.
    const BITMASK_PATTERNS: &[(u32, u32, u32)] = &[
        (2, 1, 0), (2, 1, 1),
        (4, 1, 0), (4, 2, 0), (4, 3, 1),
        (8, 1, 0), (8, 4, 0), (8, 7, 2),
        (16, 1, 0), (16, 8, 0),
        (32, 1, 0), (32, 16, 0),
        (64, 1, 0), (64, 32, 0),
    ];

    /// Inverse of `encode_bitmask_imm`: build a bitmask immediate for `width`
    /// from a repeating element that is a contiguous run of `w` ones, right-
    /// rotated by `r` within an `s`-bit element. w in 1..s guarantees the
    /// element is neither all-zeros nor all-ones, so encoding always succeeds.
    fn make_valid_bitmask(is_64: bool, s: u32, w: u32, r: u32) -> u64 {
        let width = if is_64 { 64 } else { 32 };
        let elem_mask: u64 = if s >= 64 { u64::MAX } else { (1u64 << s) - 1 };
        let base: u64 = ((1u64 << w) - 1) & elem_mask;
        let rot: u64 = if r == 0 {
            base
        } else {
            ((base >> r) | (base << (s - r))) & elem_mask
        };
        let mut val: u64 = rot;
        let mut pos = s;
        while pos < width {
            val |= rot << pos;
            pos += s;
        }
        val
    }

    proptest! {
        // Property A — shifted-register structural / field-placement oracle.
        // TST Rn, Rm -> ANDS XZR/WZR, Rn, Rm. Verifies every populated field:
        // sf tracks Rn width, opc=0b11 (ANDS), S bit set, opcode [28:24]=01010,
        // no shift, bit 21 = 0, Rm/imm6/Rn correct, and Rd == 31 (zr).
        #[test]
        fn prop_register_form_structure(
            (rn_name, rn_num) in arb_reg(),
            (rm_name, rm_num) in arb_reg(),
        ) {
            let ops = vec![Operand::Reg(rn_name.clone()), Operand::Reg(rm_name.clone())];
            let word = enc(&ops);

            let expected_sf = u32::from(rn_name.starts_with('x'));
            prop_assert_eq!(word & SF_BIT, expected_sf << 31);
            prop_assert_eq!(word & OPC_MASK, OPC_BITS);
            prop_assert_eq!(word & S_BIT, S_BIT);
            prop_assert_eq!(word & REG_OPCODE_MASK, REG_OPCODE);
            prop_assert_eq!((word >> 22) & 0x3, 0u32); // shift type = LSL (none)
            prop_assert_eq!(word & BIT21, 0u32);
            prop_assert_eq!((word >> 16) & 0x1F, rm_num);
            prop_assert_eq!((word >> 10) & 0x3F, 0u32); // imm6 = 0
            prop_assert_eq!((word >> 5) & 0x1F, rn_num);
            prop_assert_eq!(word & RD_MASK, 31u32);
        }

        // Property B — bitmask-immediate structural / field-placement oracle.
        // TST Rn, #imm -> ANDS XZR/WZR, Rn, #imm. Verifies sf tracks Rn width,
        // opc=0b11, S bit set, immediate opcode [28:23]=100100, Rn field, Rd=31,
        // and (for 32-bit) the N bit [22] is RES0 == 0 per ARM ARM.
        #[test]
        fn prop_immediate_form_structure(
            rn_num in 0u32..=30u32,
            is_64 in any::<bool>(),
            idx in 0usize..BITMASK_PATTERNS.len(),
        ) {
            let (s, w, r) = BITMASK_PATTERNS[idx];
            // A 32-bit register cannot use a 64-bit element pattern.
            prop_assume!(is_64 || s <= 32);
            let rn_name = if is_64 { format!("x{}", rn_num) } else { format!("w{}", rn_num) };
            let val = make_valid_bitmask(is_64, s, w, r);
            let ops = vec![Operand::Reg(rn_name.clone()), Operand::Imm(val as i64)];
            let word = enc(&ops);

            let expected_sf = u32::from(rn_name.starts_with('x'));
            prop_assert_eq!(word & SF_BIT, expected_sf << 31);
            prop_assert_eq!(word & OPC_MASK, OPC_BITS);
            prop_assert_eq!(word & S_BIT, S_BIT);
            prop_assert_eq!(word & IMM_OPCODE_MASK, IMM_OPCODE);
            if !rn_name.starts_with('x') {
                // N is RES0 when sf == 0.
                prop_assert_eq!((word >> 22) & 1, 0u32);
            }
            prop_assert_eq!((word >> 5) & 0x1F, rn_num);
            prop_assert_eq!(word & RD_MASK, 31u32);
        }

        // Property C — differential: 64- vs 32-bit Rn differ ONLY in bit 31 (sf).
        #[test]
        fn prop_sf_bit_is_bit31(
            rn_num in 0u32..=30u32,
            rm_num in 0u32..=30u32,
        ) {
            let ops64 = vec![Operand::Reg(format!("x{}", rn_num)),
                             Operand::Reg(format!("x{}", rm_num))];
            let ops32 = vec![Operand::Reg(format!("w{}", rn_num)),
                             Operand::Reg(format!("x{}", rm_num))];
            prop_assert_eq!(enc(&ops64) ^ enc(&ops32), SF_BIT);
        }

        // Property D — differential: TST is ANDS. (1) encode_tst must produce
        // exactly what encode_logical(zr-prefixed, opc=0b11) produces, and
        // (2) ANDS vs AND (opc=0b00) differ ONLY in the opc field [30:29].
        #[test]
        fn prop_tst_is_ands_and_differs_only_in_opc(
            (rn_name, _) in arb_reg(),
            (rm_name, _) in arb_reg(),
        ) {
            let is_32 = rn_name.starts_with('w');
            let zr = if is_32 { "wzr".to_string() } else { "xzr".to_string() };
            let fwd = vec![Operand::Reg(zr),
                           Operand::Reg(rn_name.clone()),
                           Operand::Reg(rm_name.clone())];
            let tst_word = enc(&[Operand::Reg(rn_name.clone()), Operand::Reg(rm_name.clone())]);
            let ands_via_logical = word_of(encode_logical(&fwd, 0b11));
            let and_via_logical = word_of(encode_logical(&fwd, 0b00));
            // encode_tst forwards faithfully to encode_logical(.., opc=0b11).
            prop_assert_eq!(tst_word, ands_via_logical);
            // opc is the ONLY field that distinguishes ANDS from AND.
            prop_assert_eq!(ands_via_logical ^ and_via_logical, OPC_MASK);
        }

        // Property E — negative contract (input validation).
        // * `#0` is never a valid bitmask immediate (encode_bitmask_imm rejects it).
        // * An out-of-range register (x32) is rejected by get_reg.
        // * Fewer than 2 user operands cannot form a logical op (needs 3 after
        //   prepending zr).
        #[test]
        fn prop_rejects_invalid_inputs(_seed in 0u32..=0u32) {
            let zero_imm = vec![Operand::Reg("x0".into()), Operand::Imm(0)];
            prop_assert!(encode_tst(&zero_imm).is_err(),
                "tst x0, #0 (#0 is not a valid bitmask immediate) must be rejected");

            let bad_reg = vec![Operand::Reg("x32".into()), Operand::Reg("x1".into())];
            prop_assert!(encode_tst(&bad_reg).is_err(),
                "tst x32, x1 (x32 out of range) must be rejected");

            let too_few = vec![Operand::Reg("x0".into())];
            prop_assert!(encode_tst(&too_few).is_err(),
                "tst x0 (too few operands) must be rejected");
        }

        // Property F — negative contract (shift-amount range validation).
        // Per ARM ARM ("Logical (shifted register)"), imm6 is the shift amount
        // and is valid 0..=31 for 32-bit (sf=0) and 0..=63 for 64-bit (sf=1);
        // imm6 above those bounds is RESERVED. The implementation masks the
        // amount with `& 0x3F` in encode_logical, so out-of-range amounts are
        // SILENTLY accepted/truncated instead of rejected. GAS / llvm-mc reject
        // these. No cited spec permits wrapping for the shift-amount field.
        #[test]
        fn prop_rejects_oversized_shift(
            is_64 in any::<bool>(),
            excess in 1u32..=64u32,
        ) {
            let max = if is_64 { 63u32 } else { 31u32 };
            let amount = max + excess; // strictly above the legal maximum
            let rn = if is_64 { "x5".to_string() } else { "w5".to_string() };
            let rm = if is_64 { "x6".to_string() } else { "w6".to_string() };
            let ops = vec![
                Operand::Reg(rn),
                Operand::Reg(rm),
                Operand::Shift { kind: "lsl".to_string(), amount },
            ];
            let res = encode_tst(&ops);
            prop_assert!(
                res.is_err(),
                "shift amount {} exceeds legal max {} for {}-bit and MUST be \
                 rejected (encoder silently masks with & 0x3F); got {:?}",
                amount, max, if is_64 { 64 } else { 32 }, res
            );
        }
    }
}
