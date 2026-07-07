use super::*;
use crate::backend::arm::assembler::parser::Operand;

// ── Loads/Stores ─────────────────────────────────────────────────────────

/// Auto-detect LDR/STR size from the first register operand.
pub(crate) fn encode_ldr_str_auto(operands: &[Operand], is_load: bool) -> Result<EncodeResult, String> {
    // Determine size from register: Wn -> 32-bit (size=10), Xn -> 64-bit (size=11)
    // FP: Sn -> 32-bit, Dn -> 64-bit, Qn -> 128-bit
    let reg_name = match operands.first() {
        Some(Operand::Reg(r)) => r.to_lowercase(),
        _ => return Err("ldr/str needs register operand".to_string()),
    };

    let size = if reg_name.starts_with('w') {
        0b10 // 32-bit
    } else if reg_name.starts_with('x') || reg_name == "sp" || reg_name == "xzr" || reg_name == "lr" {
        0b11 // 64-bit
    } else if reg_name.starts_with('s') {
        0b10 // 32-bit float
    } else if reg_name.starts_with('d') {
        0b11 // 64-bit float
    } else if reg_name.starts_with('q') {
        0b00 // 128-bit: size=00 with opc adjustment in encode_ldr_str
    } else {
        0b11 // default 64-bit
    };

    let is_128bit = reg_name.starts_with('q');
    encode_ldr_str(operands, is_load, size, false, is_128bit)
}

pub(crate) fn encode_ldr_str(operands: &[Operand], is_load: bool, size: u32, is_signed: bool, is_128bit: bool) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("ldr/str requires at least 2 operands".to_string());
    }

    let (rt, _) = get_reg(operands, 0)?;
    let fp = is_fp_reg(operands.first().map(|o| match o { Operand::Reg(r) => r.as_str(), _ => "" }).unwrap_or(""));

    // Use the size parameter as-is (auto-detection happens in encode_ldr_str_auto)
    let actual_size = size;

    let v = if fp { 1u32 } else { 0u32 };

    match operands.get(1) {
        // [base, #offset]
        Some(Operand::Mem { base, offset }) => {
            let rn = parse_reg_num(base).ok_or("invalid base reg")?;

            // Unsigned offset encoding
            // Size determines the shift for offset alignment
            // For 128-bit Q registers: shift=4, opc=11 (load) or 10 (store)
            let shift = if is_128bit { 4 } else { actual_size };
            let opc = if is_128bit {
                if is_load { 0b11 } else { 0b10 }
            } else if is_load {
                if is_signed { 0b10 } else { 0b01 }
            } else {
                0b00
            };

            // Check if offset is aligned and fits in 12-bit unsigned field
            let abs_offset = *offset as u64;
            let align = 1u64 << shift;
            if *offset >= 0 && abs_offset.is_multiple_of(align) {
                let imm12 = (abs_offset / align) as u32;
                if imm12 < 4096 {
                    // Unsigned offset form: size 111 V 01 opc imm12 Rn Rt
                    let word = (actual_size << 30) | (0b111 << 27) | (v << 26) | (0b01 << 24)
                        | (opc << 22) | (imm12 << 10) | (rn << 5) | rt;
                    return Ok(EncodeResult::Word(word));
                }
            }

            // Unscaled offset (LDUR/STUR form)
            let imm9 = (*offset as i32) & 0x1FF;
            let opc = if is_128bit {
                if is_load { 0b11 } else { 0b10 }
            } else if is_load {
                if is_signed { 0b10 } else { 0b01 }
            } else {
                0b00
            };
            let word = (((actual_size << 30) | (0b111 << 27) | (v << 26)) | (opc << 22)
                | ((imm9 as u32 & 0x1FF) << 12)) | (rn << 5) | rt;
            return Ok(EncodeResult::Word(word));
        }

        // [base, #offset]! (pre-index)
        Some(Operand::MemPreIndex { base, offset }) => {
            let rn = parse_reg_num(base).ok_or("invalid base reg")?;
            let imm9 = (*offset as i32) & 0x1FF;
            let opc = if is_128bit {
                if is_load { 0b11 } else { 0b10 }
            } else if is_load { 0b01 } else { 0b00 };
            let word = ((actual_size << 30) | (0b111 << 27) | (v << 26)) | (opc << 22)
                | ((imm9 as u32 & 0x1FF) << 12) | (0b11 << 10) | (rn << 5) | rt;
            return Ok(EncodeResult::Word(word));
        }

        // [base], #offset (post-index)
        Some(Operand::MemPostIndex { base, offset }) => {
            let rn = parse_reg_num(base).ok_or("invalid base reg")?;
            let imm9 = (*offset as i32) & 0x1FF;
            let opc = if is_128bit {
                if is_load { 0b11 } else { 0b10 }
            } else if is_load { 0b01 } else { 0b00 };
            let word = ((actual_size << 30) | (0b111 << 27) | (v << 26)) | (opc << 22)
                | ((imm9 as u32 & 0x1FF) << 12) | (0b01 << 10) | (rn << 5) | rt;
            return Ok(EncodeResult::Word(word));
        }

        // [base, Xm] register offset
        Some(Operand::MemRegOffset { base, index, extend, shift }) => {
            // Check if index is a :lo12: modifier
            if index.starts_with(':') {
                // Parse modifier from the index string
                let rn = parse_reg_num(base).ok_or("invalid base reg")?;
                let mod_str = index.trim_start_matches(':');
                let (kind, sym) = if let Some(colon_pos) = mod_str.find(':') {
                    (&mod_str[..colon_pos], &mod_str[colon_pos + 1..])
                } else {
                    return Err(format!("malformed modifier in memory operand: {}", index));
                };

                let (symbol, addend) = if let Some(plus_pos) = sym.find('+') {
                    let s = &sym[..plus_pos];
                    let off: i64 = sym[plus_pos + 1..].parse().unwrap_or(0);
                    (s.to_string(), off)
                } else {
                    (sym.to_string(), 0i64)
                };

                let opc = if is_128bit {
                    if is_load { 0b11 } else { 0b10 }
                } else if is_load { 0b01 } else { 0b00 };

                let reloc_type = match kind {
                    "lo12" => {
                        if is_128bit {
                            RelocType::Ldst128AbsLo12
                        } else {
                            match actual_size {
                                0b00 => RelocType::Ldst8AbsLo12,
                                0b01 => RelocType::Ldst16AbsLo12,
                                0b10 => RelocType::Ldst32AbsLo12,
                                0b11 => RelocType::Ldst64AbsLo12,
                                _ => RelocType::Ldst64AbsLo12,
                            }
                        }
                    }
                    "got_lo12" => RelocType::Ld64GotLo12,
                    _ => return Err(format!("unsupported modifier in load/store: {}", kind)),
                };

                let word = ((actual_size << 30) | (0b111 << 27) | (v << 26) | (0b01 << 24) | (opc << 22)) | (rn << 5) | rt;
                return Ok(EncodeResult::WordWithReloc {
                    word,
                    reloc: Relocation {
                        reloc_type,
                        symbol,
                        addend,
                    },
                });
            }

            let rn = parse_reg_num(base).ok_or("invalid base reg")?;
            let rm = parse_reg_num(index).ok_or("invalid index reg")?;
            let opc = if is_128bit {
                if is_load { 0b11 } else { 0b10 }
            } else if is_load { 0b01 } else { 0b00 };
            // Register offset: size 111 V opc 1 Rm option S 10 Rn Rt
            // Determine option and S from extend/shift specifiers
            let is_w_index = index.starts_with('w') || index.starts_with('W');
            let shift_amount: u8 = match shift { Some(s) => *s, None => 0 };
            let (option, s_bit) = match extend.as_deref() {
                Some("lsl") => {
                    // LSL with shift: S=1 if shift amount > 0
                    let s_val = if shift_amount > 0 { 1u32 } else { 0u32 };
                    (0b011u32, s_val)
                }
                Some("sxtw") => {
                    let s_val = if shift_amount > 0 { 1u32 } else { 0u32 };
                    (0b110u32, s_val)
                }
                Some("sxtx") => {
                    let s_val = if shift_amount > 0 { 1u32 } else { 0u32 };
                    (0b111u32, s_val)
                }
                Some("uxtw") => {
                    let s_val = if shift_amount > 0 { 1u32 } else { 0u32 };
                    (0b010u32, s_val)
                }
                Some("uxtx") => {
                    let s_val = if shift_amount > 0 { 1u32 } else { 0u32 };
                    (0b011u32, s_val)
                }
                None => {
                    // Default: if W register index, use UXTW; if X register, use LSL
                    if is_w_index {
                        (0b010u32, 0u32) // UXTW, no shift
                    } else {
                        (0b011u32, 0u32) // LSL, no shift
                    }
                }
                _ => (0b011u32, 0u32), // default LSL
            };
            let word = (actual_size << 30) | (0b111 << 27) | (v << 26) | (opc << 22)
                | (1 << 21) | (rm << 16) | (option << 13) | (s_bit << 12) | (0b10 << 10) | (rn << 5) | rt;
            return Ok(EncodeResult::Word(word));
        }

        // LDR (literal): ldr Rt, label — PC-relative load
        Some(Operand::Symbol(sym)) if is_load => {
            // opc V 011 00 imm19 Rt
            // For GP registers: opc=00 → 32-bit (W), opc=01 → 64-bit (X), opc=11 → PRFM
            // For FP/SIMD:      opc=00 → 32-bit (S), opc=01 → 64-bit (D), opc=10 → 128-bit (Q)
            // Note: actual_size uses 10=32-bit, 11=64-bit but LDR literal uses 00=32-bit, 01=64-bit
            let opc = if is_128bit {
                0b10u32
            } else if fp {
                // FP: S=00, D=01 (same mapping as GP)
                if actual_size == 0b11 { 0b01 } else { 0b00 }
            } else {
                // GP: W=00, X=01
                if actual_size == 0b11 { 0b01 } else { 0b00 }
            };
            let word = (opc << 30) | (v << 26) | (0b011 << 27) | rt;
            return Ok(EncodeResult::WordWithReloc {
                word,
                reloc: Relocation {
                    reloc_type: RelocType::Ldr19,
                    symbol: sym.clone(),
                    addend: 0,
                },
            });
        }

        _ => {}
    }

    Err(format!("unsupported ldr/str operands: {:?}", operands))
}

/// Encode LDUR/STUR (unscaled immediate offset load/store)
/// Format: size 111 V 00 opc 0 imm9 00 Rn Rt
pub(crate) fn encode_ldur_stur(operands: &[Operand], is_load: bool, op2_bits: u32) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("ldur/stur requires 2 operands".to_string());
    }
    let (rt, _) = get_reg(operands, 0)?;
    let reg_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
    let fp = is_fp_reg(&reg_name);
    let v = if fp { 1u32 } else { 0u32 };

    let (size, opc) = if fp {
        if reg_name.starts_with('q') {
            (0b00u32, if is_load { 0b11u32 } else { 0b10 })
        } else if reg_name.starts_with('d') {
            (0b11, if is_load { 0b01 } else { 0b00 })
        } else if reg_name.starts_with('s') {
            (0b10, if is_load { 0b01 } else { 0b00 })
        } else if reg_name.starts_with('h') {
            (0b01, if is_load { 0b01 } else { 0b00 })
        } else if reg_name.starts_with('b') {
            (0b00, if is_load { 0b01 } else { 0b00 })
        } else {
            (0b11, if is_load { 0b01 } else { 0b00 })
        }
    } else {
        let is_64 = reg_name.starts_with('x');
        let sz = if is_64 { 0b11u32 } else { 0b10 };
        (sz, if is_load { 0b01u32 } else { 0b00 })
    };

    let (rn, imm9) = match &operands[1] {
        Operand::Mem { base, offset } => {
            let rn = parse_reg_num(base).ok_or("invalid base reg")?;
            (rn, *offset as i32)
        }
        _ => return Err(format!("ldur/stur: expected memory operand, got {:?}", operands[1])),
    };

    let imm9_enc = (imm9 as u32) & 0x1FF;
    let word = (size << 30) | (0b111 << 27) | (v << 26) | (opc << 22)
        | (imm9_enc << 12) | (op2_bits << 10) | (rn << 5) | rt;
    Ok(EncodeResult::Word(word))
}

