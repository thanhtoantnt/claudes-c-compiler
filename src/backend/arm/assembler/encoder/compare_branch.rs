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
