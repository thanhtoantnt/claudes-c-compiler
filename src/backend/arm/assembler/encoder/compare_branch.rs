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
    }
}