/// Encode LDTR/STTR with explicit size (for ldtrh, ldtrb, etc.)
pub(crate) fn encode_ldtr_sized(operands: &[Operand], is_load: bool, size: u32) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("ldtr/sttr requires 2 operands".to_string());
    }
    let (rt, _) = get_reg(operands, 0)?;
    let opc = if is_load { 0b01u32 } else { 0b00 };
    let (rn, imm9) = match &operands[1] {
        Operand::Mem { base, offset } => {
            let rn = parse_reg_num(base).ok_or("invalid base reg")?;
            (rn, *offset as i32)
        }
        _ => return Err("ldtr/sttr: expected memory operand".to_string()),
    };
    let imm9_enc = (imm9 as u32) & 0x1FF;
    let word = (size << 30) | (0b111 << 27) | (opc << 22)
        | (imm9_enc << 12) | (0b10 << 10) | (rn << 5) | rt;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_ldrsw(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("ldrsw requires 2 operands".to_string());
    }

    let (rt, _) = get_reg(operands, 0)?;

    match operands.get(1) {
        Some(Operand::Mem { base, offset }) => {
            let rn = parse_reg_num(base).ok_or("invalid base reg")?;
            // LDRSW: size=10 111 V=0 01 opc=10 -> unsigned offset
            // Actually: 10 111 0 01 10 imm12 Rn Rt
            let abs_offset = *offset as u64;
            if *offset >= 0 && abs_offset.is_multiple_of(4) {
                let imm12 = (abs_offset / 4) as u32;
                if imm12 < 4096 {
                    let word = ((0b10 << 30) | (0b111 << 27)) | (0b01 << 24) | (0b10 << 22)
                        | (imm12 << 10) | (rn << 5) | rt;
                    return Ok(EncodeResult::Word(word));
                }
            }
            // Unscaled: LDURSW
            let imm9 = (*offset as i32) & 0x1FF;
            let word = (((0b10 << 30) | (0b111 << 27)) | (0b10 << 22)
                | ((imm9 as u32 & 0x1FF) << 12)) | (rn << 5) | rt;
            return Ok(EncodeResult::Word(word));
        }

        Some(Operand::MemPostIndex { base, offset }) => {
            let rn = parse_reg_num(base).ok_or("invalid base reg")?;
            let imm9 = (*offset as i32) & 0x1FF;
            let word = ((0b10 << 30) | (0b111 << 27)) | (0b10 << 22)
                | ((imm9 as u32 & 0x1FF) << 12) | (0b01 << 10) | (rn << 5) | rt;
            return Ok(EncodeResult::Word(word));
        }

        Some(Operand::MemPreIndex { base, offset }) => {
            let rn = parse_reg_num(base).ok_or("invalid base reg")?;
            let imm9 = (*offset as i32) & 0x1FF;
            let word = ((0b10 << 30) | (0b111 << 27)) | (0b10 << 22)
                | ((imm9 as u32 & 0x1FF) << 12) | (0b11 << 10) | (rn << 5) | rt;
            return Ok(EncodeResult::Word(word));
        }

        Some(Operand::MemRegOffset { base, index, extend, shift }) => {
            let rn = parse_reg_num(base).ok_or("invalid base reg")?;
            let rm = parse_reg_num(index).ok_or("invalid index reg")?;
            let (option, s_bit) = match (extend.as_deref(), shift) {
                (Some("lsl"), Some(2)) => (0b011u32, 1u32),
                (Some("lsl"), Some(0)) | (Some("lsl"), None) => (0b011, 0),
                (None, None) | (None, Some(0)) => (0b011, 0),
                (Some("sxtw"), Some(2)) => (0b110, 1),
                (Some("sxtw"), Some(0)) | (Some("sxtw"), None) => (0b110, 0),
                (Some("uxtw"), Some(2)) => (0b010, 1),
                (Some("uxtw"), Some(0)) | (Some("uxtw"), None) => (0b010, 0),
                (Some("sxtx"), Some(2)) => (0b111, 1),
                (Some("sxtx"), Some(0)) | (Some("sxtx"), None) => (0b111, 0),
                _ => return Err(format!("unsupported ldrsw extend/shift: {:?}/{:?}", extend, shift)),
            };
            // LDRSW reg: 10 111 0 00 10 1 Rm option S 10 Rn Rt
            let word = (0b10 << 30) | (0b111 << 27) | (0b10 << 22) | (1 << 21)
                | (rm << 16) | (option << 13) | (s_bit << 12) | (0b10 << 10) | (rn << 5) | rt;
            return Ok(EncodeResult::Word(word));
        }

        _ => {}
    }

    Err(format!("unsupported ldrsw operands: {:?}", operands))
}

pub(crate) fn encode_ldrs(operands: &[Operand], size: u32) -> Result<EncodeResult, String> {
    // LDRSB/LDRSH: sign-extending byte/halfword loads
    if operands.len() < 2 {
        return Err("ldrsb/ldrsh requires 2 operands".to_string());
    }

    let (rt, is_64) = get_reg(operands, 0)?;
    let opc = if is_64 { 0b10 } else { 0b11 }; // 64-bit target: opc=10, 32-bit: opc=11

    if let Some(Operand::Mem { base, offset }) = operands.get(1) {
        let rn = parse_reg_num(base).ok_or("invalid base reg")?;
        let shift = size;
        let abs_offset = *offset as u64;
        let align = 1u64 << shift;
        if *offset >= 0 && abs_offset.is_multiple_of(align) {
            let imm12 = (abs_offset / align) as u32;
            if imm12 < 4096 {
                let word = ((size << 30) | (0b111 << 27)) | (0b01 << 24) | (opc << 22)
                    | (imm12 << 10) | (rn << 5) | rt;
                return Ok(EncodeResult::Word(word));
            }
        }
        // Unscaled
        let imm9 = (*offset as i32) & 0x1FF;
        let word = (((size << 30) | (0b111 << 27)) | (opc << 22)
            | ((imm9 as u32 & 0x1FF) << 12)) | (rn << 5) | rt;
        return Ok(EncodeResult::Word(word));
    }

    // Post-index: ldrsb/ldrsh Rt, [Xn], #imm
    if let Some(Operand::MemPostIndex { base, offset }) = operands.get(1) {
        let rn = parse_reg_num(base).ok_or("invalid base reg")?;
        let imm9 = (*offset as i32) & 0x1FF;
        let word = (size << 30) | (0b111 << 27) | (opc << 22)
            | ((imm9 as u32 & 0x1FF) << 12) | (0b01 << 10) | (rn << 5) | rt;
        return Ok(EncodeResult::Word(word));
    }

    // Pre-index: ldrsb/ldrsh Rt, [Xn, #imm]!
    if let Some(Operand::MemPreIndex { base, offset }) = operands.get(1) {
        let rn = parse_reg_num(base).ok_or("invalid base reg")?;
        let imm9 = (*offset as i32) & 0x1FF;
        let word = (size << 30) | (0b111 << 27) | (opc << 22)
            | ((imm9 as u32 & 0x1FF) << 12) | (0b11 << 10) | (rn << 5) | rt;
        return Ok(EncodeResult::Word(word));
    }

    // Register offset: ldrsb/ldrsh Rt, [Xn, Xm{, extend {#amount}}]
    if let Some(Operand::MemRegOffset { base, index, extend, shift }) = operands.get(1) {
        let rn = parse_reg_num(base).ok_or("invalid base reg")?;
        let rm = parse_reg_num(index).ok_or("invalid index reg")?;
        let is_w_index = index.starts_with('w') || index.starts_with('W');
        let shift_amount: u8 = match shift { Some(s) => *s, None => 0 };
        let (option, s_bit) = match extend.as_deref() {
            Some("lsl") => (0b011u32, if shift_amount > 0 { 1u32 } else { 0 }),
            Some("sxtw") => (0b110u32, if shift_amount > 0 { 1u32 } else { 0 }),
            Some("sxtx") => (0b111u32, if shift_amount > 0 { 1u32 } else { 0 }),
            Some("uxtw") => (0b010u32, if shift_amount > 0 { 1u32 } else { 0 }),
            Some("uxtx") => (0b011u32, if shift_amount > 0 { 1u32 } else { 0 }),
            None => if is_w_index { (0b010u32, 0u32) } else { (0b011u32, 0u32) },
            _ => (0b011u32, 0u32),
        };
        let word = (size << 30) | (0b111 << 27) | (opc << 22) | (1 << 21)
            | (rm << 16) | (option << 13) | (s_bit << 12) | (0b10 << 10) | (rn << 5) | rt;
        return Ok(EncodeResult::Word(word));
    }

    Err(format!("unsupported ldrsb/ldrsh operands: {:?}", operands))
}

pub(crate) fn encode_ldp_stp(operands: &[Operand], is_load: bool) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("ldp/stp requires 3 operands".to_string());
    }

    let (rt1, is_64) = get_reg(operands, 0)?;
    let (rt2, _) = get_reg(operands, 1)?;
    let fp = is_fp_reg(match &operands[0] { Operand::Reg(r) => r.as_str(), _ => "" });

    let opc = if fp {
        let r = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
        if r.starts_with('s') { 0b00 }
        else if r.starts_with('d') { 0b01 }
        else if r.starts_with('q') || is_64 { 0b10 }
        else { 0b00 }
    } else if is_64 { 0b10 } else { 0b00 };

    let v = if fp { 1u32 } else { 0u32 };
    let l = if is_load { 1u32 } else { 0u32 };

    // Shift depends on register size
    let shift = if fp {
        let r = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
        if r.starts_with('s') { 2 }
        else if r.starts_with('d') { 3 }
        else if r.starts_with('q') { 4 }
        else if is_64 { 3 } else { 2 }
    } else if is_64 { 3 } else { 2 };

    match operands.get(2) {
        // STP rt1, rt2, [base, #offset]! (pre-index)
        Some(Operand::MemPreIndex { base, offset }) => {
            let rn = parse_reg_num(base).ok_or("invalid base reg")?;
            let imm7 = ((*offset >> shift) as i32) & 0x7F;
            let word = (opc << 30) | (0b101 << 27) | (v << 26) | (0b011 << 23) | (l << 22)
                | ((imm7 as u32 & 0x7F) << 15) | (rt2 << 10) | (rn << 5) | rt1;
            return Ok(EncodeResult::Word(word));
        }

        // LDP/STP rt1, rt2, [base], #offset (post-index)
        Some(Operand::MemPostIndex { base, offset }) => {
            let rn = parse_reg_num(base).ok_or("invalid base reg")?;
            let imm7 = ((*offset >> shift) as i32) & 0x7F;
            let word = (opc << 30) | (0b101 << 27) | (v << 26) | (0b001 << 23) | (l << 22)
                | ((imm7 as u32 & 0x7F) << 15) | (rt2 << 10) | (rn << 5) | rt1;
            return Ok(EncodeResult::Word(word));
        }

        // LDP/STP rt1, rt2, [base, #offset] (signed offset)
        Some(Operand::Mem { base, offset }) => {
            let rn = parse_reg_num(base).ok_or("invalid base reg")?;
            let imm7 = ((*offset >> shift) as i32) & 0x7F;
            let word = (opc << 30) | (0b101 << 27) | (v << 26) | (0b010 << 23) | (l << 22)
                | ((imm7 as u32 & 0x7F) << 15) | (rt2 << 10) | (rn << 5) | rt1;
            return Ok(EncodeResult::Word(word));
        }

        _ => {}
    }

    Err(format!("unsupported ldp/stp operands: {:?}", operands))
}

