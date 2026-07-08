use super::*;
use crate::backend::arm::assembler::parser::Operand;

// ── MOV ──────────────────────────────────────────────────────────────────

pub(crate) fn encode_mov(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("mov requires 2 operands".to_string());
    }

    // NEON register-to-register move: mov v1.16b, v0.16b -> ORR v1.16b, v0.16b, v0.16b
    if let (Some(Operand::RegArrangement { reg: rd_name, arrangement: arr_d }),
            Some(Operand::RegArrangement { reg: rm_name, arrangement: _arr_m })) =
        (operands.first(), operands.get(1))
    {
        let rd = parse_reg_num(rd_name).ok_or("invalid NEON rd")?;
        let rm = parse_reg_num(rm_name).ok_or("invalid NEON rm")?;
        let q: u32 = if arr_d == "16b" { 1 } else { 0 };
        // ORR Vd.T, Vm.T, Vm.T: 0 Q 0 01110 10 1 Rm 0 00111 Rn Rd
        let word = (q << 30) | (0b001110 << 24) | (0b10 << 22) | (1 << 21)
            | (rm << 16) | (0b000111 << 10) | (rm << 5) | rd;
        return Ok(EncodeResult::Word(word));
    }

    // NEON lane insert: mov v0.d[1], x1 -> INS Vd.D[index], Xn
    if let (Some(Operand::RegLane { reg: vd_name, elem_size, index }),
            Some(Operand::Reg(rn_name))) =
        (operands.first(), operands.get(1))
    {
        let vd = parse_reg_num(vd_name).ok_or("invalid NEON vd")?;
        let rn = parse_reg_num(rn_name).ok_or("invalid rn")?;
        // INS Vd.Ts[index], Rn
        // Encoding: 0 1 0 0 1110 000 imm5 0 0011 1 Rn Rd
        // imm5 encoding depends on element size and index
        let imm5 = match elem_size.as_str() {
            "b" => ((*index & 0xF) << 1) | 0b00001,
            "h" => ((*index & 0x7) << 2) | 0b00010,
            "s" => ((*index & 0x3) << 3) | 0b00100,
            "d" => ((*index & 0x1) << 4) | 0b01000,
            _ => return Err(format!("unsupported element size for ins: {}", elem_size)),
        };
        let word = (0b01001110000u32 << 21) | (imm5 << 16) | (0b000111 << 10) | (rn << 5) | vd;
        return Ok(EncodeResult::Word(word));
    }

    // NEON lane extract: mov x0, v0.d[1] -> UMOV Xd, Vn.D[index]
    if let (Some(Operand::Reg(rd_name)),
            Some(Operand::RegLane { reg: vn_name, elem_size, index })) =
        (operands.first(), operands.get(1))
    {
        let rd = parse_reg_num(rd_name).ok_or("invalid rd")?;
        let vn = parse_reg_num(vn_name).ok_or("invalid NEON vn")?;
        // UMOV Rd, Vn.Ts[index]
        // Encoding: 0 Q 0 0 1110 000 imm5 0 0111 1 Rn Rd
        let (q, imm5) = match elem_size.as_str() {
            "b" => (0u32, ((*index & 0xF) << 1) | 0b00001),
            "h" => (0, ((*index & 0x7) << 2) | 0b00010),
            "s" => (0, ((*index & 0x3) << 3) | 0b00100),
            "d" => (1, ((*index & 0x1) << 4) | 0b01000),
            _ => return Err(format!("unsupported element size for umov: {}", elem_size)),
        };
        let word = (q << 30) | (0b001110000u32 << 21) | (imm5 << 16) | (0b001111 << 10) | (vn << 5) | rd;
        return Ok(EncodeResult::Word(word));
    }

    // NEON element-to-element move: mov v0.s[3], v1.s[0] -> INS Vd.Ts[i1], Vn.Ts[i2]
    if let (Some(Operand::RegLane { reg: vd_name, elem_size: es_d, index: idx_d }),
            Some(Operand::RegLane { reg: vn_name, elem_size: _es_n, index: idx_n })) =
        (operands.first(), operands.get(1))
    {
        let vd = parse_reg_num(vd_name).ok_or("invalid NEON vd")?;
        let vn = parse_reg_num(vn_name).ok_or("invalid NEON vn")?;
        // INS Vd.Ts[i1], Vn.Ts[i2]
        // Encoding: 0 1 1 01110 000 imm5 0 imm4 1 Rn Rd
        let (imm5, imm4) = match es_d.as_str() {
            "b" => ((idx_d << 1) | 0b00001, *idx_n),
            "h" => ((idx_d << 2) | 0b00010, idx_n << 1),
            "s" => ((idx_d << 3) | 0b00100, idx_n << 2),
            "d" => ((idx_d << 4) | 0b01000, idx_n << 3),
            _ => return Err(format!("unsupported element size for ins: {}", es_d)),
        };
        let word = ((0b01101110000u32 << 21) | (imm5 << 16)) | (imm4 << 11) | (1 << 10) | (vn << 5) | vd;
        return Ok(EncodeResult::Word(word));
    }

    // mov Xd, #imm -> movz or movn
    if let Some(Operand::Imm(imm)) = operands.get(1) {
        let (rd, is_64) = get_reg(operands, 0)?;
        let imm = *imm;

        // Check if it can be a simple MOVZ
        if (0..=0xFFFF).contains(&imm) {
            let sf = sf_bit(is_64);
            let word = (sf << 31) | (0b10100101 << 23) | ((imm as u32 & 0xFFFF) << 5) | rd;
            return Ok(EncodeResult::Word(word));
        }

        // Negative: try MOVN
        if imm < 0 {
            let not_imm = !imm;
            if (0..=0xFFFF).contains(&not_imm) {
                let sf = sf_bit(is_64);
                let word = (sf << 31) | (0b00100101 << 23) | ((not_imm as u32 & 0xFFFF) << 5) | rd;
                return Ok(EncodeResult::Word(word));
            }
        }

        // Try encoding as ORR Rd, XZR, #imm (logical/bitmask immediate)
        // This handles patterns like 0x0101010101010101 in a single instruction
        if let Some((n, immr, imms)) = encode_bitmask_imm(imm as u64, is_64) {
            let sf = sf_bit(is_64);
            // ORR Rd, XZR, #imm: sf 01 100100 N immr imms 11111 Rd
            let word = (sf << 31) | (0b01 << 29) | (0b100100 << 23) | (n << 22) | (immr << 16) | (imms << 10) | (0b11111 << 5) | rd;
            return Ok(EncodeResult::Word(word));
        }

        // Need movz + movk sequence for large immediates
        return encode_mov_wide_imm(rd, is_64, imm as u64);
    }

    // mov Xd, Xm -> ORR Xd, XZR, Xm
    if let (Some(Operand::Reg(rd_name)), Some(Operand::Reg(rm_name))) = (operands.first(), operands.get(1)) {
        let rd = parse_reg_num(rd_name).ok_or("invalid rd")?;
        let rm = parse_reg_num(rm_name).ok_or("invalid rm")?;
        let is_64 = is_64bit_reg(rd_name);

        // Check for MOV to/from SP: uses ADD Xd, Xn, #0
        if rd_name.to_lowercase() == "sp" || rm_name.to_lowercase() == "sp" {
            let sf = sf_bit(is_64);
            // ADD Xd, Xn, #0: sf 0 0 10001 00 imm12=0 Rn Rd
            let word = ((sf << 31) | (0b10001 << 24)) | (rm << 5) | rd;
            return Ok(EncodeResult::Word(word));
        }

        let sf = sf_bit(is_64);
        // ORR Rd, XZR, Rm: sf 01 01010 00 0 Rm 000000 11111 Rd
        let word = ((sf << 31) | (0b01 << 29) | (0b01010 << 24)) | (rm << 16) | (0b11111 << 5) | rd;
        return Ok(EncodeResult::Word(word));
    }

    Err(format!("unsupported mov operands: {:?}", operands))
}

pub(crate) fn encode_mov_wide_imm(rd: u32, is_64: bool, imm: u64) -> Result<EncodeResult, String> {
    let sf = sf_bit(is_64);
    let mut words = Vec::new();
    let max_hw = if is_64 { 4 } else { 2 };
    let mut first = true;

    for hw in 0..max_hw {
        let chunk = ((imm >> (hw * 16)) & 0xFFFF) as u32;
        if chunk != 0 || (hw == 0 && imm == 0) {
            if first {
                // MOVZ
                let word = (sf << 31) | (0b10100101 << 23) | (hw << 21) | (chunk << 5) | rd;
                words.push(word);
                first = false;
            } else {
                // MOVK
                let word = (sf << 31) | (0b11100101 << 23) | (hw << 21) | (chunk << 5) | rd;
                words.push(word);
            }
        }
    }

    if words.is_empty() {
        // imm is 0
        let word = (sf << 31) | (0b10100101 << 23) | rd;
        words.push(word);
    }

    if words.len() == 1 {
        Ok(EncodeResult::Word(words[0]))
    } else {
        Ok(EncodeResult::Words(words))
    }
}

/// Resolve `:abs_g0:`, `:abs_g1:`, etc. modifiers for movz/movk.
/// If the expression is a pure constant, returns Some((imm16, hw)) where
/// imm16 is the relevant 16-bit chunk and hw is the halfword selector.
/// If the expression contains a symbol reference, returns None (needs relocation).
pub(crate) fn resolve_abs_g_modifier(kind: &str, symbol: &str) -> Result<Option<(u32, u32)>, String> {
    let shift = match kind {
        "abs_g0" | "abs_g0_nc" | "abs_g0_s" => 0,
        "abs_g1" | "abs_g1_nc" | "abs_g1_s" => 16,
        "abs_g2" | "abs_g2_nc" | "abs_g2_s" => 32,
        "abs_g3" => 48,
        _ => return Ok(None), // Not an abs_g modifier
    };
    let hw = shift / 16;
    // Try to evaluate the expression as a constant
    if let Ok(val) = crate::backend::asm_expr::parse_integer_expr(symbol) {
        let imm16 = ((val as u64) >> shift) as u32 & 0xFFFF;
        Ok(Some((imm16, hw)))
    } else {
        Ok(None) // Contains symbol reference - needs relocation
    }
}

