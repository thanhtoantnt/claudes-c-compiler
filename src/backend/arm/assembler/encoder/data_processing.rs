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
}