/// Encode LDNP/STNP (load/store pair non-temporal)
/// Encoding: opc 101 V 000 L imm7 Rt2 Rn Rt
/// TODO: Only handles integer registers (V=0). FP/SIMD register support needed for V=1.
pub(crate) fn encode_ldnp_stnp(operands: &[Operand], is_load: bool) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("ldnp/stnp requires 3 operands".to_string());
    }

    let (rt1, is_64) = get_reg(operands, 0)?;
    let (rt2, _) = get_reg(operands, 1)?;

    let opc: u32 = if is_64 { 0b10 } else { 0b00 };
    let l: u32 = if is_load { 1 } else { 0 };
    let shift = if is_64 { 3 } else { 2 }; // scale factor: 8 for 64-bit, 4 for 32-bit

    match operands.get(2) {
        Some(Operand::Mem { base, offset }) => {
            let rn = parse_reg_num(base).ok_or("invalid base reg")?;
            let imm7 = ((*offset >> shift) as i32) & 0x7F;
            // LDNP/STNP: opc 101 V=0 000 L imm7 Rt2 Rn Rt
            let word = (opc << 30) | (0b101 << 27) | (l << 22)
                | ((imm7 as u32 & 0x7F) << 15) | (rt2 << 10) | (rn << 5) | rt1;
            Ok(EncodeResult::Word(word))
        }
        _ => Err(format!("unsupported ldnp/stnp operands: {:?}", operands)),
    }
}

// ── Exclusive loads/stores ───────────────────────────────────────────────

/// Encode LDXR/STXR and byte/halfword variants.
/// `forced_size`: None = auto-detect from register width, Some(0b00) = byte, Some(0b01) = halfword
pub(crate) fn encode_ldxr_stxr(operands: &[Operand], is_load: bool, forced_size: Option<u32>) -> Result<EncodeResult, String> {
    if is_load {
        let (rt, is_64) = get_reg(operands, 0)?;
        let rn = match operands.get(1) {
            Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("invalid base")?,
            _ => return Err("ldxr needs memory operand".to_string()),
        };
        let size = forced_size.unwrap_or(if is_64 { 0b11 } else { 0b10 });
        let word = ((size << 30) | (0b001000010 << 21) | (0b11111 << 16))
            | (0b11111 << 10) | (rn << 5) | rt;
        Ok(EncodeResult::Word(word))
    } else {
        let (ws, _) = get_reg(operands, 0)?;
        let (rt, is_64) = get_reg(operands, 1)?;
        let rn = match operands.get(2) {
            Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("invalid base")?,
            _ => return Err("stxr needs memory operand".to_string()),
        };
        let size = forced_size.unwrap_or(if is_64 { 0b11 } else { 0b10 });
        let word = ((size << 30) | (0b001000000 << 21) | (ws << 16))
            | (0b11111 << 10) | (rn << 5) | rt;
        Ok(EncodeResult::Word(word))
    }
}

/// Encode LDAXR/STLXR and byte/halfword variants.
pub(crate) fn encode_ldaxr_stlxr(operands: &[Operand], is_load: bool, forced_size: Option<u32>) -> Result<EncodeResult, String> {
    if is_load {
        let (rt, is_64) = get_reg(operands, 0)?;
        let rn = match operands.get(1) {
            Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("invalid base")?,
            _ => return Err("ldaxr needs memory operand".to_string()),
        };
        let size = forced_size.unwrap_or(if is_64 { 0b11 } else { 0b10 });
        let word = (size << 30) | (0b001000010 << 21) | (0b11111 << 16) | (1 << 15)
            | (0b11111 << 10) | (rn << 5) | rt;
        Ok(EncodeResult::Word(word))
    } else {
        let (ws, _) = get_reg(operands, 0)?;
        let (rt, is_64) = get_reg(operands, 1)?;
        let rn = match operands.get(2) {
            Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("invalid base")?,
            _ => return Err("stlxr needs memory operand".to_string()),
        };
        let size = forced_size.unwrap_or(if is_64 { 0b11 } else { 0b10 });
        let word = (size << 30) | (0b001000000 << 21) | (ws << 16) | (1 << 15)
            | (0b11111 << 10) | (rn << 5) | rt;
        Ok(EncodeResult::Word(word))
    }
}

/// Encode LDXP/STXP/LDAXP/STLXP (exclusive pair) instructions.
///
/// LDXP  Xt1, Xt2, [Xn]  : sz 001000 0 1 1 11111 0 Rt2 Rn Rt
/// LDAXP Xt1, Xt2, [Xn]  : sz 001000 0 1 1 11111 1 Rt2 Rn Rt
/// STXP  Ws, Xt1, Xt2, [Xn] : sz 001000 0 0 1 Rs 0 Rt2 Rn Rt
/// STLXP Ws, Xt1, Xt2, [Xn] : sz 001000 0 0 1 Rs 1 Rt2 Rn Rt
pub(crate) fn encode_ldxp_stxp(operands: &[Operand], is_load: bool, acquire_release: bool) -> Result<EncodeResult, String> {
    let o0 = if acquire_release { 1u32 } else { 0 };
    if is_load {
        // LDXP/LDAXP Rt, Rt2, [Rn]
        let (rt, is_64) = get_reg(operands, 0)?;
        let (rt2, _) = get_reg(operands, 1)?;
        let rn = match operands.get(2) {
            Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("ldxp needs memory operand")?,
            _ => return Err("ldxp needs memory operand".to_string()),
        };
        let sz = if is_64 { 1u32 } else { 0 };
        // 1 sz 001000 0 1 1 11111 o0 Rt2 Rn Rt  (bit23=0)
        let word = (1u32 << 31) | (sz << 30) | (0b001000 << 24) | (1 << 22)
            | (1 << 21) | (0b11111 << 16) | (o0 << 15) | (rt2 << 10) | (rn << 5) | rt;
        Ok(EncodeResult::Word(word))
    } else {
        // STXP/STLXP Ws, Rt, Rt2, [Rn]
        let (ws, _) = get_reg(operands, 0)?;  // status register (always W)
        let (rt, is_64) = get_reg(operands, 1)?;
        let (rt2, _) = get_reg(operands, 2)?;
        let rn = match operands.get(3) {
            Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("stxp needs memory operand")?,
            _ => return Err("stxp needs memory operand".to_string()),
        };
        let sz = if is_64 { 1u32 } else { 0 };
        // 1 sz 001000 0 0 1 Rs o0 Rt2 Rn Rt  (bit23=0, bit22=0)
        let word = (1u32 << 31) | (sz << 30) | (0b001000 << 24)
            | (1 << 21) | (ws << 16) | (o0 << 15) | (rt2 << 10) | (rn << 5) | rt;
        Ok(EncodeResult::Word(word))
    }
}

/// Encode LDAR/STLR and byte/halfword variants.
pub(crate) fn encode_ldar_stlr(operands: &[Operand], is_load: bool, forced_size: Option<u32>) -> Result<EncodeResult, String> {
    let (rt, is_64) = get_reg(operands, 0)?;
    let rn = match operands.get(1) {
        Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("invalid base")?,
        _ => return Err("ldar/stlr needs memory operand".to_string()),
    };
    let size = forced_size.unwrap_or(if is_64 { 0b11 } else { 0b10 });
    let l = if is_load { 1u32 } else { 0 };
    // LDAR/STLR: size 001000 1 L 0 11111 1 11111 Rn Rt
    let word = ((size << 30) | (0b001000 << 24) | (1 << 23) | (l << 22))
        | (0b11111 << 16) | (1 << 15) | (0b11111 << 10) | (rn << 5) | rt;
    Ok(EncodeResult::Word(word))
}

// ── Address computation ──────────────────────────────────────────────────

pub(crate) fn encode_adrp(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;

    let (sym, addend) = match operands.get(1) {
        Some(Operand::Symbol(s)) => (s.clone(), 0i64),
        Some(Operand::Modifier { kind, symbol }) if kind == "got" => {
            // adrp x0, :got:symbol
            let word = (1u32 << 31) | (0b10000 << 24) | rd;
            return Ok(EncodeResult::WordWithReloc {
                word,
                reloc: Relocation {
                    reloc_type: RelocType::AdrGotPage21,
                    symbol: symbol.clone(),
                    addend: 0,
                },
            });
        }
        Some(Operand::SymbolOffset(s, off)) => (s.clone(), *off),
        Some(Operand::Label(s)) => (s.clone(), 0i64),
        // Parser misclassifies symbol names that collide with register names (s1, v0, d1, etc.),
        // condition codes (cc, lt, le), or barrier names (st, ld).
        // ADRP never takes these as actual operand types, so treat them as symbols.
        Some(Operand::Reg(name)) => (name.clone(), 0i64),
        Some(Operand::Cond(name)) => (name.clone(), 0i64),
        Some(Operand::Barrier(name)) => (name.clone(), 0i64),
        _ => return Err(format!("adrp needs symbol operand, got {:?}", operands.get(1))),
    };

    // ADRP: 1 immlo[1:0] 10000 immhi[18:0] Rd
    let word = (1u32 << 31) | (0b10000 << 24) | rd;
    Ok(EncodeResult::WordWithReloc {
        word,
        reloc: Relocation {
            reloc_type: RelocType::AdrpPage21,
            symbol: sym,
            addend,
        },
    })
}

pub(crate) fn encode_adr(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;

    // Check for immediate offset form: adr Rd, #imm
    // TODO: validate 21-bit signed immediate range
    if let Some(Operand::Imm(imm)) = operands.get(1) {
        let imm = *imm;
        // ADR: 0 immlo[1:0] 10000 immhi[18:0] Rd
        let immlo = ((imm as u32) & 3) << 29;
        let immhi = (((imm as u32) >> 2) & 0x7FFFF) << 5;
        let word = immlo | (0b10000 << 24) | immhi | rd;
        return Ok(EncodeResult::Word(word));
    }

    let (sym, addend) = get_symbol(operands, 1)?;
    // ADR: 0 immlo[1:0] 10000 immhi[18:0] Rd
    let word = (0b10000 << 24) | rd;
    Ok(EncodeResult::WordWithReloc {
        word,
        reloc: Relocation {
            reloc_type: RelocType::AdrPrelLo21,
            symbol: sym,
            addend,
        },
    })
}

// ── Prefetch ─────────────────────────────────────────────────────────────

/// Encode the PRFM (prefetch memory) instruction.
/// Format: PRFM <prfop>, [<Xn|SP>{, #<pimm>}]
/// Encoding: 1111 1001 10 imm12 Rn Rt
/// where Rt is the 5-bit prefetch operation type.
pub(crate) fn encode_prfm(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("prfm requires 2 operands".to_string());
    }

    // First operand: prefetch operation type (parsed as Symbol)
    let prfop = match &operands[0] {
        Operand::Symbol(s) => encode_prfop(s)?,
        Operand::Imm(v) => {
            if *v < 0 || *v > 31 {
                return Err(format!("prfm: immediate prefetch type out of range: {}", v));
            }
            *v as u32
        }
        _ => return Err(format!("prfm: expected prefetch operation name, got {:?}", operands[0])),
    };

    // Second operand: memory address [Xn{, #imm}]
    match &operands[1] {
        Operand::Mem { base, offset } => {
            let rn = parse_reg_num(base).ok_or_else(|| format!("prfm: invalid base register: {}", base))?;
            let imm = *offset;
            if imm < 0 || imm % 8 != 0 {
                return Err(format!("prfm: offset must be non-negative and 8-byte aligned, got {}", imm));
            }
            let imm12 = (imm / 8) as u32;
            if imm12 > 0xFFF {
                return Err(format!("prfm: offset too large: {}", imm));
            }
            // PRFM (imm): 1111 1001 10 imm12(12) Rn(5) Rt(5)
            let word = 0xF9800000 | (imm12 << 10) | (rn << 5) | prfop;
            Ok(EncodeResult::Word(word))
        }
        Operand::Symbol(_sym) => {
            // PRFM (literal) with symbol reference is not yet supported
            Err("prfm with symbol/label operand not yet supported".to_string())
        }
        Operand::MemRegOffset { base, index, extend, shift } => {
            // PRFM (register): 11 111 0 00 10 1 Rm option S 10 Rn Rt
            let rn = parse_reg_num(base).ok_or_else(|| format!("prfm: invalid base register: {}", base))?;
            let rm = parse_reg_num(index).ok_or_else(|| format!("prfm: invalid index register: {}", index))?;
            let is_w_index = index.starts_with('w') || index.starts_with('W');
            let shift_amount: u8 = match shift { Some(s) => *s, None => 0 };
            let (option, s_bit) = match extend.as_deref() {
                Some("lsl") => (0b011u32, if shift_amount > 0 { 1u32 } else { 0 }),
                Some("sxtw") => (0b110u32, if shift_amount > 0 { 1u32 } else { 0 }),
                Some("sxtx") => (0b111u32, if shift_amount > 0 { 1u32 } else { 0 }),
                Some("uxtw") => (0b010u32, if shift_amount > 0 { 1u32 } else { 0 }),
                None => if is_w_index { (0b010u32, 0u32) } else { (0b011u32, 0u32) },
                _ => (0b011u32, 0u32),
            };
            let word = (0b11 << 30) | (0b111 << 27) | (0b10 << 23) | (1 << 21)
                | (rm << 16) | (option << 13) | (s_bit << 12) | (0b10 << 10) | (rn << 5) | prfop;
            Ok(EncodeResult::Word(word))
        }
        _ => Err(format!("prfm: expected memory operand, got {:?}", operands[1])),
    }
}