pub(crate) fn encode_movz(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let sf = sf_bit(is_64);

    // Handle :abs_g*: modifiers
    if let Some(Operand::Modifier { kind, symbol }) = operands.get(1) {
        if let Some((imm16, hw)) = resolve_abs_g_modifier(kind, symbol)? {
            let word = (sf << 31) | (0b10100101 << 23) | (hw << 21) | ((imm16 & 0xFFFF) << 5) | rd;
            return Ok(EncodeResult::Word(word));
        }
    }

    let imm = get_imm(operands, 1)?;

    // Check for lsl #N shift
    let hw = if operands.len() > 2 {
        if let Some(Operand::Shift { kind, amount }) = operands.get(2) {
            if kind == "lsl" {
                *amount / 16
            } else {
                0
            }
        } else {
            0
        }
    } else {
        0
    };

    let word = (sf << 31) | (0b10100101 << 23) | (hw << 21) | (((imm as u32) & 0xFFFF) << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_movk(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let sf = sf_bit(is_64);

    // Handle :abs_g*: modifiers
    if let Some(Operand::Modifier { kind, symbol }) = operands.get(1) {
        if let Some((imm16, hw)) = resolve_abs_g_modifier(kind, symbol)? {
            let word = (sf << 31) | (0b11100101 << 23) | (hw << 21) | ((imm16 & 0xFFFF) << 5) | rd;
            return Ok(EncodeResult::Word(word));
        }
    }

    let imm = get_imm(operands, 1)?;

    let hw = if operands.len() > 2 {
        if let Some(Operand::Shift { kind, amount }) = operands.get(2) {
            if kind == "lsl" {
                *amount / 16
            } else {
                0
            }
        } else {
            0
        }
    } else {
        0
    };

    let word = (sf << 31) | (0b11100101 << 23) | (hw << 21) | (((imm as u32) & 0xFFFF) << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_movn(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let imm = get_imm(operands, 1)?;
    let sf = sf_bit(is_64);

    let hw = if operands.len() > 2 {
        if let Some(Operand::Shift { kind, amount }) = operands.get(2) {
            if kind == "lsl" {
                *amount / 16
            } else {
                0
            }
        } else {
            0
        }
    } else {
        0
    };

    let word = (sf << 31) | (0b00100101 << 23) | (hw << 21) | (((imm as u32) & 0xFFFF) << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── ADD/SUB ──────────────────────────────────────────────────────────────

pub(crate) fn encode_add_sub(operands: &[Operand], is_sub: bool, set_flags: bool) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err(format!("add/sub requires 3 operands, got {}", operands.len()));
    }

    // NEON vector form: ADD/SUB Vd.T, Vn.T, Vm.T
    if let Some(Operand::RegArrangement { .. }) = operands.first() {
        if !set_flags {
            return encode_neon_add_sub(operands, is_sub);
        }
    }

    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let sf = sf_bit(is_64);
    let op = if is_sub { 1u32 } else { 0u32 };
    let s_bit = if set_flags { 1u32 } else { 0u32 };

    // ADD Rd, Rn, #imm
    if let Some(Operand::Imm(imm)) = operands.get(2) {
        let imm_signed = *imm;
        // Handle negative immediates: add #-N -> sub #N and vice versa
        let (imm_val, actual_op) = if imm_signed < 0 {
            ((-imm_signed) as u64, if is_sub { 0u32 } else { 1u32 })
        } else {
            (imm_signed as u64, op)
        };
        // Check for explicit lsl #12 shift
        let explicit_shift = if operands.len() > 3 {
            if let Some(Operand::Shift { kind, amount }) = operands.get(3) {
                kind == "lsl" && *amount == 12
            } else { false }
        } else { false };

        let (imm12, sh) = if explicit_shift {
            // Explicit lsl #12: use the immediate as-is (must fit in 12 bits)
            ((imm_val as u32) & 0xFFF, 1u32)
        } else if imm_val <= 0xFFF {
            // Fits in 12 bits unshifted
            (imm_val as u32, 0u32)
        } else if (imm_val & 0xFFF) == 0 && (imm_val >> 12) <= 0xFFF {
            // Low 12 bits are zero and shifted value fits: auto-shift
            // e.g., #4096 -> #1, lsl #12
            ((imm_val >> 12) as u32, 1u32)
        } else {
            return Err(format!("immediate {} does not fit in add/sub imm12 encoding", imm_val));
        };

        let word = (sf << 31) | (actual_op << 30) | (s_bit << 29) | (0b10001 << 24) | (sh << 22) | (imm12 << 10) | (rn << 5) | rd;
        return Ok(EncodeResult::Word(word));
    }

    // ADD Rd, Rn, :lo12:symbol
    if let Some(Operand::Modifier { kind, symbol }) = operands.get(2) {
        if kind == "lo12" {
            let word = ((sf << 31) | (op << 30) | (s_bit << 29) | (0b10001 << 24)) | (rn << 5) | rd;
            return Ok(EncodeResult::WordWithReloc {
                word,
                reloc: Relocation {
                    reloc_type: RelocType::AddAbsLo12,
                    symbol: symbol.clone(),
                    addend: 0,
                },
            });
        }
        if kind == "tprel_lo12_nc" {
            let word = ((sf << 31) | (op << 30) | (s_bit << 29) | (0b10001 << 24)) | (rn << 5) | rd;
            return Ok(EncodeResult::WordWithReloc {
                word,
                reloc: Relocation {
                    reloc_type: RelocType::TlsLeAddTprelLo12,
                    symbol: symbol.clone(),
                    addend: 0,
                },
            });
        }
        if kind == "tprel_hi12" {
            let word = ((sf << 31) | (op << 30) | (s_bit << 29) | (0b10001 << 24) | (1 << 22)) | (rn << 5) | rd;
            return Ok(EncodeResult::WordWithReloc {
                word,
                reloc: Relocation {
                    reloc_type: RelocType::TlsLeAddTprelHi12,
                    symbol: symbol.clone(),
                    addend: 0,
                },
            });
        }
    }
    if let Some(Operand::ModifierOffset { kind, symbol, offset }) = operands.get(2) {
        if kind == "lo12" {
            let word = ((sf << 31) | (op << 30) | (s_bit << 29) | (0b10001 << 24)) | (rn << 5) | rd;
            return Ok(EncodeResult::WordWithReloc {
                word,
                reloc: Relocation {
                    reloc_type: RelocType::AddAbsLo12,
                    symbol: symbol.clone(),
                    addend: *offset,
                },
            });
        }
    }

    // ADD Rd, Rn, Rm
    if let Some(Operand::Reg(rm_name)) = operands.get(2) {
        let rm = parse_reg_num(rm_name).ok_or("invalid rm")?;

        // Check for extended register: add Xd, Xn, Wm, sxtw [#N]
        if let Some(Operand::Extend { kind, amount }) = operands.get(3) {
            let option = match kind.as_str() {
                "uxtb" => 0b000u32,
                "uxth" => 0b001,
                "uxtw" => 0b010,
                "uxtx" => 0b011,
                "sxtb" => 0b100,
                "sxth" => 0b101,
                "sxtw" => 0b110,
                "sxtx" => 0b111,
                _ => 0b011, // default UXTX/LSL
            };
            let imm3 = *amount & 0x7;
            // Extended register form: sf op S 01011 00 1 Rm option imm3 Rn Rd
            let word = ((sf << 31) | (op << 30) | (s_bit << 29) | (0b01011 << 24)) | (1 << 21) | (rm << 16) | (option << 13) | (imm3 << 10) | (rn << 5) | rd;
            return Ok(EncodeResult::Word(word));
        }

        // When Rn or Rd is SP (register 31), the shifted register form encodes
        // register 31 as XZR, not SP. We must use the extended register form
        // with UXTX (option=0b011) to get SP semantics.
        let rn_is_sp = matches!(&operands[1], Operand::Reg(name) if {
            let n = name.to_lowercase(); n == "sp" || n == "wsp"
        });
        let rd_is_sp = matches!(&operands[0], Operand::Reg(name) if {
            let n = name.to_lowercase(); n == "sp" || n == "wsp"
        });

        if (rn_is_sp || rd_is_sp) && operands.len() <= 3 {
            // Extended register form with UXTX #0: sf op S 01011 00 1 Rm 011 000 Rn Rd
            let option = if is_64 { 0b011u32 } else { 0b010u32 }; // UXTX for 64-bit, UXTW for 32-bit
            let word = (((sf << 31) | (op << 30) | (s_bit << 29) | (0b01011 << 24)) | (1 << 21) | (rm << 16) | (option << 13)) | (rn << 5) | rd;
            return Ok(EncodeResult::Word(word));
        }

        // Check for shifted register: add Xd, Xn, Xm, lsl #N
        let (shift_type, shift_amount) = if let Some(Operand::Shift { kind, amount }) = operands.get(3) {
            let st = match kind.as_str() {
                "lsl" => 0b00u32,
                "lsr" => 0b01,
                "asr" => 0b10,
                _ => 0b00,
            };
            (st, *amount)
        } else {
            (0, 0)
        };

        let word = ((sf << 31) | (op << 30) | (s_bit << 29) | (0b01011 << 24) | (shift_type << 22)) | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
        return Ok(EncodeResult::Word(word));
    }

    Err(format!("unsupported add/sub operands: {:?}", operands))
}

// ── Logical ──────────────────────────────────────────────────────────────

pub(crate) fn encode_logical(operands: &[Operand], opc: u32) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("logical op requires 3 operands".to_string());
    }

    // NEON vector form: ORR/AND/EOR Vd.T, Vn.T, Vm.T
    if let Some(Operand::RegArrangement { .. }) = operands.first() {
        return encode_neon_logical(operands, opc);
    }

    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let sf = sf_bit(is_64);

    // AND/ORR/EOR Rd, Rn, #imm (bitmask immediate)
    if let Some(Operand::Imm(imm)) = operands.get(2) {
        if let Some((n, immr, imms)) = encode_bitmask_imm(*imm as u64, is_64) {
            let word = (sf << 31) | (opc << 29) | (0b100100 << 23) | (n << 22) | (immr << 16) | (imms << 10) | (rn << 5) | rd;
            return Ok(EncodeResult::Word(word));
        }
        return Err(format!("cannot encode bitmask immediate: 0x{:x}", imm));
    }

    // AND/ORR/EOR Rd, Rn, Rm [, shift #amount]
    if let Some(Operand::Reg(rm_name)) = operands.get(2) {
        let rm = parse_reg_num(rm_name).ok_or("invalid rm")?;

        let (shift_type, shift_amount) = if let Some(Operand::Shift { kind, amount }) = operands.get(3) {
            let st = match kind.as_str() {
                "lsl" => 0b00u32,
                "lsr" => 0b01,
                "asr" => 0b10,
                "ror" => 0b11,
                _ => 0b00,
            };
            (st, *amount)
        } else {
            (0, 0)
        };

        let word = ((sf << 31) | (opc << 29) | (0b01010 << 24) | (shift_type << 22))
            | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
        return Ok(EncodeResult::Word(word));
    }

    Err("unsupported logical operands".to_string())
}

/// Encode a bitmask immediate for AArch64.
/// Returns (N, immr, imms) if the value is a valid bitmask immediate.
pub(crate) fn encode_bitmask_imm(val: u64, is_64: bool) -> Option<(u32, u32, u32)> {
    if val == 0 || (!is_64 && val == 0xFFFFFFFF) || (is_64 && val == u64::MAX) {
        return None; // Not a valid bitmask immediate
    }

    let width = if is_64 { 64 } else { 32 };
    let val = if !is_64 { val & 0xFFFFFFFF } else { val };

    // Try each possible element size: 2, 4, 8, 16, 32, 64
    for size in [2u32, 4, 8, 16, 32, 64] {
        if size > width {
            continue;
        }

        let mask = if size == 64 { u64::MAX } else { (1u64 << size) - 1 };
        let elem = val & mask;

        // Check that the pattern repeats
        let mut repeats = true;
        let mut pos = size;
        while pos < width {
            if ((val >> pos) & mask) != elem {
                repeats = false;
                break;
            }
            pos += size;
        }
        if !repeats {
            continue;
        }

        // Check that elem is a contiguous run of 1s (possibly rotated)
        let ones = elem.count_ones();
        if ones == 0 || ones == size {
            continue; // All zeros or all ones in element
        }

        // Find rotation: rotate elem right until the least significant bit is 1
        // and the run of 1s starts at bit 0.
        // The `r` we find is the right-rotation from actual -> base.
        // immr is the right-rotation from base -> actual = size - r (mod size).
        let mut found_rotation = false;
        let mut rotation = 0u32;
        for r in 0..size {
            let rot = if r == 0 { elem } else { ((elem >> r) | (elem << (size - r))) & mask };
            // Check if this is a contiguous run from bit 0
            let run = rot.trailing_ones();
            if run == ones {
                // r rotates actual -> base, so immr = size - r (mod size) rotates base -> actual
                rotation = if r == 0 { 0 } else { size - r };
                found_rotation = true;
                break;
            }
        }
        if !found_rotation {
            continue;
        }

        // Encode the fields
        let n = if size == 64 { 1u32 } else { 0u32 };
        let immr = rotation;
        let imms = match size {
            2 => 0b111100 | (ones - 1),
            4 => 0b111000 | (ones - 1),
            8 => 0b110000 | (ones - 1),
            16 => 0b100000 | (ones - 1),
            32 => ones - 1,
            64 => ones - 1,
            _ => unreachable!(),
        };

        return Some((n, immr, imms));
    }

    None
}

// ── MUL/DIV ──────────────────────────────────────────────────────────────

pub(crate) fn encode_mul(operands: &[Operand]) -> Result<EncodeResult, String> {
    // NEON vector form: MUL Vd.T, Vn.T, Vm.T
    if let Some(Operand::RegArrangement { .. }) = operands.first() {
        return encode_neon_mul(operands);
    }
    // MUL Rd, Rn, Rm is MADD Rd, Rn, Rm, XZR
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let sf = sf_bit(is_64);
    let word = (sf << 31) | (0b0011011000 << 21) | (rm << 16) | (0b11111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_madd(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let (ra, _) = get_reg(operands, 3)?;
    let sf = sf_bit(is_64);
    let word = ((sf << 31) | (0b0011011000 << 21) | (rm << 16)) | (ra << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_msub(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let (ra, _) = get_reg(operands, 3)?;
    let sf = sf_bit(is_64);
    let word = (sf << 31) | (0b0011011000 << 21) | (rm << 16) | (1 << 15) | (ra << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_div(operands: &[Operand], unsigned: bool) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let sf = sf_bit(is_64);
    let o1 = if unsigned { 0u32 } else { 1u32 };
    // Data-processing (2 source): sf 0 S=0 11010110 Rm 00001 o1 Rn Rd
    let word = (sf << 31) | (0b0011010110 << 21) | (rm << 16)
        | (0b00001 << 11) | (o1 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode SMULL Xd, Wn, Wm -> SMADDL Xd, Wn, Wm, XZR
pub(crate) fn encode_smull(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    // SMADDL: 1 00 11011 001 Rm 0 11111 Rn Rd (Ra=XZR makes it SMULL)
    let word = (1u32 << 31) | (0b0011011001 << 21) | (rm << 16)
        | (0b011111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode UMULL Xd, Wn, Wm -> UMADDL Xd, Wn, Wm, XZR
pub(crate) fn encode_umull(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    // UMADDL: 1 00 11011 101 Rm 0 11111 Rn Rd (Ra=XZR makes it UMULL)
    let word = (1u32 << 31) | (0b0011011101 << 21) | (rm << 16)
        | (0b011111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode SMADDL Xd, Wn, Wm, Xa (signed multiply-add long)
pub(crate) fn encode_smaddl(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let (ra, _) = get_reg(operands, 3)?;
    // SMADDL: 1 00 11011 001 Rm 0 Ra Rn Rd
    let word = (1u32 << 31) | (0b0011011001 << 21) | (rm << 16)
        | (ra << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode UMADDL Xd, Wn, Wm, Xa (unsigned multiply-add long)
pub(crate) fn encode_umaddl(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let (ra, _) = get_reg(operands, 3)?;
    // UMADDL: 1 00 11011 101 Rm 0 Ra Rn Rd
    let word = (1u32 << 31) | (0b0011011101 << 21) | (rm << 16)
        | (ra << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode MNEG Xd, Xn, Xm -> MSUB Xd, Xn, Xm, XZR
pub(crate) fn encode_mneg(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let sf = sf_bit(is_64);
    // MSUB with Ra=XZR: sf 00 11011 000 Rm 1 11111 Rn Rd
    let word = (sf << 31) | (0b0011011000 << 21) | (rm << 16)
        | (1 << 15) | (0b11111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_umulh(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    // UMULH: 1 00 11011 1 10 Rm 0 11111 Rn Rd
    let word = (1u32 << 31) | (0b0011011110 << 21) | (rm << 16) | (0b011111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_smulh(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    // SMULH: 1 00 11011 0 10 Rm 0 11111 Rn Rd
    let word = (1u32 << 31) | (0b0011011010 << 21) | (rm << 16) | (0b011111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_neg(operands: &[Operand]) -> Result<EncodeResult, String> {
    // NEG Rd, Rm [, shift #amount] -> SUB Rd, XZR, Rm [, shift #amount]
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rm, _) = get_reg(operands, 1)?;
    let sf = sf_bit(is_64);
    let (shift_type, shift_amount) = if let Some(Operand::Shift { kind, amount }) = operands.get(2) {
        let st = match kind.as_str() {
            "lsl" => 0b00u32,
            "lsr" => 0b01,
            "asr" => 0b10,
            _ => 0b00,
        };
        (st, *amount)
    } else {
        (0, 0)
    };
    let word = (sf << 31) | (1 << 30) | (0b01011 << 24) | (shift_type << 22)
        | (rm << 16) | ((shift_amount & 0x3F) << 10) | (0b11111 << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_negs(operands: &[Operand]) -> Result<EncodeResult, String> {
    // NEGS Rd, Rm [, shift #amount] -> SUBS Rd, XZR, Rm [, shift #amount]
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rm, _) = get_reg(operands, 1)?;
    let sf = sf_bit(is_64);
    let (shift_type, shift_amount) = if let Some(Operand::Shift { kind, amount }) = operands.get(2) {
        let st = match kind.as_str() {
            "lsl" => 0b00u32,
            "lsr" => 0b01,
            "asr" => 0b10,
            _ => 0b00,
        };
        (st, *amount)
    } else {
        (0, 0)
    };
    let word = (sf << 31) | (1 << 30) | (1 << 29) | (0b01011 << 24) | (shift_type << 22)
        | (rm << 16) | ((shift_amount & 0x3F) << 10) | (0b11111 << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_mvn(operands: &[Operand]) -> Result<EncodeResult, String> {
    // NEON vector form: MVN Vd.T, Vn.T (alias of NOT)
    if let Some(Operand::RegArrangement { .. }) = operands.first() {
        return encode_neon_not(operands);
    }
    // MVN Rd, Rm [, shift #amount] -> ORN Rd, XZR, Rm [, shift #amount]
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rm, _) = get_reg(operands, 1)?;
    let sf = sf_bit(is_64);
    let (shift_type, shift_amount) = if let Some(Operand::Shift { kind, amount }) = operands.get(2) {
        let st = match kind.as_str() {
            "lsl" => 0b00u32,
            "lsr" => 0b01,
            "asr" => 0b10,
            "ror" => 0b11,
            _ => 0b00,
        };
        (st, *amount)
    } else {
        (0, 0)
    };
    let word = (sf << 31) | (0b01 << 29) | (0b01010 << 24) | (shift_type << 22) | (1 << 21)
        | (rm << 16) | ((shift_amount & 0x3F) << 10) | (0b11111 << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_adc(operands: &[Operand], set_flags: bool) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let sf = sf_bit(is_64);
    let s = if set_flags { 1u32 } else { 0 };
    let word = ((sf << 31) | (s << 29) | (0b11010000 << 21) | (rm << 16)) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_sbc(operands: &[Operand], set_flags: bool) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let sf = sf_bit(is_64);
    let s = if set_flags { 1u32 } else { 0 };
    let word = ((sf << 31) | (1 << 30) | (s << 29) | (0b11010000 << 21) | (rm << 16)) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── Shifts ───────────────────────────────────────────────────────────────

pub(crate) fn encode_shift(operands: &[Operand], shift_type: u32) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;

    // LSL/LSR/ASR Rd, Rn, #imm (immediate form -> UBFM/SBFM)
    if let Some(Operand::Imm(imm)) = operands.get(2) {
        let sf = sf_bit(is_64);
        let imm = *imm as u32;
        let width = if is_64 { 64 } else { 32 };
        let n = if is_64 { 1u32 } else { 0u32 };

        match shift_type {
            0b00 => {
                // LSL #imm -> UBFM Rd, Rn, #(-imm mod width), #(width-1-imm)
                let immr = (width - imm) % width;
                let imms = width - 1 - imm;
                let word = (sf << 31) | (0b10 << 29) | (0b100110 << 23) | (n << 22) | (immr << 16) | (imms << 10) | (rn << 5) | rd;
                return Ok(EncodeResult::Word(word));
            }
            0b01 => {
                // LSR #imm -> UBFM Rd, Rn, #imm, #(width-1)
                let immr = imm;
                let imms = width - 1;
                let word = (sf << 31) | (0b10 << 29) | (0b100110 << 23) | (n << 22) | (immr << 16) | (imms << 10) | (rn << 5) | rd;
                return Ok(EncodeResult::Word(word));
            }
            0b10 => {
                // ASR #imm -> SBFM Rd, Rn, #imm, #(width-1)
                let immr = imm;
                let imms = width - 1;
                let word = (sf << 31) | (0b100110 << 23) | (n << 22) | (immr << 16) | (imms << 10) | (rn << 5) | rd;
                return Ok(EncodeResult::Word(word));
            }
            0b11 => {
                // ROR #imm -> EXTR Rd, Rn, Rn, #imm
                // EXTR: sf 0 0 100111 N 0 Rm imms Rn Rd
                let word = (sf << 31) | (0b00100111 << 23) | (n << 22) | (rn << 16)
                    | (imm << 10) | (rn << 5) | rd;
                return Ok(EncodeResult::Word(word));
            }
            _ => {}
        }
    }

    // LSL/LSR/ASR Rd, Rn, Rm (register form)
    if let Some(Operand::Reg(rm_name)) = operands.get(2) {
        let rm = parse_reg_num(rm_name).ok_or("invalid rm")?;
        let sf = sf_bit(is_64);
        // Data-processing (2 source): sf 0 S=0 11010110 Rm 0010 op2 Rn Rd
        let op2 = shift_type; // 00=LSL, 01=LSR, 10=ASR, 11=ROR
        let word = (sf << 31) | (0b0011010110 << 21) | (rm << 16) | (0b0010 << 12) | (op2 << 10) | (rn << 5) | rd;
        return Ok(EncodeResult::Word(word));
    }

    Err("unsupported shift operands".to_string())
}

// ── Extensions ───────────────────────────────────────────────────────────

pub(crate) fn encode_sxtw(operands: &[Operand]) -> Result<EncodeResult, String> {
    // SXTW Xd, Wn -> SBFM Xd, Xn, #0, #31
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let word = ((1u32 << 31) | (0b100110 << 23) | (1 << 22)) | (31 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_sxth(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0 };
    let word = ((sf << 31) | (0b100110 << 23) | (n << 22)) | (15 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_sxtb(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0 };
    let word = ((sf << 31) | (0b100110 << 23) | (n << 22)) | (7 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_uxtw(operands: &[Operand]) -> Result<EncodeResult, String> {
    // UXTW is MOV Wd, Wn (the upper 32 bits are zeroed)
    // Or: UBFM Xd, Xn, #0, #31
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    // Use 32-bit ORR (MOV alias)
    let word = (0b001010100 << 23) | (rn << 16) | (0b11111 << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_uxth(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0 };
    let word = ((sf << 31) | (0b10 << 29) | (0b100110 << 23) | (n << 22)) | (15 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_uxtb(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0 };
    let word = ((sf << 31) | (0b10 << 29) | (0b100110 << 23) | (n << 22)) | (7 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode ORN (logical OR NOT): ORN Rd, Rn, Rm (scalar or vector)
pub(crate) fn encode_orn(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("orn requires 3 operands".to_string());
    }

    // NEON vector form: ORN Vd.T, Vn.T, Vm.T
    if let Some(Operand::RegArrangement { .. }) = operands.first() {
        let (rd, arr_d) = get_neon_reg(operands, 0)?;
        let (rn, _) = get_neon_reg(operands, 1)?;
        let (rm, _) = get_neon_reg(operands, 2)?;
        let q: u32 = if arr_d == "16b" { 1 } else { 0 };
        // ORN Vd.T, Vn.T, Vm.T: 0 Q 0 01110 11 1 Rm 000111 Rn Rd
        let word = (q << 30) | (0b001110 << 24) | (0b11 << 22) | (1 << 21)
            | (rm << 16) | (0b000111 << 10) | (rn << 5) | rd;
        return Ok(EncodeResult::Word(word));
    }

    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let sf = sf_bit(is_64);

    let (shift_type, shift_amount) = if let Some(Operand::Shift { kind, amount }) = operands.get(3) {
        let st = match kind.as_str() {
            "lsl" => 0b00u32,
            "lsr" => 0b01,
            "asr" => 0b10,
            "ror" => 0b11,
            _ => 0b00,
        };
        (st, *amount)
    } else {
        (0, 0)
    };

    // ORN Rd, Rn, Rm [, shift #amount]: sf 01 01010 shift 1 Rm imm6 Rn Rd
    let word = (sf << 31) | (0b01 << 29) | (0b01010 << 24) | (shift_type << 22) | (1 << 21)
        | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode EON (exclusive OR NOT): EON Rd, Rn, Rm [, shift #amount]
pub(crate) fn encode_eon(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("eon requires 3 operands".to_string());
    }
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let sf = sf_bit(is_64);

    let (shift_type, shift_amount) = if let Some(Operand::Shift { kind, amount }) = operands.get(3) {
        let st = match kind.as_str() {
            "lsl" => 0b00u32,
            "lsr" => 0b01,
            "asr" => 0b10,
            "ror" => 0b11,
            _ => 0b00,
        };
        (st, *amount)
    } else {
        (0, 0)
    };

    // EON Rd, Rn, Rm [, shift #amount]: sf 10 01010 shift 1 Rm imm6 Rn Rd (opc=10, N=1)
    let word = (sf << 31) | (0b10 << 29) | (0b01010 << 24) | (shift_type << 22) | (1 << 21)
        | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode BICS (bitwise clear, setting flags): BICS Rd, Rn, Rm [, shift #amount]
pub(crate) fn encode_bics(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("bics requires 3 operands".to_string());
    }
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let sf = sf_bit(is_64);

    let (shift_type, shift_amount) = if let Some(Operand::Shift { kind, amount }) = operands.get(3) {
        let st = match kind.as_str() {
            "lsl" => 0b00u32,
            "lsr" => 0b01,
            "asr" => 0b10,
            "ror" => 0b11,
            _ => 0b00,
        };
        (st, *amount)
    } else {
        (0, 0)
    };

    // BICS Rd, Rn, Rm [, shift #amount]: sf 11 01010 shift 1 Rm imm6 Rn Rd
    let word = (sf << 31) | (0b11 << 29) | (0b01010 << 24) | (shift_type << 22) | (1 << 21)
        | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode BIC instruction - disambiguates between scalar and NEON forms.
/// Scalar register: BIC Xd, Xn, Xm [, shift #amount] -> AND NOT (opc=00, N=1)
/// Scalar immediate: BIC Xd, Xn, #imm -> AND Xd, Xn, #~imm
/// NEON vector: BIC Vd.T, Vn.T, Vm.T
pub(crate) fn encode_bic(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("bic requires 3 operands".to_string());
    }

    // NEON vector form: BIC Vd.T, Vn.T, Vm.T
    if let Some(Operand::RegArrangement { .. }) = operands.first() {
        return encode_neon_bic(operands);
    }

    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let sf = sf_bit(is_64);

    // BIC Xd, Xn, #imm -> AND Xd, Xn, #~imm (bitmask immediate, inverted)
    if let Some(Operand::Imm(imm)) = operands.get(2) {
        let inverted = if is_64 {
            !(*imm as u64)
        } else {
            (!(*imm as u32)) as u64
        };
        if let Some((n, immr, imms)) = encode_bitmask_imm(inverted, is_64) {
            // AND Rd, Rn, #~imm: sf 00 100100 N immr imms Rn Rd
            // AND Rd, Rn, #~imm encoding: sf=bit31, opc=00 (bits29:30), 100100 (bits23:28), N, immr, imms, Rn, Rd
            let word = (sf << 31) | (0b100100 << 23) | (n << 22) | (immr << 16) | (imms << 10) | (rn << 5) | rd;
            return Ok(EncodeResult::Word(word));
        }
        return Err(format!("cannot encode bitmask immediate for bic: 0x{:x} (inverted: 0x{:x})", imm, inverted));
    }

    // BIC Xd, Xn, Xm [, shift #amount]: sf 00 01010 shift 1 Rm imm6 Rn Rd (N=1)
    if let Some(Operand::Reg(rm_name)) = operands.get(2) {
        let rm = parse_reg_num(rm_name).ok_or("invalid rm register for bic")?;

        let (shift_type, shift_amount) = if let Some(Operand::Shift { kind, amount }) = operands.get(3) {
            let st = match kind.as_str() {
                "lsl" => 0b00u32,
                "lsr" => 0b01,
                "asr" => 0b10,
                "ror" => 0b11,
                _ => 0b00,
            };
            (st, *amount)
        } else {
            (0, 0)
        };

        // BIC is AND with N=1 (bit 21): sf opc=00(bits29:30) 01010 shift 1 Rm imm6 Rn Rd
        let word = (sf << 31) | (0b01010 << 24) | (shift_type << 22) | (1 << 21)
            | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
        return Ok(EncodeResult::Word(word));
    }

    Err("unsupported bic operands".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ── Field extractors (ARMv8 ADD/SUB encoding) ───────────────────────────
    // Immediate form:  sf op S 1000100 sh imm12 Rn Rd
    // Shifted-reg form: sf op S 0101100 shift Rm imm6 Rn Rd
    fn sf_of(w: u32) -> u32        { (w >> 31) & 1 }
    fn op_of(w: u32) -> u32        { (w >> 30) & 1 }
    fn s_of(w: u32) -> u32         { (w >> 29) & 1 }
    fn opcode5_of(w: u32) -> u32   { (w >> 24) & 0x1F } // bits 24..28
    fn sh_of(w: u32) -> u32        { (w >> 22) & 1 }    // imm form shift bit
    fn imm12_of(w: u32) -> u32     { (w >> 10) & 0xFFF }
    fn shift_type_of(w: u32) -> u32 { (w >> 22) & 0x3 }
    fn shift_amt_of(w: u32) -> u32 { (w >> 10) & 0x3F }
    fn rm_of(w: u32) -> u32        { (w >> 16) & 0x1F }
    fn rn_of(w: u32) -> u32        { (w >> 5) & 0x1F }
    fn rd_of(w: u32) -> u32        { w & 0x1F }
    // Extended-register form fields (share the imm6 region: option = bits 15:13, imm3 = bits 12:10)
    fn ext21_of(w: u32) -> u32     { (w >> 21) & 1 }    // 1 => extended register, 0 => shifted register
    fn option_of(w: u32) -> u32    { (w >> 13) & 0x7 }
    fn imm3_of(w: u32) -> u32      { (w >> 10) & 0x7 }

    fn xreg(n: u32) -> Operand { Operand::Reg(format!("x{}", n)) }

    fn expect_word(r: Result<EncodeResult, String>) -> u32 {
        match r.unwrap() {
            EncodeResult::Word(w) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    proptest! {
        // 1. ADD Xd, Xn, #imm (0..=0xFFF, unshifted): every fixed field and every
        //    register/immediate field lands exactly where the ARMv8 spec dictates.
        #[test]
        fn add_imm_unshifted_field_placement(
            rd in 0u32..=30,
            rn in 0u32..=30,
            imm in 0i64..=0xFFF,
        ) {
            let ops = vec![xreg(rd), xreg(rn), Operand::Imm(imm)];
            let w = expect_word(encode_add_sub(&ops, false, false));
            prop_assert_eq!(sf_of(w), 1);            // 64-bit
            prop_assert_eq!(op_of(w), 0);            // ADD
            prop_assert_eq!(s_of(w), 0);             // no flags
            prop_assert_eq!(opcode5_of(w), 0b10001); // add/sub immediate
            prop_assert_eq!(sh_of(w), 0);            // unshifted
            prop_assert_eq!(imm12_of(w), imm as u32);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 2. Register-form ADD/SUB: opcode, op bit, and rm/rn/rd placement.
        #[test]
        fn register_form_field_placement(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
            is_sub in any::<bool>(),
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let w = expect_word(encode_add_sub(&ops, is_sub, false));
            prop_assert_eq!(sf_of(w), 1);
            prop_assert_eq!(op_of(w), if is_sub { 1 } else { 0 });
            prop_assert_eq!(s_of(w), 0);
            prop_assert_eq!(opcode5_of(w), 0b01011); // add/sub shifted register
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(shift_type_of(w), 0);
            prop_assert_eq!(shift_amt_of(w), 0);
        }

        // 3. Negative immediate flips the operation: ADD #-N -> SUB #N,
        //    SUB #-N -> ADD #N. The op bit and imm12 must reflect the swap.
        #[test]
        fn negative_immediate_flips_op(
            rd in 0u32..=30,
            rn in 0u32..=30,
            n in 1i64..=0xFFF,
            is_sub in any::<bool>(),
        ) {
            let ops = vec![xreg(rd), xreg(rn), Operand::Imm(-n)];
            let w = expect_word(encode_add_sub(&ops, is_sub, false));
            prop_assert_eq!(op_of(w), if is_sub { 0 } else { 1 });
            prop_assert_eq!(sh_of(w), 0);
            prop_assert_eq!(imm12_of(w), n as u32);
        }

        // 4. Shifted immediate: explicit `lsl #12` (operand = chunk) and auto-shift
        //    (operand = chunk<<12) both produce sh=1 with imm12 == chunk.
        #[test]
        fn shifted_immediate_uses_sh_bit(
            rd in 0u32..=30,
            rn in 0u32..=30,
            k in 1u32..=0xFFF,
            explicit in any::<bool>(),
        ) {
            let ops = if explicit {
                vec![xreg(rd), xreg(rn), Operand::Imm(k as i64),
                     Operand::Shift { kind: "lsl".into(), amount: 12 }]
            } else {
                vec![xreg(rd), xreg(rn), Operand::Imm((k as i64) << 12)]
            };
            let w = expect_word(encode_add_sub(&ops, false, false));
            prop_assert_eq!(opcode5_of(w), 0b10001);
            prop_assert_eq!(sh_of(w), 1);
            prop_assert_eq!(imm12_of(w), k);
        }

        // 5. An immediate whose low 12 bits are nonzero and that exceeds the
        //    unshifted range cannot be encoded -> Err (never silently truncated).
        #[test]
        fn unencodable_immediate_returns_err(
            rd in 0u32..=30,
            rn in 0u32..=30,
            low in 1u32..=0xFFF,
            extra in 1u32..=0x10,
        ) {
            let imm = (low as i64) + (extra as i64) * 0x1000;
            prop_assume!(imm & 0xFFF != 0 && imm > 0xFFF);
            let ops = vec![xreg(rd), xreg(rn), Operand::Imm(imm)];
            prop_assert!(encode_add_sub(&ops, false, false).is_err());
        }

        // 6. sf (bit 31) tracks register width: W -> 0, X -> 1.
        #[test]
        fn sf_bit_tracks_register_width(
            n in 0u32..=30,
            is_w in any::<bool>(),
        ) {
            let rd = if is_w { Operand::Reg(format!("w{}", n)) } else { xreg(n) };
            let rn = if is_w { Operand::Reg(format!("w{}", n)) } else { xreg(n) };
            let ops = vec![rd, rn, Operand::Imm(0)];
            let w = expect_word(encode_add_sub(&ops, false, false));
            prop_assert_eq!(sf_of(w), if is_w { 0 } else { 1 });
        }
    }

    // ── encode_add_sub: shifted / extended / SP / relocation / width contracts ──
    proptest! {
        // 7. Shifted-register form: ADD Xd, Xn, Xm, <shift> #amount. The
        //    shift-type field (bits 23:22), imm6 shift amount (bits 15:10),
        //    S bit (set_flags), and all register fields land per the ARMv8 spec.
        #[test]
        fn add_shifted_register_fields(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
            sk in 0u32..=2u32,          // 0=lsl, 1=lsr, 2=asr
            amount in 0u32..=63u32,     // X-register imm6 range
            set_flags in any::<bool>(),
        ) {
            let (kind, want_st) = match sk {
                0 => ("lsl", 0u32),
                1 => ("lsr", 1u32),
                _ => ("asr", 2u32),
            };
            let ops = vec![xreg(rd), xreg(rn), xreg(rm),
                           Operand::Shift { kind: kind.into(), amount }];
            let w = expect_word(encode_add_sub(&ops, false, set_flags));
            prop_assert_eq!(opcode5_of(w), 0b01011); // add/sub shifted register
            prop_assert_eq!(shift_type_of(w), want_st);
            prop_assert_eq!(shift_amt_of(w), amount);
            prop_assert_eq!(s_of(w), if set_flags { 1 } else { 0 });
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(sf_of(w), 1);
        }

        // 8. Extended-register form: ADD Xd, Xn, Wm, <extend>. The option
        //    field (bits 15:13) selects the extend kind, bit 21 is the
        //    extended-register indicator, and imm3 is 0 with no extra shift.
        #[test]
        fn add_extended_register_option_field(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
            ek in 0u32..=7u32,
            set_flags in any::<bool>(),
        ) {
            let (kind, want_opt) = match ek {
                0 => ("uxtb", 0b000u32),
                1 => ("uxth", 0b001),
                2 => ("uxtw", 0b010),
                3 => ("uxtx", 0b011),
                4 => ("sxtb", 0b100),
                5 => ("sxth", 0b101),
                6 => ("sxtw", 0b110),
                _ => ("sxtx", 0b111),
            };
            let ops = vec![xreg(rd), xreg(rn), xreg(rm),
                           Operand::Extend { kind: kind.into(), amount: 0 }];
            let w = expect_word(encode_add_sub(&ops, false, set_flags));
            prop_assert_eq!(opcode5_of(w), 0b01011);
            prop_assert_eq!(ext21_of(w), 1);              // extended, not shifted
            prop_assert_eq!(option_of(w), want_opt);
            prop_assert_eq!(imm3_of(w), 0);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(s_of(w), if set_flags { 1 } else { 0 });
        }

        // 9. SP in rd or rn forces the extended-register form (option=UXTX,
        //    imm3=0) so register 31 reads as SP rather than XZR.
        #[test]
        fn sp_operand_uses_extended_register_form(
            rd in 0u32..=30,
            rm in 0u32..=30,
            sp_in_rd in any::<bool>(),
        ) {
            let (rd_op, rn_op) = if sp_in_rd {
                (Operand::Reg("sp".into()), xreg(rd))
            } else {
                (xreg(rd), Operand::Reg("sp".into()))
            };
            let ops = vec![rd_op, rn_op, xreg(rm)];
            let w = expect_word(encode_add_sub(&ops, false, false));
            prop_assert_eq!(opcode5_of(w), 0b01011);
            prop_assert_eq!(ext21_of(w), 1);          // extended register
            prop_assert_eq!(option_of(w), 0b011);     // UXTX (64-bit SP)
            prop_assert_eq!(imm3_of(w), 0);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(sf_of(w), 1);
        }

        // 10. Relocation modifiers (:lo12:, :tprel_lo12_nc:, :tprel_hi12:)
        //     emit WordWithReloc with the correct reloc type/symbol, leave
        //     imm12 zero for the linker, and (for tprel_hi12) set the sh bit.
        #[test]
        fn reloc_modifier_emits_correct_reloc(
            rd in 0u32..=30,
            rn in 0u32..=30,
            sym_id in 0u32..=1000u32,
            mk in 0u32..=2u32,
        ) {
            let (kind, want_reloc, want_sh) = match mk {
                0 => ("lo12", "AddAbsLo12", 0u32),
                1 => ("tprel_lo12_nc", "TlsLeAddTprelLo12", 0u32),
                _ => ("tprel_hi12", "TlsLeAddTprelHi12", 1u32),
            };
            let sym = format!("sym{}", sym_id);
            let ops = vec![xreg(rd), xreg(rn),
                           Operand::Modifier { kind: kind.into(), symbol: sym.clone() }];
            let (word, reloc) = match encode_add_sub(&ops, false, false) {
                Ok(EncodeResult::WordWithReloc { word, reloc }) => (word, reloc),
                Ok(other) => return Err(proptest::test_runner::TestCaseError::fail(
                    format!("expected WordWithReloc, got {:?}", other))),
                Err(e) => return Err(proptest::test_runner::TestCaseError::fail(
                    format!("encode_add_sub failed: {}", e))),
            };
            prop_assert_eq!(format!("{:?}", reloc.reloc_type), want_reloc);
            prop_assert_eq!(reloc.symbol, sym);
            prop_assert_eq!(reloc.addend, 0);
            prop_assert_eq!(imm12_of(word), 0);     // linker fills imm12
            prop_assert_eq!(sh_of(word), want_sh);  // tprel_hi12 sets sh
            prop_assert_eq!(rn_of(word), rn);
            prop_assert_eq!(rd_of(word), rd);
        }

        // 11. NEGATIVE CONTRACT: for 32-bit (W) shifted-register form, imm6 must
        //     be 0..=31; a shift of 32..63 is UNDEFINED (ARMv8 ARM) and MUST be
        //     rejected, not silently masked into the imm6 field.
        #[test]
        fn w_reg_shifted_form_rejects_shift_above_31(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
            amount in 32u32..=63u32,
            sk in 0u32..=2u32,
        ) {
            let kind = match sk { 0 => "lsl", 1 => "lsr", _ => "asr" };
            let ops = vec![Operand::Reg(format!("w{}", rd)),
                           Operand::Reg(format!("w{}", rn)),
                           Operand::Reg(format!("w{}", rm)),
                           Operand::Shift { kind: kind.into(), amount }];
            prop_assert!(encode_add_sub(&ops, false, false).is_err());
        }
    }

    // ── encode_add_sub: S-bit / addend / imm-field-range contracts ─────────
    proptest! {
        // 12. POSITIVE GAP: ADDS/SUBS immediate form must set the S bit (bit 29).
        #[test]
        fn adds_immediate_form_sets_s_bit(
            rd in 0u32..=30,
            rn in 0u32..=30,
            imm in 0i64..=0xFFF,
            is_sub in any::<bool>(),
        ) {
            let ops = vec![xreg(rd), xreg(rn), Operand::Imm(imm)];
            let w = expect_word(encode_add_sub(&ops, is_sub, true));
            prop_assert_eq!(s_of(w), 1); // set_flags => S=1
            prop_assert_eq!(opcode5_of(w), 0b10001);
            prop_assert_eq!(op_of(w), if is_sub { 1 } else { 0 });
            prop_assert_eq!(imm12_of(w), imm as u32);
        }

        // 13. POSITIVE GAP: ADD Rd, Rn, :lo12:sym+off must carry the addend
        //     through to Relocation.addend unchanged.
        #[test]
        fn modoffset_lo12_preserves_addend(
            rd in 0u32..=30,
            rn in 0u32..=30,
            sym_id in 0u32..=1000u32,
            offset in -1000i64..=1000i64,
        ) {
            let sym = format!("sym{}", sym_id);
            let ops = vec![xreg(rd), xreg(rn),
                           Operand::ModifierOffset { kind: "lo12".into(),
                                                     symbol: sym.clone(),
                                                     offset }];
            let reloc = match encode_add_sub(&ops, false, false) {
                Ok(EncodeResult::WordWithReloc { word, reloc }) => {
                    prop_assert_eq!(imm12_of(word), 0);
                    reloc
                }
                Ok(other) => return Err(proptest::test_runner::TestCaseError::fail(
                    format!("expected WordWithReloc, got {:?}", other))),
                Err(e) => return Err(proptest::test_runner::TestCaseError::fail(
                    format!("encode_add_sub failed: {}", e))),
            };
            prop_assert_eq!(format!("{:?}", reloc.reloc_type), "AddAbsLo12");
            prop_assert_eq!(reloc.symbol, sym);
            prop_assert_eq!(reloc.addend, offset);
        }

        // 14. NEGATIVE CONTRACT: the extended-register imm3 field is 3 bits
        //     (0..=7). An extend shift amount >= 8 cannot be represented and
        //     MUST be rejected (ARMv8 ARM), not silently masked with `& 0x7`.
        #[test]
        fn extend_amount_above_7_must_be_rejected(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
            ek in 0u32..=7u32,
            amount in 8u32..=255u32,
        ) {
            let kind = match ek {
                0 => "uxtb", 1 => "uxth", 2 => "uxtw", 3 => "uxtx",
                4 => "sxtb", 5 => "sxth", 6 => "sxtw", _ => "sxtx",
            };
            let ops = vec![xreg(rd), xreg(rn), xreg(rm),
                           Operand::Extend { kind: kind.into(), amount }];
            prop_assert!(encode_add_sub(&ops, false, false).is_err());
        }

        // 15. NEGATIVE CONTRACT: for 64-bit (X) shifted-register LSL, imm6 is
        //     0..=63; `lsl #64` and above are UNDEFINED and MUST be rejected,
        //     not silently masked into the imm6 field via `& 0x3F`.
        #[test]
        fn xreg_lsl_shift_above_63_must_be_rejected(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
            amount in 64u32..=255u32,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm),
                           Operand::Shift { kind: "lsl".into(), amount }];
            prop_assert!(encode_add_sub(&ops, false, false).is_err());
        }

        // 16. POSITIVE GAP: SUBS immediate form full field placement — op=1,
        //     S=1, fixed opcode, unshifted, imm12/rn/rd all land per spec.
        #[test]
        fn subs_immediate_form_fields(
            rd in 0u32..=30,
            rn in 0u32..=30,
            imm in 0i64..=0xFFF,
        ) {
            let ops = vec![xreg(rd), xreg(rn), Operand::Imm(imm)];
            let w = expect_word(encode_add_sub(&ops, true, true));
            prop_assert_eq!(sf_of(w), 1);
            prop_assert_eq!(op_of(w), 1);            // SUB
            prop_assert_eq!(s_of(w), 1);             // SUBS
            prop_assert_eq!(opcode5_of(w), 0b10001);
            prop_assert_eq!(sh_of(w), 0);
            prop_assert_eq!(imm12_of(w), imm as u32);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
        }
    }

    // ── MOVZ field extractors ─────────────────────────────────────────────
    // ARMv8 MOVZ (wide immediate): sf 10 100101 hw imm16 Rd
    //   bit 31      : sf
    //   bits 30:29  : opc (=10 for MOVZ)
    //   bits 28:23  : 100101
    //   bits 22:21  : hw (shift selector: 0=>lsl#0, 1=>lsl#16, 2=>lsl#32, 3=>lsl#48)
    //   bits 20:5   : imm16
    //   bits 4:0    : Rd
    fn opc_of(w: u32) -> u32     { (w >> 29) & 0x3 }
    fn opcode6_of(w: u32) -> u32 { (w >> 23) & 0x3F }
    fn hw_of(w: u32) -> u32      { (w >> 21) & 0x3 }
    fn imm16_of(w: u32) -> u32   { (w >> 5) & 0xFFFF }

    proptest! {
        // 1. MOVZ Xd/Wd, #imm with an in-range (0..=0xFFFF) immediate: every
        //    fixed and variable field lands exactly where the ARMv8 spec dictates.
        #[test]
        fn movz_field_placement(
            rd in 0u32..=30,
            imm in 0i64..=0xFFFF,
            is_64 in any::<bool>(),
        ) {
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let ops = vec![rd_op, Operand::Imm(imm)];
            let w = expect_word(encode_movz(&ops));
            prop_assert_eq!(sf_of(w), if is_64 { 1 } else { 0 });
            prop_assert_eq!(opc_of(w), 0b10);          // MOVZ opc
            prop_assert_eq!(opcode6_of(w), 0b100101);  // fixed wide-immediate op
            prop_assert_eq!(hw_of(w), 0);              // no shift
            prop_assert_eq!(imm16_of(w), imm as u32);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 2. The imm16 field is the low 16 bits of the immediate — for any
        //    magnitude, including those exceeding 16 bits. This documents the
        //    silent masking behaviour (see property 5 for the negative contract).
        #[test]
        fn movz_imm16_is_low_16_bits(
            rd in 0u32..=30,
            imm in 0i64..=0xFFFFFF,
        ) {
            let ops = vec![xreg(rd), Operand::Imm(imm)];
            let w = expect_word(encode_movz(&ops));
            prop_assert_eq!(imm16_of(w), (imm as u32) & 0xFFFF);
            // magnitude of the immediate must not leak into any other field
            prop_assert_eq!(opc_of(w), 0b10);
            prop_assert_eq!(opcode6_of(w), 0b100101);
            prop_assert_eq!(hw_of(w), 0);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 3. `lsl #N` selects hw = N/16 for valid multiples of 16.
        #[test]
        fn movz_hw_tracks_lsl_shift_amount(
            rd in 0u32..=30,
            imm in 0i64..=0xFFFF,
            hw in 0u32..=3, // 64-bit MOVZ permits hw 0..3
        ) {
            let amount = hw * 16;
            let ops = vec![xreg(rd), Operand::Imm(imm),
                           Operand::Shift { kind: "lsl".into(), amount }];
            let w = expect_word(encode_movz(&ops));
            prop_assert_eq!(hw_of(w), hw);
            prop_assert_eq!(imm16_of(w), imm as u32);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 4. NEGATIVE CONTRACT: ARMv8 MOVZ imm16 is a 16-bit *unsigned* field.
        //    An immediate outside [0, 0xFFFF] cannot be represented in a single
        //    MOVZ and MUST be rejected by the assembler (as GAS/LLVM do), not
        //    silently truncated.
        #[test]
        fn movz_rejects_out_of_range_immediate(
            rd in 0u32..=30,
            imm in 0x10000i64..=0xFFFFFFFF,
        ) {
            let ops = vec![xreg(rd), Operand::Imm(imm)];
            prop_assert!(encode_movz(&ops).is_err());
        }

        // 5. NEGATIVE CONTRACT: MOVZ only permits `lsl #{0,16,32,48}`. Any
        //    other shift amount (or a non-lsl shift kind) is UNDEFINED and must
        //    be rejected rather than normalized via integer division.
        #[test]
        fn movz_rejects_non_multiple_of_16_shift(
            rd in 0u32..=30,
            hw in 0u32..=3,
            rem in 1u32..=15,
        ) {
            let amount = hw * 16 + rem; // not a multiple of 16
            let ops = vec![xreg(rd), Operand::Imm(1),
                           Operand::Shift { kind: "lsl".into(), amount }];
            prop_assert!(encode_movz(&ops).is_err());
        }

        // 6. NEGATIVE CONTRACT: for 32-bit MOVZ (Wd), only hw in {0,1} is valid;
        //    lsl #32 / lsl #48 are UNDEFINED for W registers and must be Err.
        #[test]
        fn movz_w_reg_rejects_32_or_48_shift(
            rd in 0u32..=30,
            bad_amount in 32u32..=48u32,
        ) {
            prop_assume!(bad_amount == 32 || bad_amount == 48);
            let ops = vec![Operand::Reg(format!("w{}", rd)), Operand::Imm(1),
                           Operand::Shift { kind: "lsl".into(), amount: bad_amount }];
            prop_assert!(encode_movz(&ops).is_err());
        }
    }

    // ── MOVK (wide immediate, keep) ───────────────────────────────────────
    // ARMv8 MOVK: sf 11 100101 hw imm16 Rd. opc=11 distinguishes MOVK from
    // MOVZ (opc=10) and MOVN (opc=00). The extractors opc_of/opcode6_of/
    // hw_of/imm16_of/rd_of/sf_of from the MOVZ section above are reused.

    proptest! {
        // 1. MOVK Xd/Wd, #imm with an in-range (0..=0xFFFF) immediate: every fixed
        //    and variable field lands exactly where the ARMv8 spec dictates, and
        //    opc is 11 (MOVK), distinct from MOVZ's 10.
        #[test]
        fn movk_field_placement(
            rd in 0u32..=30,
            imm in 0i64..=0xFFFF,
            is_64 in any::<bool>(),
        ) {
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let ops = vec![rd_op, Operand::Imm(imm)];
            let w = expect_word(encode_movk(&ops));
            prop_assert_eq!(sf_of(w), if is_64 { 1 } else { 0 });
            prop_assert_eq!(opc_of(w), 0b11);          // MOVK opc
            prop_assert_eq!(opcode6_of(w), 0b100101);  // fixed wide-immediate op
            prop_assert_eq!(hw_of(w), 0);              // no shift
            prop_assert_eq!(imm16_of(w), imm as u32);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 2. The opc field (bits 30:29) is always 11 and is independent of
        //    register width, shift, and immediate magnitude — it must never read
        //    as MOVZ (10) or MOVN (00).
        #[test]
        fn movk_opc_is_always_11(
            rd in 0u32..=30,
            imm in 0i64..=0xFFFF,
            hw in 0u32..=3,
        ) {
            let amount = hw * 16;
            let ops = vec![xreg(rd), Operand::Imm(imm),
                           Operand::Shift { kind: "lsl".into(), amount }];
            let w = expect_word(encode_movk(&ops));
            prop_assert_eq!(opc_of(w), 0b11);
            prop_assert_eq!(opcode6_of(w), 0b100101);
        }

        // 3. `lsl #N` selects hw = N/16 for valid multiples of 16 (0/16/32/48).
        #[test]
        fn movk_hw_tracks_lsl_shift_amount(
            rd in 0u32..=30,
            imm in 0i64..=0xFFFF,
            hw in 0u32..=3, // 64-bit MOVK permits hw 0..3
        ) {
            let amount = hw * 16;
            let ops = vec![xreg(rd), Operand::Imm(imm),
                           Operand::Shift { kind: "lsl".into(), amount }];
            let w = expect_word(encode_movk(&ops));
            prop_assert_eq!(hw_of(w), hw);
            prop_assert_eq!(imm16_of(w), imm as u32);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 4. NEGATIVE CONTRACT: ARMv8 MOVK imm16 is a 16-bit *unsigned* field.
        //    An immediate outside [0, 0xFFFF] cannot be represented in a single
        //    MOVK and MUST be rejected by the assembler (as GAS/LLVM do), not
        //    silently truncated to its low 16 bits.
        #[test]
        fn movk_rejects_out_of_range_immediate(
            rd in 0u32..=30,
            imm in 0x10000i64..=0xFFFFFFFF,
        ) {
            let ops = vec![xreg(rd), Operand::Imm(imm)];
            prop_assert!(encode_movk(&ops).is_err());
        }

        // 5. NEGATIVE CONTRACT: MOVK only permits `lsl #{0,16,32,48}`. Any
        //    other shift amount (or a non-lsl shift kind) is UNDEFINED and must
        //    be rejected rather than normalized via integer division.
        #[test]
        fn movk_rejects_non_multiple_of_16_shift(
            rd in 0u32..=30,
            hw in 0u32..=3,
            rem in 1u32..=15,
        ) {
            let amount = hw * 16 + rem; // not a multiple of 16
            let ops = vec![xreg(rd), Operand::Imm(1),
                           Operand::Shift { kind: "lsl".into(), amount }];
            prop_assert!(encode_movk(&ops).is_err());
        }

        // 6. NEGATIVE CONTRACT: for 32-bit MOVK (Wd), only hw in {0,1} is valid;
        //    lsl #32 / lsl #48 are UNDEFINED for W registers and must be Err.
        #[test]
        fn movk_w_reg_rejects_32_or_48_shift(
            rd in 0u32..=30,
            bad_amount in 32u32..=48u32,
        ) {
            prop_assume!(bad_amount == 32 || bad_amount == 48);
            let ops = vec![Operand::Reg(format!("w{}", rd)), Operand::Imm(1),
                           Operand::Shift { kind: "lsl".into(), amount: bad_amount }];
            prop_assert!(encode_movk(&ops).is_err());
        }
    }

    // ── MOVN (wide immediate, NOT) ────────────────────────────────────────
    // ARMv8 MOVN: sf 00 100101 hw imm16 Rd. opc=00 distinguishes MOVN from
    // MOVZ (opc=10) and MOVK (opc=11). The extractors opc_of/opcode6_of/
    // hw_of/imm16_of/rd_of/sf_of from the MOVZ section above are reused.

    proptest! {
        // 1. MOVN Xd/Wd, #imm with an in-range (0..=0xFFFF) immediate: every fixed
        //    and variable field lands exactly where the ARMv8 spec dictates, and
        //    opc is 00 (MOVN), distinct from MOVZ's 10 and MOVK's 11.
        #[test]
        fn movn_field_placement(
            rd in 0u32..=30,
            imm in 0i64..=0xFFFF,
            is_64 in any::<bool>(),
        ) {
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let ops = vec![rd_op, Operand::Imm(imm)];
            let w = expect_word(encode_movn(&ops));
            prop_assert_eq!(sf_of(w), if is_64 { 1 } else { 0 });
            prop_assert_eq!(opc_of(w), 0b00);          // MOVN opc
            prop_assert_eq!(opcode6_of(w), 0b100101);  // fixed wide-immediate op
            prop_assert_eq!(hw_of(w), 0);              // no shift
            prop_assert_eq!(imm16_of(w), imm as u32);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 2. The opc field (bits 30:29) is always 00 and is independent of
        //    register width, shift, and immediate magnitude — it must never read
        //    as MOVZ (10) or MOVK (11).
        #[test]
        fn movn_opc_is_always_00(
            rd in 0u32..=30,
            imm in 0i64..=0xFFFF,
            hw in 0u32..=3,
        ) {
            let amount = hw * 16;
            let ops = vec![xreg(rd), Operand::Imm(imm),
                           Operand::Shift { kind: "lsl".into(), amount }];
            let w = expect_word(encode_movn(&ops));
            prop_assert_eq!(opc_of(w), 0b00);
            prop_assert_eq!(opcode6_of(w), 0b100101);
        }

        // 3. `lsl #N` selects hw = N/16 for valid multiples of 16 (0/16/32/48).
        #[test]
        fn movn_hw_tracks_lsl_shift_amount(
            rd in 0u32..=30,
            imm in 0i64..=0xFFFF,
            hw in 0u32..=3, // 64-bit MOVN permits hw 0..3
        ) {
            let amount = hw * 16;
            let ops = vec![xreg(rd), Operand::Imm(imm),
                           Operand::Shift { kind: "lsl".into(), amount }];
            let w = expect_word(encode_movn(&ops));
            prop_assert_eq!(hw_of(w), hw);
            prop_assert_eq!(imm16_of(w), imm as u32);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 4. NEGATIVE CONTRACT: ARMv8 MOVN imm16 is a 16-bit *unsigned* field.
        //    An immediate outside [0, 0xFFFF] cannot be represented in a single
        //    MOVN and MUST be rejected by the assembler (as GAS/LLVM do), not
        //    silently truncated to its low 16 bits.
        #[test]
        fn movn_rejects_out_of_range_immediate(
            rd in 0u32..=30,
            imm in 0x10000i64..=0xFFFFFFFF,
        ) {
            let ops = vec![xreg(rd), Operand::Imm(imm)];
            prop_assert!(encode_movn(&ops).is_err());
        }

        // 5. NEGATIVE CONTRACT: MOVN only permits `lsl #{0,16,32,48}`. Any
        //    other shift amount (or a non-lsl shift kind) is UNDEFINED and must
        //    be rejected rather than normalized via integer division.
        #[test]
        fn movn_rejects_non_multiple_of_16_shift(
            rd in 0u32..=30,
            hw in 0u32..=3,
            rem in 1u32..=15,
        ) {
            let amount = hw * 16 + rem; // not a multiple of 16
            let ops = vec![xreg(rd), Operand::Imm(1),
                           Operand::Shift { kind: "lsl".into(), amount }];
            prop_assert!(encode_movn(&ops).is_err());
        }

        // 6. NEGATIVE CONTRACT: for 32-bit MOVN (Wd), only hw in {0,1} is valid;
        //    lsl #32 / lsl #48 are UNDEFINED for W registers and must be Err.
        #[test]
        fn movn_w_reg_rejects_32_or_48_shift(
            rd in 0u32..=30,
            bad_amount in 32u32..=48u32,
        ) {
            prop_assume!(bad_amount == 32 || bad_amount == 48);
            let ops = vec![Operand::Reg(format!("w{}", rd)), Operand::Imm(1),
                           Operand::Shift { kind: "lsl".into(), amount: bad_amount }];
            prop_assert!(encode_movn(&ops).is_err());
        }
    }

    // ── encode_logical (AND/ORR/EOR/ANDS, scalar forms) ───────────────────
    // ARMv8 logical (shifted register): sf opc 01010 shift N Rm imm6 Rn Rd
    //   opc: 00=AND, 01=ORR, 10=EOR, 11=ANDS; N(bit21)=0 for these four.
    // ARMv8 logical (immediate):         sf opc 100100 N immr imms Rn Rd
    fn n21_of(w: u32) -> u32 { (w >> 21) & 1 }    // register-form N (bit 21)
    fn n22_of(w: u32) -> u32 { (w >> 22) & 1 }    // immediate-form N (bit 22)
    fn immr_of(w: u32) -> u32 { (w >> 16) & 0x3F }
    fn imms_of(w: u32) -> u32 { (w >> 10) & 0x3F }

    proptest! {
        // 1. AND/ORR/EOR/ANDS Xd, Xn, Xm (no shift): every fixed field and
        //    every register field lands exactly where the ARMv8 spec dictates,
        //    and N (bit 21) is 0 (distinct from ORN/EON/BIC which set N=1).
        #[test]
        fn logical_register_form_field_placement(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
            opc in 0u32..=3,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let w = expect_word(encode_logical(&ops, opc));
            prop_assert_eq!(sf_of(w), 1);
            prop_assert_eq!(opc_of(w), opc);
            prop_assert_eq!(opcode5_of(w), 0b01010);   // logical shifted register
            prop_assert_eq!(n21_of(w), 0);              // AND/ORR/EOR/ANDS: N=0
            prop_assert_eq!(shift_type_of(w), 0);
            prop_assert_eq!(shift_amt_of(w), 0);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 2. Shifted register: all four shift kinds map to the 2-bit shift
        //    field, and for X registers the imm6 amount (0..=63) is placed
        //    verbatim with no masking needed (the full legal range).
        #[test]
        fn logical_register_form_shift_mapping(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            sk in 0u32..=3u32,
            amount in 0u32..=63u32,
            opc in 0u32..=3,
        ) {
            let (kind, want) = match sk {
                0 => ("lsl", 0u32), 1 => ("lsr", 1u32),
                2 => ("asr", 2u32), _ => ("ror", 3u32),
            };
            let ops = vec![xreg(rd), xreg(rn), xreg(rm),
                           Operand::Shift { kind: kind.into(), amount }];
            let w = expect_word(encode_logical(&ops, opc));
            prop_assert_eq!(opcode5_of(w), 0b01010);
            prop_assert_eq!(shift_type_of(w), want);
            prop_assert_eq!(shift_amt_of(w), amount);
            prop_assert_eq!(n21_of(w), 0);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(opc_of(w), opc);
        }

        // 3. sf (bit 31) tracks register width: W -> 0, X -> 1.
        #[test]
        fn logical_sf_tracks_width(
            n in 0u32..=30, is_w in any::<bool>(),
        ) {
            let r = if is_w { Operand::Reg(format!("w{}", n)) } else { xreg(n) };
            let ops = vec![r.clone(), r.clone(), r];
            let w = expect_word(encode_logical(&ops, 1));
            prop_assert_eq!(sf_of(w), if is_w { 0 } else { 1 });
        }

        // 4. NEGATIVE CONTRACT: for the 32-bit (W) shifted-register form,
        //    imm6 must be 0..=31; a shift of 32..=63 is UNDEFINED (ARMv8 ARM)
        //    and MUST be rejected, not silently masked into the imm6 field.
        #[test]
        fn logical_w_reg_rejects_shift_above_31(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            amount in 32u32..=63u32,
            sk in 0u32..=3u32,
        ) {
            let kind = match sk { 0 => "lsl", 1 => "lsr", 2 => "asr", _ => "ror" };
            let ops = vec![Operand::Reg(format!("w{}", rd)),
                           Operand::Reg(format!("w{}", rn)),
                           Operand::Reg(format!("w{}", rm)),
                           Operand::Shift { kind: kind.into(), amount }];
            prop_assert!(encode_logical(&ops, 0).is_err());
        }

        // 4b. NEGATIVE CONTRACT (symmetric with the W case above): for the
        //     64-bit (X) shifted-register form the imm6 field is only 0..=63,
        //     so a shift amount of 64 and above is UNDEFINED (ARMv8 ARM) and
        //     MUST be rejected rather than silently masked via `& 0x3F`.
        //     (For X registers the full 0..=63 range is legal, so only >=64
        //     is out of range — the complement of the W-register property #4.)
        #[test]
        fn logical_x_reg_rejects_shift_above_63(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            amount in 64u32..=255u32,
            sk in 0u32..=3u32,
        ) {
            let kind = match sk { 0 => "lsl", 1 => "lsr", 2 => "asr", _ => "ror" };
            let ops = vec![xreg(rd), xreg(rn), xreg(rm),
                           Operand::Shift { kind: kind.into(), amount }];
            prop_assert!(encode_logical(&ops, 0).is_err());
        }

        // 5. Immediate form: for a valid bitmask immediate the fixed opcode
        //    100100 and opc land per spec, and the (N,immr,imms) chosen by the
        //    encoder ROUND-TRIP — decoded by an independent ARM-ARM reference
        //    (NOT encode_bitmask_imm) — back to the ORIGINAL value. A genuine
        //    reference oracle: a shared bug cannot mask a field-placement defect.
        #[test]
        fn logical_immediate_form_roundtrips(
            rd in 0u32..=30, rn in 0u32..=30,
            size_bits in 1u32..=6u32,        // element size = 1<<size_bits (2..64)
            ones_off in 0u32..=63u32,        // ones = 1 + (ones_off mod (size-1))
            rot_off in 0u32..=63u32,         // right-rotation within element
            is_64 in any::<bool>(),
            opc in 0u32..=3,
        ) {
            let size = 1u32 << size_bits;                 // 2,4,8,16,32,64
            prop_assume!(is_64 || size <= 32);            // 64-bit element needs X
            let width = if is_64 { 64 } else { 32 };
            let max_ones = (size - 1).max(1);             // ones in 1..=size-1
            let ones = 1 + (ones_off % max_ones);
            let rot = rot_off % size;
            // element = run of `ones` ones, rotated right by `rot` within size
            let welem = if ones == 64 { u64::MAX } else { (1u64 << ones) - 1 };
            let emask = if size == 64 { u64::MAX } else { (1u64 << size) - 1 };
            let elem = if rot == 0 {
                welem & emask
            } else {
                ((welem >> rot) | (welem << (size - rot))) & emask
            };
            // replicate the element across the register width
            let mut val: u64 = 0;
            let mut b = 0u32;
            while b < width { val |= elem << b; b += size; }
            let allones = if is_64 { u64::MAX } else { 0xFFFFFFFF };
            val &= allones;
            prop_assume!(val != 0 && val != allones);     // not all-0 / all-1

            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rn_op = if is_64 { xreg(rn) } else { Operand::Reg(format!("w{}", rn)) };
            let ops = vec![rd_op, rn_op, Operand::Imm(val as i64)];
            let w = expect_word(encode_logical(&ops, opc));
            prop_assert_eq!(sf_of(w), if is_64 { 1 } else { 0 });
            prop_assert_eq!(opc_of(w), opc);
            prop_assert_eq!(opcode6_of(w), 0b100100);     // logical immediate
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
            // independent reference decode of the encoder's chosen fields
            let decoded = decode_bitmask_ref(n22_of(w), immr_of(w), imms_of(w), is_64);
            prop_assert_eq!(decoded, val);
        }

        // 6. NEGATIVE CONTRACT: a value that is NOT a legal bitmask immediate
        //    (0, or all-ones for the width) MUST be rejected with Err, not
        //    silently emitted as a bogus bitmask encoding.
        #[test]
        fn logical_immediate_rejects_non_bitmask(
            rd in 0u32..=30, rn in 0u32..=30, is_64 in any::<bool>(),
            zero in any::<bool>(),
        ) {
            let allones = if is_64 { u64::MAX } else { 0xFFFFFFFF };
            let val: u64 = if zero { 0 } else { allones };
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rn_op = if is_64 { xreg(rn) } else { Operand::Reg(format!("w{}", rn)) };
            let ops = vec![rd_op, rn_op, Operand::Imm(val as i64)];
            prop_assert!(encode_logical(&ops, 0).is_err());
        }
    }

    /// Independent ARM-ARM reference decoder for an AArch64 logical bitmask
    /// immediate, used to round-trip the encoder's (N, immr, imms) fields.
    /// Reimplemented from the architecture pseudocode (DecodeBitMasks), NOT from
    /// encode_bitmask_imm, so a bug shared with the encoder cannot hide itself.
    fn decode_bitmask_ref(n: u32, immr: u32, imms: u32, is_64: bool) -> u64 {
        let len = if n == 1 {
            6u32
        } else {
            let ctmp = (!imms) & 0x3F;                 // imms XOR 0b111111
            if ctmp == 0 { return 0; }                 // reserved encoding
            31 - ctmp.leading_zeros()                  // HighestSetBit(ctmp), 0..5
        };
        let esize: u32 = 1u32 << len;                  // element size 2..64
        // S/R are the low `len` bits of imms/immr (ARM ARM DecodeBitMasks).
        let levels = (1u64 << len) - 1;                 // len-bit mask (len <= 6)
        let s = (imms as u64) & levels;
        let r = (immr as u64) & levels;
        let welem = if s >= 63 { u64::MAX } else { (1u64 << (s + 1)) - 1 };
        let emask = if esize == 64 { u64::MAX } else { (1u64 << esize) - 1 };
        let elem = if r == 0 {
            welem & emask
        } else {
            ((welem >> r) | (welem << (esize - r as u32))) & emask
        };
        let width = if is_64 { 64 } else { 32 };
        let mut result: u64 = 0;
        let mut b = 0u32;
            while b < width { result |= elem << b; b += esize; }
        result &= if is_64 { u64::MAX } else { 0xFFFFFFFF };
        result
    }

    // ── encode_shift (LSL/LSR/ASR/ROR; immediate + register forms) ────────
    // ARMv8 immediate form (alias encodings):
    //   LSL #imm -> UBFM: sf 10 100110 N immr imms Rn Rd  (immr=(-imm)%w, imms=w-1-imm)
    //   LSR #imm -> UBFM: sf 10 100110 N immr imms Rn Rd  (immr=imm, imms=w-1)
    //   ASR #imm -> SBFM: sf 00 100110 N immr imms Rn Rd  (immr=imm, imms=w-1)
    //   ROR #imm -> EXTR: sf 00 100111 N 0 Rm imms Rn Rd  (Rm=Rn, imms=imm)
    // ARMv8 register form (data-processing, 2 sources):
    //   sf 0 S=0 11010110 Rm opcode Rn Rd   where opcode = 0010(op2), op2 = shift_type
    fn opcode2src_of(w: u32) -> u32 { (w >> 10) & 0x3F } // bits 15:10

    proptest! {
        // 1. Immediate BFM form (LSL/LSR/ASR): for an in-range shift amount every
        //    fixed field (sf, opc, 100110, N) and every variable field (immr,
        //    imms, Rn, Rd) lands exactly where the ARMv8 ARM dictates.
        #[test]
        fn shift_immediate_bfm_field_placement(
            rd in 0u32..=30,
            rn in 0u32..=30,
            st in 0u32..=2u32,        // 0=lsl, 1=lsr, 2=asr
            imm in 0u32..=63u32,
            is_64 in any::<bool>(),
        ) {
            let width = if is_64 { 64 } else { 32 };
            prop_assume!(imm < width);
            // lsr/asr #0 is the lsl-only MOV alias boundary; keep imm>=1 for them.
            if st != 0 { prop_assume!(imm >= 1); }
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rn_op = if is_64 { xreg(rn) } else { Operand::Reg(format!("w{}", rn)) };
            let ops = vec![rd_op, rn_op, Operand::Imm(imm as i64)];
            let w = expect_word(encode_shift(&ops, st));
            prop_assert_eq!(sf_of(w), if is_64 { 1 } else { 0 });
            prop_assert_eq!(opcode6_of(w), 0b100110);     // BFM fixed op (bits 28:23)
            prop_assert_eq!(n22_of(w), if is_64 { 1 } else { 0 });
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
            match st {
                0 => { // LSL -> UBFM (opc=10)
                    prop_assert_eq!(opc_of(w), 0b10);
                    prop_assert_eq!(immr_of(w), (width - imm) % width);
                    prop_assert_eq!(imms_of(w), width - 1 - imm);
                }
                1 => { // LSR -> UBFM (opc=10)
                    prop_assert_eq!(opc_of(w), 0b10);
                    prop_assert_eq!(immr_of(w), imm);
                    prop_assert_eq!(imms_of(w), width - 1);
                }
                _ => { // ASR -> SBFM (opc=00)
                    prop_assert_eq!(opc_of(w), 0b00);
                    prop_assert_eq!(immr_of(w), imm);
                    prop_assert_eq!(imms_of(w), width - 1);
                }
            }
        }

        // 2. ROR immediate form -> EXTR: every fixed field (sf, opc=00, 100111,
        //    N, bit21=0) and variable field (Rm==Rn, imms=imm, Rn, Rd) is placed
        //    per the ARMv8 ARM for in-range rotation amounts.
        #[test]
        fn shift_immediate_ror_extr_field_placement(
            rd in 0u32..=30,
            rn in 0u32..=30,
            imm in 1u32..=63u32,
            is_64 in any::<bool>(),
        ) {
            let width = if is_64 { 64 } else { 32 };
            prop_assume!(imm < width);
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rn_op = if is_64 { xreg(rn) } else { Operand::Reg(format!("w{}", rn)) };
            let ops = vec![rd_op, rn_op, Operand::Imm(imm as i64)];
            let w = expect_word(encode_shift(&ops, 0b11));
            // EXTR: sf 00 100111 N 0 Rm imms Rn Rd
            prop_assert_eq!(sf_of(w), if is_64 { 1 } else { 0 });
            prop_assert_eq!(opc_of(w), 0b00);
            prop_assert_eq!(opcode6_of(w), 0b100111);     // EXTR fixed op (bits 28:23)
            prop_assert_eq!(n22_of(w), if is_64 { 1 } else { 0 });
            prop_assert_eq!(n21_of(w), 0);                // bit 21 = 0
            prop_assert_eq!(rm_of(w), rn);                // Rm == Rn for ROR
            prop_assert_eq!(imms_of(w), imm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 3. Register form (data-processing, 2 sources): every fixed field
        //    (sf, bits30:29=0S, 11010, bit21=0) and variable field (Rm, opcode,
        //    Rn, Rd) is placed per the ARMv8 ARM for all four shift kinds.
        #[test]
        fn shift_register_form_field_placement(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
            st in 0u32..=3u32,       // 0=lsl,1=lsr,2=asr,3=ror
            is_64 in any::<bool>(),
        ) {
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rn_op = if is_64 { xreg(rn) } else { Operand::Reg(format!("w{}", rn)) };
            let rm_op = if is_64 { xreg(rm) } else { Operand::Reg(format!("w{}", rm)) };
            let ops = vec![rd_op, rn_op, rm_op];
            let w = expect_word(encode_shift(&ops, st));
            // sf 0 S=0 11010110 Rm opcode Rn Rd
            prop_assert_eq!(sf_of(w), if is_64 { 1 } else { 0 });
            prop_assert_eq!(opc_of(w), 0b00);            // bits 30:29 = 0S, S=0
            prop_assert_eq!(opcode5_of(w), 0b11010);     // fixed op (bits 28:24)
            prop_assert_eq!(n21_of(w), 0);               // bit 21 = 0
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(opcode2src_of(w), 0b0010_00 | st); // opcode = 0010(op2)
        }

        // 4. NEGATIVE CONTRACT: the immediate shift amount has a finite legal
        //    range (0..width-1 for LSL, 1..width-1 for ROR; ARMv8 ARM). A shift
        //    amount strictly greater than the maximum, or negative, is UNDEFINED
        //    and MUST be rejected with Err (as GAS/LLVM do) — never silently
        //    wrapped via `*imm as u32` + modular arithmetic into a bogus word.
        #[test]
        fn shift_immediate_rejects_out_of_range(
            rd in 0u32..=30,
            rn in 0u32..=30,
            st in 0u32..=3u32,
            is_64 in any::<bool>(),
            over in 1u32..=200u32,
            neg in any::<bool>(),
        ) {
            let width = if is_64 { 64 } else { 32 };
            let imm: i64 = if neg {
                -(over as i64)
            } else {
                (width + over) as i64   // strictly above width-1 for every kind
            };
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rn_op = if is_64 { xreg(rn) } else { Operand::Reg(format!("w{}", rn)) };
            let ops = vec![rd_op, rn_op, Operand::Imm(imm)];
            prop_assert!(
                encode_shift(&ops, st).is_err(),
                "expected Err for out-of-range immediate shift {} (width={}, st={})",
                imm, width, st
            );
        }
    }

    // ── encode_shift: full-word differential oracle + range contract ──────
    // These properties re-derive the *entire* 32-bit instruction word straight
    // from the ARMv8 ARM layout (not field-by-field), giving a holistic oracle
    // that would flag any stray bit. They complement the per-field checks above.
    //
    // Reference layouts (ARMv8 ARM C4.1):
    //   LSL/LSR #imm -> UBFM : sf 10 100110 N immr imms Rn Rd
    //   ASR #imm     -> SBFM : sf 00 100110 N immr imms Rn Rd
    //   ROR #imm     -> EXTR : sf 00 100111 N 0 Rm imms Rn Rd   (Rm==Rn)
    //   <shift>  Rm        : sf 0 0 11010110 Rm 0010(op2) Rn Rd
    proptest! {
        // 5a. Full-word differential: for every in-range immediate amount and
        //     every LSL/LSR/ASR kind, the produced word is byte-identical to the
        //     ARM ARM reference assembly. Holistic: catches misplaced or stray bits.
        #[test]
        fn shift_immediate_full_word_matches_arm_reference(
            rd in 0u32..=30,
            rn in 0u32..=30,
            st in 0u32..=2u32,          // 0=lsl, 1=lsr, 2=asr
            imm in 0u32..=63u32,
            is_64 in any::<bool>(),
        ) {
            let width = if is_64 { 64u32 } else { 32u32 };
            let sf = if is_64 { 1u32 } else { 0u32 };
            let n = sf;
            prop_assume!(imm < width);
            if st != 0 { prop_assume!(imm >= 1); }   // lsr/asr #0 is the MOV alias

            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rn_op = if is_64 { xreg(rn) } else { Operand::Reg(format!("w{}", rn)) };
            let ops = vec![rd_op, rn_op, Operand::Imm(imm as i64)];
            let w = expect_word(encode_shift(&ops, st));

            let (immr, imms, opc) = match st {
                0 => ((width - imm) % width, width - 1 - imm, 0b10u32), // LSL -> UBFM
                1 => (imm, width - 1, 0b10u32),                         // LSR -> UBFM
                _ => (imm, width - 1, 0b00u32),                         // ASR -> SBFM
            };
            let expect = (sf << 31) | (opc << 29) | (0b100110 << 23) | (n << 22)
                       | (immr << 16) | (imms << 10) | (rn << 5) | rd;
            prop_assert_eq!(w, expect);
        }

        // 5b. ROR #imm full-word differential: EXTR with Rm==Rn.
        #[test]
        fn shift_ror_immediate_full_word_matches_arm_reference(
            rd in 0u32..=30,
            rn in 0u32..=30,
            imm in 1u32..=63u32,
            is_64 in any::<bool>(),
        ) {
            let width = if is_64 { 64u32 } else { 32u32 };
            prop_assume!(imm < width);
            let sf = if is_64 { 1u32 } else { 0u32 };
            let n = sf;
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rn_op = if is_64 { xreg(rn) } else { Operand::Reg(format!("w{}", rn)) };
            let ops = vec![rd_op, rn_op, Operand::Imm(imm as i64)];
            let w = expect_word(encode_shift(&ops, 0b11));
            // EXTR: sf 00 100111 N 0 Rm imms Rn Rd  (Rm==Rn, imms==imm)
            let expect = (sf << 31) | (0b00100111 << 23) | (n << 22)
                       | (rn << 16) | (imm << 10) | (rn << 5) | rd;
            prop_assert_eq!(w, expect);
        }

        // 5c. Register form full-word differential: data-processing (2 source).
        #[test]
        fn shift_register_full_word_matches_arm_reference(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
            st in 0u32..=3u32,         // 0=lsl,1=lsr,2=asr,3=ror
            is_64 in any::<bool>(),
        ) {
            let sf = if is_64 { 1u32 } else { 0u32 };
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rn_op = if is_64 { xreg(rn) } else { Operand::Reg(format!("w{}", rn)) };
            let rm_op = if is_64 { xreg(rm) } else { Operand::Reg(format!("w{}", rm)) };
            let ops = vec![rd_op, rn_op, rm_op];
            let w = expect_word(encode_shift(&ops, st));
            // sf 0 0 11010110 Rm 0010(op2) Rn Rd
            let expect = (sf << 31) | (0b0011010110 << 21) | (rm << 16)
                       | (0b0010 << 12) | (st << 10) | (rn << 5) | rd;
            prop_assert_eq!(w, expect);
        }

        // 5d. Positive range contract: every *legal* immediate amount yields Ok,
        //     and the encoded immr/imms fields never exceed width-1 (they index a
        //     [0,width) rotation/width space). Establishes the valid frontier.
        #[test]
        fn shift_immediate_in_range_is_ok_and_fields_bounded(
            rd in 0u32..=30,
            rn in 0u32..=30,
            st in 0u32..=3u32,
            imm in 0u32..=63u32,
            is_64 in any::<bool>(),
        ) {
            let width = if is_64 { 64u32 } else { 32u32 };
            // Legal immediate ranges per ARMv8 ARM:
            //   LSL: 0..=width-1 ; LSR/ASR/ROR: 1..=width-1
            let lo = if st == 0 { 0u32 } else { 1u32 };
            prop_assume!((lo..width).contains(&imm));
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rn_op = if is_64 { xreg(rn) } else { Operand::Reg(format!("w{}", rn)) };
            let ops = vec![rd_op, rn_op, Operand::Imm(imm as i64)];
            let w = expect_word(encode_shift(&ops, st));
            prop_assert!(immr_of(w) < width, "immr {} >= width {}", immr_of(w), width);
            prop_assert!(imms_of(w) < width, "imms {} >= width {}", imms_of(w), width);
        }

        // 5e. NEGATIVE CONTRACT (panic-distinguishing): an immediate shift
        //     amount at or above the width boundary, or negative, is UNDEFINED
        //     per the ARMv8 ARM and MUST be rejected with Err — the way GAS and
        //     LLVM-MC do. This variant uses catch_unwind so we can tell apart
        //     the three failure modes (Err = correct, panic = debug-underflow,
        //     Ok = silent bogus encoding) instead of just observing a crash.
        #[test]
        fn shift_immediate_out_of_range_never_returns_ok(
            st in 0u32..=3u32,
            is_64 in any::<bool>(),
            over in 0u32..=64u32,       // over==0 -> imm==width (exact boundary)
            neg in any::<bool>(),
        ) {
            let width = if is_64 { 64u32 } else { 32u32 };
            let imm: i64 = if neg { -((over + 1) as i64) } else { (width + over) as i64 };
            let rd_op = if is_64 { xreg(0) } else { Operand::Reg("w0".into()) };
            let rn_op = if is_64 { xreg(1) } else { Operand::Reg("w1".into()) };
            let ops = vec![rd_op, rn_op, Operand::Imm(imm)];
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                encode_shift(&ops, st)
            }));
            match caught {
                Ok(Err(_)) => { /* correct: rejected */ }
                Ok(Ok(_)) => return Err(proptest::test_runner::TestCaseError::fail(format!(
                    "out-of-range imm={} (width={}, st={}) was SILENTLY ACCEPTED as Ok",
                    imm, width, st))),
                Err(_) => return Err(proptest::test_runner::TestCaseError::fail(format!(
                    "out-of-range imm={} (width={}, st={}) PANICKED instead of returning Err",
                    imm, width, st))),
            }
        }
    }

    // ── encode_mvn (MVN = ORN Rd, XZR, Rm) ────────────────────────────────
    // ARMv8 logical (shifted register): sf opc 01010 shift N Rm imm6 Rn Rd
    // MVN aliases ORN with Rn hardwired to XZR (11111) and N (bit 21) = 1.
    //   opc = 01 (ORN), fixed op (bits 28:24) = 01010, Rn field = 11111.
    // Reuses sf_of/opc_of/opcode5_of/shift_type_of/shift_amt_of/rm_of/rn_of/
    // rd_of/n21_of/expect_word/xreg from the sections above.

    proptest! {
        // 1. MVN Xd/Wd, Xm/Wm (no shift): every fixed field and every register
        //    field lands exactly where the ARMv8 spec dictates. Because MVN
        //    aliases ORN Rd, XZR, Rm, Rn (bits 9:5) must be hardwired to 11111
        //    and N (bit 21) must be 1 (distinguishing ORN from ORR).
        #[test]
        fn mvn_field_placement(
            rd in 0u32..=30,
            rm in 0u32..=30,
            is_64 in any::<bool>(),
        ) {
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rm_op = if is_64 { xreg(rm) } else { Operand::Reg(format!("w{}", rm)) };
            let ops = vec![rd_op, rm_op];
            let w = expect_word(encode_mvn(&ops));
            prop_assert_eq!(sf_of(w), if is_64 { 1 } else { 0 });
            prop_assert_eq!(opc_of(w), 0b01);          // ORN opc
            prop_assert_eq!(opcode5_of(w), 0b01010);   // logical shifted register
            prop_assert_eq!(n21_of(w), 1);             // N=1 (ORN, not ORR)
            prop_assert_eq!(rn_of(w), 0b11111);        // Rn hardwired to XZR
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(shift_type_of(w), 0);
            prop_assert_eq!(shift_amt_of(w), 0);
        }

        // 2. sf (bit 31) tracks register width: W -> 0, X -> 1.
        #[test]
        fn mvn_sf_tracks_width(
            n in 0u32..=30,
            is_w in any::<bool>(),
        ) {
            let r = if is_w { Operand::Reg(format!("w{}", n)) } else { xreg(n) };
            let ops = vec![r.clone(), r];
            let w = expect_word(encode_mvn(&ops));
            prop_assert_eq!(sf_of(w), if is_w { 0 } else { 1 });
        }

        // 3. Shifted register: all four shift kinds map to the 2-bit shift field,
        //    and for X registers the imm6 amount (0..=63) is placed verbatim. The
        //    MVN alias signature (opc=01, N=1, Rn=11111) is preserved regardless
        //    of the shift operand.
        #[test]
        fn mvn_shift_mapping(
            rd in 0u32..=30, rm in 0u32..=30,
            sk in 0u32..=3u32,
            amount in 0u32..=63u32,
        ) {
            let (kind, want) = match sk {
                0 => ("lsl", 0u32), 1 => ("lsr", 1u32),
                2 => ("asr", 2u32), _ => ("ror", 3u32),
            };
            let ops = vec![xreg(rd), xreg(rm),
                           Operand::Shift { kind: kind.into(), amount }];
            let w = expect_word(encode_mvn(&ops));
            prop_assert_eq!(opcode5_of(w), 0b01010);
            prop_assert_eq!(shift_type_of(w), want);
            prop_assert_eq!(shift_amt_of(w), amount);
            prop_assert_eq!(n21_of(w), 1);
            prop_assert_eq!(rn_of(w), 0b11111);
            prop_assert_eq!(opc_of(w), 0b01);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 4. ALGEBRAIC ALIAS ORACLE: MVN is defined as ORN Rd, XZR, Rm. The
        //    encoding of `mvn Rd, Rm [, shift]` must therefore be bit-identical
        //    to `orn Rd, (XZR|WZR), Rm [, shift]`, for both widths and with or
        //    without a shift. This is the defining equivalence of the alias.
        #[test]
        fn mvn_equals_orn_rn_xzr(
            rd in 0u32..=30, rm in 0u32..=30,
            is_64 in any::<bool>(),
            shifted in any::<bool>(),
            sk in 0u32..=3u32,
            amount in 0u32..=63u32,
        ) {
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rm_op = if is_64 { xreg(rm) } else { Operand::Reg(format!("w{}", rm)) };
            let zr_op = if is_64 { Operand::Reg("xzr".into()) }
                        else { Operand::Reg("wzr".into()) };
            let shift = Operand::Shift {
                kind: match sk { 0 => "lsl", 1 => "lsr", 2 => "asr", _ => "ror" }.into(),
                amount,
            };
            let mvn_ops = if shifted { vec![rd_op.clone(), rm_op.clone(), shift.clone()] }
                          else { vec![rd_op.clone(), rm_op.clone()] };
            let orn_ops = if shifted { vec![rd_op, zr_op, rm_op, shift] }
                          else { vec![rd_op, zr_op, rm_op] };
            let w_mvn = expect_word(encode_mvn(&mvn_ops));
            let w_orn = expect_word(encode_orn(&orn_ops));
            prop_assert_eq!(w_mvn, w_orn);
        }

        // 5. NEGATIVE CONTRACT: for the 32-bit (W) shifted-register form, imm6
        //    must be 0..=31; a shift of 32..63 is UNDEFINED (ARMv8 ARM, C4.1.4:
        //    for sf=0 the shift amount must be 0..31) and MUST be rejected, not
        //    silently masked into the imm6 field via `& 0x3F`.
        #[test]
        fn mvn_w_reg_rejects_shift_above_31(
            rd in 0u32..=30,
            rm in 0u32..=30,
            amount in 32u32..=63u32,
            sk in 0u32..=3u32,
        ) {
            let kind = match sk { 0 => "lsl", 1 => "lsr", 2 => "asr", _ => "ror" };
            let ops = vec![Operand::Reg(format!("w{}", rd)),
                           Operand::Reg(format!("w{}", rm)),
                           Operand::Shift { kind: kind.into(), amount }];
            prop_assert!(encode_mvn(&ops).is_err());
        }
    }

    // ── encode_neg (NEG = SUB Rd, XZR, Rm) ────────────────────────────────
    // ARMv8 add/sub (shifted register): sf op S 01011 shift Rm imm6 Rn Rd
    // NEG aliases SUB with Rn hardwired to XZR (11111), op=1 (sub), S=0.
    //   op (bit 30) = 1, S (bit 29) = 0, fixed op (bits 28:24) = 01011,
    //   bit 21 = 0 (shifted register, not extended), Rn (bits 9:5) = 11111.
    // Reuses sf_of/op_of/s_of/opcode5_of/shift_type_of/shift_amt_of/
    // ext21_of/rm_of/rn_of/rd_of/expect_word/xreg from the sections above.

    proptest! {
        // 1. NEG Xd/Wd, Xm/Wm (no shift): every fixed field and every register
        //    field lands exactly where the ARMv8 spec dictates. Because NEG
        //    aliases SUB Rd, XZR, Rm, Rn (bits 9:5) must be hardwired to 11111,
        //    op (bit 30) must be 1 (subtraction), S (bit 29) must be 0 (no
        //    flags — that is NEGS), and bit 21 must be 0 (shifted register).
        #[test]
        fn neg_field_placement(
            rd in 0u32..=30,
            rm in 0u32..=30,
            is_64 in any::<bool>(),
        ) {
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rm_op = if is_64 { xreg(rm) } else { Operand::Reg(format!("w{}", rm)) };
            let ops = vec![rd_op, rm_op];
            let w = expect_word(encode_neg(&ops));
            prop_assert_eq!(sf_of(w), if is_64 { 1 } else { 0 });
            prop_assert_eq!(op_of(w), 1);            // SUB (subtraction)
            prop_assert_eq!(s_of(w), 0);             // no flags (NEG, not NEGS)
            prop_assert_eq!(opcode5_of(w), 0b01011); // add/sub shifted register
            prop_assert_eq!(ext21_of(w), 0);         // shifted register, not extended
            prop_assert_eq!(rn_of(w), 0b11111);      // Rn hardwired to XZR
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(shift_type_of(w), 0);
            prop_assert_eq!(shift_amt_of(w), 0);
        }

        // 2. sf (bit 31) tracks register width: W -> 0, X -> 1.
        #[test]
        fn neg_sf_tracks_width(
            n in 0u32..=30,
            is_w in any::<bool>(),
        ) {
            let r = if is_w { Operand::Reg(format!("w{}", n)) } else { xreg(n) };
            let ops = vec![r.clone(), r];
            let w = expect_word(encode_neg(&ops));
            prop_assert_eq!(sf_of(w), if is_w { 0 } else { 1 });
        }

        // 3. Shifted register: the three legal shift kinds (lsl/lsr/asr) map to
        //    the 2-bit shift field, and for X registers the imm6 amount (0..=63)
        //    is placed verbatim. The NEG alias signature (op=1, S=0, Rn=11111)
        //    is preserved regardless of the shift operand.
        #[test]
        fn neg_shift_mapping(
            rd in 0u32..=30, rm in 0u32..=30,
            sk in 0u32..=2u32,            // 0=lsl, 1=lsr, 2=asr (ROR invalid for ADD/SUB)
            amount in 0u32..=63u32,
        ) {
            let (kind, want) = match sk {
                0 => ("lsl", 0u32), 1 => ("lsr", 1u32), _ => ("asr", 2u32),
            };
            let ops = vec![xreg(rd), xreg(rm),
                           Operand::Shift { kind: kind.into(), amount }];
            let w = expect_word(encode_neg(&ops));
            prop_assert_eq!(opcode5_of(w), 0b01011);
            prop_assert_eq!(shift_type_of(w), want);
            prop_assert_eq!(shift_amt_of(w), amount);
            prop_assert_eq!(op_of(w), 1);
            prop_assert_eq!(s_of(w), 0);
            prop_assert_eq!(rn_of(w), 0b11111);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 4. ALGEBRAIC ALIAS ORACLE: NEG is defined as SUB Rd, XZR, Rm. The
        //    encoding of `neg Rd, Rm [, shift]` must therefore be bit-identical
        //    to `sub Rd, (XZR|WZR), Rm [, shift]`, for both widths and with or
        //    without a shift. This is the defining equivalence of the alias.
        #[test]
        fn neg_equals_sub_rn_xzr(
            rd in 0u32..=30, rm in 0u32..=30,
            is_64 in any::<bool>(),
            shifted in any::<bool>(),
            sk in 0u32..=2u32,
            amount in 0u32..=63u32,
        ) {
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rm_op = if is_64 { xreg(rm) } else { Operand::Reg(format!("w{}", rm)) };
            let zr_op = if is_64 { Operand::Reg("xzr".into()) }
                        else { Operand::Reg("wzr".into()) };
            let shift = Operand::Shift {
                kind: match sk { 0 => "lsl", 1 => "lsr", _ => "asr" }.into(),
                amount,
            };
            let neg_ops = if shifted { vec![rd_op.clone(), rm_op.clone(), shift.clone()] }
                          else { vec![rd_op.clone(), rm_op.clone()] };
            let sub_ops = if shifted { vec![rd_op, zr_op, rm_op, shift] }
                          else { vec![rd_op, zr_op, rm_op] };
            let w_neg = expect_word(encode_neg(&neg_ops));
            let w_sub = expect_word(encode_add_sub(&sub_ops, true, false));
            prop_assert_eq!(w_neg, w_sub);
        }

        // 5. NEGATIVE CONTRACT: for the 32-bit (W) shifted-register form, imm6
        //    must be 0..=31; a shift of 32..63 is UNDEFINED (ARMv8 ARM, C4.1.4:
        //    for sf=0 the shift amount must be 0..31) and MUST be rejected, not
        //    silently masked into the imm6 field via `& 0x3F`.
        #[test]
        fn neg_w_reg_rejects_shift_above_31(
            rd in 0u32..=30,
            rm in 0u32..=30,
            amount in 32u32..=63u32,
            sk in 0u32..=2u32,
        ) {
            let kind = match sk { 0 => "lsl", 1 => "lsr", _ => "asr" };
            let ops = vec![Operand::Reg(format!("w{}", rd)),
                           Operand::Reg(format!("w{}", rm)),
                           Operand::Shift { kind: kind.into(), amount }];
            prop_assert!(encode_neg(&ops).is_err());
        }

        // 6. NEGATIVE CONTRACT: ADD/SUB shifted register only permits LSL/LSR/ASR
        //    (ROR is reserved for logical ops; ARMv8 ARM C4.1.66). A `ror` shift
        //    operand to NEG MUST be rejected with Err, not silently re-encoded
        //    as LSL via the default arm of the `match kind.as_str()`.
        #[test]
        fn neg_rejects_ror_shift(
            rd in 0u32..=30,
            rm in 0u32..=30,
            amount in 0u32..=63u32,
            is_64 in any::<bool>(),
        ) {
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rm_op = if is_64 { xreg(rm) } else { Operand::Reg(format!("w{}", rm)) };
            let ops = vec![rd_op, rm_op,
                           Operand::Shift { kind: "ror".into(), amount }];
            prop_assert!(encode_neg(&ops).is_err());
        }
    }

    // ── encode_negs (NEGS = SUBS Rd, XZR, Rm) ───────────────────────────
    // ARMv8 add/sub (shifted register): sf op S 01011 shift Rm imm6 Rn Rd
    // NEGS aliases SUBS with Rn hardwired to XZR (11111), op=1 (sub), S=1
    // (flags set). The ONLY field that differs from NEG is S (bit 29): NEGS
    // sets it, NEG clears it.
    //   op (bit 30) = 1, S (bit 29) = 1, fixed op (bits 28:24) = 01011,
    //   bit 21 = 0 (shifted register, not extended), Rn (bits 9:5) = 11111.
    // Reuses sf_of/op_of/s_of/opcode5_of/shift_type_of/shift_amt_of/
    // ext21_of/rm_of/rn_of/rd_of/expect_word/xreg from the sections above.

    proptest! {
        // 1. NEGS Xd/Wd, Xm/Wm (no shift): every fixed field and every register
        //    field lands exactly where the ARMv8 spec dictates. Because NEGS
        //    aliases SUBS Rd, XZR, Rm, Rn (bits 9:5) must be hardwired to 11111,
        //    op (bit 30) must be 1 (subtraction), S (bit 29) must be 1 (flags
        //    set — this is what distinguishes NEGS from NEG), bit 21 must be 0
        //    (shifted register), and sf (bit 31) must track the register width.
        #[test]
        fn negs_field_placement(
            rd in 0u32..=30,
            rm in 0u32..=30,
            is_64 in any::<bool>(),
        ) {
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rm_op = if is_64 { xreg(rm) } else { Operand::Reg(format!("w{}", rm)) };
            let ops = vec![rd_op, rm_op];
            let w = expect_word(encode_negs(&ops));
            prop_assert_eq!(sf_of(w), if is_64 { 1 } else { 0 });
            prop_assert_eq!(op_of(w), 1);            // SUB (subtraction)
            prop_assert_eq!(s_of(w), 1);             // flags set (NEGS, not NEG)
            prop_assert_eq!(opcode5_of(w), 0b01011); // add/sub shifted register
            prop_assert_eq!(ext21_of(w), 0);         // shifted register, not extended
            prop_assert_eq!(rn_of(w), 0b11111);      // Rn hardwired to XZR
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(shift_type_of(w), 0);
            prop_assert_eq!(shift_amt_of(w), 0);
        }

        // 2. NEGS vs NEG differ only in the S bit: the same operands must
        //    produce encodings that are identical except S (bit 29) = 1 for
        //    NEGS and 0 for NEG. This is the defining relationship between the
        //    two aliases (both alias SUB/SUBS Rd, XZR, Rm).
        #[test]
        fn negs_vs_neg_differs_only_in_s_bit(
            rd in 0u32..=30, rm in 0u32..=30,
            is_64 in any::<bool>(),
            sk in 0u32..=2u32, amount in 0u32..=63u32,
        ) {
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rm_op = if is_64 { xreg(rm) } else { Operand::Reg(format!("w{}", rm)) };
            let shift = Operand::Shift {
                kind: match sk { 0 => "lsl", 1 => "lsr", _ => "asr" }.into(),
                amount,
            };
            let ops = vec![rd_op, rm_op, shift];
            let w_s = expect_word(encode_negs(&ops));
            let w = expect_word(encode_neg(&ops));
            prop_assert_eq!(w_s & !(1u32 << 29), w & !(1u32 << 29)); // identical except S
            prop_assert_eq!(s_of(w_s), 1);
            prop_assert_eq!(s_of(w), 0);
        }

        // 3. Shifted register: the three legal shift kinds (lsl/lsr/asr) map to
        //    the 2-bit shift field, and for X registers the imm6 amount (0..=63)
        //    is placed verbatim. The NEGS alias signature (op=1, S=1, Rn=11111)
        //    is preserved regardless of the shift operand.
        #[test]
        fn negs_shift_mapping(
            rd in 0u32..=30, rm in 0u32..=30,
            sk in 0u32..=2u32,            // 0=lsl, 1=lsr, 2=asr (ROR invalid for ADD/SUB)
            amount in 0u32..=63u32,
        ) {
            let (kind, want) = match sk {
                0 => ("lsl", 0u32), 1 => ("lsr", 1u32), _ => ("asr", 2u32),
            };
            let ops = vec![xreg(rd), xreg(rm),
                           Operand::Shift { kind: kind.into(), amount }];
            let w = expect_word(encode_negs(&ops));
            prop_assert_eq!(opcode5_of(w), 0b01011);
            prop_assert_eq!(shift_type_of(w), want);
            prop_assert_eq!(shift_amt_of(w), amount);
            prop_assert_eq!(op_of(w), 1);
            prop_assert_eq!(s_of(w), 1);
            prop_assert_eq!(rn_of(w), 0b11111);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 4. ALGEBRAIC ALIAS ORACLE: NEGS is defined as SUBS Rd, XZR, Rm. The
        //    encoding of `negs Rd, Rm [, shift]` must therefore be bit-identical
        //    to `subs Rd, (XZR|WZR), Rm [, shift]`, for both widths and with or
        //    without a shift. This is the defining equivalence of the alias.
        #[test]
        fn negs_equals_subs_rn_xzr(
            rd in 0u32..=30, rm in 0u32..=30,
            is_64 in any::<bool>(),
            shifted in any::<bool>(),
            sk in 0u32..=2u32, amount in 0u32..=63u32,
        ) {
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rm_op = if is_64 { xreg(rm) } else { Operand::Reg(format!("w{}", rm)) };
            let zr_op = if is_64 { Operand::Reg("xzr".into()) }
                        else { Operand::Reg("wzr".into()) };
            let shift = Operand::Shift {
                kind: match sk { 0 => "lsl", 1 => "lsr", _ => "asr" }.into(),
                amount,
            };
            let negs_ops = if shifted { vec![rd_op.clone(), rm_op.clone(), shift.clone()] }
                           else { vec![rd_op.clone(), rm_op.clone()] };
            let subs_ops = if shifted { vec![rd_op, zr_op, rm_op, shift] }
                           else { vec![rd_op, zr_op, rm_op] };
            let w_negs = expect_word(encode_negs(&negs_ops));
            let w_subs = expect_word(encode_add_sub(&subs_ops, true, true));
            prop_assert_eq!(w_negs, w_subs);
        }

        // 5. NEGATIVE CONTRACT: for the 32-bit (W) shifted-register form, imm6
        //    must be 0..=31; a shift of 32..63 is UNDEFINED (ARMv8 ARM, C4.1.4:
        //    for sf=0 the shift amount must be 0..=31) and MUST be rejected, not
        //    silently masked into the imm6 field via `& 0x3F`.
        #[test]
        fn negs_w_reg_rejects_shift_above_31(
            rd in 0u32..=30, rm in 0u32..=30,
            amount in 32u32..=63u32, sk in 0u32..=2u32,
        ) {
            let kind = match sk { 0 => "lsl", 1 => "lsr", _ => "asr" };
            let ops = vec![Operand::Reg(format!("w{}", rd)),
                           Operand::Reg(format!("w{}", rm)),
                           Operand::Shift { kind: kind.into(), amount }];
            prop_assert!(encode_negs(&ops).is_err());
        }
    }

    // ── encode_negs: additional negative contract (separate proptest block) ─
    proptest! {
        // 6. NEGATIVE CONTRACT: ADD/SUB shifted register only permits LSL/LSR/ASR
        //    (ROR is reserved for logical ops; ARMv8 ARM C4.1.66). A `ror` shift
        //    operand to NEGS MUST be rejected with Err, not silently re-encoded
        //    as LSL via the default arm of the `match kind.as_str()`.
        #[test]
        fn negs_rejects_ror_shift(
            rd in 0u32..=30, rm in 0u32..=30,
            amount in 0u32..=63u32, is_64 in any::<bool>(),
        ) {
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rm_op = if is_64 { xreg(rm) } else { Operand::Reg(format!("w{}", rm)) };
            let ops = vec![rd_op, rm_op,
                           Operand::Shift { kind: "ror".into(), amount }];
            prop_assert!(encode_negs(&ops).is_err());
        }
    }

    // ── encode_adc / encode_adcs (ADC family, ARMv8 ARM C4.1.4) ─────────────
    // Encoding: sf | 0 | S | 11010000 | Rm | 000000 | Rn | Rd
    //   bit 31 = sf (1=X, 0=W); bit 30 = op (0=ADC, 1=SBC); bit 29 = S (flags);
    //   bits 28:21 = 0b11010000 fixed; bits 20:16 = Rm; bits 15:10 = 000000 (reserved 0);
    //   bits 9:5 = Rn; bits 4:0 = Rd.
    fn adc_fixed_of(w: u32) -> u32 { (w >> 21) & 0xFF }   // bits 28:21 = 11010000
    fn reserved6_of(w: u32) -> u32 { (w >> 10) & 0x3F }  // bits 15:10 (must be 0)

    proptest! {
        // 1. Every fixed and operand field lands exactly where the ARMv8 spec
        //    dictates for `ADC Xd, Xn, Xm`: sf=1, op(bit30)=0, opcode=0xD0,
        //    reserved6=0, and Rm/Rn/Rd extracted from their operands.
        #[test]
        fn adc_field_placement(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30, set_flags in any::<bool>(),
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let w = expect_word(encode_adc(&ops, set_flags));
            prop_assert_eq!(sf_of(w), 1);
            prop_assert_eq!(op_of(w), 0);            // ADC => add family, op=0
            prop_assert_eq!(adc_fixed_of(w), 0xD0);  // bits 28:21 = 11010000
            prop_assert_eq!(reserved6_of(w), 0);     // bits 15:10 must be zero
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 2. The S bit (bit 29) is exactly set_flags: ADC (S=0) vs ADCS (S=1).
        #[test]
        fn adc_s_bit_tracks_set_flags(
            n in 0u32..=30, set_flags in any::<bool>(), is_w in any::<bool>(),
        ) {
            let r = if is_w { Operand::Reg(format!("w{}", n)) } else { xreg(n) };
            let ops = vec![r.clone(), r.clone(), r];
            let w = expect_word(encode_adc(&ops, set_flags));
            prop_assert_eq!(s_of(w), if set_flags { 1 } else { 0 });
        }

        // 3. sf (bit 31) tracks register width: W -> 0, X -> 1.
        #[test]
        fn adc_sf_tracks_register_width(n in 0u32..=30, is_w in any::<bool>()) {
            let r = if is_w { Operand::Reg(format!("w{}", n)) } else { xreg(n) };
            let ops = vec![r.clone(), r.clone(), r];
            let w = expect_word(encode_adc(&ops, false));
            prop_assert_eq!(sf_of(w), if is_w { 0 } else { 1 });
        }

        // 4. Differential: within the add/sub carry family, ADC always sets
        //    op=0 and SBC always sets op=1 (bit 30), regardless of flags/width.
        //    A regression that drops or swaps the op term would be caught here.
        #[test]
        fn adc_vs_sbc_op_bit(n in 0u32..=30, set_flags in any::<bool>(), is_w in any::<bool>()) {
            let r = if is_w { Operand::Reg(format!("w{}", n)) } else { xreg(n) };
            let ops = vec![r.clone(), r.clone(), r];
            prop_assert_eq!(op_of(expect_word(encode_adc(&ops, set_flags))), 0);
            prop_assert_eq!(op_of(expect_word(encode_sbc(&ops, set_flags))), 1);
        }

        // 5. NEGATIVE CONTRACT: ADC requires exactly three register operands.
        //    Missing operands or a non-register (immediate) in an operand slot
        //    MUST be rejected with Err, never silently encoded.
        #[test]
        fn adc_rejects_bad_operand_arities(
            rd in 0u32..=30, rn in 0u32..=30, bad_imm in 0i64..=0xFFF,
        ) {
            // too few operands (< 3)
            let two = vec![xreg(rd), xreg(rn)];
            prop_assert!(encode_adc(&two, false).is_err());
            // third operand is an immediate, not a register
            let imm_third = vec![xreg(rd), xreg(rn), Operand::Imm(bad_imm)];
            prop_assert!(encode_adc(&imm_third, false).is_err());
        }
        // 6. NEGATIVE CONTRACT: all ADC operands must share the same register width.
        // The encoder currently derives sf from Rd and discards the width flags
        // for Rn/Rm, so mixed W/X operands are silently re-encoded as the Rd width.
        #[test]
        fn adc_rejects_mixed_width_operands(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30, set_flags in any::<bool>(),
        ) {
            let cases = [
                vec![xreg(rd), Operand::Reg(format!("w{}", rn)), xreg(rm)],
                vec![xreg(rd), xreg(rn), Operand::Reg(format!("w{}", rm))],
                vec![Operand::Reg(format!("w{}", rd)), xreg(rn), Operand::Reg(format!("w{}", rm))],
            ];
            for ops in cases {
                prop_assert!(
                    encode_adc(&ops, set_flags).is_err(),
                    "encode_adc should reject mixed-width operands: {:?}", ops
                );
            }
        }
    }
    // Spec (ARMv8 ARM): SBC  <Xd>,<Xn>,<Xm> = sf 1 0 11010000 Rm 000000 Rn Rd
    //                   SBCS <Xd>,<Xn>,<Xm> = sf 1 1 11010000 Rm 000000 Rn Rd
    // bit31 sf, bit30 op=1 (subtract), bit29 S, bits28..21 == 0b11010000,
    // bits20..16 Rm, bits15..10 reserved 0, bits9..5 Rn, bits4..0 Rd.

    /// Independent reference oracle (hand-decoded constant, not field extraction).
    #[test]
    fn sbc_known_constant_encoding() {
        // sbc x0, x1, x2  =>  sf=1 op=1 S=0 11010000 Rm=2 000000 Rn=1 Rd=0 = 0xDA020020
        let ops = vec![xreg(0), xreg(1), xreg(2)];
        let w = expect_word(encode_sbc(&ops, false));
        assert_eq!(w, 0xDA02_0020);
        // sbcs x0, x1, x2  =>  same with S=1
        assert_eq!(expect_word(encode_sbc(&ops, true)), 0xDA02_0020 | (1u32 << 29));
    }

    proptest! {
        // 1. Every fixed and variable field of SBC (64-bit, no flags) lands
        //    exactly where the ARMv8 spec dictates; reserved bits stay 0.
        #[test]
        fn sbc_64bit_field_placement(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let w = expect_word(encode_sbc(&ops, false));
            prop_assert_eq!(sf_of(w), 1);                   // 64-bit
            prop_assert_eq!(op_of(w), 1);                   // subtract op bit
            prop_assert_eq!(s_of(w), 0);                    // SBC, not SBCS
            prop_assert_eq!(opcode5_of(w), 0b11010);        // top 5 of fixed opcode
            prop_assert_eq!((w >> 21) & 0xFF, 0b1101_0000); // full opcode bits 28..21
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!((w >> 10) & 0x3F, 0);           // reserved bits 15..10 == 0
        }

        // 2. SBCS (set_flags=true) differs from SBC only in bit 29 (S).
        #[test]
        fn sbcs_flips_only_s_bit(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let sbc  = expect_word(encode_sbc(&ops, false));
            let sbcs = expect_word(encode_sbc(&ops, true));
            prop_assert_eq!(sbcs ^ sbc, 1u32 << 29);
            prop_assert_eq!(s_of(sbcs), 1);
            prop_assert_eq!(s_of(sbc), 0);
        }

        // 3. SBC is ADC with the subtract op bit (30) set — the two carry
        //    instructions are structurally identical apart from op.
        #[test]
        fn sbc_is_adc_with_op_bit_set(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            set_flags in any::<bool>(),
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let sbc = expect_word(encode_sbc(&ops, set_flags));
            let adc = expect_word(encode_adc(&ops, set_flags));
            prop_assert_eq!(sbc ^ adc, 1u32 << 30);
            prop_assert_eq!(op_of(sbc), 1);
            prop_assert_eq!(op_of(adc), 0);
        }

        // 4. sf (bit 31) tracks register width: W -> 0, X -> 1.
        #[test]
        fn sbc_sf_tracks_register_width(
            n in 0u32..=31,
            is_w in any::<bool>(),
        ) {
            let r = |w: bool| -> Operand {
                if w { Operand::Reg(format!("w{}", n)) } else { xreg(n) }
            };
            let ops = vec![r(is_w), r(is_w), r(is_w)];
            let w = expect_word(encode_sbc(&ops, false));
            prop_assert_eq!(sf_of(w), if is_w { 0 } else { 1 });
        }

        // 5. NEGATIVE CONTRACT: SBC requires exactly three register operands.
        //    Too few operands, or an immediate where a register is required,
        //    MUST be rejected with Err — never silently encoded.
        #[test]
        fn sbc_rejects_bad_operand_arities(
            rd in 0u32..=31, rn in 0u32..=31, bad_imm in 0i64..=0xFFF,
        ) {
            // too few operands (< 3)
            let two = vec![xreg(rd), xreg(rn)];
            prop_assert!(encode_sbc(&two, false).is_err());
            // third operand is an immediate, not a register
            let imm_third = vec![xreg(rd), xreg(rn), Operand::Imm(bad_imm)];
            prop_assert!(encode_sbc(&imm_third, false).is_err());
        }
        // 6. NEGATIVE CONTRACT: all SBC operands must share the same register width.
        // The encoder currently derives sf from Rd and discards the width flags
        // for Rn/Rm, so mixed W/X operands are silently re-encoded as the Rd width.
        #[test]
        fn sbc_rejects_mixed_width_operands(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30, set_flags in any::<bool>(),
        ) {
            let cases = [
                vec![xreg(rd), Operand::Reg(format!("w{}", rn)), xreg(rm)],
                vec![xreg(rd), xreg(rn), Operand::Reg(format!("w{}", rm))],
                vec![Operand::Reg(format!("w{}", rd)), xreg(rn), Operand::Reg(format!("w{}", rm))],
            ];
            for ops in cases {
                prop_assert!(
                    encode_sbc(&ops, set_flags).is_err(),
                    "encode_sbc should reject mixed-width operands: {:?}", ops
                );
            }
        }
    }
    // ARMv8 logical (shifted register): sf opc shift 01010 N Rm imm6 Rn Rd
    //   BIC = opc=00, N=1 (bit 21). Distinct from AND (opc=00,N=0), ORR (01,0),
    //   EOR (10,0), ORN (01,1), EON (10,1), BICS (11,1).
    // ARMv8 logical (immediate): sf opc 100100 N immr imms Rn Rd
    //   BIC #imm is an alias of AND #~imm (opc=00).
    // NEON vector: 0 Q 0 01110 01 1 Rm 000111 Rn Rd.
    // Reuses sf_of/opc_of/opcode5_of/opcode6_of/n21_of/shift_type_of/
    // shift_amt_of/immr_of/imms_of/rm_of/rn_of/rd_of/expect_word/xreg from above.
    fn q_of(w: u32) -> u32 { (w >> 30) & 1 }  // NEON Q (quadword) bit
    fn b31_of(w: u32) -> u32 { (w >> 31) & 1 }
    fn b29_of(w: u32) -> u32 { (w >> 29) & 1 }
    fn op5_28_of(w: u32) -> u32 { (w >> 24) & 0x1F } // bits 28:24 (NEON fixed op)
    fn op2_23_of(w: u32) -> u32 { (w >> 22) & 0x3 }  // bits 23:22 (NEON size/op)
    fn fixed6_of(w: u32) -> u32 { (w >> 10) & 0x3F } // bits 15:10 (NEON fixed 000111)
    fn neonreg(n: u32, arr: &str) -> Operand {
        Operand::RegArrangement { reg: format!("v{}", n), arrangement: arr.into() }
    }

    proptest! {
        // 1. REGISTER-FORM FIELD PLACEMENT: BIC Xd, Xn, Xm (no shift), both widths.
        //    Every fixed and variable field lands per the ARMv8 spec: opc=00
        //    (bits 30:29, same family as AND), fixed op 01010 (bits 28:24), N=1
        //    (bit 21, distinguishing BIC from AND), zero shift, and Rm/Rn/Rd placed.
        #[test]
        fn bic_register_form_field_placement(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            is_64 in any::<bool>(),
        ) {
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rn_op = if is_64 { xreg(rn) } else { Operand::Reg(format!("w{}", rn)) };
            let rm_op = if is_64 { xreg(rm) } else { Operand::Reg(format!("w{}", rm)) };
            let ops = vec![rd_op, rn_op, rm_op];
            let w = expect_word(encode_bic(&ops));
            prop_assert_eq!(sf_of(w), if is_64 { 1 } else { 0 });
            prop_assert_eq!(opc_of(w), 0b00);          // AND-family (BIC)
            prop_assert_eq!(opcode5_of(w), 0b01010);   // logical shifted register
            prop_assert_eq!(n21_of(w), 1);             // N=1 marks the NOT variants
            prop_assert_eq!(shift_type_of(w), 0);
            prop_assert_eq!(shift_amt_of(w), 0);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 2. REGISTER-FORM SHIFT MAPPING: the four shift kinds map to the 2-bit
        //    shift field (lsl=00,lsr=01,asr=10,ror=11) and, for X registers,
        //    imm6 (0..=63) is placed verbatim. The N=1 signature is preserved
        //    regardless of the shift operand (so BIC never degrades to AND).
        #[test]
        fn bic_register_form_shift_mapping(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            sk in 0u32..=3u32, amount in 0u32..=63u32,
        ) {
            let (kind, want) = match sk {
                0 => ("lsl", 0u32), 1 => ("lsr", 1u32),
                2 => ("asr", 2u32), _ => ("ror", 3u32),
            };
            let ops = vec![xreg(rd), xreg(rn), xreg(rm),
                           Operand::Shift { kind: kind.into(), amount }];
            let w = expect_word(encode_bic(&ops));
            prop_assert_eq!(opc_of(w), 0b00);
            prop_assert_eq!(opcode5_of(w), 0b01010);
            prop_assert_eq!(n21_of(w), 1);
            prop_assert_eq!(shift_type_of(w), want);
            prop_assert_eq!(shift_amt_of(w), amount); // 0..63 verbatim (X register)
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 3. ALGEBRAIC / DIFFERENTIAL ORACLE: BIC Xd, Xn, #imm is defined as
        //    AND Xd, Xn, #(~imm) (the immediate is bitwise-inverted, then
        //    encoded as an AND bitmask immediate). Therefore encode_bic with
        //    #imm MUST produce a bit-identical word to encode_logical (opc=00,
        //    i.e. AND) with #(~imm), and the two succeed or fail together for
        //    every immediate (a value is encodable as BIC iff ~value is encodable
        //    as AND). This is the defining equivalence of the alias.
        #[test]
        fn bic_immediate_equals_and_of_inverted(
            rd in 0u32..=30, rn in 0u32..=30,
            imm64 in any::<u64>(), is_64 in any::<bool>(),
        ) {
            let mask = if is_64 { u64::MAX } else { 0xFFFF_FFFF };
            let eff = imm64 & mask;                 // value the encoder actually sees
            let inv = (!eff) & mask;                // ~value in the active width
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rn_op = if is_64 { xreg(rn) } else { Operand::Reg(format!("w{}", rn)) };
            let bic_ops = vec![rd_op.clone(), rn_op.clone(), Operand::Imm(eff as i64)];
            let and_ops = vec![rd_op, rn_op, Operand::Imm(inv as i64)];
            let bic = encode_bic(&bic_ops);
            let and = encode_logical(&and_ops, 0b00); // opc=00 => AND
            // Encodability is identical: BIC accepts iff AND accepts the inverse.
            prop_assert_eq!(bic.is_ok(), and.is_ok());
            if let (Ok(b), Ok(a)) = (bic, and) {
                let bw = match b { EncodeResult::Word(x) => x, _ => unreachable!() };
                let aw = match a { EncodeResult::Word(x) => x, _ => unreachable!() };
                prop_assert_eq!(bw, aw);
                // opc=00 immediate-family fixed field and placed registers.
                prop_assert_eq!(opcode6_of(bw), 0b100100);
                prop_assert_eq!(rn_of(bw), rn);
                prop_assert_eq!(rd_of(bw), rd);
            }
        }

        // 4. NEON VECTOR FORM: BIC Vd.T, Vn.T, Vm.T (T in {8b, 16b}). The Q bit
        //    (bit 30) selects 128-bit (16b) vs 64-bit (8b); bits 31 and 29 are 0;
        //    the fixed NEON logical opcode 01110 sits at bits 28:24 with size/op
        //    01 at bits 23:22, N=1 at bit 21, fixed 000111 at bits 15:10, and the
        //    three vector registers are placed in Rm/Rn/Rd.
        #[test]
        fn bic_neon_vector_form_fields(
            rd in 0u32..=31, rn in 0u32..=31, rm in 0u32..=31,
            big in any::<bool>(),
        ) {
            let arr = if big { "16b" } else { "8b" };
            let ops = vec![neonreg(rd, arr), neonreg(rn, arr), neonreg(rm, arr)];
            let w = expect_word(encode_bic(&ops));
            prop_assert_eq!(b31_of(w), 0);
            prop_assert_eq!(q_of(w), if big { 1 } else { 0 });
            prop_assert_eq!(b29_of(w), 0);
            prop_assert_eq!(op5_28_of(w), 0b01110);
            prop_assert_eq!(op2_23_of(w), 0b01);
            prop_assert_eq!(n21_of(w), 1);
            prop_assert_eq!(fixed6_of(w), 0b000111);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 5. NEGATIVE CONTRACT: for the 32-bit (W) shifted-register form, imm6
        //    must be 0..=31; a shift of 32..63 is UNDEFINED (ARMv8 ARM, C4.1.4:
        //    for sf=0 the shift amount must be 0..31) and MUST be rejected, not
        //    silently masked into the imm6 field via `& 0x3F`.
        #[test]
        fn bic_w_register_rejects_shift_above_31(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            amount in 32u32..=63u32, sk in 0u32..=3u32,
        ) {
            let kind = match sk { 0 => "lsl", 1 => "lsr", 2 => "asr", _ => "ror" };
            let ops = vec![Operand::Reg(format!("w{}", rd)),
                           Operand::Reg(format!("w{}", rn)),
                           Operand::Reg(format!("w{}", rm)),
                           Operand::Shift { kind: kind.into(), amount }];
            prop_assert!(encode_bic(&ops).is_err());
        }
    }

    // ── encode_bics: BICS (bitwise clear, setting flags), shifted-register form ──
    // Oracle: reference (ARMv8 ARM §C4.1.64) + differential vs encode_bic.
    // BICS is ANDS with an inverted second operand, encoding as
    //   sf 11 01010 shift 1 Rm imm6 Rn Rd   (opc=11, N=1).
    // It differs from BIC (opc=00) ONLY in bits 30:29.
    proptest! {
        // 1. Fixed-field + register/shift placement for the register form, with
        //    and without an explicit shift. opc=11, op5=01010, N=1; the default
        //    (no shift operand) is LSL #0.
        #[test]
        fn bics_field_placement(
            rd in 0u32..=31, rn in 0u32..=31, rm in 0u32..=31,
            shifted in any::<bool>(), sk in 0u32..=3u32, amount in 0u32..=63u32,
        ) {
            let (kind, want_st) = match sk {
                0 => ("lsl", 0u32), 1 => ("lsr", 1u32),
                2 => ("asr", 2u32), _ => ("ror", 3u32),
            };
            let ops = if shifted {
                vec![xreg(rd), xreg(rn), xreg(rm),
                     Operand::Shift { kind: kind.into(), amount }]
            } else {
                vec![xreg(rd), xreg(rn), xreg(rm)]
            };
            let w = expect_word(encode_bics(&ops));
            prop_assert_eq!(sf_of(w), 1);                              // 64-bit
            prop_assert_eq!((op_of(w) << 1) | s_of(w), 0b11u32);       // opc = ANDS family
            prop_assert_eq!(opcode5_of(w), 0b01010);                   // logical shifted reg
            prop_assert_eq!(n21_of(w), 1);                             // "NOT" variant
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(shift_type_of(w), if shifted { want_st } else { 0 });
            prop_assert_eq!(shift_amt_of(w), if shifted { amount } else { 0 });
        }

        // 2. sf (bit 31) tracks operand width: W registers -> 0, X -> 1.
        #[test]
        fn bics_sf_tracks_width(n in 0u32..=31, is_w in any::<bool>()) {
            let r = if is_w { format!("w{}", n) } else { format!("x{}", n) };
            let ops = vec![Operand::Reg(r.clone()), Operand::Reg(r.clone()), Operand::Reg(r)];
            let w = expect_word(encode_bics(&ops));
            prop_assert_eq!(sf_of(w), if is_w { 0 } else { 1 });
        }

        // 3. Differential: BICS and BIC (register form) differ ONLY in the opc
        //    field (bits 30:29): BIC=00, BICS=11. Every other field is identical.
        #[test]
        fn bics_differs_from_bic_only_in_opc(
            rd in 0u32..=31, rn in 0u32..=31, rm in 0u32..=31, is_w in any::<bool>(),
            shifted in any::<bool>(), sk in 0u32..=3u32, amount in 0u32..=63u32,
        ) {
            let mk = |n: u32| if is_w { format!("w{}", n) } else { format!("x{}", n) };
            let ops = if shifted {
                let kind = match sk { 0 => "lsl", 1 => "lsr", 2 => "asr", _ => "ror" };
                vec![Operand::Reg(mk(rd)), Operand::Reg(mk(rn)), Operand::Reg(mk(rm)),
                     Operand::Shift { kind: kind.into(), amount }]
            } else {
                vec![Operand::Reg(mk(rd)), Operand::Reg(mk(rn)), Operand::Reg(mk(rm))]
            };
            let bic = expect_word(encode_bic(&ops));
            let bics = expect_word(encode_bics(&ops));
            prop_assert_eq!(bics ^ bic, 0b11u32 << 29);
        }

        // 4. NEGATIVE CONTRACT: fewer than 3 operands must be rejected.
        #[test]
        fn bics_rejects_too_few_operands(n in 0u32..=2u32) {
            let ops: Vec<Operand> = (0..n).map(xreg).collect();
            prop_assert!(encode_bics(&ops).is_err());
        }

        // 5. NEGATIVE CONTRACT: for sf=0 (W register) the imm6 shift must be
        //    0..=31 (ARMv8 ARM §C4.1.64); 32..=63 is UNPREDICTABLE and MUST be
        //    rejected. The encoder currently masks with `& 0x3F` and accepts it.
        #[test]
        fn bics_w_register_rejects_shift_above_31(
            rd in 0u32..=31, rn in 0u32..=31, rm in 0u32..=31,
            amount in 32u32..=63u32, sk in 0u32..=3u32,
        ) {
            let kind = match sk { 0 => "lsl", 1 => "lsr", 2 => "asr", _ => "ror" };
            let ops = vec![Operand::Reg(format!("w{}", rd)),
                           Operand::Reg(format!("w{}", rn)),
                           Operand::Reg(format!("w{}", rm)),
                           Operand::Shift { kind: kind.into(), amount }];
            prop_assert!(encode_bics(&ops).is_err());
        }
    }

    // ── encode_orn (ORN = ORR with N=1, i.e. OR NOT) ───────────────────────
    // Oracle: reference (ARMv8 ARM §C4.1.115) + differential vs encode_logical.
    // Scalar (shifted register): sf opc 01010 shift N Rm imm6 Rn Rd
    //   ORN = opc=01 (bits 30:29), N=1 (bit 21). Distinct from ORR (01,0),
    //   EOR (10,0), EON (10,1), AND (00,0), BIC (00,1), BICS (11,1).
    // NEON vector: 0 Q 0 01110 11 1 Rm 000111 Rn Rd (size/op = 11 distinguishes
    //   ORN from ORR=00, AND=00, BIC=01, EOR=10, EON=10, BIC variants).
    // Reuses sf_of/opc_of/opcode5_of/n21_of/shift_type_of/shift_amt_of/rm_of/
    // rn_of/rd_of/expect_word/xreg and b31_of/q_of/b29_of/op5_28_of/op2_23_of/
    // fixed6_of/neonreg from above.

    proptest! {
        // 1. REGISTER-FORM FIELD PLACEMENT: ORN Xd, Xn, Xm (no shift), both widths.
        //    Every fixed and variable field lands per the ARMv8 spec: opc=01
        //    (bits 30:29, same family as ORR), fixed op 01010 (bits 28:24), N=1
        //    (bit 21, distinguishing ORN from ORR), zero shift, and Rm/Rn/Rd placed.
        #[test]
        fn orn_register_form_field_placement(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            is_64 in any::<bool>(),
        ) {
            let rd_op = if is_64 { xreg(rd) } else { Operand::Reg(format!("w{}", rd)) };
            let rn_op = if is_64 { xreg(rn) } else { Operand::Reg(format!("w{}", rn)) };
            let rm_op = if is_64 { xreg(rm) } else { Operand::Reg(format!("w{}", rm)) };
            let ops = vec![rd_op, rn_op, rm_op];
            let w = expect_word(encode_orn(&ops));
            prop_assert_eq!(sf_of(w), if is_64 { 1 } else { 0 });
            prop_assert_eq!(opc_of(w), 0b01);          // ORR-family (ORN)
            prop_assert_eq!(opcode5_of(w), 0b01010);   // logical shifted register
            prop_assert_eq!(n21_of(w), 1);             // N=1 marks the NOT variant
            prop_assert_eq!(shift_type_of(w), 0);
            prop_assert_eq!(shift_amt_of(w), 0);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 2. REGISTER-FORM SHIFT MAPPING: the four shift kinds map to the 2-bit
        //    shift field (lsl=00,lsr=01,asr=10,ror=11) and, for X registers,
        //    imm6 (0..=63) is placed verbatim. The N=1 signature is preserved
        //    regardless of the shift operand (so ORN never degrades to ORR).
        #[test]
        fn orn_register_form_shift_mapping(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            sk in 0u32..=3u32, amount in 0u32..=63u32,
        ) {
            let (kind, want) = match sk {
                0 => ("lsl", 0u32), 1 => ("lsr", 1u32),
                2 => ("asr", 2u32), _ => ("ror", 3u32),
            };
            let ops = vec![xreg(rd), xreg(rn), xreg(rm),
                           Operand::Shift { kind: kind.into(), amount }];
            let w = expect_word(encode_orn(&ops));
            prop_assert_eq!(opc_of(w), 0b01);
            prop_assert_eq!(opcode5_of(w), 0b01010);
            prop_assert_eq!(n21_of(w), 1);
            prop_assert_eq!(shift_type_of(w), want);
            prop_assert_eq!(shift_amt_of(w), amount); // 0..63 verbatim (X register)
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 3. DIFFERENTIAL ORACLE: ORN is defined as ORR with the second operand
        //    inverted, i.e. the ONLY encoding difference between ORN and ORR is
        //    bit 21 (N): ORN sets N=1, ORR leaves N=0. Therefore encode_orn and
        //    encode_logical(opc=01 == ORR) must produce words that differ in
        //    EXACTLY that one bit, for both widths and every shift, and both
        //    must succeed together. This is the defining equivalence of the alias.
        #[test]
        fn orn_differs_from_orr_only_in_n_bit(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            is_64 in any::<bool>(),
            shifted in any::<bool>(), sk in 0u32..=3u32, amount in 0u32..=63u32,
        ) {
            let mk = |n: u32| if is_64 { xreg(n) } else { Operand::Reg(format!("w{}", n)) };
            let ops = if shifted {
                let kind = match sk { 0 => "lsl", 1 => "lsr", 2 => "asr", _ => "ror" };
                vec![mk(rd), mk(rn), mk(rm),
                     Operand::Shift { kind: kind.into(), amount }]
            } else {
                vec![mk(rd), mk(rn), mk(rm)]
            };
            let orn = encode_orn(&ops);
            let orr = encode_logical(&ops, 0b01); // opc=01 => ORR
            prop_assert!(orn.is_ok());
            prop_assert!(orr.is_ok());
            let ow = expect_word(orn);
            let rw = expect_word(orr);
            prop_assert_eq!(ow ^ rw, 1u32 << 21); // differ in exactly bit 21
        }

        // 4. NEON VECTOR FORM: ORN Vd.T, Vn.T, Vm.T (T in {8b, 16b}). The Q bit
        //    (bit 30) selects 128-bit (16b) vs 64-bit (8b); bits 31 and 29 are 0;
        //    the fixed NEON logical opcode 01110 sits at bits 28:24 with size/op
        //    11 at bits 23:22 (distinguishing ORN from the other logical vector
        //    ops), N=1 at bit 21, fixed 000111 at bits 15:10, and the three
        //    vector registers are placed in Rm/Rn/Rd.
        #[test]
        fn orn_neon_vector_form_fields(
            rd in 0u32..=31, rn in 0u32..=31, rm in 0u32..=31,
            big in any::<bool>(),
        ) {
            let arr = if big { "16b" } else { "8b" };
            let ops = vec![neonreg(rd, arr), neonreg(rn, arr), neonreg(rm, arr)];
            let w = expect_word(encode_orn(&ops));
            prop_assert_eq!(b31_of(w), 0);
            prop_assert_eq!(q_of(w), if big { 1 } else { 0 });
            prop_assert_eq!(b29_of(w), 0);
            prop_assert_eq!(op5_28_of(w), 0b01110);
            prop_assert_eq!(op2_23_of(w), 0b11); // ORN size/op (vs ORR=00, BIC=01, EOR=10)
            prop_assert_eq!(n21_of(w), 1);
            prop_assert_eq!(fixed6_of(w), 0b000111);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 5. NEGATIVE CONTRACTS:
        //    (a) fewer than 3 operands must be rejected (ORN is not an alias
        //        that supplies an implicit operand here).
        //    (b) For sf=0 (W register) the imm6 shift must be 0..=31 (ARMv8 ARM
        //        §C4.1.115); 32..=63 is UNPREDICTABLE and MUST be rejected, not
        //        silently masked into the imm6 field via `& 0x3F`.
        #[test]
        fn orn_negative_contracts(
            n in 0u32..=2u32,
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            amount in 32u32..=63u32, sk in 0u32..=3u32,
        ) {
            // (a) too few operands -> Err
            let ops: Vec<Operand> = (0..n).map(xreg).collect();
            prop_assert!(encode_orn(&ops).is_err());
            // (b) W-register shift above 31 -> Err
            let kind = match sk { 0 => "lsl", 1 => "lsr", 2 => "asr", _ => "ror" };
            let ops = vec![Operand::Reg(format!("w{}", rd)),
                           Operand::Reg(format!("w{}", rn)),
                           Operand::Reg(format!("w{}", rm)),
                           Operand::Shift { kind: kind.into(), amount }];
            prop_assert!(encode_orn(&ops).is_err());
        }
    }

    // ── encode_orn: additional gaps (shift range, XZR, width consistency, NEON arr) ─
    // These target contracts NOT exercised by properties 1-5 above.
    proptest! {
        // 6. NEGATIVE CONTRACT (GAP): for 64-bit (X) shifted-register ORN, imm6
        //    (bits 15:10) is a 6-bit field holding the shift amount 0..=63
        //    (ARMv8 ARM §C4.1.115). `lsl #64` and above are UNDEFINED and MUST be
        //    rejected (GAS/llvm-mc: "immediate value out of range"), not silently
        //    masked into imm6 via `& 0x3F` — which would alias `lsl #64` to
        //    `lsl #0` and silently corrupt the instruction. (Complements the
        //    W-register 32..63 contract in property 5.)
        #[test]
        fn orn_xreg_shift_above_63_must_be_rejected(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            amount in 64u32..=255u32,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm),
                           Operand::Shift { kind: "lsl".into(), amount }];
            prop_assert!(encode_orn(&ops).is_err());
        }

        // 7. POSITIVE GAP: there is no SP special-casing for ORN (unlike
        //    ADD/SUB), so register 31 reads as XZR for the scalar logical
        //    shifted-register form. All three register fields across the full
        //    0..=31 range (including XZR=31) must land verbatim in Rm/Rn/Rd.
        //    Existing property 1 restricted to 0..=30; this closes the gap.
        #[test]
        fn orn_scalar_accepts_register_31_xzr(
            rd in 0u32..=31, rn in 0u32..=31, rm in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let w = expect_word(encode_orn(&ops));
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(opc_of(w), 0b01);
            prop_assert_eq!(n21_of(w), 1);
        }

        // 8. NEGATIVE CONTRACT (GAP): ORN requires all three operands to share
        //    the same register width. `orn x0, w1, w2` mixes X/W widths and is
        //    rejected by GAS/llvm-mc ("operand size mismatch"); the encoder must
        //    return Err rather than silently deriving sf from operand 0 alone
        //    and encoding W register numbers into a 64-bit instruction.
        #[test]
        fn orn_rejects_mixed_register_widths(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            mix in 0u32..=2u32,
        ) {
            let x = |n: u32| xreg(n);
            let w = |n: u32| Operand::Reg(format!("w{}", n));
            let (rd_op, rn_op, rm_op) = match mix {
                0 => (x(rd), w(rn), w(rm)), // X dest, W sources
                1 => (w(rd), x(rn), x(rm)), // W dest, X sources
                _ => (x(rd), x(rn), w(rm)), // one W source among X
            };
            let ops = vec![rd_op, rn_op, rm_op];
            prop_assert!(encode_orn(&ops).is_err());
        }

        // 9. NEGATIVE CONTRACT (GAP): the ORN *vector* instruction is ONLY
        //    defined for the .8b (Q=0) and .16b (Q=1) arrangements (ARMv8 ARM
        //    §C7.2.2 — bitwise logical vector ops are byte-element only). Non-byte
        //    arrangements (.4h/.8h/.2s/.1d) and mismatched arrangements must be
        //    rejected; the encoder currently derives Q solely from operand 0's
        //    arrangement and silently accepts anything else.
        #[test]
        fn orn_neon_rejects_non_byte_or_mismatched_arrangement(
            rd in 0u32..=31, rn in 0u32..=31, rm in 0u32..=31,
            bad in 0u32..=5u32,
        ) {
            let (a0, a1, a2) = match bad {
                0 => ("4h", "4h", "4h"),
                1 => ("2s", "2s", "2s"),
                2 => ("1d", "1d", "1d"),
                3 => ("8h", "8h", "8h"),
                4 => ("16b", "8b", "16b"), // mismatched
                _ => ("8b", "16b", "8b"),   // mismatched
            };
            let ops = vec![neonreg(rd, a0), neonreg(rn, a1), neonreg(rm, a2)];
            prop_assert!(encode_orn(&ops).is_err());
        }
    }

    // ── encode_eon (EON = EOR with N=1, i.e. exclusive-OR NOT) ─────────────
    // Oracle: reference (ARMv8 ARM §C4.1.66) + differential vs encode_orn.
    // Scalar (shifted register): sf opc 01010 shift N Rm imm6 Rn Rd
    //   EON = opc=10 (bits 30:29), N=1 (bit 21). Sibling of ORN (opc=01, N=1),
    //   BICS (opc=11, N=1), and the non-inverted EOR (opc=10, N=0).
    // EON has no NEON vector form; only the scalar register path is exercised.
    proptest! {
        // 1. REGISTER-FORM FIELD PLACEMENT: EON Xd, Xn, Xm (no shift), both
        //    widths. opc=10, fixed op 01010 (bits 28:24), N=1 (bit 21),
        //    zero shift, and Rm/Rn/Rd placed; sf tracks register width.
        #[test]
        fn eon_register_form_field_placement(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            is_64 in any::<bool>(),
        ) {
            let mk = |n: u32| if is_64 { xreg(n) } else { Operand::Reg(format!("w{}", n)) };
            let ops = vec![mk(rd), mk(rn), mk(rm)];
            let w = expect_word(encode_eon(&ops));
            prop_assert_eq!(sf_of(w), if is_64 { 1 } else { 0 });
            prop_assert_eq!(opc_of(w), 0b10);          // EON
            prop_assert_eq!(opcode5_of(w), 0b01010);   // logical shifted register
            prop_assert_eq!(n21_of(w), 1);             // N=1 marks the NOT variant
            prop_assert_eq!(shift_type_of(w), 0);      // default LSL
            prop_assert_eq!(shift_amt_of(w), 0);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 2. REGISTER-FORM SHIFT MAPPING: the four shift kinds map to the 2-bit
        //    shift field (lsl=00,lsr=01,asr=10,ror=11) and, for X registers,
        //    imm6 (0..=63) is placed verbatim. The N=1 + opc=10 signature is
        //    preserved regardless of the shift operand (so EON never degrades
        //    to EOR, ORN, or BICS).
        #[test]
        fn eon_register_form_shift_mapping(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            sk in 0u32..=3u32, amount in 0u32..=63u32,
        ) {
            let (kind, want_st) = match sk {
                0 => ("lsl", 0u32), 1 => ("lsr", 1u32),
                2 => ("asr", 2u32), _ => ("ror", 3u32),
            };
            let ops = vec![xreg(rd), xreg(rn), xreg(rm),
                           Operand::Shift { kind: kind.into(), amount }];
            let w = expect_word(encode_eon(&ops));
            prop_assert_eq!(opc_of(w), 0b10);          // still EON
            prop_assert_eq!(n21_of(w), 1);             // still the NOT variant
            prop_assert_eq!(shift_type_of(w), want_st);
            prop_assert_eq!(shift_amt_of(w), amount);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 3. DIFFERENTIAL vs encode_orn: EON and ORN are identical encodings
        //    except in the opc field (bits 30:29): ORN=01, EON=10 (so the words
        //    differ in exactly 0b11 << 29). Every other field — sf, fixed op,
        //    N, shift, Rm/Rn/Rd — must match bit-for-bit.
        #[test]
        fn eon_differs_from_orn_only_in_opc(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            is_w in any::<bool>(),
            shifted in any::<bool>(), sk in 0u32..=3u32, amount in 0u32..=63u32,
        ) {
            let mk = |n: u32| if is_w { format!("w{}", n) } else { format!("x{}", n) };
            let ops = if shifted {
                let kind = match sk { 0 => "lsl", 1 => "lsr", 2 => "asr", _ => "ror" };
                vec![Operand::Reg(mk(rd)), Operand::Reg(mk(rn)), Operand::Reg(mk(rm)),
                     Operand::Shift { kind: kind.into(), amount }]
            } else {
                vec![Operand::Reg(mk(rd)), Operand::Reg(mk(rn)), Operand::Reg(mk(rm))]
            };
            let orn = expect_word(encode_orn(&ops));
            let eon = expect_word(encode_eon(&ops));
            prop_assert_eq!(eon ^ orn, 0b11u32 << 29);
        }

        // 4. NEGATIVE CONTRACT: fewer than 3 operands must be rejected.
        #[test]
        fn eon_rejects_too_few_operands(n in 0u32..=2u32) {
            let ops: Vec<Operand> = (0..n).map(xreg).collect();
            prop_assert!(encode_eon(&ops).is_err());
        }

        // 5. NEGATIVE CONTRACT: for sf=0 (W register) the imm6 shift must be
        //    0..=31 (ARMv8 ARM §C4.1.66); 32..=63 is UNPREDICTABLE and MUST be
        //    rejected. The encoder currently masks with `& 0x3F` and accepts it.
        #[test]
        fn eon_w_register_rejects_shift_above_31(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            amount in 32u32..=63u32, sk in 0u32..=3u32,
        ) {
            let kind = match sk { 0 => "lsl", 1 => "lsr", 2 => "asr", _ => "ror" };
            let ops = vec![Operand::Reg(format!("w{}", rd)),
                           Operand::Reg(format!("w{}", rn)),
                           Operand::Reg(format!("w{}", rm)),
                           Operand::Shift { kind: kind.into(), amount }];
            prop_assert!(encode_eon(&ops).is_err());
        }
    }

    // ── encode_mul: MUL Rd, Rn, Rm  ==  MADD Rd, Rn, Rm, XZR ──
    // ARMv8 MADD encoding:  sf 0 0 11011 000 Rm 0 Ra Rn Rd
    //   bits 31     : sf (register width)
    //   bits 30..21 : 0 0 11011 000   (fixed; data-processing, 3-source, MADD)
    //   bits 20..16 : Rm
    //   bit  15     : o0   (0 = MADD/MUL, 1 = MSUB/MNEG)
    //   bits 14..10 : Ra   (MUL forces XZR = 0b11111)
    //   bits  9..5  : Rn
    //   bits  4..0  : Rd
    proptest! {
        // 1. Every fixed field and every register field lands exactly where
        //    the ARMv8 MADD(MUL) encoding requires (rd/rn/rm < 31 keeps Ra
        //    distinct from Rd/Rm so a field-misplacement bug can't hide).
        #[test]
        fn mul_xregs_field_placement(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let w = expect_word(encode_mul(&ops));
            prop_assert_eq!(sf_of(w), 1);                      // 64-bit
            prop_assert_eq!((w >> 21) & 0x3FF, 0b0011011000);  // fixed opcode (bits 30..21)
            prop_assert_eq!((w >> 15) & 1, 0);                 // o0 = 0 (MADD, not MSUB)
            prop_assert_eq!((w >> 10) & 0x1F, 0b11111);        // Ra = XZR
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 2. The sf bit (31) tracks register width: X-regs -> 1, W-regs -> 0.
        #[test]
        fn mul_sf_tracks_width(n in 0u32..=30, is_w in any::<bool>()) {
            let name = if is_w { format!("w{}", n) } else { format!("x{}", n) };
            let ops = vec![
                Operand::Reg(name.clone()),
                Operand::Reg(name.clone()),
                Operand::Reg(name),
            ];
            let w = expect_word(encode_mul(&ops));
            prop_assert_eq!(sf_of(w), if is_w { 0 } else { 1 });
        }

        // 3. DIFFERENTIAL / reference oracle: the encoder's own comment states
        //    "MUL Rd, Rn, Rm is MADD Rd, Rn, Rm, XZR". The two encoders must
        //    therefore emit the identical 32-bit word for every width/operand set.
        #[test]
        fn mul_equals_madd_with_xzr(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
            is_w in any::<bool>(),
        ) {
            let pf = |n: u32| if is_w { format!("w{}", n) } else { format!("x{}", n) };
            let mul_ops = vec![Operand::Reg(pf(rd)), Operand::Reg(pf(rn)), Operand::Reg(pf(rm))];
            let zr = if is_w { "wzr" } else { "xzr" };
            let madd_ops = vec![Operand::Reg(pf(rd)), Operand::Reg(pf(rn)),
                                Operand::Reg(pf(rm)), Operand::Reg(zr.into())];
            let mw = expect_word(encode_mul(&mul_ops));
            let aw = expect_word(encode_madd(&madd_ops));
            prop_assert_eq!(mw, aw);
        }

        // 4. Width is encoded purely in bit 31: swapping X<->W for the same
        //    register numbers must change only the sf bit (all other bits equal).
        #[test]
        fn mul_width_only_flips_sf_bit(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
        ) {
            let xw = expect_word(encode_mul(&[xreg(rd), xreg(rn), xreg(rm)]));
            let ww = expect_word(encode_mul(&[
                Operand::Reg(format!("w{}", rd)),
                Operand::Reg(format!("w{}", rn)),
                Operand::Reg(format!("w{}", rm)),
            ]));
            prop_assert_eq!(xw ^ ww, 1u32 << 31);
        }

        // 5. NEGATIVE CONTRACT: MUL needs exactly 3 register operands (Rd,Rn,Rm);
        //    fewer must be rejected rather than silently producing a bad word.
        #[test]
        fn mul_rejects_too_few_operands(n in 0u32..=2u32) {
            let ops: Vec<Operand> = (0..n).map(xreg).collect();
            prop_assert!(encode_mul(&ops).is_err());
        }

        // 6. NEGATIVE CONTRACT: a non-register operand (e.g. an immediate where
        //    Rm is expected) must be rejected — no silent acceptance.
        #[test]
        fn mul_rejects_immediate_operand(
            rd in 0u32..=30,
            rn in 0u32..=30,
            imm in 0i64..=0xFFF,
        ) {
        }
    }

    // ── encode_madd ──────────────────────────────────────────────────────────
    // MADD encoding (ARMv8-A):  sf 0 0 11011 000 Rm o0 Ra Rn Rd
    //   bit 31    = sf  (0=W, 1=X)
    //   bits 30:21 = 0b0011011000  (fixed opcode)
    //   bits 20:16 = Rm
    //   bit 15     = o0 (0 = MADD, 1 = MSUB)
    //   bits 14:10 = Ra   bits 9:5 = Rn   bits 4:0 = Rd
    fn ra_of(w: u32) -> u32       { (w >> 10) & 0x1F }      // bits 14:10
    fn o0_of(w: u32) -> u32       { (w >> 15) & 1 }         // bit 15
    fn opcode10_of(w: u32) -> u32 { (w >> 21) & 0x3FF }     // bits 30:21

    proptest! {
        // 1. FIELD PLACEMENT: every fixed field and every register field lands
        //    exactly where the ARMv8 MADD spec dictates, for both widths and
        //    with Ra=31 (xzr) allowed since that is the MUL alias.
        #[test]
        fn madd_field_placement(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
            ra in 0u32..=31,
            is_w in any::<bool>(),
        ) {
            let pf = |n: u32| if is_w { format!("w{}", n) } else { format!("x{}", n) };
            let ops = vec![Operand::Reg(pf(rd)), Operand::Reg(pf(rn)),
                           Operand::Reg(pf(rm)), Operand::Reg(pf(ra))];
            let w = expect_word(encode_madd(&ops));
            prop_assert_eq!(sf_of(w), if is_w { 0 } else { 1 });
            prop_assert_eq!(opcode10_of(w), 0b0011011000); // bits 30:21 fixed
            prop_assert_eq!(o0_of(w), 0);                  // MADD: o0 == 0
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(ra_of(w), ra);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 2. DIFFERENTIAL: MADD and MSUB differ ONLY in the o0 bit (bit 15).
        //    XOR of the two words must equal exactly (1 << 15) for every input.
        #[test]
        fn madd_msub_differ_only_in_o0(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
            ra in 0u32..=31,
            is_w in any::<bool>(),
        ) {
            let pf = |n: u32| if is_w { format!("w{}", n) } else { format!("x{}", n) };
            let ops = vec![Operand::Reg(pf(rd)), Operand::Reg(pf(rn)),
                           Operand::Reg(pf(rm)), Operand::Reg(pf(ra))];
            let mw = expect_word(encode_madd(&ops));
            let sw = expect_word(encode_msub(&ops));
            prop_assert_eq!(mw ^ sw, 1u32 << 15);
        }

        // 3. WIDTH INVARIANCE: sf (bit 31) is the only bit that changes when
        //    swapping X<->W for identical register numbers.
        #[test]
        fn madd_width_only_flips_sf_bit(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
            ra in 0u32..=31,
        ) {
            let pf = |p: &str, n: u32| format!("{}{}", p, n);
            let xw = expect_word(encode_madd(&[
                Operand::Reg(pf("x", rd)), Operand::Reg(pf("x", rn)),
                Operand::Reg(pf("x", rm)), Operand::Reg(pf("x", ra))]));
            let ww = expect_word(encode_madd(&[
                Operand::Reg(pf("w", rd)), Operand::Reg(pf("w", rn)),
                Operand::Reg(pf("w", rm)), Operand::Reg(pf("w", ra))]));
            prop_assert_eq!(xw ^ ww, 1u32 << 31);
        }

        // 4. NEGATIVE CONTRACT: MADD needs exactly 4 register operands; any
        //    count < 4 must be rejected rather than emitting a garbage word.
        #[test]
        fn madd_rejects_too_few_operands(n in 0u32..=3u32) {
            let ops: Vec<Operand> = (0..n).map(xreg).collect();
            prop_assert!(encode_madd(&ops).is_err());
        }

        // 5. ALGEBRAIC (reference oracle): the ARMv8 aliasing rule
        //    "MUL Rd,Rn,Rm == MADD Rd,Rn,Rm,XZR" requires that encoding MADD
        //    with Ra=31 (xzr) yield the identical word to MUL, per width.
        #[test]
        fn madd_ra_xzr_equals_mul(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
            is_w in any::<bool>(),
        ) {
            let pf = |n: u32| if is_w { format!("w{}", n) } else { format!("x{}", n) };
            let zr = if is_w { "wzr" } else { "xzr" };
            let madd_ops = vec![Operand::Reg(pf(rd)), Operand::Reg(pf(rn)),
                                Operand::Reg(pf(rm)), Operand::Reg(zr.into())];
            let mul_ops = vec![Operand::Reg(pf(rd)), Operand::Reg(pf(rn)), Operand::Reg(pf(rm))];
            prop_assert_eq!(expect_word(encode_madd(&madd_ops)),
                            expect_word(encode_mul(&mul_ops)));
        }
    }

    // CHARACTERIZATION (functional finding, see COVERAGE.md): the encoder
    // derives the sf bit ONLY from operand 0 (Rd) and never validates that all
    // four operands share the same width. `madd x0, w1, x2, x3` is silently
    // encoded as a 64-bit (sf=1) MADD even though Rn is a 32-bit register.
    // AArch64 requires a consistent width across all operands.
    #[test]
    fn madd_silently_accepts_mixed_width_operands() {
        let ops = vec![Operand::Reg("x0".into()), Operand::Reg("w1".into()),
                       Operand::Reg("x2".into()), Operand::Reg("x3".into())];
        let r = encode_madd(&ops);
        println!("madd x0,w1,x2,x3 -> {:?}", r);
        // Documents current behavior: accepted with sf=1 (64-bit) from Rd only.
        if let Ok(EncodeResult::Word(w)) = r {
            println!("  sf = {}, rn field = {} (from w1, a 32-bit reg)", sf_of(w), rn_of(w));
            assert_eq!(sf_of(w), 1, "sf taken from Rd (x0), ignoring w1");
        }
    }

    // CHARACTERIZATION (temporary, for bug report): ARMv8 MADD/MUL permits
    // only X0-X30 or XZR in every operand; SP is UNPREDICTABLE/UNDEFINED.
    // The shared `get_reg` helper maps "sp"->31, so `mul x0,x1,sp` should be
    // rejected but is instead silently encoded as `mul x0,x1,xzr` (mul-by-0).
    #[test]
    fn mul_sp_in_rm_is_silently_accepted_as_xzr() {
        let ops = vec![Operand::Reg("x0".into()), Operand::Reg("x1".into()),
                       Operand::Reg("sp".into())];
        let r = encode_mul(&ops);
        // Document the current (buggy) behavior: succeeds, rm field == 31.
        println!("mul x0,x1,sp -> {:?}", r);
        if let Ok(EncodeResult::Word(w)) = r {
            println!("  rm field = {} (== 31 means XZR, not SP)", rm_of(w));
        }
    }

    // ── Field extractors for the data-processing (3-source) format ──────────
    // MSUB/MADD: sf 00 11011 000 Rm o1 Ra Rn Rd
    fn m_sf(w: u32) -> u32 { (w >> 31) & 1 }
    fn m_rm(w: u32) -> u32 { (w >> 16) & 0x1F }
    fn m_o1(w: u32) -> u32 { (w >> 15) & 1 } // bit 15: 1 = MSUB, 0 = MADD
    fn m_ra(w: u32) -> u32 { (w >> 10) & 0x1F }
    fn m_rn(w: u32) -> u32 { (w >> 5) & 0x1F }
    fn m_rd(w: u32) -> u32 { w & 0x1F }

    proptest! {
        // Reference oracle: ARMv8 fixed-format spec. MSUB encodes as
        //   sf 00 11011 000 Rm 1 Ra Rn Rd   (Rd = Ra - Rn*Rm)
        // Every fixed field and every register field must land exactly where
        // the architecture manual dictates. Register 31 == XZR, a legal data-
        // processing operand (MNEG == MSUB ... , XZR), so 0..=31 is the full range.
        #[test]
        fn msub_full_field_placement(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            ra in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm), xreg(ra)];
            let w = expect_word(encode_msub(&ops));
            prop_assert_eq!(m_sf(w), 1);                       // 64-bit (X regs)
            prop_assert_eq!((w >> 21) & 0x3FF, 0b0011011000);   // op=00,11011,o0=000
            prop_assert_eq!(m_o1(w), 1);                       // MSUB bit (15) set
            prop_assert_eq!(m_rm(w), rm);
            prop_assert_eq!(m_ra(w), ra);
            prop_assert_eq!(m_rn(w), rn);
            prop_assert_eq!(m_rd(w), rd);
        }

        // Differential oracle: MSUB and MADD are the two members of the
        // data-processing (3-source) class sharing this opcode; they differ in
        // precisely one bit (bit 15). MSUB must set it, MADD must clear it.
        #[test]
        fn msub_differs_from_madd_only_in_bit15(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            ra in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm), xreg(ra)];
            let ms = expect_word(encode_msub(&ops));
            let ma = expect_word(encode_madd(&ops));
            prop_assert_eq!(ms ^ ma, 1u32 << 15); // exactly one bit differs
            prop_assert_eq!(m_o1(ms), 1);
            prop_assert_eq!(m_o1(ma), 0);
        }

        // Width contract: the sf bit (31) tracks the destination register's
        // width only — Xd -> sf=1, Wd -> sf=0. (This encoder ignores the width
        // of Rn/Rm/Ra, matching its documented behavior.)
        #[test]
        fn msub_sf_tracks_rd_width(
            n in 0u32..=30,
            rd_is_x in any::<bool>(),
        ) {
            let rd = if rd_is_x { xreg(n) } else { Operand::Reg(format!("w{}", n)) };
            let ops = vec![rd, xreg(0), xreg(1), xreg(2)];
            let w = expect_word(encode_msub(&ops));
            prop_assert_eq!(m_sf(w), if rd_is_x { 1 } else { 0 });
        }

        // Purity: encoding is a pure function of its operands, so repeated
        // calls with identical input must yield byte-identical output.
        #[test]
        fn msub_is_deterministic(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            ra in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm), xreg(ra)];
            let a = expect_word(encode_msub(&ops));
            let b = expect_word(encode_msub(&ops));
            prop_assert_eq!(a, b);
        }

        // Negative/error contract: MSUB requires exactly 4 register operands.
        // Fewer must be rejected with Err rather than silently producing a
        // mis-encoded word from a missing (zeroed) register field.
        #[test]
        fn msub_missing_operands_return_err(n in 0usize..=3) {
            let mut ops = vec![xreg(0), xreg(1), xreg(2), xreg(3)];
            ops.truncate(n);
            prop_assert!(encode_msub(&ops).is_err(),
                "encode_msub with {} operands should error", n);
        }
    }

    // ── DIV (SDIV/UDIV) field extractors ──────────────────────────────────
    // ARMv8 data-processing (2 source): sf 0 0 11010110 Rm 00001 o1 Rn Rd
    fn d_sf(w: u32) -> u32      { (w >> 31) & 1 }
    fn d_op30(w: u32) -> u32    { (w >> 30) & 1 }    // reserved, must be 0
    fn d_s(w: u32) -> u32       { (w >> 29) & 1 }    // S, must be 0
    fn d_opcode8(w: u32) -> u32 { (w >> 21) & 0xFF } // bits 28:21 == 0b11010110
    fn d_rm(w: u32) -> u32      { (w >> 16) & 0x1F }
    fn d_opcode6(w: u32) -> u32 { (w >> 10) & 0x3F } // 000011=SDIV, 000010=UDIV
    fn d_o1(w: u32) -> u32      { (w >> 10) & 1 }
    fn d_rn(w: u32) -> u32      { (w >> 5) & 0x1F }
    fn d_rd(w: u32) -> u32      { w & 0x1F }

    proptest! {
        // 1. SDIV field placement: every fixed field (sf=1, reserved bit 30=0,
        //    S=0, opcode bits 28:21=11010110, opcode6=000011, o1=1) and every
        //    register field (Rm/Rn/Rd) lands exactly per the ARMv8 ARM.
        #[test]
        fn div_sdiv_field_placement(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let w = expect_word(encode_div(&ops, false)); // SDIV (signed)
            prop_assert_eq!(d_sf(w), 1);
            prop_assert_eq!(d_op30(w), 0);              // reserved bit, must be 0
            prop_assert_eq!(d_s(w), 0);                 // S=0
            prop_assert_eq!(d_opcode8(w), 0b11010110);  // bits 28:21
            prop_assert_eq!(d_rm(w), rm);
            prop_assert_eq!(d_opcode6(w), 0b000011);    // SDIV opcode
            prop_assert_eq!(d_o1(w), 1);
            prop_assert_eq!(d_rn(w), rn);
            prop_assert_eq!(d_rd(w), rd);
        }

        // 2. UDIV field placement: identical to SDIV except opcode6=000010 and
        //    o1=0 (the only bits distinguishing UDIV from SDIV).
        #[test]
        fn div_udiv_field_placement(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let w = expect_word(encode_div(&ops, true)); // UDIV (unsigned)
            prop_assert_eq!(d_sf(w), 1);
            prop_assert_eq!(d_op30(w), 0);
            prop_assert_eq!(d_s(w), 0);
            prop_assert_eq!(d_opcode8(w), 0b11010110);
            prop_assert_eq!(d_rm(w), rm);
            prop_assert_eq!(d_opcode6(w), 0b000010);    // UDIV opcode
            prop_assert_eq!(d_o1(w), 0);
            prop_assert_eq!(d_rn(w), rn);
            prop_assert_eq!(d_rd(w), rd);
        }

        // 3. sf (bit 31) tracks the destination register width: W -> 0, X -> 1.
        //    (All three operands kept at the same width to avoid the separate
        //    mixed-width finding documented elsewhere.)
        #[test]
        fn div_sf_tracks_register_width(
            n in 0u32..=30,
            is_w in any::<bool>(),
        ) {
            let rd = if is_w { Operand::Reg(format!("w{}", n)) } else { xreg(n) };
            let rn = if is_w { Operand::Reg("w0".into()) } else { xreg(0) };
            let rm = if is_w { Operand::Reg("w1".into()) } else { xreg(1) };
            let ops = vec![rd, rn, rm];
            let w = expect_word(encode_div(&ops, false));
            prop_assert_eq!(d_sf(w), if is_w { 0 } else { 1 });
        }

        // 4. Differential: for identical operands, SDIV and UDIV encodings
        //    differ in EXACTLY one bit — o1 (bit 10). Algebraic relationship
        //    between the two halves of encode_div (sibling-encoder oracle).
        #[test]
        fn div_sdiv_udiv_differ_only_in_o1(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let s = expect_word(encode_div(&ops, false)); // SDIV
            let u = expect_word(encode_div(&ops, true));  // UDIV
            prop_assert_eq!(s ^ u, 1u32 << 10);           // only bit 10 differs
            prop_assert_ne!(s, u);
            prop_assert_eq!(d_o1(s), 1);
            prop_assert_eq!(d_o1(u), 0);
        }

        // 5. Register-field isolation: Rm occupies only bits 20:16, Rn only
        //    bits 9:5, Rd only bits 4:0 — varying one register never bleeds
        //    into another field (no aliasing / truncation between fields).
        #[test]
        fn div_register_fields_isolated(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            rd2 in 0u32..=30, rn2 in 0u32..=30, rm2 in 0u32..=30,
        ) {
            prop_assume!(rm != rm2);
            prop_assume!(rn != rn2);
            prop_assume!(rd != rd2);
            let base = expect_word(encode_div(&[xreg(rd), xreg(rn), xreg(rm)], false));
            // Rm changes only bits 20:16
            let w_rm = expect_word(encode_div(&[xreg(rd), xreg(rn), xreg(rm2)], false));
            let diff = base ^ w_rm;
            prop_assert_eq!(diff & !0x001F0000, 0);
            prop_assert_eq!((diff >> 16) & 0x1F, rm ^ rm2);
            // Rn changes only bits 9:5
            let w_rn = expect_word(encode_div(&[xreg(rd), xreg(rn2), xreg(rm)], false));
            let diff = base ^ w_rn;
            prop_assert_eq!(diff & !0x000003E0, 0);
            prop_assert_eq!((diff >> 5) & 0x1F, rn ^ rn2);
            // Rd changes only bits 4:0
            let w_rd = expect_word(encode_div(&[xreg(rd2), xreg(rn), xreg(rm)], false));
            let diff = base ^ w_rd;
            prop_assert_eq!(diff & !0x0000001F, 0);
            prop_assert_eq!(diff & 0x1F, rd ^ rd2);
        }
    }

    // ── encode_smull: SMULL Xd, Wn, Wm -> SMADDL Xd, Wn, Wm, XZR ──────────────
    // ARMv8 SMADDL reference with Ra = XZR (31):
    //   bit 31 = 1 (sf)            bits 30:29 = 00
    //   bits 28:24 = 11011         bits 23:21 = 001
    //   bits 20:16 = Rm            bit 15 = 0
    //   bits 14:10 = Ra (= 11111)   bits 9:5 = Rn   bits 4:0 = Rd
    fn smull_ref(rd: u32, rn: u32, rm: u32) -> u32 {
        0x9B207C00u32 | (rm << 16) | (rn << 5) | rd
    }

    proptest! {
        // 1. Differential: every valid register triple encodes to exactly the
        //    ARMv8 SMADDL-with-XZR word. Strongest spec check.
        #[test]
        fn smull_matches_armv8_reference(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let w = expect_word(encode_smull(&ops));
            prop_assert_eq!(w, smull_ref(rd, rn, rm));
        }

        // 2. All non-register bits are the constant SMADDL+XZR opcode,
        //    independent of which registers are chosen.
        #[test]
        fn smull_opcode_bits_constant(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let w = expect_word(encode_smull(&ops));
            // Register fields: bits 4:0 (Rd), 9:5 (Rn), 20:16 (Rm).
            let reg_mask = 0x001F0000u32 | 0x000003E0u32 | 0x0000001Fu32; // 0x001F03FF
            prop_assert_eq!(w & !reg_mask, 0x9B207C00u32);
            // Spot-check each fixed field against the spec.
            prop_assert_eq!((w >> 31) & 1, 1);                 // sf
            prop_assert_eq!((w >> 29) & 0x3, 0b00);            // bits 30:29
            prop_assert_eq!((w >> 24) & 0x1F, 0b11011);        // opcode
            prop_assert_eq!((w >> 21) & 0x7, 0b001);           // class (SMADDL)
            prop_assert_eq!((w >> 15) & 1, 0);                 // o0 bit = 0
            prop_assert_eq!((w >> 10) & 0x1F, 0b11111);        // Ra = XZR = 31
        }

        // 3. Each register is placed in exactly its 5-bit field and changing
        //    it perturbs only that field: Rd -> 4:0, Rn -> 9:5, Rm -> 20:16.
        #[test]
        fn smull_register_fields_isolated(
            rd in 0u32..=31, rn in 0u32..=31, rm in 0u32..=31,
            rd2 in 0u32..=31, rn2 in 0u32..=31, rm2 in 0u32..=31,
        ) {
            let base = expect_word(encode_smull(&[xreg(rd), xreg(rn), xreg(rm)]));
            // Rd -> bits 4:0
            let w = expect_word(encode_smull(&[xreg(rd2), xreg(rn), xreg(rm)]));
            let diff = base ^ w;
            prop_assert_eq!(diff & !0x0000001Fu32, 0);
            prop_assert_eq!(diff & 0x1F, rd ^ rd2);
            // Rn -> bits 9:5
            let w = expect_word(encode_smull(&[xreg(rd), xreg(rn2), xreg(rm)]));
            let diff = base ^ w;
            prop_assert_eq!(diff & !0x000003E0u32, 0);
            prop_assert_eq!((diff >> 5) & 0x1F, rn ^ rn2);
            // Rm -> bits 20:16
            let w = expect_word(encode_smull(&[xreg(rd), xreg(rn), xreg(rm2)]));
            let diff = base ^ w;
            prop_assert_eq!(diff & !0x001F0000u32, 0);
            prop_assert_eq!((diff >> 16) & 0x1F, rm ^ rm2);
        }

        // 4. Determinism: identical operands always yield the identical word.
        #[test]
        fn smull_deterministic(
            rd in 0u32..=31, rn in 0u32..=31, rm in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let a = expect_word(encode_smull(&ops));
            let b = expect_word(encode_smull(&ops));
            prop_assert_eq!(a, b);
        }

        // 5. Negative / error contract: SMULL requires exactly 3 register
        //    operands; non-register operands and out-of-range register numbers
        //    (> 31) are rejected with Err (no silent truncation, no panic).
        #[test]
        fn smull_rejects_invalid_operands(
            n in 0u32..=2u32,                      // too few operands
            bad in 32u32..=4096u32,               // out-of-range register
            pos in 0u32..=2u32,                   // which operand is non-register
        ) {
            // Too few operands -> Err
            let ops: Vec<Operand> = (0..n).map(|i| xreg(i % 31)).collect();
            prop_assert!(encode_smull(&ops).is_err());

            // A non-register operand anywhere -> Err
            let mut ops = vec![xreg(0), xreg(1), xreg(2)];
            ops[pos as usize] = Operand::Imm(7);
            prop_assert!(encode_smull(&ops).is_err());

            // Out-of-range register number -> Err (parse_reg_num caps at 31)
            let ops = vec![Operand::Reg(format!("x{}", bad)), xreg(1), xreg(2)];
            prop_assert!(encode_smull(&ops).is_err());
        }
    }

    // ── encode_umull: UMULL Xd, Wn, Wm -> UMADDL Xd, Wn, Wm, XZR ──────────────
    // ARMv8 UMADDL reference with Ra = XZR (31):
    //   bit 31 = 1 (sf)            bits 30:29 = 00
    //   bits 28:24 = 11011         bits 23:21 = 101   (UMADDL class; SMADDL is 001)
    //   bits 20:16 = Rm            bit 15 = 0
    //   bits 14:10 = Ra (= 11111)   bits 9:5 = Rn   bits 4:0 = Rd
    //   => fixed base word 0x9BA07C00 | (Rm<<16) | (Rn<<5) | Rd
    fn umull_ref(rd: u32, rn: u32, rm: u32) -> u32 {
        0x9BA07C00u32 | (rm << 16) | (rn << 5) | rd
    }

    proptest! {
        // 1. Differential/reference: every valid register triple encodes to
        //    exactly the ARMv8 UMADDL (Ra=XZR) word. Oracle: reference model
        //    derived independently from the ARM ARM bit layout.
        #[test]
        fn umull_matches_reference(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let w = expect_word(encode_umull(&ops));
            prop_assert_eq!(w, umull_ref(rd, rn, rm));
        }

        // 2. All non-register bits are the constant UMADDL+XZR opcode, and
        //    every fixed field matches the spec: op3=101 (NOT SMADDL's 001).
        #[test]
        fn umull_opcode_bits_constant(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let w = expect_word(encode_umull(&ops));
            let reg_mask = 0x001F0000u32 | 0x000003E0u32 | 0x0000001Fu32; // 0x001F03FF
            prop_assert_eq!(w & !reg_mask, 0x9BA07C00u32);
            prop_assert_eq!((w >> 31) & 1, 1);                 // sf = 1 (always 64-bit)
            prop_assert_eq!((w >> 29) & 0x3, 0b00);            // bits 30:29
            prop_assert_eq!((w >> 24) & 0x1F, 0b11011);       // opcode
            prop_assert_eq!((w >> 21) & 0x7, 0b101);          // class = UMADDL
            prop_assert_eq!((w >> 15) & 1, 0);                // o0 = 0 (additive)
            prop_assert_eq!((w >> 10) & 0x1F, 0b11111);       // Ra = XZR = 31
        }

        // 3. Each register is placed in exactly its 5-bit field and changing
        //    it perturbs only that field: Rd -> 4:0, Rn -> 9:5, Rm -> 20:16.
        #[test]
        fn umull_register_fields_isolated(
            rd in 0u32..=31, rn in 0u32..=31, rm in 0u32..=31,
            rd2 in 0u32..=31, rn2 in 0u32..=31, rm2 in 0u32..=31,
        ) {
            let base = expect_word(encode_umull(&[xreg(rd), xreg(rn), xreg(rm)]));
            // Rd -> bits 4:0
            let w = expect_word(encode_umull(&[xreg(rd2), xreg(rn), xreg(rm)]));
            let diff = base ^ w;
            prop_assert_eq!(diff & !0x0000001Fu32, 0);
            prop_assert_eq!(diff & 0x1F, rd ^ rd2);
            // Rn -> bits 9:5
            let w = expect_word(encode_umull(&[xreg(rd), xreg(rn2), xreg(rm)]));
            let diff = base ^ w;
            prop_assert_eq!(diff & !0x000003E0u32, 0);
            prop_assert_eq!((diff >> 5) & 0x1F, rn ^ rn2);
            // Rm -> bits 20:16
            let w = expect_word(encode_umull(&[xreg(rd), xreg(rn), xreg(rm2)]));
            let diff = base ^ w;
            prop_assert_eq!(diff & !0x001F0000u32, 0);
            prop_assert_eq!((diff >> 16) & 0x1F, rm ^ rm2);
        }

        // 4. Differential vs SMULL: signed vs unsigned long multiply share the
        //    entire encoding except op3 (bits 23:21). UMULL=101, SMULL=001, so
        //    the words differ by exactly one bit: bit 23 (0x00800000).
        #[test]
        fn umull_vs_smull_differs_only_in_op3(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let u = expect_word(encode_umull(&ops));
            let s = expect_word(encode_smull(&ops));
            prop_assert_eq!(u ^ s, 0x00800000u32);
            prop_assert_eq!((u >> 23) & 1, 1);   // UMADDL op3 MSB (101) set
            prop_assert_eq!((s >> 23) & 1, 0);   // SMADDL op3 MSB (001) clear
        }

        // 5. Negative / error contract: UMULL requires exactly 3 register
        //    operands; too few operands, a non-register operand anywhere, and
        //    out-of-range register numbers (> 31) are all rejected with Err
        //    (no silent truncation, no panic).
        #[test]
        fn umull_rejects_invalid_operands(
            n in 0u32..=2u32,                      // too few operands
            bad in 32u32..=4096u32,               // out-of-range register
            pos in 0u32..=2u32,                   // which operand is non-register
        ) {
            // Too few operands -> Err
            let ops: Vec<Operand> = (0..n).map(|i| xreg(i % 31)).collect();
            prop_assert!(encode_umull(&ops).is_err());

            // A non-register operand anywhere -> Err
            let mut ops = vec![xreg(0), xreg(1), xreg(2)];
            ops[pos as usize] = Operand::Imm(7);
            prop_assert!(encode_umull(&ops).is_err());

            // Out-of-range register number -> Err (parse_reg_num caps at 31)
            let ops = vec![Operand::Reg(format!("x{}", bad)), xreg(1), xreg(2)];
            prop_assert!(encode_umull(&ops).is_err());
        }
    }

    // ── encode_mneg ───────────────────────────────────────────────────────
    // MNEG Xd, Xn, Xm is the architectural alias of MSUB Xd, Xn, Xm, XZR:
    //   sf 0 0 11011 000 Rm 1 11111 Rn Rd
    // o1 (bit 15) = 1 selects MSUB; Ra (bits 14:10) = 11111 (XZR).
    fn mneg_ref(rd: u32, rn: u32, rm: u32, is_64: bool) -> u32 {
        let sf = if is_64 { 1u32 } else { 0 };
        let mut w = 0u32;
        w |= sf << 31;            // [31]    size
        w |= 0b00 << 29;          // [30:29] reserved
        w |= 0b11011 << 24;       // [28:24] Data-processing (3 source)
        w |= 0b000 << 21;         // [23:21] o0 = MADD/MSUB class
        w |= (rm & 0x1F) << 16;   // [20:16] Rm
        w |= 1u32 << 15;          // [15]    o1 = 1 (MSUB)
        w |= 0b11111 << 10;       // [14:10] Ra = XZR = 31
        w |= (rn & 0x1F) << 5;    // [9:5]   Rn
        w |= rd & 0x1F;           // [4:0]   Rd
        w
    }

    proptest! {
        // 1. Every field lands where the ARMv8 spec dictates: full-word
        //    equality vs an independently constructed reference, for both
        //    32-bit (W) and 64-bit (X) destination widths.
        #[test]
        fn mneg_field_placement(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            is_w in any::<bool>(),
        ) {
            let rd_op = if is_w { Operand::Reg(format!("w{}", rd)) } else { xreg(rd) };
            let ops = vec![rd_op, xreg(rn), xreg(rm)];
            let w = expect_word(encode_mneg(&ops));
            prop_assert_eq!(w, mneg_ref(rd, rn, rm, !is_w));
            prop_assert_eq!((w >> 16) & 0x1F, rm);
            prop_assert_eq!((w >> 5) & 0x1F, rn);
            prop_assert_eq!(w & 0x1F, rd);
        }

        // 2. Each register perturbs ONLY its own 5-bit field (Rd -> 4:0,
        //    Rn -> 9:5, Rm -> 20:16); no register bleeds into another field
        //    or into the opcode bits.
        #[test]
        fn mneg_register_fields_isolated(
            rd in 0u32..=31, rn in 0u32..=31, rm in 0u32..=31,
            rd2 in 0u32..=31, rn2 in 0u32..=31, rm2 in 0u32..=31,
        ) {
            let base = expect_word(encode_mneg(&[xreg(rd), xreg(rn), xreg(rm)]));
            // Rd -> bits 4:0
            let w = expect_word(encode_mneg(&[xreg(rd2), xreg(rn), xreg(rm)]));
            let diff = base ^ w;
            prop_assert_eq!(diff & !0x0000001Fu32, 0);
            prop_assert_eq!(diff & 0x1F, rd ^ rd2);
            // Rn -> bits 9:5
            let w = expect_word(encode_mneg(&[xreg(rd), xreg(rn2), xreg(rm)]));
            let diff = base ^ w;
            prop_assert_eq!(diff & !0x000003E0u32, 0);
            prop_assert_eq!((diff >> 5) & 0x1F, rn ^ rn2);
            // Rm -> bits 20:16
            let w = expect_word(encode_mneg(&[xreg(rd), xreg(rn), xreg(rm2)]));
            let diff = base ^ w;
            prop_assert_eq!(diff & !0x001F0000u32, 0);
            prop_assert_eq!((diff >> 16) & 0x1F, rm ^ rm2);
        }

        // 3. Constant opcode fields are invariant across every register combo.
        //    Critically, o1 (bit 15) MUST be 1 (MSUB/negate) — never 0 (MADD) —
        //    and Ra (bits 14:10) MUST be 11111 (XZR).
        #[test]
        fn mneg_constant_fields_invariant(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            is_w in any::<bool>(),
        ) {
            let rd_op = if is_w { Operand::Reg(format!("w{}", rd)) } else { xreg(rd) };
            let ops = vec![rd_op, xreg(rn), xreg(rm)];
            let w = expect_word(encode_mneg(&ops));
            let reg_mask = 0x001F0000u32 | 0x000003E0u32 | 0x0000001Fu32; // Rm|Rn|Rd
            // 64-bit constant base = 0x9B00FC00; 32-bit = 0x1B00FC00
            let want = if is_w { 0x1B00FC00u32 } else { 0x9B00FC00u32 };
            prop_assert_eq!(w & !reg_mask, want);
            prop_assert_eq!((w >> 15) & 1, 1);            // o1 = 1 (MSUB, NOT MADD)
            prop_assert_eq!((w >> 10) & 0x1F, 0b11111);   // Ra = XZR = 31
            prop_assert_eq!((w >> 24) & 0x1F, 0b11011);   // 3-source opcode
            prop_assert_eq!((w >> 21) & 0x7, 0b000);      // o0
        }

        // 4. Negative contract: MNEG requires exactly 3 register operands.
        //    Too few operands, a non-register operand anywhere, and an
        //    out-of-range register number (> 31) are all rejected with Err
        //    (no silent truncation, no panic).
        #[test]
        fn mneg_rejects_invalid_operands(
            n in 0u32..=2u32,                      // too few operands
            bad in 32u32..=4096u32,                // out-of-range register
            pos in 0u32..=2u32,                    // which operand is non-register
        ) {
            // Too few operands -> Err
            let ops: Vec<Operand> = (0..n).map(|i| xreg(i % 31)).collect();
            prop_assert!(encode_mneg(&ops).is_err());

            // A non-register operand anywhere -> Err
            let mut ops = vec![xreg(0), xreg(1), xreg(2)];
            ops[pos as usize] = Operand::Imm(7);
            prop_assert!(encode_mneg(&ops).is_err());

            // Out-of-range register number -> Err (parse_reg_num caps at 31)
            let ops = vec![Operand::Reg(format!("x{}", bad)), xreg(1), xreg(2)];
            prop_assert!(encode_mneg(&ops).is_err());
        }

        // 5. Architectural alias: MNEG Xd, Xn, Xm == MSUB Xd, Xn, Xm, XZR.
        //    Per the ARM ARM, MNEG is defined as MSUB with Ra = XZR, so the two
        //    must produce bit-identical instruction words, both selecting MSUB
        //    (o1 = bit 15 = 1), never MADD (o1 = 0).
        #[test]
        fn mneg_alias_equals_msub_with_xzr(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let mneg = expect_word(encode_mneg(&ops));
            // encode_mneg matches the independently constructed reference word.
            prop_assert_eq!(mneg, mneg_ref(rd, rn, rm, true));
            let msub = expect_word(encode_msub(&[
                xreg(rd), xreg(rn), xreg(rm), Operand::Reg("xzr".into()),
            ]));
            // MNEG and MSUB+XZR must encode to the same instruction word.
            prop_assert_eq!(mneg, msub);
            // Both must select MSUB (o1 = bit 15 = 1), not MADD (o1 = 0).
            prop_assert_eq!((mneg >> 15) & 1, 1);
            prop_assert_eq!((msub >> 15) & 1, 1);
        }
    }

    // ── Field extractors for Data-processing (3 source): SMULH / UMULH ──
    // Layout:  sf 00 11011 op31 Rm o0 Ra Rn Rd
    // bit:     31 30:29 28:24 23:21 20:16 15 14:10 9:5 4:0
    // (o0_of / ra_of for bits 15 and 14:10 are reused from above.)
    fn op31_of(w: u32) -> u32 { (w >> 21) & 0x7 }

    /// SMULH reference word per ARMv8 ARM ("SMULH Xd, Xn, Xm"):
    ///   1 00 11011 010 Rm 0 11111 Rn Rd
    /// sf=1, op31=010, o0=0, Ra hardwired to XZR (11111). Signed multiply high.
    fn smulh_ref(rd: u32, rn: u32, rm: u32) -> u32 {
        (1u32 << 31) | (0b11011u32 << 24) | (0b010u32 << 21) | (rm << 16)
            | (0b11111u32 << 10) | (rn << 5) | rd
    }

    fn wreg(n: u32) -> Operand { Operand::Reg(format!("w{}", n)) }

    proptest! {
        // 1. Fixed fields: regardless of operands, SMULH pins sf=1, the 11011
        //    class opcode, op31=010, o0=0, and Ra=XZR (11111), per the ARMv8
        //    spec for "Signed multiply high".
        #[test]
        fn smulh_fixed_fields(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let w = expect_word(encode_smulh(&ops));
            prop_assert_eq!((w >> 31) & 1, 1);               // sf
            prop_assert_eq!((w >> 29) & 0x3, 0b00);          // bits 30:29
            prop_assert_eq!((w >> 24) & 0x1F, 0b11011);      // class opcode
            prop_assert_eq!(op31_of(w), 0b010);              // op31 selects SMULH
            prop_assert_eq!(o0_of(w), 0);                    // o0 = 0
            prop_assert_eq!(ra_of(w), 0b11111);              // Ra hardwired to XZR
        }

        // 2. Register field placement: Rd/Rn/Rm land in bits 4:0 / 9:5 / 20:16
        //    exactly as supplied, and the whole word matches an independent
        //    reference construction.
        #[test]
        fn smulh_register_field_placement(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let w = expect_word(encode_smulh(&ops));
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(w, smulh_ref(rd, rn, rm));
        }

        // 3. Width: SMULH is defined ONLY in the 64-bit (X) form, so the sf
        //    bit is always 1 for every valid operand combination.
        #[test]
        fn smulh_always_64bit(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let w = expect_word(encode_smulh(&ops));
            prop_assert_eq!((w >> 31) & 1, 1);
        }

        // 4. Differential vs UMULH: with identical operands, SMULH and UMULH
        //    differ ONLY in op31's sign-select bit (bit 23). All register
        //    fields, Ra=XZR, o0, and the class opcode are shared.
        #[test]
        fn smulh_vs_umulh_only_sign_bit_differs(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let s = expect_word(encode_smulh(&ops));
            let u = expect_word(encode_umulh(&ops));
            prop_assert_eq!(s ^ u, 1u32 << 23);
            // UMULH selects op31=110 (bit 23 set); SMULH op31=010 (clear).
            prop_assert_eq!((u >> 23) & 1, 1);
            prop_assert_eq!((s >> 23) & 1, 0);
        }

        // 5. NEGATIVE CONTRACT: SMULH has no 32-bit (W) form. The ARMv8 ARM
        //    defines SMULH exclusively as "SMULH Xd, Xn, Xm"; W-register
        //    operands are architecturally UNDEF and must be rejected with Err,
        //    not silently re-encoded as a 64-bit instruction.
        #[test]
        fn smulh_rejects_32bit_w_registers(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
        ) {
            let ops = vec![wreg(rd), wreg(rn), wreg(rm)];
            prop_assert!(encode_smulh(&ops).is_err());
        }
    }

    // ── UMULH (Unsigned multiply high) ─────────────────────────────────────
    // Oracle: reference (literal-spec) encoding derived from the ARMv8 ARM.
    // "UMULH Xd, Xn, Xm":  1 00 11011 110 Rm 0 11111 Rn Rd
    // sf=1 fixed (64-bit only), op31=110 (bit 23 set selects unsigned),
    // o0=0, Ra hardwired to XZR (11111). Differs from SMULH only in op31 bit 23.
    fn umulh_ref(rd: u32, rn: u32, rm: u32) -> u32 {
        (1u32 << 31) | (0b11011u32 << 24) | (0b110u32 << 21) | (rm << 16)
            | (0b11111u32 << 10) | (rn << 5) | rd
    }

    proptest! {
        // 1. Fixed fields: regardless of operands, UMULH pins sf=1, the 11011
        //    data-processing (3 source) class opcode, op31=110 (the unsigned
        //    multiply-high selector), o0=0, and Ra=XZR (11111), per the ARMv8
        //    spec for "Unsigned multiply high".
        #[test]
        fn umulh_fixed_fields(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let w = expect_word(encode_umulh(&ops));
            prop_assert_eq!((w >> 31) & 1, 1);               // sf always 1 (64-bit only)
            prop_assert_eq!((w >> 29) & 0x3, 0b00);          // bits 30:29 = 00
            prop_assert_eq!((w >> 24) & 0x1F, 0b11011);      // class opcode
            prop_assert_eq!(op31_of(w), 0b110);              // op31=110 selects UMULH
            prop_assert_eq!(o0_of(w), 0);                    // o0 = 0
            prop_assert_eq!(ra_of(w), 0b11111);              // Ra hardwired to XZR
        }

        // 2. Register placement + reference match: Rd/Rn/Rm land in bits 4:0 /
        //    9:5 / 20:16 exactly as supplied, and the whole word equals an
        //    independent literal-spec reconstruction of the UMULH encoding.
        #[test]
        fn umulh_register_fields_match_reference(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm)];
            let w = expect_word(encode_umulh(&ops));
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(w, umulh_ref(rd, rn, rm));
        }

        // 3. NEGATIVE CONTRACT (operand count): UMULH takes exactly three
        //    register operands. Any subset of fewer than three must yield Err
        //    (get_reg fails on the missing operand), never a truncated word.
        #[test]
        fn umulh_missing_operand_errors(
            n in 0u32..=2u32,         // 0, 1, or 2 operands — never the required 3
        ) {
            let ops: Vec<Operand> = (0..n).map(xreg).collect();
            prop_assert!(encode_umulh(&ops).is_err());
        }

        // 4. NEGATIVE CONTRACT (width): UMULH is defined ONLY in the 64-bit (X)
        //    form — "UMULH Xd, Xn, Xm". 32-bit (W) operands are architecturally
        //    UNDEF and must be rejected with Err, not silently re-encoded as a
        //    64-bit instruction. (Mirrors the SMULH finding; see bug report.)
        #[test]
        fn umulh_rejects_32bit_w_registers(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
        ) {
            let ops = vec![wreg(rd), wreg(rn), wreg(rm)];
            prop_assert!(encode_umulh(&ops).is_err());
        }
    }

    // ── encode_smaddl: SMADDL Xd, Wn, Wm, Xa (signed multiply-add long) ───────
    // ARMv8 SMADDL reference:
    //   bit 31 = 1 (sf)            bits 30:29 = 00
    //   bits 28:24 = 11011         bits 23:21 = 001 (op31)
    //   bits 20:16 = Rm            bit 15 = 0 (o0)
    //   bits 14:10 = Ra            bits 9:5 = Rn   bits 4:0 = Rd
    fn smaddl_ref(rd: u32, rn: u32, rm: u32, ra: u32) -> u32 {
        0x9B200000u32 | (rm << 16) | (ra << 10) | (rn << 5) | rd
    }

    proptest! {
        // 1. Differential / reference match: every valid register quadruple
        //    encodes to exactly the ARMv8 SMADDL word reconstructed from the
        //    bit-level specification. Strongest spec check.
        #[test]
        fn smaddl_matches_armv8_reference(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            ra in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm), xreg(ra)];
            let w = expect_word(encode_smaddl(&ops));
            prop_assert_eq!(w, smaddl_ref(rd, rn, rm, ra));
        }

        // 2. Fixed fields: regardless of operands, SMADDL pins sf=1 (64-bit
        //    only), bits 30:29 = 00, the data-processing (3 source) class
        //    opcode (11011), op31=001 (the SMADDL selector), and o0=0.
        #[test]
        fn smaddl_fixed_fields(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            ra in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm), xreg(ra)];
            let w = expect_word(encode_smaddl(&ops));
            prop_assert_eq!((w >> 31) & 1, 1);               // sf always 1 (64-bit only)
            prop_assert_eq!((w >> 29) & 0x3, 0b00);          // bits 30:29 = 00
            prop_assert_eq!(opcode5_of(w), 0b11011);         // data-processing (3 source) class
            prop_assert_eq!(op31_of(w), 0b001);              // op31=001 selects SMADDL
            prop_assert_eq!(o0_of(w), 0);                    // o0 = 0
        }

        // 3. Register field placement: Rd/Rn/Rm/Ra land in bits 4:0 / 9:5 /
        //    20:16 / 14:10 exactly as supplied — non-overlapping, no spill.
        #[test]
        fn smaddl_register_fields(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            ra in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm), xreg(ra)];
            let w = expect_word(encode_smaddl(&ops));
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(ra_of(w), ra);
        }

        // 4. NEGATIVE CONTRACT (operand count): SMADDL takes exactly four
        //    register operands. Any subset of fewer than four must yield Err
        //    (get_reg fails on the missing operand), never a truncated word.
        #[test]
        fn smaddl_missing_operand_errors(
            n in 0u32..=3u32,         // 0..3 operands — never the required 4
        ) {
            let ops: Vec<Operand> = (0..n).map(xreg).collect();
            prop_assert!(encode_smaddl(&ops).is_err());
        }

        // 5. NEGATIVE CONTRACT (width): SMADDL is "Xd, Wn, Wm, Xa" — it is
        //    defined ONLY in the 64-bit (sf=1) form, so the destination Rd
        //    (and accumulator Ra) MUST be a 64-bit X register. Supplying a
        //    32-bit W register for the destination is architecturally UNDEF
        //    and must be rejected with Err, not silently re-encoded as a
        //    64-bit instruction. (Mirrors the SMULH/UMULH width finding.)
        #[test]
        fn smaddl_rejects_w_destination_register(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
            ra in 0u32..=30,
        ) {
            let ops = vec![wreg(rd), xreg(rn), xreg(rm), xreg(ra)];
            prop_assert!(encode_smaddl(&ops).is_err());
        }
    }

    // ── encode_umaddl: UMADDL Xd, Wn, Wm, Xa (unsigned multiply-add long) ─────
    // ARMv8 UMADDL reference (data-processing, 3 source):
    //   bit 31 = 1 (sf)            bits 30:29 = 00
    //   bits 28:24 = 11011         bits 23:21 = 101 (op31, selects UMADDL)
    //   bits 20:16 = Rm            bit 15 = 0 (o0)
    //   bits 14:10 = Ra            bits 9:5 = Rn   bits 4:0 = Rd
    //   Rn/Rm are 32-bit (W) sources; Rd/Ra are 64-bit (X) dest/accumulator.
    fn umaddl_ref(rd: u32, rn: u32, rm: u32, ra: u32) -> u32 {
        0x9BA00000u32 | (rm << 16) | (ra << 10) | (rn << 5) | rd
    }

    proptest! {
        // 1. Differential / reference match: every valid register quadruple
        //    encodes to exactly the ARMv8 UMADDL word reconstructed from the
        //    bit-level spec (op31=101 -> 0x9BA00000 base). Strongest spec check.
        #[test]
        fn umaddl_matches_armv8_reference(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            ra in 0u32..=31,
        ) {
            // Architecturally correct widths: Xd, Wn, Wm, Xa.
            let ops = vec![xreg(rd), wreg(rn), wreg(rm), xreg(ra)];
            let w = expect_word(encode_umaddl(&ops));
            prop_assert_eq!(w, umaddl_ref(rd, rn, rm, ra));
        }

        // 2. Fixed fields: regardless of operands, UMADDL pins sf=1 (64-bit
        //    only), bits 30:29 = 00, the data-processing (3 source) class
        //    opcode (11011), op31=101 (the UMADDL selector), and o0=0.
        #[test]
        fn umaddl_fixed_fields(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            ra in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), wreg(rn), wreg(rm), xreg(ra)];
            let w = expect_word(encode_umaddl(&ops));
            prop_assert_eq!((w >> 31) & 1, 1);               // sf always 1 (64-bit only)
            prop_assert_eq!((w >> 29) & 0x3, 0b00);          // bits 30:29 = 00
            prop_assert_eq!(opcode5_of(w), 0b11011);         // data-processing (3 source) class
            prop_assert_eq!(op31_of(w), 0b101);              // op31=101 selects UMADDL
            prop_assert_eq!(o0_of(w), 0);                    // o0 = 0
        }

        // 3. Register field placement: Rd/Rn/Rm/Ra land in bits 4:0 / 9:5 /
        //    20:16 / 14:10 exactly as supplied — non-overlapping, no spill.
        #[test]
        fn umaddl_register_fields(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            ra in 0u32..=31,
        ) {
            let ops = vec![xreg(rd), wreg(rn), wreg(rm), xreg(ra)];
            let w = expect_word(encode_umaddl(&ops));
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(ra_of(w), ra);
        }

        // 4. NEGATIVE CONTRACT (operand count): UMADDL takes exactly four
        //    register operands. Any subset of fewer than four must yield Err
        //    (get_reg fails on the missing operand), never a truncated word.
        #[test]
        fn umaddl_missing_operand_errors(
            n in 0u32..=3u32,         // 0..3 operands — never the required 4
        ) {
            let ops: Vec<Operand> = (0..n).map(xreg).collect();
            prop_assert!(encode_umaddl(&ops).is_err());
        }

        // 5. NEGATIVE CONTRACT (width, destination): UMADDL is "Xd, Wn, Wm, Xa"
        //    — it is defined ONLY in the 64-bit (sf=1) form, so the destination
        //    Rd (and accumulator Ra) MUST be a 64-bit X register. Supplying a
        //    32-bit W register for the destination is architecturally UNDEF and
        //    must be rejected with Err, not silently re-encoded. (Mirrors the
        //    SMADDL width finding.)
        #[test]
        fn umaddl_rejects_w_destination_register(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
            ra in 0u32..=30,
        ) {
            let ops = vec![wreg(rd), wreg(rn), wreg(rm), xreg(ra)];
            prop_assert!(encode_umaddl(&ops).is_err());
        }

        // 6. NEGATIVE CONTRACT (width, sources): UMADDL widens 32x32->64, so
        //    the multiplier inputs Rn and Rm MUST be 32-bit (W) registers.
        //    Supplying 64-bit (X) source registers is a syntax error / UNDEF
        //    and must be rejected with Err. (UMADDL-specific: unlike SMADDL,
        //    the "long" form constrains width in BOTH directions.)
        #[test]
        fn umaddl_rejects_x_source_registers(
            rd in 0u32..=30,
            rn in 0u32..=30,
            rm in 0u32..=30,
            ra in 0u32..=30,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm), xreg(ra)];
            prop_assert!(encode_umaddl(&ops).is_err());
        }
    }

    // ── encode_sbc: SBC/SBCS Rd, Rn, Rm (subtract with carry) ──────────────
    // Oracle: reference (literal-spec) encoding from the ARMv8 ARM.
    // "Add/subtract (with carry)" group:  sf 1 S 11010000 Rm 000000 Rn Rd
    //   bit 31 = sf           bit 30 = 1 (op: subtract)        bit 29 = S
    //   bits 28:21 = 11010000  bits 20:16 = Rm   bits 15:10 = 000000 (imm6, fixed 0)
    //   bits 9:5 = Rn          bits 4:0 = Rd
    //   SBC = set_flags=false -> S=0 ;  SBCS = set_flags=true -> S=1.
    //   Differs from ADC (op=0) ONLY in bit 30.
    fn sbc_ref(rd: u32, rn: u32, rm: u32, is_64: bool, set_flags: bool) -> u32 {
        let sf = if is_64 { 1u32 } else { 0 };
        let s = if set_flags { 1u32 } else { 0 };
        (sf << 31) | (1u32 << 30) | (s << 29) | (0b11010000u32 << 21)
            | (rm << 16) | (rn << 5) | rd
    }

    proptest! {
        // 1. Reference / differential match: every valid (rd, rn, rm, width,
        //    set_flags) encodes to exactly the ARMv8 SBC/SBCS word rebuilt from
        //    the bit-level spec. Strongest spec check (no input is an immediate,
        //    shift, lane, or relocation field, so no truncation concern).
        #[test]
        fn sbc_matches_armv8_reference(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            is_64 in any::<bool>(),
            set_flags in any::<bool>(),
        ) {
            let mk = |n: u32| if is_64 { xreg(n) } else { wreg(n) };
            let ops = vec![mk(rd), mk(rn), mk(rm)];
            let w = expect_word(encode_sbc(&ops, set_flags));
            prop_assert_eq!(w, sbc_ref(rd, rn, rm, is_64, set_flags));
        }

        // 2. Fixed fields: regardless of operands, the op bit (bit 30) is 1,
        //    the add/sub-with-carry opcode (bits 28:21) is 11010000, and the
        //    imm6 field (bits 15:10) is hardwired to 0 per the ARMv8 spec.
        #[test]
        fn sbc_fixed_fields(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            is_64 in any::<bool>(),
            set_flags in any::<bool>(),
        ) {
            let mk = |n: u32| if is_64 { xreg(n) } else { wreg(n) };
            let ops = vec![mk(rd), mk(rn), mk(rm)];
            let w = expect_word(encode_sbc(&ops, set_flags));
            prop_assert_eq!((w >> 30) & 1, 1);                   // op = subtract
            prop_assert_eq!((w >> 21) & 0xFF, 0b11010000u32);    // add/sub-with-carry opcode
            prop_assert_eq!((w >> 10) & 0x3F, 0);                // imm6 fixed to 0
        }

        // 3. Register field placement + sf width: Rd/Rn/Rm land in bits
        //    4:0 / 9:5 / 20:16 exactly as supplied, and sf (bit 31) tracks the
        //    register width (W -> 0, X -> 1) for every operand combination.
        #[test]
        fn sbc_register_fields_and_width(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            is_64 in any::<bool>(),
        ) {
            let mk = |n: u32| if is_64 { xreg(n) } else { wreg(n) };
            let ops = vec![mk(rd), mk(rn), mk(rm)];
            let w = expect_word(encode_sbc(&ops, false));
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(sf_of(w), if is_64 { 1 } else { 0 });
        }

        // 4. S bit tracks set_flags (the SBC vs SBCS distinction): bit 29 is 0
        //    for SBC and 1 for SBCS, and ONLY bit 29 changes between them.
        #[test]
        fn sbc_s_bit_tracks_set_flags(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            is_64 in any::<bool>(),
        ) {
            let mk = |n: u32| if is_64 { xreg(n) } else { wreg(n) };
            let ops = vec![mk(rd), mk(rn), mk(rm)];
            let sbc  = expect_word(encode_sbc(&ops, false));
            let sbcs = expect_word(encode_sbc(&ops, true));
            prop_assert_eq!(s_of(sbc), 0);
            prop_assert_eq!(s_of(sbcs), 1);
            // Only bit 29 differs between the two encodings.
            prop_assert_eq!(sbc ^ sbcs, 1u32 << 29);
        }

        // 5. Differential vs ADC: with identical operands and identical
        //    set_flags, SBC and ADC differ ONLY in the op bit (bit 30). This
        //    is the defining structural distinction between the two
        //    "add/subtract (with carry)" siblings.
        #[test]
        fn sbc_vs_adc_only_op_bit_differs(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            is_64 in any::<bool>(),
            set_flags in any::<bool>(),
        ) {
            let mk = |n: u32| if is_64 { xreg(n) } else { wreg(n) };
            let ops = vec![mk(rd), mk(rn), mk(rm)];
            let sbc = expect_word(encode_sbc(&ops, set_flags));
            let adc = expect_word(encode_adc(&ops, set_flags));
            prop_assert_eq!(sbc ^ adc, 1u32 << 30);
            prop_assert_eq!((adc >> 30) & 1, 0);  // ADC op = 0
            prop_assert_eq!((sbc >> 30) & 1, 1);  // SBC op = 1
        }
    }
}

// ── encode_mvn property tests ────────────────────────────────────────────
// Scalar MVN Xd, Xm [, shift] is an alias of ORN Xd, XZR, Xm [, shift]:
//   sf opc[30:29]=01 01010[28:24] shift[23:22] N[21]=1 Rm[20:16] imm6[15:10] Rn[9:5]=11111 Rd[4:0]
#[cfg(test)]
mod mvn_props {
    use super::*;
    use proptest::prelude::*;

    fn sf_of(w: u32) -> u32         { (w >> 31) & 1 }
    fn opc_of(w: u32) -> u32        { (w >> 29) & 0x3 }
    fn opcode5_of(w: u32) -> u32    { (w >> 24) & 0x1F }
    fn shift_type_of(w: u32) -> u32 { (w >> 22) & 0x3 }
    fn n_of(w: u32) -> u32          { (w >> 21) & 1 }
    fn rm_of(w: u32) -> u32         { (w >> 16) & 0x1F }
    fn imm6_of(w: u32) -> u32       { (w >> 10) & 0x3F }
    fn rn_of(w: u32) -> u32         { (w >> 5) & 0x1F }
    fn rd_of(w: u32) -> u32         { w & 0x1F }

    fn xreg(n: u32) -> Operand { Operand::Reg(format!("x{}", n)) }
    fn wreg(n: u32) -> Operand { Operand::Reg(format!("w{}", n)) }
    fn shift(kind: &str, amount: u32) -> Operand {
        Operand::Shift { kind: kind.into(), amount }
    }
    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r.unwrap() {
            EncodeResult::Word(w) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    proptest! {
        // 1. Default form MVN Xd, Xm: every fixed field matches the ARMv8 ORN
        //    encoding that MVN aliases. sf=1, opc=01, 01010, N=1, Rn=31 (XZR),
        //    shift=0, and Rm/Rd land in their exact bitfields.
        #[test]
        fn mvn_default_form_field_placement(
            rd in 0u32..=30,
            rm in 0u32..=30,
        ) {
            let ops = vec![xreg(rd), xreg(rm)];
            let w = word(encode_mvn(&ops));
            prop_assert_eq!(sf_of(w), 1);
            prop_assert_eq!(opc_of(w), 0b01);          // ORN
            prop_assert_eq!(opcode5_of(w), 0b01010);   // logical shifted register
            prop_assert_eq!(n_of(w), 1);               // ORN sets N=1
            prop_assert_eq!(rn_of(w), 31);             // Rn == XZR
            prop_assert_eq!(shift_type_of(w), 0);      // LSL
            prop_assert_eq!(imm6_of(w), 0);            // no shift
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 2. The Rn field is ALWAYS 31 (XZR) regardless of registers/shift/width.
        //    This is the defining invariant of MVN -> ORN Rd, XZR, Rm.
        #[test]
        fn rn_field_always_xzr(
            rd in 0u32..=30,
            rm in 0u32..=30,
            sk in 0u32..=3u32,
            amount in 0u32..=63u32,
            is_64 in any::<bool>(),
        ) {
            let kind = ["lsl", "lsr", "asr", "ror"][sk as usize];
            let mk = |n: u32| if is_64 { xreg(n) } else { wreg(n) };
            let ops = vec![mk(rd), mk(rm), shift(kind, amount)];
            let w = word(encode_mvn(&ops));
            prop_assert_eq!(rn_of(w), 31);
        }

        // 3. Shift type and amount land in their exact bitfields for all four
        //    shift kinds; imm6 faithfully carries 0..=63.
        #[test]
        fn mvn_shift_type_and_amount_fields(
            rd in 0u32..=30,
            rm in 0u32..=30,
            sk in 0u32..=3u32,
            amount in 0u32..=63u32,
        ) {
            let (kind, want_st) = match sk {
                0 => ("lsl", 0u32),
                1 => ("lsr", 1u32),
                2 => ("asr", 2u32),
                _ => ("ror", 3u32),
            };
            let ops = vec![xreg(rd), xreg(rm), shift(kind, amount)];
            let w = word(encode_mvn(&ops));
            prop_assert_eq!(shift_type_of(w), want_st);
            prop_assert_eq!(imm6_of(w), amount);
        }

        // 4. sf (bit 31) tracks register width: W -> 0, X -> 1.
        #[test]
        fn sf_bit_tracks_register_width(
            n in 0u32..=30,
            is_w in any::<bool>(),
        ) {
            let mk = |n: u32| if is_w { wreg(n) } else { xreg(n) };
            let ops = vec![mk(n), mk(n)];
            let w = word(encode_mvn(&ops));
            prop_assert_eq!(sf_of(w), if is_w { 0 } else { 1 });
        }

        // 5. Differential oracle: MVN Xd, Xm [, shift] must encode identically
        //    to the explicit ORN Xd, XZR, Xm [, shift] it aliases.
        #[test]
        fn mvn_equals_orn_with_xzr_rn(
            rd in 0u32..=30,
            rm in 0u32..=30,
            sk in 0u32..=3u32,
            amount in 0u32..=63u32,
            is_64 in any::<bool>(),
        ) {
            let kind = ["lsl", "lsr", "asr", "ror"][sk as usize];
            let mk = |n: u32| if is_64 { xreg(n) } else { wreg(n) };
            let mvn_ops = vec![mk(rd), mk(rm), shift(kind, amount)];
            let orn_ops = vec![mk(rd), Operand::Reg("xzr".into()), mk(rm), shift(kind, amount)];
            let mvn_w = word(encode_mvn(&mvn_ops));
            let orn_w = word(encode_orn(&orn_ops));
            prop_assert_eq!(mvn_w, orn_w);
        }

        // 6. Negative contract: a shift amount outside the imm6 range (> 63) is
        //    architecturally illegal. Reference assemblers (GAS) reject it with
        //    "immediate value out of range"; encode_mvn must return Err rather
        //    than silently truncate with `& 0x3F`.
        #[test]
        fn out_of_range_shift_amount_is_rejected(
            rd in 0u32..=30,
            rm in 0u32..=30,
            amount in 64u32..=1000u32,
        ) {
            let ops = vec![xreg(rd), xreg(rm), shift("lsl", amount)];
            prop_assert!(encode_mvn(&ops).is_err());
        }
    }
}

// ── encode_eon: complementary property suite ─────────────────────────────
// ARMv8 EON (logical shifted register, EOR-NOT) encoding:
//   sf | opc(10) | 01010 | shift | N(1) | Rm | imm6 | Rn | Rd
//   bit 31     : sf  (register width)
//   bits 30:29 : opc = 10 (EON)
//   bits 28:24 : 01010   (logical shifted register class)
//   bits 23:22 : shift   (00=LSL 01=LSR 10=ASR 11=ROR)
//   bit  21    : N = 1   (the NOT variant; EOR has N=0)
//   bits 20:16 : Rm
//   bits 15:10 : imm6    (shift amount; for sf=0 the legal range is 0..=31)
//   bits  9:5  : Rn
//   bits  4:0  : Rd
//
// The pre-existing `tests` module already covers: 3-register field placement,
// X-register shift mapping for all four shift kinds, the EON-vs-ORN opc
// differential, the too-few-operands contract, and the W-register
// shift-above-31 finding (known-failing). This module fills the remaining
// gaps: W-register happy-path shifts, X-register shift > 63, mixed-width
// operands, unknown shift-kind coercion, and XZR(31) acceptance.
#[cfg(test)]
mod eon_props {
    use super::*;
    use proptest::prelude::*;

    fn sf_of(w: u32) -> u32         { (w >> 31) & 1 }
    fn opc_of(w: u32) -> u32        { (w >> 29) & 0x3 }
    fn opcode5_of(w: u32) -> u32    { (w >> 24) & 0x1F }
    fn shift_type_of(w: u32) -> u32 { (w >> 22) & 0x3 }
    fn n_of(w: u32) -> u32          { (w >> 21) & 1 }
    fn rm_of(w: u32) -> u32         { (w >> 16) & 0x1F }
    fn imm6_of(w: u32) -> u32       { (w >> 10) & 0x3F }
    fn rn_of(w: u32) -> u32         { (w >> 5) & 0x1F }
    fn rd_of(w: u32) -> u32         { w & 0x1F }

    fn xreg(n: u32) -> Operand { Operand::Reg(format!("x{}", n)) }
    fn wreg(n: u32) -> Operand { Operand::Reg(format!("w{}", n)) }
    fn shift(kind: &str, amount: u32) -> Operand {
        Operand::Shift { kind: kind.into(), amount }
    }
    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r.unwrap() {
            EncodeResult::Word(w) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    proptest! {
        // 1. W-REGISTER HAPPY PATH with a valid shift (0..=31): sf must be 0,
        //    the EON signature (opc=10, N=1, class=01010) is preserved, and the
        //    shift type/amount land in their exact bitfields. This is the
        //    sf=0 complement to the existing X-register shift-mapping test.
        #[test]
        fn eon_w_register_valid_shift_placement(
            rd in 0u32..=31, rn in 0u32..=31, rm in 0u32..=31,
            sk in 0u32..=3u32, amount in 0u32..=31u32,
        ) {
            let (kind, want_st) = match sk {
                0 => ("lsl", 0u32), 1 => ("lsr", 1u32),
                2 => ("asr", 2u32), _ => ("ror", 3u32),
            };
            let ops = vec![wreg(rd), wreg(rn), wreg(rm), shift(kind, amount)];
            let w = word(encode_eon(&ops));
            prop_assert_eq!(sf_of(w), 0);
            prop_assert_eq!(opc_of(w), 0b10);          // EON
            prop_assert_eq!(opcode5_of(w), 0b01010);   // logical shifted register
            prop_assert_eq!(n_of(w), 1);               // NOT variant
            prop_assert_eq!(shift_type_of(w), want_st);
            prop_assert_eq!(imm6_of(w), amount);
            prop_assert_eq!(rm_of(w), rm);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);
        }

        // 2. NEGATIVE CONTRACT — X-register shift > 63: imm6 is a 6-bit field,
        //    so a shift amount of 64..=u32::MAX cannot be encoded. GAS rejects
        //    `lsl #64` with "immediate value out of range". The encoder masks
        //    with `& 0x3F` and silently truncates (e.g. #100 -> #36), so this
        //    property currently FAILS — documenting the silent-truncation gap.
        //    (The W-register 32..=63 case is covered by the existing test
        //    `eon_w_register_rejects_shift_above_31`; this covers sf=1.)
        #[test]
        fn eon_x_register_shift_above_63_is_rejected(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            amount in 64u32..=1000u32, sk in 0u32..=3u32,
        ) {
            let kind = match sk { 0 => "lsl", 1 => "lsr", 2 => "asr", _ => "ror" };
            let ops = vec![xreg(rd), xreg(rn), xreg(rm), shift(kind, amount)];
            prop_assert!(encode_eon(&ops).is_err());
        }

        // 3. NEGATIVE CONTRACT — mixed register widths: AArch64 requires Rd,
        //    Rn and Rm to share the same width in a logical shifted-register
        //    op (`eon x0, w1, x2` is illegal; GAS errors "mismatched register
        //    sizes"). encode_eon derives sf only from operand 0 and ignores the
        //    width of Rn/Rm, so it silently emits sf=1 for a W source — this
        //    property currently FAILS. (encode_orn rejects this case; EON does
        //    not — an inconsistency.)
        #[test]
        fn eon_rejects_mixed_register_widths(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            rd_is_x in any::<bool>(), rn_is_x in any::<bool>(), rm_is_x in any::<bool>(),
        ) {
            prop_assume!(!(rd_is_x && rn_is_x && rm_is_x));
            prop_assume!(!(!rd_is_x && !rn_is_x && !rm_is_x));
            let mk = |is_x: bool, n: u32| if is_x { xreg(n) } else { wreg(n) };
            let ops = vec![mk(rd_is_x, rd), mk(rn_is_x, rn), mk(rm_is_x, rm)];
            prop_assert!(encode_eon(&ops).is_err());
        }

        // 4. NEGATIVE CONTRACT — unknown shift kind: only lsl/lsr/asr/ror are
        //    defined. A garbage kind like "foo" should be rejected, but the
        //    match arm `_ => 0b00` silently coerces it to LSL — this property
        //    currently FAILS.
        #[test]
        fn eon_unknown_shift_kind_is_rejected(
            rd in 0u32..=30, rn in 0u32..=30, rm in 0u32..=30,
            amount in 0u32..=63u32,
        ) {
            let ops = vec![xreg(rd), xreg(rn), xreg(rm), shift("foo", amount)];
            prop_assert!(encode_eon(&ops).is_err());
        }

        // 5. POSITIVE — register 31 (XZR): in the logical shifted-register
        //    class, register 31 encodes the zero register, not SP. encode_eon
        //    must accept XZR/WZR in the Rd, Rn or Rm field and place 31 there.
        #[test]
        fn eon_accepts_register_31_xzr_in_any_field(
            r in 0u32..=30, a in 0u32..=30, b in 0u32..=30,
            field in 0u32..=2u32, // 0=Rd, 1=Rn, 2=Rm
        ) {
            let z = |is_64: bool| {
                Operand::Reg(if is_64 { "xzr".into() } else { "wzr".into() })
            };
            let mk = |is_64: bool, n: u32| if is_64 { xreg(n) } else { wreg(n) };
            // 64-bit operands so all fields share width.
            let ops = match field {
                0 => vec![z(true),  mk(true, a), mk(true, b)],
                1 => vec![mk(true, r), z(true),  mk(true, b)],
                _ => vec![mk(true, r), mk(true, a), z(true)],
            };
            let w = word(encode_eon(&ops));
            prop_assert_eq!(sf_of(w), 1);
            prop_assert_eq!(opc_of(w), 0b10);
            prop_assert_eq!(n_of(w), 1);
            match field {
                0 => prop_assert_eq!(rd_of(w), 31),
                1 => prop_assert_eq!(rn_of(w), 31),
                _ => prop_assert_eq!(rm_of(w), 31),
            }
        }
    }
}