/// Map prefetch operation name to its 5-bit encoding.
pub(crate) fn encode_prfop(name: &str) -> Result<u32, String> {
    match name.to_lowercase().as_str() {
        "pldl1keep" => Ok(0b00000),
        "pldl1strm" => Ok(0b00001),
        "pldl2keep" => Ok(0b00010),
        "pldl2strm" => Ok(0b00011),
        "pldl3keep" => Ok(0b00100),
        "pldl3strm" => Ok(0b00101),
        "plil1keep" => Ok(0b01000),
        "plil1strm" => Ok(0b01001),
        "plil2keep" => Ok(0b01010),
        "plil2strm" => Ok(0b01011),
        "plil3keep" => Ok(0b01100),
        "plil3strm" => Ok(0b01101),
        "pstl1keep" => Ok(0b10000),
        "pstl1strm" => Ok(0b10001),
        "pstl2keep" => Ok(0b10010),
        "pstl2strm" => Ok(0b10011),
        "pstl3keep" => Ok(0b10100),
        "pstl3strm" => Ok(0b10101),
        _ => Err(format!("prfm: unknown prefetch operation: {}", name)),
    }
}

// ── LSE Atomics ──────────────────────────────────────────────────────────

/// Encode CAS/CASA/CASAL/CASL and byte/halfword variants (Compare and Swap).
/// CAS Xs, Xt, [Xn]: size 001000 1 L 1 Rs o0 11111 Rn Rt
pub(crate) fn encode_cas(mnemonic: &str, operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err(format!("{} requires 3 operands", mnemonic));
    }
    let (rs, is_64) = get_reg(operands, 0)?;
    let (rt, _) = get_reg(operands, 1)?;
    let rn = match operands.get(2) {
        Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("cas: invalid base")?,
        _ => return Err("cas requires memory operand [Xn]".to_string()),
    };
    let mn = mnemonic.to_lowercase();
    let suffix = mn.strip_prefix("cas").unwrap_or("");
    // Determine size: 'b' suffix = byte (00), 'h' suffix = half (01), else register-based
    let size = if suffix.contains('b') {
        0b00u32
    } else if suffix.contains('h') {
        0b01u32
    } else if is_64 {
        0b11u32
    } else {
        0b10u32
    };
    // L bit (acquire): set for casa, casal
    let l = if suffix.contains('a') { 1u32 } else { 0u32 };
    // o0 bit (release): set for casl, casal
    let o0 = if suffix.contains('l') { 1u32 } else { 0u32 };
    // size 001000 1 L 1 Rs o0 11111 Rn Rt
    let word = (size << 30) | (0b001000 << 24) | (1 << 23) | (l << 22) | (1 << 21)
        | (rs << 16) | (o0 << 15) | (0b11111 << 10) | (rn << 5) | rt;
    Ok(EncodeResult::Word(word))
}

/// Encode SWP/SWPA/SWPAL/SWPL and byte/halfword variants (Swap).
/// SWP Xs, Xt, [Xn]: size 111000 AR 1 Rs 1 000 00 Rn Rt
/// Variants: swp, swpa, swpal, swpl, swpb, swpab, swpalb, swplb, swph, swpah, swpalh, swplh
pub(crate) fn encode_swp(mnemonic: &str, operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err(format!("{} requires 3 operands", mnemonic));
    }
    let (rs, is_64) = get_reg(operands, 0)?;
    let (rt, _) = get_reg(operands, 1)?;
    let rn = match operands.get(2) {
        Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("swp: invalid base")?,
        _ => return Err("swp requires memory operand [Xn]".to_string()),
    };
    let mn = mnemonic.to_lowercase();
    let suffix = mn.strip_prefix("swp").unwrap_or("");
    // Determine size: 'b' suffix = byte (00), 'h' suffix = half (01), else register-based
    let size = if suffix.contains('b') {
        0b00u32
    } else if suffix.contains('h') {
        0b01u32
    } else if is_64 {
        0b11u32
    } else {
        0b10u32
    };
    let a = if suffix.contains('a') { 1u32 } else { 0u32 };
    let r = if suffix.contains('l') { 1u32 } else { 0u32 };
    // size 111000 A R 1 Rs 1 000 00 Rn Rt
    let word = (size << 30) | (0b111000 << 24) | (a << 23) | (r << 22) | (1 << 21)
        | (rs << 16) | (1 << 15) | (rn << 5) | rt;
    Ok(EncodeResult::Word(word))
}

/// Encode LDADD/LDCLR/LDEOR/LDSET and their acquire/release/byte/halfword variants (LSE atomics).
/// LDADD Rs, Rt, [Xn]: size 111000 A R 1 Rs 0 opc 00 Rn Rt
/// opc: LDADD=000, LDCLR=001, LDEOR=010, LDSET=011
pub(crate) fn encode_ldop(mnemonic: &str, operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err(format!("{} requires 3 operands", mnemonic));
    }
    let (rs, is_64) = get_reg(operands, 0)?;
    let (rt, _) = get_reg(operands, 1)?;
    let rn = match operands.get(2) {
        Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("ldop: invalid base")?,
        _ => return Err(format!("{} requires memory operand [Xn]", mnemonic)),
    };
    let mn = mnemonic.to_lowercase();
    // Determine base op and suffix
    let (base, suffix) = if let Some(s) = mn.strip_prefix("ldadd") {
        (0b000u32, s)
    } else if let Some(s) = mn.strip_prefix("ldclr") {
        (0b001u32, s)
    } else if let Some(s) = mn.strip_prefix("ldeor") {
        (0b010u32, s)
    } else if let Some(s) = mn.strip_prefix("ldset") {
        (0b011u32, s)
    } else {
        return Err(format!("unknown ld atomic op: {}", mnemonic));
    };
    // Determine size: 'b' suffix = byte (00), 'h' suffix = half (01), else register-based
    let size = if suffix.contains('b') {
        0b00u32
    } else if suffix.contains('h') {
        0b01u32
    } else if is_64 {
        0b11u32
    } else {
        0b10u32
    };
    let a = if suffix.contains('a') { 1u32 } else { 0u32 };
    let r = if suffix.contains('l') { 1u32 } else { 0u32 };
    // size 111000 A R 1 Rs 0 opc 00 Rn Rt
    let word = (size << 30) | (0b111000 << 24) | (a << 23) | (r << 22) | (1 << 21)
        | (rs << 16) | (base << 12) | (rn << 5) | rt;
    Ok(EncodeResult::Word(word))
}

/// Encode STADD/STCLR/STEOR/STSET and their release/byte/halfword variants.
/// These are aliases for LDADD/LDCLR/LDEOR/LDSET with Rt=XZR (register 31).
/// STADD Ws, [Xn] encodes as LDADD Ws, WZR, [Xn]
/// Variants: stadd/stclr/steor/stset, plus 'l' (release), 'b' (byte), 'h' (half).
pub(crate) fn encode_stop(mnemonic: &str, operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err(format!("{} requires 2 operands", mnemonic));
    }
    let (rs, is_64) = get_reg(operands, 0)?;
    let rn = match operands.get(1) {
        Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or_else(|| format!("{}: invalid base", mnemonic))?,
        _ => return Err(format!("{} requires memory operand [Xn]", mnemonic)),
    };
    let mn = mnemonic.to_lowercase();
    // Determine base op from the prefix
    let (opc, suffix) = if let Some(s) = mn.strip_prefix("stadd") {
        (0b000u32, s)
    } else if let Some(s) = mn.strip_prefix("stclr") {
        (0b001u32, s)
    } else if let Some(s) = mn.strip_prefix("steor") {
        (0b010u32, s)
    } else if let Some(s) = mn.strip_prefix("stset") {
        (0b011u32, s)
    } else {
        return Err(format!("unknown st atomic op: {}", mnemonic));
    };
    // Determine size: 'b' suffix = byte (00), 'h' suffix = half (01), else register-based
    let size = if suffix.contains('b') {
        0b00u32
    } else if suffix.contains('h') {
        0b01u32
    } else if is_64 {
        0b11u32
    } else {
        0b10u32
    };
    // A=0 (no acquire for store aliases), R from 'l' suffix (release)
    let r = if suffix.contains('l') { 1u32 } else { 0u32 };
    let rt = 31u32; // XZR/WZR - discard result
    // size 111000 A R 1 Rs 0 opc 00 Rn Rt
    let word = (size << 30) | (0b111000 << 24) | (r << 22) | (1 << 21)
        | (rs << 16) | (opc << 12) | (rn << 5) | rt;
    Ok(EncodeResult::Word(word))
}

#[cfg(test)]
mod prop_ldr_str_auto_tests {
    use super::*;
    use proptest::prelude::*;

    // Oracle: encode_ldr_str_auto auto-detects `size` from the Rt register prefix
    // (w/s -> 0b10, x/d -> 0b11, q -> 0b00) and always passes is_signed=false.
    // For Mem/Pre/Post-index forms the word layout is:
    //   size[31:30] | 111[29:27] | v[26] | opc[23:22] | imm[21:10] | Rn[9:5] | Rt[4:0]
    // so opc_load XOR opc_store is always 0b01<<22 because is_signed is hard-wired false.

    /// Register classes auto-detection covers: GP (x,w) and FP/SIMD (d,s,q).
    const REG_CLASSES: &[char] = &['x', 'w', 'd', 's', 'q'];

    fn expected_size(prefix: char) -> u32 {
        match prefix {
            'w' | 's' => 0b10,
            'x' | 'd' => 0b11,
            'q' => 0b00,
            _ => unreachable!("unexpected reg prefix {}", prefix),
        }
    }

    /// V (vector) bit [26]: 1 for FP/SIMD registers, 0 for GP.
    fn expected_v(prefix: char) -> u32 {
        match prefix {
            'd' | 's' | 'q' => 1,
            _ => 0,
        }
    }

    fn word_of(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    prop_compose! {
        fn arb_reg()(idx in 0usize..REG_CLASSES.len(), num in 0u32..=30u32) -> (String, u32, char) {
            let prefix = REG_CLASSES[idx];
            (format!("{}{}", prefix, num), num, prefix)
        }
    }

    proptest! {
        // Property A — size field [31:30] matches the register-width class of Rt.
        // Uses Mem{base, #0}: offset 0 is always aligned and < 4096, so the unsigned
        // offset form is taken and size lands in bits [31:30].
        #[test]
        fn prop_size_field_matches_reg_class(
            (rt_name, _, prefix) in arb_reg(),
            (base_name, _, _) in arb_reg(),
        ) {
            let ops = vec![
                Operand::Reg(rt_name.clone()),
                Operand::Mem { base: base_name.clone(), offset: 0 },
            ];
            let w = word_of(encode_ldr_str_auto(&ops, true));
            prop_assert_eq!((w >> 30) & 0b11, expected_size(prefix));
        }

        // Property B — differential: for every memory-operand form, swapping
        // is_load only ever flips the opc field [23:22], and specifically only
        // bit 22 (load opc=0b01 vs store opc=0b00 for GP; 0b11 vs 0b10 for Q).
        // Both XOR to 0b01, so load ^ store == 0x0040_0000 with no other bits set.
        #[test]
        fn prop_load_xor_store_is_opc_bit22(
            (rt_name, _, _) in arb_reg(),
            (base_name, _, _) in arb_reg(),
            form in 0u8..3, // 0 = Mem, 1 = pre-index, 2 = post-index
        ) {
            let mem_op = match form {
                0 => Operand::Mem { base: base_name.clone(), offset: 0 },
                1 => Operand::MemPreIndex { base: base_name.clone(), offset: 0 },
                _ => Operand::MemPostIndex { base: base_name.clone(), offset: 0 },
            };
            let ops = vec![Operand::Reg(rt_name.clone()), mem_op];
            let load = word_of(encode_ldr_str_auto(&ops, true));
            let store = word_of(encode_ldr_str_auto(&ops, false));
            prop_assert_eq!(load ^ store, 0x0040_0000u32);
        }

        // Property C — field placement: Rt occupies [4:0] and Rn occupies [9:5]
        // of the encoded word, for arbitrary register numbers.
        #[test]
        fn prop_rt_and_rn_field_placement(
            (rt_name, rt_num, _) in arb_reg(),
            (base_name, base_num, _) in arb_reg(),
        ) {
            let ops = vec![
                Operand::Reg(rt_name.clone()),
                Operand::Mem { base: base_name.clone(), offset: 0 },
            ];
            let w = word_of(encode_ldr_str_auto(&ops, true));
            prop_assert_eq!(w & 0x1F, rt_num);            // Rt [4:0]
            prop_assert_eq!((w >> 5) & 0x1F, base_num);    // Rn [9:5]
        }

        // Property D — V (vector) bit [26]: set iff Rt is an FP/SIMD register.
        #[test]
        fn prop_v_bit_tracks_fp_register(
            (rt_name, _, prefix) in arb_reg(),
            (base_name, _, _) in arb_reg(),
        ) {
            let ops = vec![
                Operand::Reg(rt_name.clone()),
                Operand::Mem { base: base_name.clone(), offset: 0 },
            ];
            let w = word_of(encode_ldr_str_auto(&ops, true));
            prop_assert_eq!((w >> 26) & 1, expected_v(prefix));
        }

        // Property E — negative/error contract: a non-Reg first operand is rejected.
        #[test]
        fn prop_non_reg_first_operand_errors(kind in 0u8..3) {
            let non_reg = match kind {
                0 => Operand::Imm(5),
                1 => Operand::Symbol("foo".to_string()),
                _ => Operand::Mem { base: "x0".to_string(), offset: 0 },
            };
            let ops = vec![non_reg];
            let r = encode_ldr_str_auto(&ops, true);
            prop_assert!(r.is_err(), "expected error, got {:?}", r);
        }
    }
}

#[cfg(test)]
mod prop_encode_ldr_str_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Independent oracle ───────────────────────────────────────────────
    // ORACLE: reference-encoding / field-placement (ARMv8-A Architecture
    // Reference Manual, §C4.1.64 “LDR/STR (immediate, unsigned offset)” and
    // §C4.1.66 “LDR/STR (immediate, pre/post-index)”).
    //
    // We anchor field positions to *hand-derived* golden encodings (not this
    // crate's own formula), so each property is an independent check that the
    // function places fields where the ARM ARM mandates.
    //
    //   ldr x0,[x1]      = 0xF9400020  (unsigned offset; opc=01; [25:24]=01)
    //   str x0,[x1]      = 0xF9000020  (unsigned offset; opc=00; [25:24]=01)
    //   ldr x0,[x1,#8]!  = 0xF8408C20  (pre-index;  imm9=8; [25:24]=00; [11:10]=11)
    //   ldr x0,[x1],#8   = 0xF8408420  (post-index; imm9=8; [25:24]=00; [11:10]=01)
    //
    // GP field layout (V=0):
    //   size[31:30] | 111[29:27] | V[26] | {01 unscaled-off | 00 idx} | opc[23:22]
    //   | imm12[21:10] (or imm9[20:12] + idx-marker[11:10]) | Rn[9:5] | Rt[4:0]

    const GOLDEN_LDR_X0_X1_0: u32 = 0xF9400020;
    const GOLDEN_STR_X0_X1_0: u32 = 0xF9000020;
    const GOLDEN_PRE_X0_X1_8: u32 = 0xF8408C20;
    const GOLDEN_POST_X0_X1_8: u32 = 0xF8408420;

    fn gp_xreg(num: u32) -> Operand {
        Operand::Reg(format!("x{}", num))
    }

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    prop_compose! {
        fn arb_reg_num()(n in 0u32..=30u32) -> u32 { n }
    }

    proptest! {
        // Property 1 — unsigned-offset field layout.
        // For `ldr xRt,[xRn,#(imm12*8)]` the word must equal the golden
        // `ldr x0,[x1,#0]` offset additively by Rt[4:0], Rn[9:5], imm12[21:10].
        #[test]
        fn prop_unsigned_offset_layout(
            rt in arb_reg_num(),
            rn in arb_reg_num(),
            imm12 in 0u32..4096u32,
        ) {
            let offset = (imm12 as i64) * 8; // align = 1 << size = 8 for size=0b11
            let ops = vec![gp_xreg(rt), Operand::Mem { base: format!("x{}", rn), offset }];
            let w = word(encode_ldr_str(&ops, true, 0b11, false, false));
            // golden `ldr x0,[x1]` is rt=0, Rn=1, imm12=0; offset by field deltas.
            let expected = (GOLDEN_LDR_X0_X1_0 as i64
                + (rt as i64)
                + (((rn as i64) - 1) << 5)
                + ((imm12 as i64) << 10)) as u32;
            prop_assert_eq!(w, expected);
        }

        // Property 2 — load vs store differ ONLY in opc bit 22 (0x0040_0000).
        #[test]
        fn prop_load_xor_store_is_opc_bit22(
            rt in arb_reg_num(),
            rn in arb_reg_num(),
            imm12 in 0u32..4096u32,
        ) {
            let offset = (imm12 as i64) * 8;
            let ops = vec![gp_xreg(rt), Operand::Mem { base: format!("x{}", rn), offset }];
            let load  = word(encode_ldr_str(&ops, true,  0b11, false, false));
            let store = word(encode_ldr_str(&ops, false, 0b11, false, false));
            prop_assert_eq!(load ^ store, GOLDEN_LDR_X0_X1_0 ^ GOLDEN_STR_X0_X1_0);
            prop_assert_eq!(load ^ store, 0x0040_0000);
        }

        // Property 3 — the explicit `size` parameter lands in bits [31:30].
        #[test]
        fn prop_size_param_in_top_two_bits(size in 0u32..4u32) {
            let ops = vec![gp_xreg(0), Operand::Mem { base: "x1".to_string(), offset: 0 }];
            let w = word(encode_ldr_str(&ops, true, size, false, false));
            prop_assert_eq!((w >> 30) & 0b11, size);
        }

        // Property 4 — pre/post-index layout vs golden.
        // imm9 occupies [20:12]; the idx-marker [11:10] is 11 (pre) vs 01 (post).
        // Restricted to non-negative imm9 to keep the golden arithmetic additive.
        #[test]
        fn prop_pre_post_index_layout(
            rt in arb_reg_num(),
            rn in arb_reg_num(),
            imm9 in 0i32..=255i32,
        ) {
            // pre-index: golden is rt=0, rn=1, imm9=8
            {
                let ops = vec![gp_xreg(rt), Operand::MemPreIndex { base: format!("x{}", rn), offset: imm9 as i64 }];
                let w = word(encode_ldr_str(&ops, true, 0b11, false, false));
                let expected = (GOLDEN_PRE_X0_X1_8 as i64
                    + (rt as i64)
                    + (((rn as i64) - 1) << 5)
                    + (((imm9 as i64) - 8) << 12)) as u32;
                prop_assert_eq!(w, expected);
            }
            // post-index
            {
                let ops = vec![gp_xreg(rt), Operand::MemPostIndex { base: format!("x{}", rn), offset: imm9 as i64 }];
                let w = word(encode_ldr_str(&ops, true, 0b11, false, false));
                let expected = (GOLDEN_POST_X0_X1_8 as i64
                    + (rt as i64)
                    + (((rn as i64) - 1) << 5)
                    + (((imm9 as i64) - 8) << 12)) as u32;
                prop_assert_eq!(w, expected);
            }
        }

        // Property 5 — NEGATIVE CONTRACT.
        // For the [base,#imm] form the encodable range is the UNION of the
        // unsigned-offset field (imm12*8 ∈ [0, 32760]) and the unscaled imm9
        // ([-256, 255]). An offset strictly outside [-256, 32760] cannot be
        // represented by EITHER encoding, so the encoder MUST return Err
        // rather than silently truncating the immediate to 9 bits.
        #[test]
        fn prop_out_of_range_offset_is_rejected(
            excess in 1u32..2000u32,
            negative in any::<bool>(),
        ) {
            let offset = if negative {
                -256i64 - excess as i64
            } else {
                32760i64 + excess as i64
            };
            let ops = vec![gp_xreg(0), Operand::Mem { base: "x1".to_string(), offset }];
            let r = encode_ldr_str(&ops, true, 0b11, false, false);
            prop_assert!(
                r.is_err(),
                "offset {} is outside the LDR/STR immediate encodable range \
                 [-256, 32760] and must be rejected, but the encoder returned {:?}",
                offset, r
            );
        }
    }
}

#[cfg(test)]
mod prop_encode_ldur_stur_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Independent oracle ───────────────────────────────────────────────
    // ORACLE: reference-encoding / field-placement (ARMv8-A Architecture
    // Reference Manual, §C4.1.66 LDUR/STUR “Load/Store Register (unscaled
    // immediate)”).
    //
    // Encoding (GP, V=0):
    //   size[31:30] 111[29:27] V[26] 00[25:24] opc[23:22] 0[21]
    //     imm9[20:12] op2[11:10] Rn[9:5] Rt[4:0]
    //
    // Hand-derived golden encodings (independently cross-checked against
    // `llvm-mc`/objdump), used as an anchor rather than this crate's own
    // bit-fiddling formula:
    //
    //   ldur x0, [x1]      = 0xF8400020   (size=11, opc=01, imm9=0, op2=00)
    //   ldur x0, [x1, #8]  = 0xF8408020   (imm9=8)
    //   ldur x0, [x1, #-1] = 0xF85FF020   (imm9=0x1FF = -1 in 9-bit 2's-comp)
    //   ldur w0, [x1]      = 0xB8400020   (size=10)
    //
    // The 9-bit imm9 is a SIGNED immediate: encodable range [-256, 255].

    const GOLDEN_LDUR_X0_X1_0: u32 = 0xF8400020;
    const GOLDEN_LDUR_X0_X1_8: u32 = 0xF8408020;
    const GOLDEN_LDUR_X0_X1_M1: u32 = 0xF85FF020;
    const GOLDEN_LDUR_W0_X1_0: u32 = 0xB8400020;

    fn gp_xreg(num: u32) -> Operand {
        Operand::Reg(format!("x{}", num))
    }

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    prop_compose! {
        fn arb_reg_num()(n in 0u32..=30u32) -> u32 { n }
    }

    proptest! {
        // Property 1 — full-word field layout vs golden.
        // For `ldur xRt,[xRn,#off]` with off in [0,255] (inside the imm9
        // range, so no masking aliasing), the word equals the golden
        // `ldur x0,[x1,#0]` offset additively by Rt[4:0], Rn[9:5], and
        // imm9[20:12]. Anchored to the ARM ARM, not to this crate's formula.
        #[test]
        fn prop_gp_layout_matches_golden(
            rt in arb_reg_num(),
            rn in arb_reg_num(),
            off in 0i64..=255i64,
        ) {
            let ops = vec![gp_xreg(rt), Operand::Mem { base: format!("x{}", rn), offset: off }];
            let w = word(encode_ldur_stur(&ops, true, 0b00));
            let expected = (GOLDEN_LDUR_X0_X1_0 as i64
                + (rt as i64)
                + (((rn as i64) - 1) << 5)
                + (off << 12)) as u32;
            prop_assert_eq!(w, expected);
        }

        // Property 1b — negative imm9 round-trips through the 9-bit field.
        // The field at [20:12], sign-extended back to i32, must equal the
        // input offset for every imm9 in the valid range [-256, 255], and
        // the three documented golden offsets must match exactly.
        #[test]
        fn prop_imm9_field_sign_extended_equals_input(off in -256i64..=255i64) {
            let ops = vec![gp_xreg(0), Operand::Mem { base: "x1".to_string(), offset: off }];
            let w = word(encode_ldur_stur(&ops, true, 0b00));
            let field = ((w >> 12) & 0x1FF) as i32;
            let sx = if field & 0x100 != 0 { field | (!0x1FF) } else { field };
            prop_assert_eq!(sx, off as i32);
            if off == 0   { prop_assert_eq!(w, GOLDEN_LDUR_X0_X1_0); }
            if off == 8   { prop_assert_eq!(w, GOLDEN_LDUR_X0_X1_8); }
            if off == -1  { prop_assert_eq!(w, GOLDEN_LDUR_X0_X1_M1); }
        }

        // Property 2 — size field [31:30] tracks register width:
        // xN -> 0b11 (64-bit), wN -> 0b10 (32-bit); matches golden offsets.
        #[test]
        fn prop_size_field_tracks_reg_width(rt in arb_reg_num()) {
            let mem = || Operand::Mem { base: "x1".to_string(), offset: 0 };
            let xw = word(encode_ldur_stur(&[Operand::Reg(format!("x{}", rt)), mem()], true, 0b00));
            let ww = word(encode_ldur_stur(&[Operand::Reg(format!("w{}", rt)), mem()], true, 0b00));
            prop_assert_eq!((xw >> 30) & 0b11, 0b11u32);
            prop_assert_eq!((ww >> 30) & 0b11, 0b10u32);
            prop_assert_eq!(xw, GOLDEN_LDUR_X0_X1_0 + rt);
            prop_assert_eq!(ww, GOLDEN_LDUR_W0_X1_0 + rt);
        }

        // Property 3 — differential: load vs store differ ONLY in opc bit 22.
        // For GP registers ldur opc=01, stur opc=00, so load ^ store == 0x0040_0000.
        #[test]
        fn prop_load_xor_store_is_opc_bit22(
            rt in arb_reg_num(),
            rn in arb_reg_num(),
            off in -256i64..=255i64,
        ) {
            let ops = vec![gp_xreg(rt), Operand::Mem { base: format!("x{}", rn), offset: off }];
            let load  = word(encode_ldur_stur(&ops, true,  0b00));
            let store = word(encode_ldur_stur(&ops, false, 0b00));
            prop_assert_eq!(load ^ store, 0x0040_0000u32);
        }

        // Property 4 — op2_bits parameter lands in bits [11:10].
        // The function is shared by LDUR/STUR (op2=0b00) and LDTR/STTR
        // (op2=0b10); only bits [11:10] may differ, (w>>10)&0b11 must equal
        // the parameter, and op2=0b10 flips exactly bit 11 (0x0000_0800).
        #[test]
        fn prop_op2_bits_in_field_11_10(op2 in 0u32..4u32) {
            let ops = vec![gp_xreg(0), Operand::Mem { base: "x1".to_string(), offset: 0 }];
            let w = word(encode_ldur_stur(&ops, true, op2));
            prop_assert_eq!((w >> 10) & 0b11, op2 & 0b11);
            let base = word(encode_ldur_stur(&ops, true, 0b00));
            prop_assert_eq!(w & !0xC00u32, base & !0xC00u32);
            let w10 = word(encode_ldur_stur(&ops, true, 0b10));
            prop_assert_eq!(w10 ^ base, 0x0000_0800u32);
        }

        // Property 5 — NEGATIVE CONTRACT (expected to FAIL: silent truncation).
        // The 9-bit imm9 is a SIGNED immediate covering [-256, 255]. An offset
        // strictly outside that range cannot be represented by LDUR/STUR and the
        // encoder MUST return Err (the user should use the LDR unsigned-offset
        // form instead). Instead the implementation does
        //   imm9_enc = (imm9 as u32) & 0x1FF
        // silently wrapping out-of-range offsets (#256 -> #0, #-257 -> #-1).
        #[test]
        fn prop_out_of_range_imm9_is_rejected(
            excess in 1u32..2000u32,
            negative in any::<bool>(),
        ) {
            let offset = if negative {
                -256i64 - excess as i64
            } else {
                255i64 + excess as i64
            };
            let ops = vec![gp_xreg(0), Operand::Mem { base: "x1".to_string(), offset }];
            let r = encode_ldur_stur(&ops, true, 0b00);
            prop_assert!(
                r.is_err(),
                "offset {} is outside the LDUR/STUR imm9 range [-256, 255] \
                 and must be rejected, but the encoder returned {:?}",
                offset, r
            );
        }
    }
}

#[cfg(test)]
mod prop_encode_ldp_stp_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Independent oracle ───────────────────────────────────────────────
    // ORACLE: reference-encoding / field-placement (ARMv8-A Architecture
    // Reference Manual, §C4.1.48 “LDP/STP (pair)”).
    //
    // Encoding:
    //   opc[31:30] 101[29:27] V[26] idx[25:23] L[22] imm7[21:15] Rt2[14:10] Rn[9:5] Rt[4:0]
    // where idx = 010 (signed offset), 011 (pre-index), 001 (post-index).
    //
    // imm7 is a SIGNED 7-bit field: range [-64, 63]. The actual address offset
    // must be a multiple of the access size (scale = 1<<shift) and within
    //   [-64 * scale, 63 * scale].
    //
    // Hand-derived golden encodings (independently derived from the ARM ARM
    // field layout, NOT from this crate's formula):
    //
    //   stp x0, x1, [x2]       = 0xA9000440   (opc=10, idx=010, L=0, imm7=0, Rt2=1, Rn=2)
    //   ldp x0, x1, [x2]       = 0xA9400440   (L=1)
    //   stp w0, w1, [x2]       = 0x29000440   (opc=00, 32-bit)
    //   stp x0, x1, [x2, #16]  = 0xA9010440   (imm7=2, signed offset)
    //   stp x0, x1, [x2, #16]! = 0xA9810440   (idx=011, pre-index)
    //   stp x0, x1, [x2], #16  = 0xA8810440   (idx=001, post-index)
    //   stp d0, d1, [x2]       = 0x6D000440   (opc=01, V=1)
    //   stp s0, s1, [x2]       = 0x2D000440   (opc=00, V=1)
    //   stp q0, q1, [x2]       = 0xAD000440   (opc=10, V=1)

    const GOLDEN_STP_X0_X1_X2_0: u32 = 0xA9000440;
    const GOLDEN_LDP_X0_X1_X2_0: u32 = 0xA9400440;
    const GOLDEN_STP_W0_W1_X2_0: u32 = 0x29000440;
    const GOLDEN_STP_X0_X1_X2_16: u32 = 0xA9010440;
    const GOLDEN_STP_PRE_16: u32 = 0xA9810440;
    const GOLDEN_STP_POST_16: u32 = 0xA8810440;
    const GOLDEN_STP_D0_D1_X2_0: u32 = 0x6D000440;
    const GOLDEN_STP_S0_S1_X2_0: u32 = 0x2D000440;
    const GOLDEN_STP_Q0_Q1_X2_0: u32 = 0xAD000440;

    fn gp_reg(prefix: char, num: u32) -> Operand {
        Operand::Reg(format!("{}{}", prefix, num))
    }

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    prop_compose! {
        fn arb_reg_num()(n in 0u32..=30u32) -> u32 { n }
    }

    // Register-class prefix for LDP/STP operands: GP (x,w) and FP/SIMD (d,s,q).
    prop_compose! {
        fn arb_prefix()(idx in 0usize..5usize) -> char {
            ['x', 'w', 'd', 's', 'q'][idx]
        }
    }

    // Scale (1<<shift) per register class: x/zr -> 8, w -> 4, s -> 4, d -> 8, q -> 16.
    fn scale_of(prefix: char) -> i64 {
        match prefix {
            'x' | 'd' => 8,
            'w' | 's' => 4,
            'q' => 16,
            _ => 8,
        }
    }

    proptest! {
        // Property 1 — full-word field layout vs golden (signed offset form).
        // For `stp xRt1, xRt2, [xRn]` the word must equal the golden
        // `stp x0, x1, [x2]` offset additively by Rt[4:0], Rt2[14:10], Rn[9:5].
        #[test]
        fn prop_signed_offset_layout(
            rt1 in arb_reg_num(),
            rt2 in arb_reg_num(),
            rn in arb_reg_num(),
        ) {
            let ops = vec![
                gp_reg('x', rt1),
                gp_reg('x', rt2),
                Operand::Mem { base: format!("x{}", rn), offset: 0 },
            ];
            let w = word(encode_ldp_stp(&ops, false));
            let expected = (GOLDEN_STP_X0_X1_X2_0 as i64
                + (rt1 as i64)
                + (((rt2 as i64) - 1) << 10)
                + (((rn as i64) - 2) << 5)) as u32;
            prop_assert_eq!(w, expected);
        }

        // Property 2 — differential: load vs store differ ONLY in L bit 22.
        // stp opc=10/L=0 vs ldp opc=10/L=1, so load ^ store == 0x0040_0000.
        #[test]
        fn prop_load_xor_store_is_l_bit22(
            rt1 in arb_reg_num(),
            rt2 in arb_reg_num(),
            rn in arb_reg_num(),
            off_units in -64i64..=63i64,
        ) {
            let offset = off_units * 8; // 64-bit GP scale
            let ops = vec![
                gp_reg('x', rt1),
                gp_reg('x', rt2),
                Operand::Mem { base: format!("x{}", rn), offset },
            ];
            let load = word(encode_ldp_stp(&ops, true));
            let store = word(encode_ldp_stp(&ops, false));
            prop_assert_eq!(load ^ store, GOLDEN_LDP_X0_X1_X2_0 ^ GOLDEN_STP_X0_X1_X2_0);
            prop_assert_eq!(load ^ store, 0x0040_0000u32);
        }

        // Property 3 — opc[31:30] and V[26] track register class.
        //   xN -> opc=10, V=0 ; wN -> opc=00, V=0 ; dN -> opc=01, V=1 ; qN -> opc=10, V=1.
        #[test]
        fn prop_opc_and_v_track_reg_class(prefix in arb_prefix()) {
            // Use the exact golden operands (rt1=0, rt2=1, base=x2) so the
            // full-word cross-check is a direct equality.
            let ops = vec![
                gp_reg(prefix, 0),
                gp_reg(prefix, 1),
                Operand::Mem { base: "x2".to_string(), offset: 0 },
            ];
            let w = word(encode_ldp_stp(&ops, false));
            let (exp_opc, exp_v) = match prefix {
                'x' => (0b10u32, 0u32),
                'w' => (0b00u32, 0u32),
                'd' => (0b01u32, 1u32),
                's' => (0b00u32, 1u32),
                'q' => (0b10u32, 1u32),
                _ => unreachable!(),
            };
            prop_assert_eq!((w >> 30) & 0b11, exp_opc);
            prop_assert_eq!((w >> 26) & 1, exp_v);
            // Cross-check against the hand-derived goldens for each class.
            let golden = match prefix {
                'x' => GOLDEN_STP_X0_X1_X2_0,
                'w' => GOLDEN_STP_W0_W1_X2_0,
                'd' => GOLDEN_STP_D0_D1_X2_0,
                's' => GOLDEN_STP_S0_S1_X2_0,
                'q' => GOLDEN_STP_Q0_Q1_X2_0,
                _ => unreachable!(),
            };
            prop_assert_eq!(w, golden);
        }

        // Property 4 — index form occupies bits [25:23]: pre=011, signed=010, post=001.
        #[test]
        fn prop_index_form_bits(prefix in arb_prefix(), off_units in -64i64..=63i64) {
            let scale = scale_of(prefix);
            let offset = off_units * scale;
            let signed_ops = vec![gp_reg(prefix, 0), gp_reg(prefix, 1),
                Operand::Mem { base: "x2".to_string(), offset }];
            let pre_ops = vec![gp_reg(prefix, 0), gp_reg(prefix, 1),
                Operand::MemPreIndex { base: "x2".to_string(), offset }];
            let post_ops = vec![gp_reg(prefix, 0), gp_reg(prefix, 1),
                Operand::MemPostIndex { base: "x2".to_string(), offset }];
            let signed = word(encode_ldp_stp(&signed_ops, false));
            let pre = word(encode_ldp_stp(&pre_ops, false));
            let post = word(encode_ldp_stp(&post_ops, false));
            prop_assert_eq!((signed >> 23) & 0b111, 0b010u32);
            prop_assert_eq!((pre >> 23) & 0b111, 0b011u32);
            prop_assert_eq!((post >> 23) & 0b111, 0b001u32);
            // Only bits [25:23] should differ between the three forms.
            prop_assert_eq!(pre ^ signed, 0b001u32 << 23);
            prop_assert_eq!(post ^ signed, 0b011u32 << 23);
        }

        // Property 5 — imm7 sign-extends back to the scaled offset within the
        // valid 7-bit signed range. Field [21:15], 7-bit two's-complement.
        #[test]
        fn prop_imm7_sign_extended_equals_scaled_offset(
            off_units in -64i64..=63i64,
            prefix in arb_prefix(),
        ) {
            let scale = scale_of(prefix);
            let offset = off_units * scale;
            let ops = vec![
                gp_reg(prefix, 0),
                gp_reg(prefix, 1),
                Operand::Mem { base: "x2".to_string(), offset },
            ];
            let w = word(encode_ldp_stp(&ops, false));
            let field = ((w >> 15) & 0x7F) as i32;
            let sx = if field & 0x40 != 0 { field | (!0x7F) } else { field };
            prop_assert_eq!(sx, off_units as i32);
        }

        // Property 6 — error contract: fewer than 3 operands is rejected.
        #[test]
        fn prop_too_few_operands_errors(n in 0usize..3) {
            let ops: Vec<Operand> = (0..n)
                .map(|i| gp_reg('x', i as u32))
                .collect();
            let r = encode_ldp_stp(&ops, true);
            prop_assert!(r.is_err(), "expected error, got {:?}", r);
        }

        // Property 7 — NEGATIVE CONTRACT (expected to FAIL: silent wrapping).
        // imm7 is a SIGNED 7-bit field. For a 64-bit GP register the scaled
        // offset must lie in [-512, 504] (i.e. off_units in [-64, 63]). An
        // offset strictly outside this range is not representable and the
        // assembler MUST reject it. Instead the implementation does
        //   imm7 = ((*offset >> shift) as i32) & 0x7F
        // silently wrapping, e.g. #512 -> imm7=0x40 -> -64 -> encodes #-512,
        // and #-520 -> imm7=0x3F -> +63 -> encodes #+504.
        #[test]
        fn prop_out_of_range_offset_is_rejected(
            excess in 1u32..2000u32,
            negative in any::<bool>(),
        ) {
            let offset = if negative {
                -512i64 - excess as i64
            } else {
                504i64 + excess as i64
            };
            let ops = vec![
                gp_reg('x', 0),
                gp_reg('x', 1),
                Operand::Mem { base: "x2".to_string(), offset },
            ];
            let r = encode_ldp_stp(&ops, true);
            prop_assert!(
                r.is_err(),
                "offset {} is outside the LDP/STP imm7 range [-512, 504] \
                 (64-bit GP, step 8) and must be rejected, but the encoder \
                 returned {:?}",
                offset, r
            );
        }

        // Property 8 — NEGATIVE CONTRACT (expected to FAIL: silent rounding).
        // The LDP/STP immediate MUST be a multiple of the access size
        // (scale = 1<<shift). An unaligned offset (e.g. #1 for 64-bit regs)
        // is not representable and must be rejected. Instead the right-shift
        // `offset >> shift` silently floors it (e.g. #1 -> imm7=0 -> #0).
        #[test]
        fn prop_unaligned_offset_is_rejected(prefix in arb_prefix()) {
            let scale = scale_of(prefix);
            // A small positive offset that is NOT a multiple of the scale.
            let offset = scale + 1;
            let ops = vec![
                gp_reg(prefix, 0),
                gp_reg(prefix, 1),
                Operand::Mem { base: "x2".to_string(), offset },
            ];
            let r = encode_ldp_stp(&ops, true);
            prop_assert!(
                r.is_err(),
                "offset {} is not a multiple of the LDP/STP access size {} \
                 and must be rejected, but the encoder returned {:?}",
                offset, scale, r
            );
        }
    }

    // Golden cross-check (no inputs): pre/post/signed index forms with
    // offset #16 must match the hand-derived ARMv8 goldens exactly.
    #[test]
    fn golden_pre_post_index_forms() {
        let pre_ops = vec![
            gp_reg('x', 0),
            gp_reg('x', 1),
            Operand::MemPreIndex { base: "x2".to_string(), offset: 16 },
        ];
        assert_eq!(word(encode_ldp_stp(&pre_ops, false)), GOLDEN_STP_PRE_16);
        let post_ops = vec![
            gp_reg('x', 0),
            gp_reg('x', 1),
            Operand::MemPostIndex { base: "x2".to_string(), offset: 16 },
        ];
        assert_eq!(word(encode_ldp_stp(&post_ops, false)), GOLDEN_STP_POST_16);
        let signed_ops = vec![
            gp_reg('x', 0),
            gp_reg('x', 1),
            Operand::Mem { base: "x2".to_string(), offset: 16 },
        ];
        assert_eq!(word(encode_ldp_stp(&signed_ops, false)), GOLDEN_STP_X0_X1_X2_16);
    }
}

#[cfg(test)]
mod prop_encode_ldnp_stnp_tests {
    use super::*;
    use proptest::prelude::*;

    // ORACLE: reference-encoding / field-placement (ARMv8-A ARM, §C4.1.66
    // “LDNP/STNP — Load/store no-allocate pair”). Bit layout:
    //   opc[31:30] | 101[29:27] | V[26] | 000[25:23] | L[22] | imm7[21:15]
    //   | Rt2[14:10] | Rn[9:5] | Rt[4:0]
    // - opc=10 for 64-bit (Xn), opc=00 for 32-bit (Wn). V is hard-wired 0
    //   (integer-only; FP/SIMD is a documented TODO in the source).
    // - imm7 is a SIGNED 7-bit scaled immediate: scale=8 (64-bit) / scale=4
    //   (32-bit). Architectural range: imm7 ∈ [-64, +63], i.e. offset ∈
    //   [-512, +504] (64-bit) / [-256, +252] (32-bit), and the offset MUST be a
    //   multiple of the scale. The ARM ARM mandates that an assembler REJECT
    //   out-of-range and misaligned immediates (it is a constraint, not UB).

    fn gp_reg(prefix: char, num: u32) -> Operand {
        Operand::Reg(format!("{}{}", prefix, num))
    }

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    prop_compose! {
        fn arb_reg_num()(n in 0u32..=30u32) -> u32 { n }
    }

    // imm7 in the valid signed range [-63, +63]; offset = imm7 * scale stays
    // aligned and in-range for both 64- and 32-bit forms.
    prop_compose! {
        fn arb_imm7()(v in -63i32..=63i32) -> i32 { v }
    }

    proptest! {
        // Property 1 — load vs store differ ONLY in the L bit [22].
        // Swapping is_load flips exactly 0x0040_0000, for both 64- and 32-bit.
        #[test]
        fn prop_load_xor_store_is_l_bit(
            rt1 in arb_reg_num(),
            rt2 in arb_reg_num(),
            rn in arb_reg_num(),
            is64 in any::<bool>(),
        ) {
            let p = if is64 { 'x' } else { 'w' };
            let ops = vec![
                gp_reg(p, rt1),
                gp_reg(p, rt2),
                Operand::Mem { base: format!("x{}", rn), offset: 0 },
            ];
            let load  = word(encode_ldnp_stnp(&ops, true));
            let store = word(encode_ldnp_stnp(&ops, false));
            prop_assert_eq!(load ^ store, 0x0040_0000u32);
        }

        // Property 2 — opc [31:30] + fixed field placement.
        // opc=0b10 for Xn, 0b00 for Wn; Rt→[4:0], Rn→[9:5], Rt2→[14:10];
        // V=0 (integer), [25:23]=000, [29:27]=101.
        #[test]
        fn prop_opc_and_register_fields(
            rt1 in arb_reg_num(),
            rt2 in arb_reg_num(),
            rn in arb_reg_num(),
            is64 in any::<bool>(),
        ) {
            let p = if is64 { 'x' } else { 'w' };
            let ops = vec![
                gp_reg(p, rt1),
                gp_reg(p, rt2),
                Operand::Mem { base: format!("x{}", rn), offset: 0 },
            ];
            let w = word(encode_ldnp_stnp(&ops, true));
            prop_assert_eq!((w >> 30) & 0b11, if is64 { 0b10u32 } else { 0b00u32 });
            prop_assert_eq!(w & 0x1F, rt1);             // Rt  [4:0]
            prop_assert_eq!((w >> 5) & 0x1F, rn);         // Rn  [9:5]
            prop_assert_eq!((w >> 10) & 0x1F, rt2);       // Rt2 [14:10]
            prop_assert_eq!((w >> 26) & 1, 0u32);         // V = 0
            prop_assert_eq!((w >> 23) & 0b111, 0u32);     // [25:23] = 000
            prop_assert_eq!((w >> 27) & 0b111, 0b101u32); // [29:27] = 101
        }

        // Property 3 — imm7 scaling lands in [21:15] for in-range aligned offsets.
        // 64-bit scale=8, 32-bit scale=4; imm7 ∈ [-63,+63].
        #[test]
        fn prop_imm7_scaling(
            imm7 in arb_imm7(),
            rn in arb_reg_num(),
            is64 in any::<bool>(),
        ) {
            let scale = if is64 { 3i64 } else { 2 };
            let offset = (imm7 as i64) << scale;
            let p = if is64 { 'x' } else { 'w' };
            let ops = vec![
                gp_reg(p, 0),
                gp_reg(p, 1),
                Operand::Mem { base: format!("x{}", rn), offset },
            ];
            let w = word(encode_ldnp_stnp(&ops, true));
            prop_assert_eq!((w >> 15) & 0x7F, (imm7 as u32) & 0x7F);
        }

        // Property 4 — NEGATIVE contract: out-of-range imm7 MUST be rejected.
        // imm7 is signed 7-bit: valid range [-64, +63]. For 64-bit (scale 8),
        // offset #512 ⇒ imm7 = +64 (> +63, out of range). The ARM ARM mandates
        // rejection. The encoder currently does `(*offset >> shift) & 0x7F`,
        // so #512 → imm7 field 0b1000000 = -64 ⇒ decoded offset -512 (silent
        // corruption: +512 → -512). This property FAILS, documenting the bug.
        #[test]
        fn prop_negative_imm7_range_violation_rejects(
            is64 in any::<bool>(),
        ) {
            let scale = if is64 { 3i64 } else { 2 };
            let offset = 64i64 << scale; // imm7 = +64, just past the valid +63
            let p = if is64 { 'x' } else { 'w' };
            let ops = vec![
                gp_reg(p, 0),
                gp_reg(p, 1),
                Operand::Mem { base: "x2".to_string(), offset },
            ];
            let r = encode_ldnp_stnp(&ops, true);
            prop_assert!(
                r.is_err(),
                "out-of-range imm7 (offset {}, imm7=+64) must be rejected, got {:?}",
                offset, r
            );
        }

        // Property 5 — NEGATIVE contract: misaligned offset MUST be rejected.
        // The offset must be a multiple of the scale (8 or 4). #5 is misaligned
        // for both, yet the encoder computes imm7 = (5 >> shift) & 0x7F = 0 and
        // silently encodes #5 as #0. ARM ARM requires rejection. FAILS → bug.
        #[test]
        fn prop_negative_misaligned_offset_rejects(
            is64 in any::<bool>(),
        ) {
            let p = if is64 { 'x' } else { 'w' };
            let ops = vec![
                gp_reg(p, 0),
                gp_reg(p, 1),
                Operand::Mem { base: "x2".to_string(), offset: 5 },
            ];
            let r = encode_ldnp_stnp(&ops, true);
            prop_assert!(
                r.is_err(),
                "misaligned offset #5 must be rejected, got {:?}",
                r
            );
        }

        // Property 6 — error contract: arity < 3 or a non-Mem third operand → Err.
        #[test]
        fn prop_error_contract(
            nregs in 0u8..3u8,
            bad_kind in 0u8..3u8,
        ) {
            let bad = match bad_kind {
                0 => Operand::MemPreIndex { base: "x2".to_string(), offset: 0 },
                1 => Operand::Imm(7),
                _ => Operand::Symbol("foo".to_string()),
            };
            let mut ops: Vec<Operand> = vec![gp_reg('x', 0), gp_reg('x', 1)];
            if nregs == 2 {
                ops.push(bad); // wrong shape at slot 2
            }
            let r = encode_ldnp_stnp(&ops, true);
            prop_assert!(r.is_err(), "expected error for ops={:?}, got {:?}", ops, r);
        }
    }

    // Golden cross-check (no inputs): hand-derived ARMv8 LDNP/STNP encodings.
    //   ldnp x0, x1, [x2]  → opc=10,101,V=0,000,L=1,imm7=0,Rt2=1,Rn=2,Rt=0 = 0xA8400440
    //   stnp x0, x1, [x2]  → L=0                                          = 0xA8000440
    // NOTE: derived from the ARMv8 ARM bit layout; no AArch64 cross-assembler
    // (llvm-mc / aarch64-as) is available in this environment to objdump-verify,
    // so the relationship properties above carry the independent-checking weight.
    #[test]
    fn golden_ldnp_stnp_encodings() {
        let ops = vec![
            gp_reg('x', 0),
            gp_reg('x', 1),
            Operand::Mem { base: "x2".to_string(), offset: 0 },
        ];
        assert_eq!(word(encode_ldnp_stnp(&ops, true)), 0xA8400440);
        assert_eq!(word(encode_ldnp_stnp(&ops, false)), 0xA8000440);
    }
}

#[cfg(test)]
mod prop_encode_ldtr_sized_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Independent oracle ───────────────────────────────────────────────
    // ORACLE: reference-encoding / field-placement (ARMv8-A Architecture
    // Reference Manual, §C4.1.66 LDTR/STTR “Load/Store Register (simm9,
    // unprivileged)”).
    //
    // Encoding (GP, V=0):
    //   size[31:30] 111[29:27] V=0[26] 00[25:24] opc[23:22] 0[21]
    //     imm9[20:12] 10[11:10] Rn[9:5] Rt[4:0]
    //
    // - opc=01 (load) / 00 (store); V is hard-wired 0 (GP only).
    // - imm9 is a SIGNED 9-bit immediate: encodable range [-256, 255].
    //
    // Hand-derived golden encodings (built up directly from the ARM ARM
    // field layout, NOT from this crate's bit-fiddling formula):
    //
    //   ldtr x0, [x1]      = 0xF8400820   (size=11, opc=01, imm9=0)
    //   sttr x0, [x1]      = 0xF8000820   (opc=00)
    //   ldtr x0, [x1, #8]  = 0xF8408820   (imm9=8 → <<12)
    //   ldtr x0, [x1, #-1] = 0xF85FF820   (imm9=0x1FF = -1 in 9-bit 2's-comp)
    //   ldtrb w0, [x1]     = 0x38400820   (size=00)
    //
    // No AArch64 cross-assembler (llvm-mc / aarch64-as) is available in this
    // environment to objdump-verify; the relationship / field-placement
    // properties below carry the independent-checking weight.

    const GOLDEN_LDTR_X0_X1_0: u32 = 0xF8400820;
    const GOLDEN_STTR_X0_X1_0: u32 = 0xF8000820;
    const GOLDEN_LDTR_X0_X1_8: u32 = 0xF8408820;
    const GOLDEN_LDTR_X0_X1_M1: u32 = 0xF85FF820;
    const GOLDEN_LDTRB_W0_X1_0: u32 = 0x38400820;

    fn gp_xreg(num: u32) -> Operand {
        Operand::Reg(format!("x{}", num))
    }

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    prop_compose! {
        fn arb_reg_num()(n in 0u32..=30u32) -> u32 { n }
    }

    proptest! {
        // Property 1 — full-word field layout vs golden.
        // For `ldtr xRt,[xRn,#off]` with off in [0,255] (inside the imm9 range,
        // so no masking aliasing), the word equals the golden `ldtr x0,[x1,#0]`
        // offset additively by Rt[4:0], Rn[9:5], and imm9[20:12].
        #[test]
        fn prop_gp_layout_matches_golden(
            rt in arb_reg_num(),
            rn in arb_reg_num(),
            off in 0i64..=255i64,
        ) {
            let ops = vec![gp_xreg(rt), Operand::Mem { base: format!("x{}", rn), offset: off }];
            let w = word(encode_ldtr_sized(&ops, true, 0b11));
            let expected = (GOLDEN_LDTR_X0_X1_0 as i64
                + (rt as i64)
                + (((rn as i64) - 1) << 5)
                + (off << 12)) as u32;
            prop_assert_eq!(w, expected);
        }

        // Property 2 — differential: load vs store differ ONLY in opc bit 22.
        // ldtr opc=01, sttr opc=00, so load ^ store == 0x0040_0000 for any size.
        #[test]
        fn prop_load_xor_store_is_opc_bit22(
            rt in arb_reg_num(),
            rn in arb_reg_num(),
            off in -256i64..=255i64,
            size in 0u32..4u32,
        ) {
            let ops = vec![gp_xreg(rt), Operand::Mem { base: format!("x{}", rn), offset: off }];
            let load  = word(encode_ldtr_sized(&ops, true,  size));
            let store = word(encode_ldtr_sized(&ops, false, size));
            prop_assert_eq!(load ^ store, GOLDEN_LDTR_X0_X1_0 ^ GOLDEN_STTR_X0_X1_0);
            prop_assert_eq!(load ^ store, 0x0040_0000);
        }

        // Property 3 — the explicit `size` parameter lands in bits [31:30],
        // matching the mnemonic-derived size (ldtrb=00, ldtrh=01, ldtr(W)=10,
        // ldtr(X)=11). Golden cross-check for size=00 (ldtrb).
        #[test]
        fn prop_size_param_in_top_two_bits(size in 0u32..4u32) {
            let ops = vec![gp_xreg(0), Operand::Mem { base: "x1".to_string(), offset: 0 }];
            let w = word(encode_ldtr_sized(&ops, true, size));
            prop_assert_eq!((w >> 30) & 0b11, size);
        }

        // Property 4 — imm9 field [20:12], sign-extended back to i32, equals
        // the input offset for every imm9 in the valid range [-256, 255].
        // Also pins the three documented golden offsets exactly.
        #[test]
        fn prop_imm9_field_sign_extended_equals_input(off in -256i64..=255i64) {
            let ops = vec![gp_xreg(0), Operand::Mem { base: "x1".to_string(), offset: off }];
            let w = word(encode_ldtr_sized(&ops, true, 0b11));
            let field = ((w >> 12) & 0x1FF) as i32;
            let sx = if field & 0x100 != 0 { field | (!0x1FF) } else { field };
            prop_assert_eq!(sx, off as i32);
            if off == 0   { prop_assert_eq!(w, GOLDEN_LDTR_X0_X1_0); }
            if off == 8   { prop_assert_eq!(w, GOLDEN_LDTR_X0_X1_8); }
            if off == -1  { prop_assert_eq!(w, GOLDEN_LDTR_X0_X1_M1); }
        }

        // Property 5 — NEGATIVE CONTRACT (expected to FAIL: silent truncation).
        // The 9-bit imm9 is a SIGNED immediate covering [-256, 255]. An offset
        // strictly outside that range cannot be represented by LDTR/STTR and the
        // encoder MUST return Err. Instead the implementation does
        //   imm9_enc = (imm9 as u32) & 0x1FF
        // silently wrapping out-of-range offsets (#256 -> #0, #-257 -> #-1),
        // emitting a wrong instruction word with no diagnostic.
        #[test]
        fn prop_out_of_range_imm9_is_rejected(
            excess in 1u32..2000u32,
            negative in any::<bool>(),
        ) {
            let offset = if negative {
                -256i64 - excess as i64
            } else {
                255i64 + excess as i64
            };
            let ops = vec![gp_xreg(0), Operand::Mem { base: "x1".to_string(), offset }];
            let r = encode_ldtr_sized(&ops, true, 0b11);
            prop_assert!(
                r.is_err(),
                "offset {} is outside the LDTR/STTR imm9 range [-256, 255] \
                 and must be rejected, but the encoder returned {:?}",
                offset, r
            );
        }

        // Property 6 — error contract: arity < 2 or a non-Mem second operand
        // (pre/post-index, register offset, immediate, symbol) is rejected.
        #[test]
        fn prop_error_contract(
            nregs in 0u8..2u8,
            bad_kind in 0u8..4u8,
        ) {
            let bad = match bad_kind {
                0 => Operand::MemPreIndex { base: "x1".to_string(), offset: 0 },
                1 => Operand::MemPostIndex { base: "x1".to_string(), offset: 0 },
                2 => Operand::Imm(7),
                _ => Operand::Symbol("foo".to_string()),
            };
            let ops: Vec<Operand> = if nregs == 0 {
                Vec::new()
            } else {
                vec![gp_xreg(0), bad]
            };
            let r = encode_ldtr_sized(&ops, true, 0b11);
            prop_assert!(r.is_err(), "expected error for ops={:?}, got {:?}", ops, r);
        }
    }

    // Golden cross-check (no inputs): the two base forms + ldtrb must match
    // the hand-derived ARMv8 encodings exactly, including V=0 / [25:24]=00.
    #[test]
    fn golden_ldtr_sttr_encodings() {
        let ops = vec![
            gp_xreg(0),
            Operand::Mem { base: "x1".to_string(), offset: 0 },
        ];
        assert_eq!(word(encode_ldtr_sized(&ops, true, 0b11)), GOLDEN_LDTR_X0_X1_0);
        assert_eq!(word(encode_ldtr_sized(&ops, false, 0b11)), GOLDEN_STTR_X0_X1_0);
        // ldtrb (size=00) — note Rt=Rt regardless of Wn/Xn (get_reg ignores width here).
        let ops_b = vec![
            Operand::Reg("w0".to_string()),
            Operand::Mem { base: "x1".to_string(), offset: 0 },
        ];
        assert_eq!(word(encode_ldtr_sized(&ops_b, true, 0b00)), GOLDEN_LDTRB_W0_X1_0);
    }
}
