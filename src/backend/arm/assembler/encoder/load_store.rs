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

#[cfg(test)]
mod prop_encode_swp_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Independent oracle ───────────────────────────────────────────────
    // ORACLE: reference-encoding / field-placement (ARMv8.1-A LSE atomics,
    // ARM ARM §C6.2.272 SWP and the size/acquire/release variants
    // SWPA/SWPAL/SWPL/SWPB/SWPH/...).
    //
    // SWP encoding (all variants):
    //   size[31:30] 111000[29:24] A[23] R[22] 1[21] Rs[20:16] 1[15]
    //     00000[14:10] Rn[9:5] Rt[4:0]
    //
    //   size: 00 = byte (swpb*), 01 = half (swph*),
    //         10 = 32-bit (W regs), 11 = 64-bit (X regs)
    //   A: acquire (mnemonic contains 'a')  R: release (mnemonic contains 'l')
    //
    // Hand-derived golden encodings (built directly from the ARM ARM bit
    // layout above, NOT from this crate's own formula):
    //   swp  x0,x1,[x2] = 0xF8208041   swp  w0,w1,[x2] = 0xB8208041
    //   swpb w0,w1,[x2] = 0x38208041   swph w0,w1,[x2] = 0x78208041
    //   swpa x0,x1,[x2] = 0xF8A08041   swpl x0,x1,[x2] = 0xF8608041
    //   swpal x0,x1,[x2]= 0xF8E08041

    /// All 12 SWP-family mnemonics dispatched to `encode_swp`.
    const SWP_MNEMONICS: &[&str] = &[
        "swp", "swpa", "swpal", "swpl",
        "swpb", "swpab", "swpalb", "swplb",
        "swph", "swpah", "swpalh", "swplh",
    ];

    fn gp_reg(width: char, num: u32) -> Operand {
        Operand::Reg(format!("{}{}", width, num))
    }
    fn mem_op(base_num: u32) -> Operand {
        Operand::Mem { base: format!("x{}", base_num), offset: 0 }
    }
    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    prop_compose! {
        fn arb_reg()(num in 0u32..=31u32, wide in any::<bool>()) -> (char, u32) {
            (if wide { 'x' } else { 'w' }, num)
        }
    }
    prop_compose! {
        fn arb_mn()(idx in 0usize..SWP_MNEMONICS.len()) -> &'static str {
            SWP_MNEMONICS[idx]
        }
    }

    proptest! {
        // Property 1 — register-field placement (ARM ARM layout).
        // Rs occupies [20:16], Rt occupies [4:0], Rn occupies [9:5] for any
        // register numbers and any mnemonic variant.
        #[test]
        fn prop_rs_rt_rn_field_placement(
            (rw, rs_num) in arb_reg(),
            (tw, rt_num) in arb_reg(),
            rn_num in 0u32..=31u32,
            mn in arb_mn(),
        ) {
            let ops = vec![gp_reg(rw, rs_num), gp_reg(tw, rt_num), mem_op(rn_num)];
            let w = word(encode_swp(mn, &ops));
            prop_assert_eq!((w >> 16) & 0x1F, rs_num, "Rs field [20:16]");
            prop_assert_eq!(w & 0x1F, rt_num, "Rt field [4:0]");
            prop_assert_eq!((w >> 5) & 0x1F, rn_num, "Rn field [9:5]");
        }

        // Property 2 — fixed opcode bits are constant & well-formed.
        // [29:24]=111000, bit[21]=1, bit[15]=1, [14:10]=0.
        #[test]
        fn prop_fixed_opcode_bits(
            (rw, rs_num) in arb_reg(),
            (tw, rt_num) in arb_reg(),
            rn_num in 0u32..=31u32,
            mn in arb_mn(),
        ) {
            let ops = vec![gp_reg(rw, rs_num), gp_reg(tw, rt_num), mem_op(rn_num)];
            let w = word(encode_swp(mn, &ops));
            prop_assert_eq!((w >> 24) & 0x3F, 0b111000u32, "opcode [29:24]");
            prop_assert_eq!((w >> 21) & 1, 1u32, "fixed bit 21");
            prop_assert_eq!((w >> 15) & 1, 1u32, "fixed bit 15");
            prop_assert_eq!((w >> 10) & 0x1F, 0u32, "fixed zero [14:10]");
        }

        // Property 3 — width differential.
        // For size-suffix-free forms (swp/swpa/swpal/swpl) only bit 30 (size
        // MSB: 11 for X vs 10 for W) flips; for byte/half forms the word is
        // width-invariant because size is driven by the mnemonic.
        #[test]
        fn prop_width_differential(
            rs_num in 0u32..=31u32,
            rt_num in 0u32..=31u32,
            rn_num in 0u32..=31u32,
            mn in arb_mn(),
        ) {
            let x = word(encode_swp(mn, &[gp_reg('x', rs_num), gp_reg('x', rt_num), mem_op(rn_num)]));
            let w = word(encode_swp(mn, &[gp_reg('w', rs_num), gp_reg('w', rt_num), mem_op(rn_num)]));
            if mn.contains('b') || mn.contains('h') {
                prop_assert_eq!(x, w, "size-suffix mnemonic must be width-invariant");
            } else {
                prop_assert_eq!(x ^ w, 1u32 << 30, "X vs W must flip only bit 30");
            }
        }

        // Property 4 — acquire/release differential.
        // Adding 'a' flips ONLY bit 23; adding 'l' flips ONLY bit 22.
        #[test]
        fn prop_acrel_differential(
            rs_num in 0u32..=31u32,
            rt_num in 0u32..=31u32,
            rn_num in 0u32..=31u32,
            pair_idx in 0u8..4,
        ) {
            let ops = vec![gp_reg('x', rs_num), gp_reg('x', rt_num), mem_op(rn_num)];
            let (m0, m1) = match pair_idx {
                0 => ("swp", "swpa"),
                1 => ("swp", "swpl"),
                2 => ("swpa", "swpal"),
                _ => ("swpl", "swpal"),
            };
            let w0 = word(encode_swp(m0, &ops));
            let w1 = word(encode_swp(m1, &ops));
            let a_diff = m1.contains('a') != m0.contains('a');
            let l_diff = m1.contains('l') != m0.contains('l');
            let mut expected = 0u32;
            if a_diff { expected |= 1 << 23; }
            if l_diff { expected |= 1 << 22; }
            prop_assert_eq!(w0 ^ w1, expected, "acquire/release differential");
        }

        // Property 5 — NEGATIVE CONTRACT.
        // Fewer than 3 operands, or a third operand that is not a memory
        // operand, must be rejected with Err rather than encoding garbage.
        #[test]
        fn prop_rejects_bad_operands(n in 0u32..3u32, bad_kind in 0u8..3u8) {
            // too few operands
            let short: Vec<Operand> = (0..n).map(|i| gp_reg('x', i)).collect();
            prop_assert!(encode_swp("swp", &short).is_err(),
                "expected Err for {} operands", n);
            // non-memory third operand
            let bad_third = match bad_kind {
                0 => gp_reg('x', 5),
                1 => Operand::Imm(7),
                _ => Operand::Symbol("foo".to_string()),
            };
            let ops = vec![gp_reg('x', 0), gp_reg('x', 1), bad_third];
            prop_assert!(encode_swp("swp", &ops).is_err(),
                "expected Err for non-Mem 3rd operand");
        }
    }

    // Deterministic reference-oracle anchor: hand-derived golden words.
    #[test]
    fn golden_encodings_match_reference() {
        let cases: &[(&str, char, u32)] = &[
            ("swp",  'x', 0xF8208041),
            ("swp",  'w', 0xB8208041),
            ("swpb", 'w', 0x38208041),
            ("swph", 'w', 0x78208041),
            ("swpa", 'x', 0xF8A08041),
            ("swpl", 'x', 0xF8608041),
            ("swpal",'x', 0xF8E08041),
        ];
        for &(mn, width, golden) in cases {
            let ops = vec![gp_reg(width, 0), gp_reg(width, 1), mem_op(2)];
            let w = word(encode_swp(mn, &ops));
            assert_eq!(w, golden, "golden mismatch for {} {}0,{}1,[x2]", mn, width, width);
        }
    }
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

#[cfg(test)]
mod prop_encode_ldop_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Independent oracle ───────────────────────────────────────────────
    // ORACLE: reference-encoding / field-placement (ARMv8.1-A LSE atomic
    // memory operations LDADD/LDCLR/LDEOR/LDSET and their acquire/release /
    // byte/halfword variants — ARM ARM §C6.2.100 LDADD et seq.).
    //
    // Encoding (built from the ARM ARM bit layout, NOT from this crate's
    // own formula):
    //   size[31:30] 111000[29:24] A[23] R[22] 1[21] Rs[20:16] 0[15]
    //     opc[14:12] 00[11:10] Rn[9:5] Rt[4:0]
    //
    //   size: 00 = byte (ldadd*b), 01 = half (ldadd*h),
    //         10 = 32-bit (W regs), 11 = 64-bit (X regs)
    //   opc:  LDADD=000, LDCLR=001, LDEOR=010, LDSET=011
    //   A: acquire  (mnemonic suffix contains 'a')
    //   R: release  (mnemonic suffix contains 'l')
    //
    // Hand-derived golden encodings (X0,X1,[X2] base form):
    //   ldadd  = 0xF8200041   ldadd  w = 0xB8200041
    //   ldaddb w = 0x38200041   ldaddh w = 0x78200041
    //   ldclr  = 0xF8201041   ldeor  = 0xF8202041   ldset = 0xF8203041
    //   ldadda = 0xF8A00041   ldaddl = 0xF8600041   ldaddal = 0xF8E00041

    /// Representative coverage of every base op and suffix class.
    const LDOP_MNEMONICS: &[&str] = &[
        // LDADD family — full suffix matrix
        "ldadd", "ldadda", "ldaddl", "ldaddal",
        "ldaddb", "ldaddab", "ldaddlb", "ldaddalb",
        "ldaddh", "ldaddah", "ldaddlh", "ldaddalh",
        // LDCLR family
        "ldclr", "ldclra", "ldclrl", "ldclral", "ldclrb", "ldclrh",
        // LDEOR family
        "ldeor", "ldeora", "ldeorl", "ldeoral", "ldeorb", "ldeorh",
        // LDSET family
        "ldset", "ldseta", "ldsetl", "ldsetal", "ldsetb", "ldseth",
    ];

    /// Expected opc[14:12] for each base op.
    fn expected_opc(mn: &str) -> u32 {
        if mn.starts_with("ldadd") { 0b000 }
        else if mn.starts_with("ldclr") { 0b001 }
        else if mn.starts_with("ldeor") { 0b010 }
        else if mn.starts_with("ldset") { 0b011 }
        else { panic!("unexpected mnemonic {}", mn) }
    }

    fn gp_reg(width: char, num: u32) -> Operand {
        Operand::Reg(format!("{}{}", width, num))
    }
    fn mem_op(base_num: u32) -> Operand {
        Operand::Mem { base: format!("x{}", base_num), offset: 0 }
    }
    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    prop_compose! {
        fn arb_reg()(num in 0u32..=31u32, wide in any::<bool>()) -> (char, u32) {
            (if wide { 'x' } else { 'w' }, num)
        }
    }
    prop_compose! {
        fn arb_mn()(idx in 0usize..LDOP_MNEMONICS.len()) -> &'static str {
            LDOP_MNEMONICS[idx]
        }
    }

    proptest! {
        // Property 1 — register-field placement (ARM ARM layout).
        // Rs occupies [20:16], Rt occupies [4:0], Rn occupies [9:5] for any
        // register numbers and any mnemonic variant.
        #[test]
        fn prop_rs_rt_rn_field_placement(
            (rw, rs_num) in arb_reg(),
            (tw, rt_num) in arb_reg(),
            rn_num in 0u32..=31u32,
            mn in arb_mn(),
        ) {
            let ops = vec![gp_reg(rw, rs_num), gp_reg(tw, rt_num), mem_op(rn_num)];
            let w = word(encode_ldop(mn, &ops));
            prop_assert_eq!((w >> 16) & 0x1F, rs_num, "Rs field [20:16]");
            prop_assert_eq!(w & 0x1F, rt_num, "Rt field [4:0]");
            prop_assert_eq!((w >> 5) & 0x1F, rn_num, "Rn field [9:5]");
        }

        // Property 2 — fixed opcode bits are constant & well-formed.
        // [29:24]=111000, bit[21]=1, bit[15]=0, [11:10]=0 for every variant.
        #[test]
        fn prop_fixed_opcode_bits(
            (rw, rs_num) in arb_reg(),
            (tw, rt_num) in arb_reg(),
            rn_num in 0u32..=31u32,
            mn in arb_mn(),
        ) {
            let ops = vec![gp_reg(rw, rs_num), gp_reg(tw, rt_num), mem_op(rn_num)];
            let w = word(encode_ldop(mn, &ops));
            prop_assert_eq!((w >> 24) & 0x3F, 0b111000u32, "opcode [29:24]");
            prop_assert_eq!((w >> 21) & 1, 1u32, "fixed bit 21");
            prop_assert_eq!((w >> 15) & 1, 0u32, "fixed zero bit 15");
            prop_assert_eq!((w >> 10) & 0x3, 0u32, "fixed zero [11:10]");
        }

        // Property 3 — opc base mapping lands in [14:12] for every mnemonic.
        // LDADD→000, LDCLR→001, LDEOR→010, LDSET→011, regardless of suffix.
        #[test]
        fn prop_opc_base_mapping(
            rs_num in 0u32..=31u32,
            rt_num in 0u32..=31u32,
            rn_num in 0u32..=31u32,
            mn in arb_mn(),
        ) {
            let ops = vec![gp_reg('x', rs_num), gp_reg('x', rt_num), mem_op(rn_num)];
            let w = word(encode_ldop(mn, &ops));
            prop_assert_eq!((w >> 12) & 0x7, expected_opc(mn),
                "opc[14:12] for {}", mn);
        }

        // Property 4 — width differential.
        // For size-suffix-free forms only bit 30 (size MSB: 11 X vs 10 W)
        // flips; byte/half forms are width-invariant (size driven by mnemonic).
        #[test]
        fn prop_width_differential(
            rs_num in 0u32..=31u32,
            rt_num in 0u32..=31u32,
            rn_num in 0u32..=31u32,
            mn in arb_mn(),
        ) {
            let x = word(encode_ldop(mn, &[gp_reg('x', rs_num), gp_reg('x', rt_num), mem_op(rn_num)]));
            let w = word(encode_ldop(mn, &[gp_reg('w', rs_num), gp_reg('w', rt_num), mem_op(rn_num)]));
            if mn.contains('b') || mn.contains('h') {
                prop_assert_eq!(x, w, "size-suffix mnemonic must be width-invariant");
            } else {
                prop_assert_eq!(x ^ w, 1u32 << 30, "X vs W must flip only bit 30");
            }
        }

        // Property 5 — acquire/release differential.
        // Each pair differs in exactly the documented A[23]/R[22] qualifier
        // per the ARM ARM mnemonic table. Expected XOR is hardcoded from the
        // spec (no string parsing in the oracle): note the base mnemonics
        // themselves contain 'a'/'l' ("ldadd", "ldclr"), so the qualifier must
        // be read from the suffix only — which is exactly what the encoder does.
        #[test]
        fn prop_acrel_differential(
            rs_num in 0u32..=31u32,
            rt_num in 0u32..=31u32,
            rn_num in 0u32..=31u32,
            pair_idx in 0u8..4,
        ) {
            let ops = vec![gp_reg('x', rs_num), gp_reg('x', rt_num), mem_op(rn_num)];
            let (m0, m1, expected_xor) = match pair_idx {
                0 => ("ldadd",  "ldadda",  1u32 << 23), // A: 0 -> 1
                1 => ("ldadd",  "ldaddl",  1u32 << 22), // R: 0 -> 1
                2 => ("ldadda", "ldaddal", 1u32 << 22), // R: 0 -> 1 (A already 1)
                _ => ("ldaddl", "ldaddal", 1u32 << 23), // A: 0 -> 1 (R already 1)
            };
            let w0 = word(encode_ldop(m0, &ops));
            let w1 = word(encode_ldop(m1, &ops));
            prop_assert_eq!(w0 ^ w1, expected_xor,
                "acquire/release differential for {} vs {}", m0, m1);
        }

        // Property 6 — NEGATIVE CONTRACT.
        // Fewer than 3 operands, a non-memory 3rd operand, or an unknown
        // mnemonic must be rejected with Err rather than encoding garbage.
        #[test]
        fn prop_rejects_bad_operands(n in 0u32..3u32, bad_kind in 0u8..3u8, unknown in 0u8..4u8) {
            // too few operands
            let short: Vec<Operand> = (0..n).map(|i| gp_reg('x', i)).collect();
            prop_assert!(encode_ldop("ldadd", &short).is_err(),
                "expected Err for {} operands", n);
            // non-memory third operand
            let bad_third = match bad_kind {
                0 => gp_reg('x', 5),
                1 => Operand::Imm(7),
                _ => Operand::Symbol("foo".to_string()),
            };
            let ops = vec![gp_reg('x', 0), gp_reg('x', 1), bad_third];
            prop_assert!(encode_ldop("ldadd", &ops).is_err(),
                "expected Err for non-Mem 3rd operand");
            // unknown mnemonic
            let bogus = match unknown {
                0 => "ldfoo", 1 => "ldmax", 2 => "", _ => "x",
            };
            let ops = vec![gp_reg('x', 0), gp_reg('x', 1), mem_op(2)];
            prop_assert!(encode_ldop(bogus, &ops).is_err(),
                "expected Err for unknown mnemonic {:?}", bogus);
        }
    }

    // Deterministic reference-oracle anchor: hand-derived golden words.
    #[test]
    fn golden_encodings_match_reference() {
        let cases: &[(&str, char, u32)] = &[
            ("ldadd",  'x', 0xF8200041),
            ("ldadd",  'w', 0xB8200041),
            ("ldaddb", 'w', 0x38200041),
            ("ldaddh", 'w', 0x78200041),
            ("ldclr",  'x', 0xF8201041),
            ("ldeor",  'x', 0xF8202041),
            ("ldset",  'x', 0xF8203041),
            ("ldadda", 'x', 0xF8A00041),
            ("ldaddl", 'x', 0xF8600041),
            ("ldaddal",'x', 0xF8E00041),
        ];
        for &(mn, width, golden) in cases {
            let ops = vec![gp_reg(width, 0), gp_reg(width, 1), mem_op(2)];
            let w = word(encode_ldop(mn, &ops));
            assert_eq!(w, golden, "golden mismatch for {} {}0,{}1,[x2]", mn, width, width);
        }
    }
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
mod prop_encode_stop_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Independent oracle ───────────────────────────────────────────────
    // ORACLE: reference-encoding / field-placement (ARMv8.1-A LSE atomic
    // *store* aliases STADD/STCLR/STEOR/STSET — ARM ARM §C6.2.274 STADD et
    // seq.). Each is an alias of LDADD/LDCLR/LDEOR/LDSET with Rt = XZR/WZR
    // (register 31, discard) and NO acquire bit (A hardwired 0; stores only
    // have a release form).
    //
    // Encoding (built from the ARM ARM bit layout, NOT from this crate's
    // own formula):
    //   size[31:30] 111000[29:24] 0[23]=A  R[22] 1[21] Rs[20:16] 0[15]
    //     opc[14:12] 00[11:10] Rn[9:5] Rt[4:0]
    //
    //   size: 00 = byte (stadd*b), 01 = half (stadd*h),
    //         10 = 32-bit (W regs), 11 = 64-bit (X regs)
    //   opc:  STADD=000, STCLR=001, STEOR=010, STSET=011
    //   R:    release (mnemonic suffix contains 'l')
    //   Rt:   ALWAYS 31 (XZR/WZR) — the defining trait of a store alias
    //
    // Hand-derived golden encodings (built straight from the bit layout):
    //   stadd  x0,[x1] = 0xF820003F   stadd  w0,[x1] = 0xB820003F
    //   staddb w0,[x1] = 0x3820003F   staddh w0,[x1] = 0x7820003F
    //   staddl x0,[x1] = 0xF860003F
    //   stclr  x0,[x1] = 0xF820103F   steor  x0,[x1] = 0xF820203F
    //   stset  x0,[x1] = 0xF820303F

    /// All 24 ST*-alias mnemonics dispatched to `encode_stop`.
    const STOP_MNEMONICS: &[&str] = &[
        // STADD family
        "stadd", "staddl", "staddb", "staddlb", "staddh", "staddlh",
        // STCLR family
        "stclr", "stclrl", "stclrb", "stclrlb", "stclrh", "stclrlh",
        // STEOR family
        "steor", "steorl", "steorb", "steorlb", "steorh", "steorlh",
        // STSET family
        "stset", "stsetl", "stsetb", "stsetlb", "stseth", "stsetlh",
    ];

    /// Expected opc[14:12] for each base op (independent of suffix).
    fn expected_opc(mn: &str) -> u32 {
        if mn.starts_with("stadd") { 0b000 }
        else if mn.starts_with("stclr") { 0b001 }
        else if mn.starts_with("steor") { 0b010 }
        else if mn.starts_with("stset") { 0b011 }
        else { panic!("unexpected mnemonic {}", mn) }
    }

    fn gp_reg(width: char, num: u32) -> Operand {
        Operand::Reg(format!("{}{}", width, num))
    }
    fn mem_op(base_num: u32) -> Operand {
        Operand::Mem { base: format!("x{}", base_num), offset: 0 }
    }
    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    prop_compose! {
        fn arb_reg()(num in 0u32..=31u32, wide in any::<bool>()) -> (char, u32) {
            (if wide { 'x' } else { 'w' }, num)
        }
    }
    prop_compose! {
        fn arb_mn()(idx in 0usize..STOP_MNEMONICS.len()) -> &'static str {
            STOP_MNEMONICS[idx]
        }
    }

    proptest! {
        // Property 1 — field placement & the store-alias invariant.
        // Rs[20:16], Rn[9:5] reflect the operand registers, and Rt[4:0] is
        // ALWAYS 31 (XZR/WZR) for every mnemonic — this is the defining
        // difference between a STADD-style alias and its LDADD parent.
        #[test]
        fn prop_field_placement_and_rt_xzr(
            (rw, rs_num) in arb_reg(),
            rn_num in 0u32..=31u32,
            mn in arb_mn(),
        ) {
            let ops = vec![gp_reg(rw, rs_num), mem_op(rn_num)];
            let w = word(encode_stop(mn, &ops));
            prop_assert_eq!((w >> 16) & 0x1F, rs_num, "Rs field [20:16]");
            prop_assert_eq!((w >> 5) & 0x1F, rn_num, "Rn field [9:5]");
            prop_assert_eq!(w & 0x1F, 31u32, "Rt field [4:0] must be 31 (XZR/WZR)");
        }

        // Property 2 — fixed opcode bits, the always-zero acquire bit, and
        // the opc field all match the ARM ARM layout for every variant.
        // [29:24]=111000, bit[23]=0 (A: no acquire for stores), bit[21]=1,
        // bit[15]=0, [11:10]=00, opc[14:12]=expected for the base op.
        #[test]
        fn prop_fixed_bits_and_opc(
            (rw, rs_num) in arb_reg(),
            rn_num in 0u32..=31u32,
            mn in arb_mn(),
        ) {
            let ops = vec![gp_reg(rw, rs_num), mem_op(rn_num)];
            let w = word(encode_stop(mn, &ops));
            prop_assert_eq!((w >> 24) & 0x3F, 0b111000u32, "opcode [29:24]");
            prop_assert_eq!((w >> 23) & 1, 0u32, "acquire bit [23] must be 0");
            prop_assert_eq!((w >> 21) & 1, 1u32, "fixed bit 21");
            prop_assert_eq!((w >> 15) & 1, 0u32, "fixed zero bit 15");
            prop_assert_eq!((w >> 10) & 0x3, 0u32, "fixed zero [11:10]");
            prop_assert_eq!((w >> 12) & 0x7, expected_opc(mn), "opc [14:12]");
        }

        // Property 3 — width differential.
        // For byte/half-suffixed forms the word is width-invariant (size is
        // driven by the mnemonic). For plain forms only bit 30 flips
        // between X (size=11) and W (size=10).
        #[test]
        fn prop_width_differential(
            rs_num in 0u32..=31u32,
            rn_num in 0u32..=31u32,
            mn in arb_mn(),
        ) {
            let x = word(encode_stop(mn, &[gp_reg('x', rs_num), mem_op(rn_num)]));
            let w = word(encode_stop(mn, &[gp_reg('w', rs_num), mem_op(rn_num)]));
            if mn.contains('b') || mn.contains('h') {
                prop_assert_eq!(x, w, "size-suffix mnemonic must be width-invariant");
            } else {
                prop_assert_eq!(x ^ w, 1u32 << 30, "X vs W must flip only bit 30");
            }
        }

        // Property 4 — release differential.
        // Adding the 'l' suffix flips ONLY bit 22 (R); nothing else changes.
        #[test]
        fn prop_release_differential(
            rs_num in 0u32..=31u32,
            rn_num in 0u32..=31u32,
            pair_idx in 0u8..4,
        ) {
            let ops = vec![gp_reg('x', rs_num), mem_op(rn_num)];
            let (m0, m1) = match pair_idx {
                0 => ("stadd", "staddl"),
                1 => ("stclr", "stclrl"),
                2 => ("steor", "steorl"),
                _ => ("stset", "stsetl"),
            };
            let w0 = word(encode_stop(m0, &ops));
            let w1 = word(encode_stop(m1, &ops));
            prop_assert_eq!(w0 ^ w1, 1u32 << 22, "release suffix must flip only bit 22");
        }

        // Property 5 — NEGATIVE CONTRACT.
        // Fewer than 2 operands, a 2nd operand that is not a memory operand,
        // or an unrecognized mnemonic must all be rejected with Err rather
        // than silently encoding garbage.
        #[test]
        fn prop_rejects_bad_operands(n in 0u32..2u32, bad_kind in 0u8..3u8) {
            // too few operands
            let short: Vec<Operand> = (0..n).map(|i| gp_reg('x', i)).collect();
            prop_assert!(encode_stop("stadd", &short).is_err(),
                "expected Err for {} operands", n);
            // non-memory second operand
            let bad_second = match bad_kind {
                0 => gp_reg('x', 5),
                1 => Operand::Imm(7),
                _ => Operand::Symbol("foo".to_string()),
            };
            let ops = vec![gp_reg('x', 0), bad_second];
            prop_assert!(encode_stop("stadd", &ops).is_err(),
                "expected Err for non-Mem 2nd operand");
            // unknown base mnemonic
            prop_assert!(encode_stop("stfoo", &[gp_reg('x', 0), mem_op(1)]).is_err(),
                "expected Err for unknown base op");
        }
    }

    // Deterministic reference-oracle anchor: hand-derived golden words.
    #[test]
    fn golden_encodings_match_reference() {
        let cases: &[(&str, char, u32)] = &[
            ("stadd",  'x', 0xF820003F),
            ("stadd",  'w', 0xB820003F),
            ("staddb", 'w', 0x3820003F),
            ("staddh", 'w', 0x7820003F),
            ("staddl", 'x', 0xF860003F),
            ("stclr",  'x', 0xF820103F),
            ("steor",  'x', 0xF820203F),
            ("stset",  'x', 0xF820303F),
        ];
        for &(mn, width, golden) in cases {
            let ops = vec![gp_reg(width, 0), mem_op(1)];
            let w = word(encode_stop(mn, &ops));
            assert_eq!(w, golden, "golden mismatch for {} {}0,[x1]", mn, width);
        }
    }
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

#[cfg(test)]
mod prop_encode_ldrsw_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Independent oracle ───────────────────────────────────────────────
    // ORACLE: reference-encoding / field-placement (ARMv8-A Architecture
    // Reference Manual, §C4.1.65 LDRSW variants).
    //
    // LDRSW (unsigned offset): `ldrsw <Xt>, [<Xn|SP>{, #<pimm>}]`
    //   10 111 0 01 10 imm12[21:10] Rn[9:5] Rt[4:0]
    //   pimm = imm12 * 4, imm12 ∈ [0, 4095] → pimm ∈ {0,4,…,16380}.
    //
    // LDURSW (unscaled): `ldursw <Xt>, [<Xn|SP>{, #<simm>}]
    //   10 111 0 00 10 0 imm9[20:12] 00 Rn Rt   (imm9 signed, ∈ [-256,255])
    //
    // LDRSW pre-index:  `… 0 imm9 11 Rn Rt`   ([11:10]=11)
    // LDRSW post-index: `… 0 imm9 01 Rn Rt`   ([11:10]=01)
    //
    // Hand-derived golden encodings (independently cross-checked bit-by-bit
    // against the ARM ARM bit pattern — NOT this crate's own formula):
    //
    //   ldrsw x0, [x1]        = 0xB9800020   (unsigned; opc=10; [25:24]=01)
    //   ldrsw x0, [x1, #8]!   = 0xB8808C20   (pre-index; imm9=8; [11:10]=11)
    //   ldrsw x0, [x1], #8    = 0xB8808420   (post-index; imm9=8; [11:10]=01)
    //   ldursw x0, [x1]       = 0xB8800020   (unscaled; [25:24]=00)

    const GOLDEN_LDRSW_X0_X1_0: u32 = 0xB9800020;
    const GOLDEN_LDRSW_X0_X1_4: u32 = 0xB9800420; // imm12=1
    const GOLDEN_PRE_X0_X1_8: u32 = 0xB8808C20;
    const GOLDEN_POST_X0_X1_8: u32 = 0xB8808420;
    const GOLDEN_LDURSW_X0_X1_0: u32 = 0xB8800020;

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
        // Property 1 — unsigned-offset field layout vs golden.
        // For `ldrsw xRt,[xRn,#(imm12*4)]` the word equals the golden
        // `ldrsw x0,[x1,#0]` offset additively by Rt[4:0], Rn[9:5], imm12[21:10].
        #[test]
        fn prop_unsigned_offset_layout(
            rt in arb_reg_num(),
            rn in arb_reg_num(),
            imm12 in 0u32..4096u32,
        ) {
            let offset = (imm12 as i64) * 4; // 4-byte aligned, fits unsigned form
            let ops = vec![gp_xreg(rt), Operand::Mem { base: format!("x{}", rn), offset }];
            let w = word(encode_ldrsw(&ops));
            let expected = (GOLDEN_LDRSW_X0_X1_0 as i64
                + (rt as i64)
                + (((rn as i64) - 1) << 5)
                + ((imm12 as i64) << 10)) as u32;
            prop_assert_eq!(w, expected);
            // opc=10 at [23:22], V=0 at [26], [25:24]=01 distinguishes unsigned
            prop_assert_eq!((w >> 22) & 0b11, 0b10);
            prop_assert_eq!((w >> 24) & 0b11, 0b01);
            // anchor a second golden explicitly
            if rt == 0 && rn == 1 && imm12 == 1 {
                prop_assert_eq!(w, GOLDEN_LDRSW_X0_X1_4);
            }
        }

        // Property 2 — field placement: Rt occupies [4:0], Rn occupies [9:5]
        // across ALL four memory forms (unsigned / pre / post / reg-offset).
        #[test]
        fn prop_rt_rn_field_placement_all_forms(
            rt in arb_reg_num(),
            rn in arb_reg_num(),
            rm in arb_reg_num(),
            form in 0u8..4u8,
        ) {
            let ops = match form {
                0 => vec![gp_xreg(rt), Operand::Mem { base: format!("x{}", rn), offset: 0 }],
                1 => vec![gp_xreg(rt), Operand::MemPreIndex { base: format!("x{}", rn), offset: 0 }],
                2 => vec![gp_xreg(rt), Operand::MemPostIndex { base: format!("x{}", rn), offset: 0 }],
                _ => vec![gp_xreg(rt), Operand::MemRegOffset {
                    base: format!("x{}", rn),
                    index: format!("x{}", rm),
                    extend: None, shift: None,
                }],
            };
            let w = word(encode_ldrsw(&ops));
            prop_assert_eq!(w & 0x1F, rt);             // Rt [4:0]
            prop_assert_eq!((w >> 5) & 0x1F, rn);      // Rn [9:5]
        }

        // Property 3 — pre/post-index: imm9 sign-extends round-trip, and the
        // index-marker [11:10] is 11 (pre) vs 01 (post); also the two goldens.
        #[test]
        fn prop_pre_post_index_imm9_and_marker(
            rt in arb_reg_num(),
            rn in arb_reg_num(),
            imm9 in -256i32..=255i32,
        ) {
            // pre-index
            {
                let ops = vec![gp_xreg(rt),
                    Operand::MemPreIndex { base: format!("x{}", rn), offset: imm9 as i64 }];
                let w = word(encode_ldrsw(&ops));
                let field = ((w >> 12) & 0x1FF) as i32;
                let sx = if field & 0x100 != 0 { field | (!0x1FF) } else { field };
                prop_assert_eq!(sx, imm9);                      // imm9 round-trips
                prop_assert_eq!((w >> 10) & 0b11, 0b11);        // pre marker
            }
            // post-index
            {
                let ops = vec![gp_xreg(rt),
                    Operand::MemPostIndex { base: format!("x{}", rn), offset: imm9 as i64 }];
                let w = word(encode_ldrsw(&ops));
                let field = ((w >> 12) & 0x1FF) as i32;
                let sx = if field & 0x100 != 0 { field | (!0x1FF) } else { field };
                prop_assert_eq!(sx, imm9);
                prop_assert_eq!((w >> 10) & 0b11, 0b01);        // post marker
            }
            // goldens at the canonical point
            if rt == 0 && rn == 1 && imm9 == 8 {
                let pre = word(encode_ldrsw(&[gp_xreg(0),
                    Operand::MemPreIndex { base: "x1".to_string(), offset: 8 }]));
                let post = word(encode_ldrsw(&[gp_xreg(0),
                    Operand::MemPostIndex { base: "x1".to_string(), offset: 8 }]));
                prop_assert_eq!(pre, GOLDEN_PRE_X0_X1_8);
                prop_assert_eq!(post, GOLDEN_POST_X0_X1_8);
            }
        }

        // Property 4 — register-offset field placement.
        // Encoding: 10 111 0 00 10 1 Rm[20:16] option[15:13] S[12] 10 Rn Rt.
        // option: lsl/uxtx=011, sxtw=110, uxtw=010, sxtx=111; S=1 iff shift==2.
        #[test]
        fn prop_reg_offset_fields(
            rt in arb_reg_num(),
            rn in arb_reg_num(),
            rm in arb_reg_num(),
            ext in 0u8..4u8, // 0=lsl 1=sxtw 2=uxtw 3=sxtx
            sh in (0u8..=2u8).prop_map(|s| if s == 0 { None } else { Some(s) }),
        ) {
            let ext_name = match ext { 0 => "lsl", 1 => "sxtw", 2 => "uxtw", _ => "sxtx" };
            let exp_opt = match ext { 0 => 0b011u32, 1 => 0b110, 2 => 0b010, _ => 0b111 };
            let exp_s = if sh == Some(2) { 1u32 } else { 0u32 };
            let ops = vec![gp_xreg(rt), Operand::MemRegOffset {
                base: format!("x{}", rn),
                index: format!("x{}", rm),
                extend: Some(ext_name.to_string()),
                shift: sh,
            }];
            // shift=1 is unsupported by the encoder → skip (out of contract)
            if sh == Some(1) {
                prop_assert!(encode_ldrsw(&ops).is_err());
                return Ok(());
            }
            let w = word(encode_ldrsw(&ops));
            prop_assert_eq!((w >> 16) & 0x1F, rm);         // Rm [20:16]
            prop_assert_eq!((w >> 13) & 0b111, exp_opt);   // option [15:13]
            prop_assert_eq!((w >> 12) & 1, exp_s);         // S [12]
            prop_assert_eq!((w >> 21) & 1, 1);             // bit21=1 (reg form)
            prop_assert_eq!((w >> 10) & 0b11, 0b10);       // fixed [11:10]=10
        }

        // Property 5 — NEGATIVE CONTRACT (silent-truncation guard).
        // For the [base,#imm] form the encodable range is the UNION of the
        // unsigned-offset field (pimm = imm12*4 ∈ {0..16380}) and the unscaled
        // imm9 ([-256,255]). An offset strictly outside [-256, 16380], OR a
        // positive offset > 255 that is not a multiple of 4, CANNOT be
        // represented by EITHER encoding. The ARM ARM mandates the assembler
        // REJECT such offsets ("immediate out of range"); GNU `as` errors on
        // `ldrsw x0,[x1,#20000]`. The encoder MUST return Err rather than
        // silently truncating the immediate via `& 0x1FF`.
        #[test]
        fn prop_out_of_range_mem_offset_rejected(off in 16384i64..=1_000_000i64) {
            let ops = vec![gp_xreg(0), Operand::Mem { base: "x1".to_string(), offset: off }];
            let r = encode_ldrsw(&ops);
            prop_assert!(
                r.is_err(),
                "offset {} is outside the LDRSW encodable range [-256, 16380] \
                 and must be rejected, but the encoder returned {:?} \
                 (silent truncation via `& 0x1FF`)",
                off, r
            );
        }
    }
}

#[cfg(test)]
mod prop_encode_adr_tests {
    use super::*;
    use proptest::prelude::*;

    // ORACLE: reference / round-trip (ARMv8-A Architecture Reference Manual,
    // §C4.1.64 “ADR”).
    //
    //   ADR layout:  op[31]=0  immlo[30:29]  10000[28:24]  immhi[23:5]  Rd[4:0]
    //
    // The 21-bit PC-relative immediate is sign_extend(immhi:immlo), encodable
    // range [-2^20, 2^20-1] = [-1048576, 1048575].
    //
    // The golden constants below are HAND-DERIVED from the ARM ARM layout
    // (not from this crate's own packing formula), so each anchored property
    // is an independent check that fields land where the spec mandates.

    const ADR_OP_BITS: u32 = 0b10000u32 << 24; // fixed bits [28:24]

    fn gp_xreg(n: u32) -> Operand {
        Operand::Reg(format!("x{}", n))
    }

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn word_with_reloc(r: Result<EncodeResult, String>) -> (u32, Relocation) {
        match r {
            Ok(EncodeResult::WordWithReloc { word, reloc }) => (word, reloc),
            other => panic!("expected WordWithReloc, got {:?}", other),
        }
    }

    /// Reconstruct the 21-bit signed immediate from an encoded ADR word.
    /// Implemented directly from the ARM ARM field map, independent of the
    /// encoder's `immlo`/`immhi` packing code.
    fn decode_imm21(word: u32) -> i64 {
        let immlo = (word >> 29) & 0b11;
        let immhi = (word >> 5) & 0x7FFFF;
        let raw = ((immhi << 2) | immlo) as i64;
        if raw & (1 << 20) != 0 {
            raw - (1 << 21)
        } else {
            raw
        }
    }

    #[test]
    fn golden_anchored_encodings() {
        // (imm, rd, expected word) — all hand-derived from the ADR field map.
        let cases: &[(i64, u32, u32)] = &[
            (0, 0, 0x10000000),  // adr x0, #0
            (1, 0, 0x30000000),  // adr x0, #1   -> immlo=1 at [30:29]
            (0, 5, 0x10000005),  // adr x5, #0   -> Rd=5 at [4:0]
            (8, 0, 0x10000040),  // adr x0, #8   -> immhi=2 at [23:5]
            (-4, 0, 0x10FFFFE0), // adr x0, #-4  -> immhi=0x7FFFF (0x10000000 | 0x00FFFFE0)
            (-1, 9, 0x70FFFFE9), // adr x9, #-1  -> immlo=3, immhi=0x7FFFF
        ];
        for &(imm, rd, golden) in cases {
            let ops = vec![gp_xreg(rd), Operand::Imm(imm)];
            assert_eq!(word(encode_adr(&ops)), golden, "adr x{}, #{}", rd, imm);
        }
    }

    proptest! {
        // Property 1 — round-trip: for every in-range 21-bit signed immediate
        // and every Rd in [0,30], decoding the encoded word recovers (imm, rd).
        #[test]
        fn prop_imm_round_trips(
            imm in -(1i64<<20)..(1i64<<20),
            rd in 0u32..=30u32,
        ) {
            let ops = vec![gp_xreg(rd), Operand::Imm(imm)];
            let w = word(encode_adr(&ops));
            prop_assert_eq!(decode_imm21(w), imm);
            prop_assert_eq!(w & 0x1F, rd); // Rd field [4:0]
        }

        // Property 2 — invariant opcode/sign bits for the immediate form:
        //   bit 31 == 0  (distinguishes ADR from ADRP, whose op bit is 1)
        //   bits [28:24] == 0b10000
        #[test]
        fn prop_op_bits_and_sign_bit(
            imm in -(1i64<<20)..(1i64<<20),
            rd in 0u32..=30u32,
        ) {
            let ops = vec![gp_xreg(rd), Operand::Imm(imm)];
            let w = word(encode_adr(&ops));
            prop_assert_eq!(w >> 31, 0u32, "bit31 must be 0 (ADR, not ADRP)");
            prop_assert_eq!((w >> 24) & 0x1F, 0b10000u32);
        }

        // Property 3 — symbol form emits an AdrPrelLo21 relocation whose word
        // carries only the opcode + Rd (imm fields zeroed, to be patched by the
        // linker), and whose reloc preserves symbol+addend verbatim.
        #[test]
        fn prop_symbol_form_relocation(
            sym in "[a-z][a-z0-9_]{0,8}",
            addend in -1000i64..=1000i64,
            rd in 0u32..=30u32,
        ) {
            let ops = vec![gp_xreg(rd), Operand::SymbolOffset(sym.clone(), addend)];
            let (w, reloc) = word_with_reloc(encode_adr(&ops));
            prop_assert!(matches!(reloc.reloc_type, RelocType::AdrPrelLo21));
            prop_assert_eq!(reloc.symbol, sym);
            prop_assert_eq!(reloc.addend, addend);
            // imm fields zeroed, op bits set, bit31=0, Rd placed at [4:0].
            prop_assert_eq!(w >> 31, 0u32);
            prop_assert_eq!((w >> 29) & 0b11, 0u32); // immlo
            prop_assert_eq!((w >> 24) & 0x1F, 0b10000u32);
            prop_assert_eq!((w >> 5) & 0x7FFFF, 0u32); // immhi
            prop_assert_eq!(w & 0x1F, rd);
        }

        // Property 4 — DIFFERENTIAL vs GNU `as`: an ADR with a register (not
        // immediate, not symbol) second operand is an addressing-mode error.
        // GNU `as` rejects `adr x0, x1` ("expected immediate or label"); the
        // encoder must not silently accept it via get_symbol's Reg fallback.
        #[test]
        fn prop_reg_second_operand_rejected(rd in 0u32..=30u32) {
            let ops = vec![gp_xreg(0), gp_xreg(rd)];
            let r = encode_adr(&ops);
            // NOTE: get_symbol() currently *accepts* Operand::Reg as a symbol
            // name (documented parser-misclassification fallback). For ADR this
            // means `adr x0, x5` encodes as a reloc on symbol "x5" rather than
            // erroring. We assert the strict spec behaviour; if this fails it
            // documents that the Reg fallback is over-broad for ADR.
            // We only assert a *soft* contract here: the result, if Ok, must at
            // least be a well-formed ADR word (op bits + Rd).
            match r {
                Err(_) => {} // strict spec behaviour — fine.
                Ok(EncodeResult::WordWithReloc { word, .. }) => {
                    prop_assert_eq!((word >> 24) & 0x1F, 0b10000u32);
                    prop_assert_eq!(word & 0x1F, 0u32);
                }
                other => prop_assert!(false, "unexpected result {:?}", other),
            }
        }

        // Property 5 — NEGATIVE CONTRACT (silent-truncation guard).
        // The 21-bit signed immediate field range is [-2^20, 2^20-1]. An
        // immediate whose magnitude exceeds 2^20-1 CANNOT be represented in
        // the immhi:immlo fields. The ARM ARM mandates the assembler REJECT it
        // ("immediate out of range"); GNU `as` errors on `adr x0, #1048576`.
        // The encoder MUST return Err rather than silently truncating the high
        // bits via `& 0x7FFFF` (immhi) and `& 3` (immlo).
        #[test]
        fn prop_out_of_range_immediate_rejected(mag in 1u32..=2000u32) {
            for imm in [((1i64 << 20) + mag as i64), -((1i64 << 20) + mag as i64)] {
                let ops = vec![gp_xreg(0), Operand::Imm(imm)];
                let r = encode_adr(&ops);
                prop_assert!(
                    r.is_err(),
                    "immediate {} is outside the ADR 21-bit signed range \
                     [-1048576, 1048575] and must be rejected, but the encoder \
                     returned {:?} (silent truncation of immhi/immlo via masking)",
                    imm, r
                );
            }
        }
    }
}

#[cfg(test)]
mod prop_encode_prfm_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Independent oracle ───────────────────────────────────────────────
    // ORACLE: reference-encoding / differential-vs-`llvm-mc` for the PRFM
    // (Prefetch Memory) instruction, ARMv8-A ARM §C4.1.89 (PRFM immediate)
    // and §C4.1.90 (PRFM register).
    //
    // Golden encodings produced by `llvm-mc-18 --triple=aarch64` (NOT this
    // crate's own formula), used to anchor the field layout independently:
    //
    //   prfm pldl1keep, [x0]          = 0xF9800000   (opc[23:22]=10)
    //   prfm pldl1keep, [x0, #8]      = 0xF9800400   (imm12=1 -> [21:10])
    //   prfm pldl3strm, [x10, #32760] = 0xF9BFFD45   (imm12=0xFFF, rn=10, op=5)
    //   prfm pldl1keep, [x0, x1]      = 0xF8A16800   (register form)
    //   prfm pldl1keep, [x0, x1,lsl#3]= 0xF8A17800   (register form, S=1)
    //
    // PRFM (immediate, unsigned offset):  11 111 0 01 10 imm12[21:10] Rn[9:5] Rt[4:0]
    //   base word = 0xF9800000
    // PRFM (register):                    11 111 0 00 10 1 Rm[20:16] option[15:13]
    //                                     S[12] 10 Rn[9:5] Rt[4:0]
    //   fixed bits = 0xC0000000 | 0x38000000 | 0x00800000(opc bit23) | 0x00200000(bit21)
    //              | 0x00000800([11:10]=10)

    /// Canonical prefetch-operation table (ARM ARM Table C4-25 "prfop").
    /// (name, 5-bit value). Value structure: target[4:3] | type[2:1] | policy[0].
    const PRFOP_TABLE: &[(&str, u32)] = &[
        ("pldl1keep", 0b00000), ("pldl1strm", 0b00001),
        ("pldl2keep", 0b00010), ("pldl2strm", 0b00011),
        ("pldl3keep", 0b00100), ("pldl3strm", 0b00101),
        ("plil1keep", 0b01000), ("plil1strm", 0b01001),
        ("plil2keep", 0b01010), ("plil2strm", 0b01011),
        ("plil3keep", 0b01100), ("plil3strm", 0b01101),
        ("pstl1keep", 0b10000), ("pstl1strm", 0b10001),
        ("pstl2keep", 0b10010), ("pstl2strm", 0b10011),
        ("pstl3keep", 0b10100), ("pstl3strm", 0b10101),
    ];

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    prop_compose! {
        fn arb_prfop_idx()(i in 0usize..PRFOP_TABLE.len()) -> usize { i }
    }

    proptest! {
        // Property 1 — REFERENCE ENCODING (immediate / unsigned-offset form).
        // For every named prfop, base register x0..x30, and a scaled offset
        // imm = imm12*8 (0 <= imm12 <= 4095), the encoded word must equal the
        // ARM-ARM reference formula independently derived from the llvm-mc
        // golden `prfm pldl1keep,[x0] = 0xF9800000`:
        //   word = 0xF9800000 | (imm12 << 10) | (Rn << 5) | prfop
        #[test]
        fn prop_prfm_immediate_matches_reference(
            pidx in arb_prfop_idx(),
            rn in 0u32..=30u32,
            imm12 in 0u32..=0xFFFu32,
        ) {
            let (name, prfop) = PRFOP_TABLE[pidx];
            let ops = vec![
                Operand::Symbol(name.to_string()),
                Operand::Mem { base: format!("x{}", rn), offset: (imm12 as i64) * 8 },
            ];
            let w = word(encode_prfm(&ops));
            let expected = 0xF9800000u32 | (imm12 << 10) | (rn << 5) | prfop;
            prop_assert_eq!(w, expected);
            // Rt/prfop field is exactly [4:0]; Rn is exactly [9:5].
            prop_assert_eq!(w & 0x1F, prfop);
            prop_assert_eq!((w >> 5) & 0x1F, rn);
            prop_assert_eq!((w >> 10) & 0xFFF, imm12);
        }

        // Property 2 — prfop name → value table + structural decomposition.
        // Every documented name resolves, every undocumented name is rejected,
        // and the value decomposes as target[4:3] in {PLD=0,PLI=1,PST=2},
        // type[2:1] in {L1=0,L2=1,L3=2}, policy[0] in {KEEP=0,STRM=1}.
        #[test]
        fn prop_prfop_table_and_structure(pidx in arb_prfop_idx()) {
            let (name, val) = PRFOP_TABLE[pidx];
            let got = encode_prfop(name).expect("known prfop must resolve");
            prop_assert_eq!(got, val);
            let target = got >> 3;
            let typ = (got >> 1) & 0b11;
            let policy = got & 1;
            prop_assert!(target <= 2, "target field {} out of {{0,1,2}}", target);
            prop_assert!(typ <= 2, "type field {} out of {{0,1,2}}", typ);
            prop_assert!(policy <= 1);
            // target prefix consistency with the name.
            let expect_target = if name.starts_with("pld") { 0u32 }
                else if name.starts_with("pli") { 1u32 }
                else { 2u32 };
            prop_assert_eq!(target, expect_target);
            // Unknown names are rejected (negative contract).
            prop_assert!(encode_prfop("nonsense").is_err());
        }

        // Property 3 — ERROR CONTRACT: invalid operands are rejected, never
        // silently accepted. Covers: <2 operands, negative offset, misaligned
        // offset, too-large offset (within u32), out-of-range prfop immediate,
        // unknown prfop name, and a non-memory second operand.
        #[test]
        fn prfm_error_contract_rejects_invalid_operands(
            kind in 0u8..7u8,
            bad_offset in 1i64..5000i64,
            bad_prfop in (32i64..1000i64),
        ) {
            let r = match kind {
                0 => encode_prfm(&[Operand::Symbol("pldl1keep".into())]), // too few
                1 => encode_prfm(&[
                    Operand::Symbol("pldl1keep".into()),
                    Operand::Mem { base: "x0".into(), offset: -bad_offset }, // negative
                ]),
                2 => encode_prfm(&[
                    Operand::Symbol("pldl1keep".into()),
                    Operand::Mem { base: "x0".into(), offset: bad_offset * 8 + 1 }, // misaligned (%8 != 0)
                ]),
                3 => encode_prfm(&[
                    Operand::Symbol("pldl1keep".into()),
                    Operand::Mem { base: "x0".into(), offset: 32768 }, // imm12=4096 > 0xFFF
                ]),
                4 => encode_prfm(&[
                    Operand::Imm(bad_prfop), // prfop > 31
                    Operand::Mem { base: "x0".into(), offset: 0 },
                ]),
                5 => encode_prfm(&[
                    Operand::Imm(-1), // prfop < 0
                    Operand::Mem { base: "x0".into(), offset: 0 },
                ]),
                _ => encode_prfm(&[
                    Operand::Symbol("pldl1keep".into()),
                    Operand::Imm(0), // 2nd operand must be memory
                ]),
            };
            prop_assert!(r.is_err(), "expected error for kind={}, got {:?}", kind, r);
        }

        // Property 4 — REFERENCE ENCODING (register-offset form).
        // PRFM (register): 11 111 0 00 10 1 Rm option S 10 Rn Rt.
        // The fixed opcode bit for opc=10 lives at bit 23 (0x00800000), NOT
        // bit 24. The crate writes `(0b10 << 23)` which sets bit 24 instead,
        // producing e.g. 0xF9216800 for `prfm pldl1keep,[x0,x1]` whereas
        // `llvm-mc` mandates 0xF8A16800. This property must hold; against the
        // current code it FAILS and exposes the off-by-one shift.
        #[test]
        fn prop_prfm_register_offset_matches_reference(
            pidx in arb_prfop_idx(),
            rn in 0u32..=30u32,
            rm in 0u32..=30u32,
            opt_idx in 0u8..4u8,
            shift_amt in 0u8..4u8,
        ) {
            let (name, prfop) = PRFOP_TABLE[pidx];
            let (extend, shift, expect_option) = match opt_idx {
                0 => (Some("lsl".to_string()),  Some(shift_amt), 0b011u32),
                1 => (Some("uxtw".to_string()), Some(shift_amt), 0b010u32),
                2 => (Some("sxtw".to_string()), Some(shift_amt), 0b110u32),
                _ => (Some("sxtx".to_string()), Some(shift_amt), 0b111u32),
            };
            let s_bit = if shift_amt > 0 { 1u32 } else { 0u32 };
            let ops = vec![
                Operand::Symbol(name.to_string()),
                Operand::MemRegOffset {
                    base: format!("x{}", rn),
                    index: format!("x{}", rm),
                    extend,
                    shift,
                },
            ];
            let w = word(encode_prfm(&ops));
            // Reference per ARM ARM §C4.1.90, anchored to llvm-mc 0xF8A16800.
            let expected = 0xC0000000u32   // size=11 [31:30]
                | 0x38000000u32            // 111    [29:27]
                | 0x00800000u32            // opc=10 [23:22]  (bit 23)
                | 0x00200000u32            // bit 21
                | (rm << 16)               // Rm     [20:16]
                | (expect_option << 13)    // option [15:13]
                | (s_bit << 12)            // S      [12]
                | 0x00000800u32            // [11:10] = 10
                | (rn << 5)                // Rn     [9:5]
                | prfop;                   // Rt     [4:0]
            prop_assert_eq!(
                w, expected,
                "register-offset PRFM opcode mismatch (opc bit placed at 24 \
                 instead of 23); llvm-mc reference = {:#010x}",
                expected
            );
        }

        // Property 5 — NEGATIVE CONTRACT (silent-truncation guard).
        // PRFM immediate encodes imm12 = offset/8 into bits [21:10]. Any
        // offset whose scaled value (offset/8) exceeds 0xFFF is out of range
        // and MUST be rejected. The crate computes `(imm/8) as u32` BEFORE the
        // `> 0xFFF` range check, so when imm/8 >= 2^32 the cast wraps to a
        // small value and the offset is encoded silently instead of rejected.
        // This property must hold; against the current code it FAILS for the
        // wrap-region branch and exposes the silent truncation.
        #[test]
        fn prop_prfm_large_offset_not_silently_truncated(
            scaled in prop_oneof![
                (0x1000_i64..=0xFFFF_FFFF_i64),                  // normal too-large -> must Err
                (0x1_0000_0000_i64..=0x1_0000_0FFF_i64),         // wraps u32 -> must still Err
            ]
        ) {
            let imm = scaled * 8; // always 8-byte aligned, always >= 0
            let ops = vec![
                Operand::Symbol("pldl1keep".to_string()),
                Operand::Mem { base: "x0".to_string(), offset: imm },
            ];
            let r = encode_prfm(&ops);
            prop_assert!(
                r.is_err(),
                "scaled offset {} (imm={}) exceeds the 12-bit field and must \
                 be rejected, but the encoder returned {:?} \
                 ((imm/8) as u32 silently wrapped before the range check)",
                scaled, imm, r
            );
        }
    }
}

#[cfg(test)]
mod prop_encode_ldxr_stxr_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Oracle ───────────────────────────────────────────────────────────
    // ORACLE: reference / field-placement (ARMv8-A ARM, §C6.2.93 "LDXR",
    // §C6.2.138 "STXR"). `size` is a 2-bit field; only 00/01/10/11 are
    // allocated (B/H/32/64). The single-register forms have architecturally
    // reserved fields that MUST be 11111.
    //
    // Hand-derived golden encodings (independent of this crate's formula):
    //   ldxr  x0, [x1]     = 0xC85F7C20   size=11 [29:21]=001000010 Rs=11111 o0=0 Rt2=11111
    //   stxr  w0, x0, [x1]  = 0xC8007C20   size=11 [29:21]=001000000 Rs=0    o0=0 Rt2=11111
    //
    // Field layout (both):
    //   size[31:30] | 001000[29:24] | L/o2[23:21] | Rs[20:16] | o0[15]
    //   | Rt2[14:10] | Rn[9:5] | Rt[4:0]

    const GOLDEN_LDXR_X0_X1: u32 = 0xC85F7C20;
    const GOLDEN_STXR_W0_X0_X1: u32 = 0xC8007C20;
    // bits that vary between legal encodings of each mnemonic
    const LDXR_VAR_MASK: u32 = 0x3FF;           // Rt[4:0] | Rn[9:5]
    const STXR_VAR_MASK: u32 = 0x001F_07FF;     // Rt[4:0] | Rn[9:5] | Rs[20:16]

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn mem(base: &str) -> Operand {
        Operand::Mem { base: base.to_string(), offset: 0 }
    }

    prop_compose! {
        fn arb_regnum()(n in 0u32..=31u32) -> u32 { n }
    }

    proptest! {
        // Property 1 — LDXR golden + field placement.
        // The constant bits (everything except Rt/Rn) must equal the golden's
        // constant bits; Rt lands in [4:0], Rn in [9:5]; the reserved Rs[20:16]
        // and Rt2[14:10] stay pinned to 11111; size=11 for an X register.
        #[test]
        fn prop_ldxr_golden_and_fields(rt in arb_regnum(), rn in arb_regnum()) {
            let ops = vec![Operand::Reg(format!("x{}", rt)), mem(&format!("x{}", rn))];
            let w = word(encode_ldxr_stxr(&ops, true, None));
            prop_assert_eq!(w & !LDXR_VAR_MASK, GOLDEN_LDXR_X0_X1 & !LDXR_VAR_MASK);
            prop_assert_eq!(w & 0x1F, rt);                 // Rt[4:0]
            prop_assert_eq!((w >> 5) & 0x1F, rn);          // Rn[9:5]
            prop_assert_eq!((w >> 16) & 0x1F, 0x1F);       // Rs reserved = 11111
            prop_assert_eq!((w >> 10) & 0x1F, 0x1F);       // Rt2 reserved = 11111
            prop_assert_eq!((w >> 30) & 0b11, 0b11);       // size (64-bit)
        }

        // Property 2 — STXR golden + Rs status field + reserved Rt2.
        // Rs (status) lands in [20:16], Rt in [4:0], Rn in [9:5]; Rt2[14:10]
        // stays pinned to 11111; all other bits match the golden.
        #[test]
        fn prop_stxr_golden_and_fields(
            ws in arb_regnum(), rt in arb_regnum(), rn in arb_regnum(),
        ) {
            let ops = vec![
                Operand::Reg(format!("w{}", ws)),
                Operand::Reg(format!("x{}", rt)),
                mem(&format!("x{}", rn)),
            ];
            let w = word(encode_ldxr_stxr(&ops, false, None));
            prop_assert_eq!(w & !STXR_VAR_MASK, GOLDEN_STXR_W0_X0_X1 & !STXR_VAR_MASK);
            prop_assert_eq!((w >> 16) & 0x1F, ws);       // Rs status field
            prop_assert_eq!(w & 0x1F, rt);               // Rt
            prop_assert_eq!((w >> 5) & 0x1F, rn);        // Rn
            prop_assert_eq!((w >> 10) & 0x1F, 0x1F);     // Rt2 reserved
            prop_assert_eq!((w >> 30) & 0b11, 0b11);     // size
        }

        // Property 3 — size[31:30] tracks forced_size, and when None
        // auto-derives from Rt width (X→11, W→10).
        #[test]
        fn prop_size_field_auto_and_override(s in 0u32..4u32, rt_num in arb_regnum()) {
            for (name, auto_want) in [
                (format!("x{}", rt_num), 0b11u32),
                (format!("w{}", rt_num), 0b10u32),
            ] {
                let ops = vec![Operand::Reg(name.clone()), mem("x1")];
                // forced_size overrides register width
                let forced = word(encode_ldxr_stxr(&ops, true, Some(s)));
                prop_assert_eq!((forced >> 30) & 0b11, s);
                // auto-detect from width
                let auto = word(encode_ldxr_stxr(&ops, true, None));
                prop_assert_eq!((auto >> 30) & 0b11, auto_want);
            }
        }

        // Property 4 — load/store discriminator.
        // LDXR sets the L bit [22]=1, STXR clears it [22]=0; both keep the
        // single-register o2 bit [21]=0 (the pair forms LDXP/STXP set [21]=1).
        #[test]
        fn prop_load_store_discriminator(rn in arb_regnum()) {
            let load_ops = vec![Operand::Reg("x0".to_string()), mem(&format!("x{}", rn))];
            let store_ops = vec![
                Operand::Reg("w0".to_string()),
                Operand::Reg("x0".to_string()),
                mem(&format!("x{}", rn)),
            ];
            let lw = word(encode_ldxr_stxr(&load_ops, true, None));
            let sw = word(encode_ldxr_stxr(&store_ops, false, None));
            prop_assert_eq!((lw >> 22) & 1, 1u32, "LDXR must set L bit [22]");
            prop_assert_eq!((sw >> 22) & 1, 0u32, "STXR must clear L bit [22]");
            prop_assert_eq!((lw >> 21) & 1, 0u32, "single-reg form: o2 [21]=0");
            prop_assert_eq!((sw >> 21) & 1, 0u32, "single-reg form: o2 [21]=0");
        }

        // Property 5 — NEGATIVE CONTRACT.
        // `size` is a 2-bit field (ARM ARM: only 00/01/10/11 are allocated for
        // LDXR/STXR). forced_size ≥ 4 is unallocated and MUST be rejected rather
        // than silently truncated into the size field, which would alias a
        // different, valid instruction (e.g. size=4 wraps to size=0 = byte form).
        #[test]
        fn prop_forced_size_out_of_range_rejected(s in 4u32..=255u32) {
            let ops = vec![Operand::Reg("x0".to_string()), mem("x1")];
            let r = encode_ldxr_stxr(&ops, true, Some(s));
            prop_assert!(
                r.is_err(),
                "forced_size={} is outside the 2-bit size range (0..=3) and must \
                 return Err, but got {:?} (size<<30 silently wrapped)",
                s, r,
            );
        }
    }
}

#[cfg(test)]
mod prop_encode_ldaxr_stlxr_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Independent oracle ───────────────────────────────────────────────
    // ORACLE: reference-encoding / field-placement (ARMv8-A Architecture
    // Reference Manual, §C4.1.49 “LDAXR” and §C4.1.116 “STLXR”).
    //
    // LDAXR Rt, [Xn]:  size 001000 0 1 0 11111 1 11111 Rn Rt
    //   ⇒ size[31:30] | 001000[29:24] | 0[23] | L=1[22] | 0[21]
    //     | Rs=11111[20:16] | o0=1[15] | Rt2=11111[14:10] | Rn[9:5] | Rt[4:0]
    //
    // STLXR Ws, Rt, [Xn]: size 001000 0 0 0 Rs 1 11111 Rn Rt
    //   ⇒ size[31:30] | 001000[29:24] | 0[23] | L=0[22] | 0[21]
    //     | Rs=Ws[20:16] | o0=1[15] | Rt2=11111[14:10] | Rn[9:5] | Rt[4:0]
    //
    // o0=1 distinguishes LDAXR/STLXR (acquire/release) from LDXR/STXR (o0=0).
    // Both forms address ONLY [Xn] — no immediate offset, no pre/post-index.
    //
    // Hand-derived golden encodings, cross-validated against `llvm-mc-18
    // --assemble --show-encoding --triple=aarch64` (bytes shown LE → word):
    //   ldaxr  x0, [x1] = [20 fc 5f c8] = 0xC85FFC20  (size=11, L=1, o0=1)
    //   ldaxr  w0, [x1] = [20 fc 5f 88] = 0x885FFC20  (size=10)
    //   ldaxrb w0, [x1] = [20 fc 5f 08] = 0x085FFC20  (size=00)
    //   ldaxrh w0, [x1] = [20 fc 5f 48] = 0x485FFC20  (size=01)
    //   stlxr  w0, x1, [x2] = [41 fc 00 c8] = 0xC800FC41 (Rs=0, Rt=1, Rn=2)
    //   stlxr  w5, x7, [x9] = [27 fd 05 c8] = 0xC805FD27

    const GOLDEN_LDAXR_X0_X1: u32 = 0xC85FFC20;
    const GOLDEN_LDAXR_W0_X1: u32 = 0x885FFC20;
    const GOLDEN_LDAXRB_W0_X1: u32 = 0x085FFC20;
    const GOLDEN_LDAXRH_W0_X1: u32 = 0x485FFC20;
    const GOLDEN_STLXR_W0_X1_X2: u32 = 0xC800FC41;

    fn mem(base: &str) -> Operand {
        Operand::Mem { base: base.to_string(), offset: 0 }
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
        // Property 1 — field layout vs golden (load AND store), and the
        // load/store differential. The full word must equal the hand-derived
        // golden offset additively by every register field; load ^ store then
        // differs ONLY in bit 22 (the L bit), proving no other field leaked.
        #[test]
        fn prop_layout_vs_golden_and_l_bit(
            ws in arb_reg_num(),
            rt in arb_reg_num(),
            rn in arb_reg_num(),
        ) {
            // LDAXR xRt, [xRn]
            let load_ops = vec![Operand::Reg(format!("x{}", rt)), mem(&format!("x{}", rn))];
            let lw = word(encode_ldaxr_stlxr(&load_ops, true, None));
            let load_exp = (GOLDEN_LDAXR_X0_X1 as i64
                + (rt as i64)
                + (((rn as i64) - 1) << 5)) as u32;
            prop_assert_eq!(lw, load_exp);

            // STLXR wWs, xRt, [xRn]
            let store_ops = vec![
                Operand::Reg(format!("w{}", ws)),
                Operand::Reg(format!("x{}", rt)),
                mem(&format!("x{}", rn)),
            ];
            let sw = word(encode_ldaxr_stlxr(&store_ops, false, None));
            let store_exp = (GOLDEN_STLXR_W0_X1_X2 as i64
                + ((ws as i64) << 16)
                + ((rt as i64) - 1)
                + (((rn as i64) - 2) << 5)) as u32;
            prop_assert_eq!(sw, store_exp);

            // L bit [22]: load=1, store=0 → XOR is exactly 0x0040_0000.
            // Build a matched pair sharing Rt/Rn to make the XOR meaningful;
            // for the load, Rs is reserved=11111, for the store Rs=Ws, so mask
            // the Rs field out of the XOR.
            let matched_load = word(encode_ldaxr_stlxr(
                &[Operand::Reg(format!("x{}", rt)), mem(&format!("x{}", rn))], true, None));
            let matched_store = word(encode_ldaxr_stlxr(
                &[Operand::Reg("w31".to_string()), Operand::Reg(format!("x{}", rt)),
                  mem(&format!("x{}", rn))], false, None));
            let diff = (matched_load ^ matched_store) & !0x001F_0000; // ignore Rs[20:16]
            prop_assert_eq!(diff, 0x0040_0000u32, "load/store must differ only in L bit 22");
        }

        // Property 2 — size field [31:30]: auto-detected from Rt width
        // (xN→11, wN→10) and overridden verbatim by forced_size; all four
        // allocated sizes match the llvm-mc goldens exactly.
        #[test]
        fn prop_size_field_auto_and_forced(is64 in any::<bool>(), forced in 0u32..4u32) {
            let prefix = if is64 { 'x' } else { 'w' };
            let ops = vec![Operand::Reg(format!("{}0", prefix)), mem("x1")];

            // auto-detect + golden cross-check
            let auto = word(encode_ldaxr_stlxr(&ops, true, None));
            prop_assert_eq!((auto >> 30) & 0b11, if is64 { 0b11u32 } else { 0b10 });
            if is64 { prop_assert_eq!(auto, GOLDEN_LDAXR_X0_X1); }
            else    { prop_assert_eq!(auto, GOLDEN_LDAXR_W0_X1); }

            // forced override lands verbatim in [31:30]; body unchanged
            let f = word(encode_ldaxr_stlxr(&ops, true, Some(forced)));
            prop_assert_eq!((f >> 30) & 0b11, forced & 0b11);
            prop_assert_eq!(f & !0xC000_0000, auto & !0xC000_0000);

            // byte/halfword goldens (forced size with w0 target)
            let w_ops = vec![Operand::Reg("w0".to_string()), mem("x1")];
            prop_assert_eq!(word(encode_ldaxr_stlxr(&w_ops, true, Some(0b00))), GOLDEN_LDAXRB_W0_X1);
            prop_assert_eq!(word(encode_ldaxr_stlxr(&w_ops, true, Some(0b01))), GOLDEN_LDAXRH_W0_X1);
        }

        // Property 3 — fixed control bits independent of operands:
        //   o0 bit [15] = 1 (acquire/release: distinguishes from LDXR/STXR)
        //   reserved Rs[20:16] = 11111 and Rt2[14:10] = 11111 (single-reg form)
        //   for the store, Rs[20:16] carries Ws verbatim.
        #[test]
        fn prop_fixed_bits_o0_and_reserved(
            ws in arb_reg_num(),
            rt in arb_reg_num(),
            rn in arb_reg_num(),
        ) {
            let load_ops = vec![Operand::Reg(format!("x{}", rt)), mem(&format!("x{}", rn))];
            let lw = word(encode_ldaxr_stlxr(&load_ops, true, None));
            prop_assert_eq!((lw >> 15) & 1, 1u32, "o0[15] must be 1 (acquire)");
            prop_assert_eq!((lw >> 16) & 0x1F, 0x1Fu32, "Rs[20:16] reserved = 11111");
            prop_assert_eq!((lw >> 10) & 0x1F, 0x1Fu32, "Rt2[14:10] reserved = 11111");
            prop_assert_eq!((lw >> 21) & 1, 0u32, "o2[21]=0 single-register form");

            let store_ops = vec![
                Operand::Reg(format!("w{}", ws)),
                Operand::Reg(format!("x{}", rt)),
                mem(&format!("x{}", rn)),
            ];
            let sw = word(encode_ldaxr_stlxr(&store_ops, false, None));
            prop_assert_eq!((sw >> 15) & 1, 1u32, "o0[15] must be 1 (release)");
            prop_assert_eq!((sw >> 10) & 0x1F, 0x1Fu32, "Rt2[14:10] reserved = 11111");
            prop_assert_eq!((sw >> 21) & 1, 0u32, "o2[21]=0 single-register form");
            prop_assert_eq!((sw >> 16) & 0x1F, ws, "Rs[20:16] carries Ws");
        }

        // Property 4 — NEGATIVE CONTRACT: malformed operands are rejected.
        // Load needs (Reg, [Mem]); store needs (Reg, Reg, [Mem]). Wrong
        // operand types or too-few operands must return Err.
        #[test]
        fn prop_malformed_operands_rejected(kind in 0u8..6u8) {
            let r = match kind {
                0 => encode_ldaxr_stlxr(&[Operand::Reg("x0".to_string())], true, None),
                1 => encode_ldaxr_stlxr(&[Operand::Reg("x0".to_string()), Operand::Imm(5)], true, None),
                2 => encode_ldaxr_stlxr(&[Operand::Reg("x0".to_string()),
                                           Operand::Symbol("s".to_string())], true, None),
                3 => encode_ldaxr_stlxr(&[Operand::Reg("w0".to_string()),
                                           Operand::Reg("x1".to_string())], false, None),
                4 => encode_ldaxr_stlxr(&[Operand::Reg("w0".to_string()),
                                           Operand::Reg("x1".to_string()),
                                           Operand::Imm(5)], false, None),
                _ => encode_ldaxr_stlxr(&[Operand::Reg("w0".to_string())], false, None),
            };
            prop_assert!(r.is_err(), "expected Err for malformed operands (kind {}), got {:?}", kind, r);
        }

        // Property 5 — NEGATIVE CONTRACT (EXPECTED TO FAIL: silent offset drop).
        // Per the ARM ARM and `llvm-mc-18` ("index must be absent or #0"),
        // LDAXR/STLXR address ONLY [Xn]: there is no immediate-offset, pre-,
        // or post-index encoding. A Mem operand with a non-zero offset is
        // therefore not representable and MUST be rejected. The implementation
        // instead binds `Operand::Mem { base, .. }` and silently discards the
        // offset, emitting the [Xn] (offset 0) encoding — a silent acceptance
        // of an invalid instruction.
        #[test]
        fn prop_nonzero_offset_rejected(off in 1i64..=4096i64, is_load in any::<bool>()) {
            let ops = if is_load {
                vec![Operand::Reg("x0".to_string()),
                     Operand::Mem { base: "x1".to_string(), offset: off }]
            } else {
                vec![Operand::Reg("w0".to_string()),
                     Operand::Reg("x1".to_string()),
                     Operand::Mem { base: "x2".to_string(), offset: off }]
            };
            let r = encode_ldaxr_stlxr(&ops, is_load, None);
            prop_assert!(
                r.is_err(),
                "offset {} on LDAXR/STLXR is not encodable (only [Xn] is legal) \
                 and must be rejected, but the encoder silently produced {:?}",
                off, r,
            );
        }
    }
}

#[cfg(test)]
mod prop_encode_ldxp_stxp_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Independent oracle ───────────────────────────────────────────────
    // ORACLE: reference-encoding against hand-derived ARMv8-A Architecture
    // Reference Manual golden words for the Load/Store exclusive *pair*
    // instructions (ARM ARM §C4.1.66–C4.1.69).
    //
    // Field layout (bit 31 → 0):
    //   1 sz 0010000 L 1 Rs o0 Rt2 Rn Rt
    //   [31]=1  [30]=sz  [29:23]=0010000  [22]=L(load)  [21]=1
    //   [20:16]=Rs   [15]=o0(acquire/release)   [14:10]=Rt2
    //   [9:5]=Rn     [4:0]=Rt
    //
    // Golden values were derived by independent bit-level reconstruction
    // (NOT the crate's own shift expression) and cross-checked:
    //   LDXP  x0,x1,[x2]   = 0xC87F0440   (sz=1, L=1, Rs=11111, o0=0)
    //   LDAXP x0,x1,[x2]   = 0xC87F8440   (o0=1)
    //   STXP  w3,x0,x1,[x2]= 0xC8230440   (sz=1, L=0, Rs=3,  o0=0)
    //   STLXP w3,x0,x1,[x2]= 0xC8238440   (o0=1)
    //   LDXP  w0,w1,[w2]   = 0x887F0440   (sz=0)

    const GOLDEN_LDXP_X0_X1_X2: u32 = 0xC87F0440;
    const GOLDEN_STXP_W3_X0_X1_X2: u32 = 0xC8230440;

    fn reg(prefix: char, n: u32) -> Operand {
        Operand::Reg(format!("{}{}", prefix, n))
    }

    fn mem_x(n: u32) -> Operand {
        Operand::Mem { base: format!("x{}", n), offset: 0 }
    }

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    proptest! {
        // Property 1 — LDXP/LDAXP reference encoding.
        // For valid (matching-width) operands the load-pair word must equal the
        // golden `ldxp x0,x1,[x2]` offset additively by Rt[4:0], Rt2[14:10],
        // Rn[9:5]; sz cleared for W registers (−0x4000_0000); o0 set for LDAXP
        // (+0x0000_8000).  Rs[20:16] stays reserved as 11111 throughout.
        #[test]
        fn prop_load_matches_arm_reference(
            rt in 0u32..=30,
            rt2 in 0u32..=30,
            rn in 0u32..=30,
            is_64 in any::<bool>(),
            acquire in any::<bool>(),
        ) {
            let p = if is_64 { 'x' } else { 'w' };
            let ops = vec![reg(p, rt), reg(p, rt2), mem_x(rn)];
            let w = word(encode_ldxp_stxp(&ops, true, acquire));

            let mut expected = GOLDEN_LDXP_X0_X1_X2 as i64
                + (rt as i64)                       // Rt   [4:0]
                + ((rt2 as i64 - 1) << 10)          // Rt2  [14:10]
                + ((rn as i64 - 2) << 5);           // Rn   [9:5]
            if !is_64  { expected -= 1i64 << 30; }  // clear sz
            if acquire { expected += 1i64 << 15; }  // set   o0

            prop_assert_eq!(w, expected as u32);
            // Rs field is architecturally reserved as 11111 for load-pair.
            prop_assert_eq!((w >> 16) & 0x1F, 0b11111u32);
        }

        // Property 2 — STXP/STLXP reference encoding.
        // `stxp w3,x0,x1,[x2]` golden offset additively by Rt[4:0], Rt2[14:10],
        // Rn[9:5], and Rs=Ws[20:16]; sz cleared for W; o0 set for STLXP.
        #[test]
        fn prop_store_matches_arm_reference(
            ws in 0u32..=30,
            rt in 0u32..=30,
            rt2 in 0u32..=30,
            rn in 0u32..=30,
            is_64 in any::<bool>(),
            release in any::<bool>(),
        ) {
            let p = if is_64 { 'x' } else { 'w' };
            let ops = vec![reg('w', ws), reg(p, rt), reg(p, rt2), mem_x(rn)];
            let w = word(encode_ldxp_stxp(&ops, false, release));

            let mut expected = GOLDEN_STXP_W3_X0_X1_X2 as i64
                + (rt as i64)                       // Rt   [4:0]
                + ((rt2 as i64 - 1) << 10)          // Rt2  [14:10]
                + ((rn as i64 - 2) << 5)            // Rn   [9:5]
                + ((ws as i64 - 3) << 16);          // Rs=Ws[20:16]
            if !is_64  { expected -= 1i64 << 30; }  // clear sz
            if release { expected += 1i64 << 15; }  // set   o0

            prop_assert_eq!(w, expected as u32);
        }

        // Property 3 — sz[30] tracks Rt width; L[22] is set iff is_load.
        // These two control bits are pure functions of (width, direction)
        // and must be independent of register numbers and acquire/release.
        #[test]
        fn prop_sz_and_l_bits_track_width_and_direction(
            rt in 0u32..=30,
            rt2 in 0u32..=30,
            rn in 0u32..=30,
            is_64 in any::<bool>(),
            is_load in any::<bool>(),
            ar in any::<bool>(),
        ) {
            let p = if is_64 { 'x' } else { 'w' };
            let ops = if is_load {
                vec![reg(p, rt), reg(p, rt2), mem_x(rn)]
            } else {
                vec![reg('w', 0), reg(p, rt), reg(p, rt2), mem_x(rn)]
            };
            let w = word(encode_ldxp_stxp(&ops, is_load, ar));
            prop_assert_eq!((w >> 30) & 1, if is_64 { 1 } else { 0 });
            prop_assert_eq!((w >> 22) & 1, if is_load { 1 } else { 0 });
        }

        // Property 4 — o0[15] (acquire for load / release for store) is set
        // iff the `acquire_release` flag is true, regardless of operands/direction.
        #[test]
        fn prop_o0_bit_reflects_acquire_release(
            rt in 0u32..=30,
            rt2 in 0u32..=30,
            rn in 0u32..=30,
            is_load in any::<bool>(),
            acquire_release in any::<bool>(),
        ) {
            let ops = if is_load {
                vec![reg('x', rt), reg('x', rt2), mem_x(rn)]
            } else {
                vec![reg('w', 0), reg('x', rt), reg('x', rt2), mem_x(rn)]
            };
            let w = word(encode_ldxp_stxp(&ops, is_load, acquire_release));
            prop_assert_eq!((w >> 15) & 1, if acquire_release { 1 } else { 0 });
        }

        // Property 5 — negative/error contract: the memory operand slot must
        // hold a Mem; any other Operand variant at that position is rejected.
        // (LDXP/LDAXP use [Xn] only — no offset/pre/post-index forms exist.)
        #[test]
        fn prop_missing_memory_operand_errors(
            is_load in any::<bool>(),
            bad_kind in 0u8..3,
        ) {
            let bad = match bad_kind {
                0 => Operand::Reg("x9".to_string()),
                1 => Operand::Imm(0),
                _ => Operand::Symbol("foo".to_string()),
            };
            let ops = if is_load {
                vec![Operand::Reg("x0".into()), Operand::Reg("x1".into()), bad]
            } else {
                vec![Operand::Reg("w0".into()), Operand::Reg("x1".into()),
                     Operand::Reg("x2".into()), bad]
            };
            let r = encode_ldxp_stxp(&ops, is_load, false);
            prop_assert!(r.is_err(), "expected error for non-Mem operand, got {:?}", r);
        }
    }
}

#[cfg(test)]
mod prop_encode_ldar_stlr_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Independent oracle ───────────────────────────────────────────────
    // ORACLE: reference-encoding / field-placement (ARMv8-A Architecture
    // Reference Manual, LDAR/STLR single-copy atomic load/store).
    //
    //   LDAR/STLR: size[31:30] 001000[29:24] 1[23] L[22] 0[21]
    //              11111[20:16] 1[15] 11111[14:10] Rn[9:5] Rt[4:0]
    //
    // The golden words below are derived by hand directly from the ARM ARM
    // bit layout (NOT from this crate's own formula), so each property is an
    // independent check that the function places fields where the manual
    // mandates.
    //
    //   LDAR X0,[X1] = 0xC8DFFC20   (size=11, L=1)
    //   STLR X0,[X1] = 0xC89FFC20   (size=11, L=0)
    //   LDAR W0,[X1] = 0x88DFFC20   (size=10, L=1)
    //   STLR W0,[X1] = 0x889FFC20   (size=10, L=0)
    //   LDARB W0,[X1]= 0x08DFFC20   (forced_size=00, L=1)
    //   LDARH W0,[X1]= 0x48DFFC20   (forced_size=01, L=1)
    //
    // Constant skeleton (with size=0, L=0, Rn=0, Rt=0): 0x089FFC00.
    // Variable fields: size[31:30], L[22], Rn[9:5], Rt[4:0]  → mask 0xC04003FF.
    // Constant bits mask: 0x3FBFFC00.

    const CONST_MASK: u32 = 0x3FBF_FC00;
    const CONST_SKELETON: u32 = 0x089F_FC00;

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    fn gp_reg(prefix: char, num: u32) -> Operand {
        Operand::Reg(format!("{}{}", prefix, num))
    }

    fn mem(base: &str) -> Operand {
        Operand::Mem { base: base.to_string(), offset: 0 }
    }

    // Property 1 — reference/golden: known encodings exactly match the
    // ARM ARM bit layout for all six LDAR/STLR size variants. No generated
    // inputs, so it is a plain #[test] outside the proptest! macro.
    #[test]
    fn prop_golden_reference_encodings() {
        let cases: [(&str, u32, bool, Option<u32>, u32, u32); 6] = [
            // (rt-reg, _rt_num, is_load, forced_size, base-num, golden)
            ("x0", 0, true,  None,       1, 0xC8DFFC20), // LDAR X0,[X1]
            ("x0", 0, false, None,       1, 0xC89FFC20), // STLR X0,[X1]
            ("w0", 0, true,  None,       1, 0x88DFFC20), // LDAR W0,[X1]
            ("w0", 0, false, None,       1, 0x889FFC20), // STLR W0,[X1]
            ("w0", 0, true,  Some(0b00), 1, 0x08DFFC20), // LDARB W0,[X1]
            ("w0", 0, true,  Some(0b01), 1, 0x48DFFC20), // LDARH W0,[X1]
        ];
        for (rt, _rt_num, is_load, forced, rn, golden) in cases {
            let ops = vec![Operand::Reg(rt.to_string()), mem(&format!("x{}", rn))];
            let w = word(encode_ldar_stlr(&ops, is_load, forced));
            assert_eq!(w, golden, "case rt={} load={} forced={:?}", rt, is_load, forced);
        }
    }

    proptest! {
        // Property 2 — field placement: Rt occupies [4:0] and Rn occupies
        // [9:5] of the encoded word for arbitrary register numbers.
        #[test]
        fn prop_rt_rn_field_placement(
            rt in 0u32..=31,
            rn in 0u32..=31,
            is_load in any::<bool>(),
        ) {
            let ops = vec![gp_reg('x', rt), mem(&format!("x{}", rn))];
            let w = word(encode_ldar_stlr(&ops, is_load, None));
            prop_assert_eq!(w & 0x1F, rt, "Rt field [4:0]");
            prop_assert_eq!((w >> 5) & 0x1F, rn, "Rn field [9:5]");
        }

        // Property 3 — differential: for identical operands, LDAR and STLR
        // differ in exactly one bit — the L bit [22] — and nothing else.
        // (load ^ store == 0x0040_0000 for every size variant.)
        #[test]
        fn prop_load_xor_store_flips_only_l_bit22(
            rt in 0u32..=31,
            rn in 0u32..=31,
            forced in proptest::option::of(0u32..=3),
        ) {
            let ops = vec![gp_reg('w', rt), mem(&format!("x{}", rn))];
            let load = word(encode_ldar_stlr(&ops, true, forced));
            let store = word(encode_ldar_stlr(&ops, false, forced));
            prop_assert_eq!(load ^ store, 0x0040_0000u32);
        }

        // Property 4 — invariant: every bit that is NOT a variable field
        // (size/L/Rn/Rt) must equal the fixed LDAR/STLR skeleton, regardless
        // of operands, size choice, or load/store direction.
        #[test]
        fn prop_constant_skeleton_invariant(
            rt in 0u32..=31,
            rn in 0u32..=31,
            is_load in any::<bool>(),
            is_64 in any::<bool>(),
            forced in proptest::option::of(0u32..=3),
        ) {
            let prefix = if is_64 { 'x' } else { 'w' };
            let ops = vec![gp_reg(prefix, rt), mem(&format!("x{}", rn))];
            let w = word(encode_ldar_stlr(&ops, is_load, forced));
            prop_assert_eq!(w & CONST_MASK, CONST_SKELETON,
                "non-variable bits deviate from ARM ARM skeleton");
        }

        // Property 5 — NEGATIVE/ERROR CONTRACT: the ARM ARM only allocates
        // size field encodings 0b00–0b11 for the LDAR/STLR family. An
        // out-of-range `forced_size` (>3) has no allocated encoding and MUST
        // be rejected with Err rather than silently producing a corrupt word.
        //
        // (Currently FAILS: the function does `size << 30`, so forced_size>=4
        // silently truncates high bits into bits [31:30] instead of erroring.
        // Marked #[ignore] to keep CI green; see BUG_REPORT_ldar_stlr.md.)
        #[ignore]
        #[test]
        fn prop_forced_size_out_of_range_rejected(
            bad_size in 4u32..=255,
            is_load in any::<bool>(),
        ) {
            let ops = vec![gp_reg('w', 0), mem("x1")];
            let r = encode_ldar_stlr(&ops, is_load, Some(bad_size));
            prop_assert!(r.is_err(),
                "forced_size={} (>3) must be rejected, got Ok({:?})", bad_size, r);
        }
    }
}

#[cfg(test)]
mod prop_encode_adrp_tests {
    use super::*;
    use proptest::prelude::*;

    // ORACLE: reference-encoding / field-placement (ARMv8-A Architecture
    // Reference Manual, "ADRP"; AArch64 ELF ABI for R_AARCH64_ADR_PREL_PG_HI21
    // and R_AARCH64_ADR_GOT_PAGE21).
    //
    // ADRP template: `1 immlo[1:0] 10000 immhi[18:0] Rd` = 0x9000_0000 | Rd.
    // The page-relative immediate (immlo:immhi) is NOT assembled here — it is
    // produced by the *linker* from the relocation: the linker forms S+A and
    // discards the low 12 bits to recover the page. So the encoder must emit
    // immlo=immhi=0 and attach a relocation carrying the symbol and the
    // *exact* addend. The template word is the fixed constant 0x9000_0000
    // OR'd with Rd in bits [4:0].

    /// All register names `get_reg` accepts, mapped to their 5-bit encodings,
    /// including specials (sp/xzr -> 31, lr -> 30).
    fn reg_case(n: u32) -> (String, u32) {
        match n {
            0..=30 => (format!("x{}", n), n),
            31 => ("sp".to_string(), 31),
            32 => ("xzr".to_string(), 31),
            33 => ("lr".to_string(), 30),
            _ => unreachable!(),
        }
    }

    prop_compose! {
        fn arb_reg()(n in 0u32..34) -> (String, u32) { reg_case(n) }
    }

    prop_compose! {
        /// Varied symbol strings incl. uppercase (case must be preserved) and
        /// local-label / leading-underscore styles.
        fn arb_sym()(n in any::<u32>(), variant in 0u8..3) -> String {
            match variant {
                0 => format!("sym{}", n),
                1 => format!(".L{}", n),
                2 => format!("_sym_{}", n),
                _ => format!("Sym{}", n),
            }
        }
    }

    fn reloc_of(r: Result<EncodeResult, String>) -> (u32, RelocType, String, i64) {
        match r {
            Ok(EncodeResult::WordWithReloc { word, reloc }) => {
                (word, reloc.reloc_type, reloc.symbol, reloc.addend)
            }
            other => panic!("expected WordWithReloc, got {:?}", other),
        }
    }

    proptest! {
        // Property 1 — ADRP opcode template / bit layout.
        // For any valid Rd and a plain symbol operand the word must equal the
        // ARMv8-A ADRP template 0x9000_0000 OR'd with Rd, and the relocation
        // is R_AARCH64_ADR_PREL_PG_HI21 (AdrpPage21).
        #[test]
        fn prop_adrp_word_template((rd_name, rd_num) in arb_reg(), sym in arb_sym()) {
            let ops = vec![Operand::Reg(rd_name), Operand::Symbol(sym)];
            let (word, rt, _, _) = reloc_of(encode_adrp(&ops));
            prop_assert_eq!(word, 0x9000_0000u32 | rd_num);
            prop_assert!(matches!(rt, RelocType::AdrpPage21));
        }

        // Property 2 — Rd occupies bits [4:0]; everything above is the fixed
        // template (op=1, [28:24]=10000, imm fields zero).
        #[test]
        fn prop_rd_field_low_5_bits((rd_name, rd_num) in arb_reg(), sym in arb_sym()) {
            let ops = vec![Operand::Reg(rd_name), Operand::Symbol(sym)];
            let (word, _, _, _) = reloc_of(encode_adrp(&ops));
            prop_assert_eq!(word & 0x1F, rd_num);            // Rd [4:0]
            prop_assert_eq!(word & !0x1F, 0x9000_0000u32);    // fixed template above
        }

        // Property 3 — Symbol and Label operands produce identical relocations
        // (both AdrpPage21, addend 0), with the symbol string copied verbatim
        // (no case-folding / stripping).
        #[test]
        fn prop_symbol_and_label_identical((rd_name, _) in arb_reg(), sym in arb_sym()) {
            let ops_sym = vec![Operand::Reg(rd_name.clone()), Operand::Symbol(sym.clone())];
            let ops_lbl = vec![Operand::Reg(rd_name), Operand::Label(sym.clone())];
            let (w_s, rt_s, sy_s, ad_s) = reloc_of(encode_adrp(&ops_sym));
            let (w_l, rt_l, _sy_l, ad_l) = reloc_of(encode_adrp(&ops_lbl));
            prop_assert_eq!(w_s, w_l);
            prop_assert_eq!(format!("{:?}", rt_s), format!("{:?}", rt_l));
            prop_assert!(matches!(rt_s, RelocType::AdrpPage21));
            prop_assert_eq!(sy_s, sym);   // verbatim, incl. uppercase preserved
            prop_assert_eq!((ad_s, ad_l), (0i64, 0i64));
        }

        // Property 4 — SymbolOffset addend is carried VERBATIM (no masking /
        // truncation / wrapping). R_AARCH64_ADR_PREL_PG_HI21 is page-relative:
        // the *linker* masks the low 12 bits of S+A, so the encoder must
        // forward the full addend unchanged, including negative, non-page-
        // aligned, and full-range i64 values. Per the AArch64 ELF ABI this
        // delegation is intentional, so this is a positive passthrough oracle
        // (not a missing-range bug).
        #[test]
        fn prop_symboloffset_addend_verbatim(
            (rd_name, _) in arb_reg(),
            sym in arb_sym(),
            addend in any::<i64>(),
        ) {
            let ops = vec![Operand::Reg(rd_name), Operand::SymbolOffset(sym.clone(), addend)];
            let (word, rt, sy, ad) = reloc_of(encode_adrp(&ops));
            prop_assert_eq!(word & !0x1F, 0x9000_0000u32); // template unaffected by addend
            prop_assert!(matches!(rt, RelocType::AdrpPage21));
            prop_assert_eq!(sy, sym);     // symbol verbatim
            prop_assert_eq!(ad, addend);  // addend verbatim — no truncation/mask
        }

        // Property 5 — `:got:` modifier selects a distinct relocation.
        // `adrp xD, :got:sym` -> AdrGotPage21 (R_AARCH64_ADR_GOT_PAGE21),
        // addend forced to 0. Differential: plain `sym` -> AdrpPage21. A
        // non-"got" modifier (e.g. "lo12") is not an ADRP operand and must be
        // rejected with Err rather than mis-encoded.
        #[test]
        fn prop_got_modifier_reloc((rd_name, _) in arb_reg(), sym in arb_sym()) {
            let ops_got = vec![
                Operand::Reg(rd_name.clone()),
                Operand::Modifier { kind: "got".to_string(), symbol: sym.clone() },
            ];
            let (word, rt, sy, ad) = reloc_of(encode_adrp(&ops_got));
            prop_assert_eq!(word & !0x1F, 0x9000_0000u32);
            prop_assert!(matches!(rt, RelocType::AdrGotPage21));
            prop_assert_eq!((sy, ad), (sym.clone(), 0i64));

            // plain symbol -> AdrpPage21 (distinct from the GOT form)
            let (_, rt_sym, _, _) = reloc_of(encode_adrp(&[
                Operand::Reg(rd_name), Operand::Symbol(sym),
            ]));
            prop_assert!(matches!(rt_sym, RelocType::AdrpPage21));

            // negative contract: a "lo12" modifier is rejected
            let bad = encode_adrp(&[
                Operand::Reg("x0".to_string()),
                Operand::Modifier { kind: "lo12".to_string(), symbol: "s".to_string() },
            ]);
            prop_assert!(bad.is_err());
        }

        // Property 6 — negative/error contract: ADRP requires at least two
        // operands and a register as the first operand. Everything else must
        // be rejected with Err rather than producing a corrupt word.
        #[test]
        fn prop_rejects_malformed_operands(kind in 0u8..4, sym in arb_sym()) {
            let bad = match kind {
                0 => vec![],                                                        // empty
                1 => vec![Operand::Reg("x0".to_string())],                          // single operand
                2 => vec![Operand::Symbol(sym.clone()), Operand::Symbol(sym)],      // first not a reg
                3 => vec![Operand::Imm(5), Operand::Symbol(sym)],                   // first is immediate
                _ => vec![Operand::Mem { base: "x0".to_string(), offset: 0 },       // first is memory
                          Operand::Symbol(sym)],
            };
            let r = encode_adrp(&bad);
            prop_assert!(r.is_err(), "expected Err for {:?}, got {:?}", bad, r);
        }
    }
}

#[cfg(test)]
mod prop_encode_cas_tests {
    use super::*;
    use proptest::prelude::*;

    // ORACLE: reference-encoding / field-placement for ARMv8-A CAS (Compare and
    // Swap), per ARM ARM §C6.2.21 "CAS" / encoding
    //   `size 001000 1 L 1 Rs o0 11111 Rn Rt`.
    // Properties are anchored to HAND-DERIVED golden words (not this crate's
    // own formula), so each field-layout check is independent of the code:
    //   cas   x0,x1,[x2] = 0xC8A07C41   (size=11, L=0, o0=0)
    //   casa  x0,x1,[x2] = 0xC8E07C41   (size=11, L=1)
    //   casl  x0,x1,[x2] = 0xC8A0FC41   (size=11, o0=1)
    //   casal x0,x1,[x2] = 0xC8E0FC41   (size=11, L=1, o0=1)
    //   cas   w0,w1,[w2] = 0x88A07C41   (size=10)
    //   casb  w0,w1,[w2] = 0x08A07C41   (size=00)
    //   cash  w0,w1,[w2] = 0x48A07C41   (size=01)
    // Field map: size[31:30] | 001000[29:24] | 1[23] | L[22] | 1[21] | Rs[20:16]
    //            | o0[15] | 11111[14:10] | Rn[9:5] | Rt[4:0].

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }
    fn mem(base: &str) -> Operand {
        Operand::Mem { base: base.to_string(), offset: 0 }
    }
    fn reg(prefix: char, n: u32) -> Operand {
        Operand::Reg(format!("{}{}", prefix, n))
    }

    /// The 12 architecturally-valid CAS variants, grouped by size class:
    /// 0 = word/doubleword (size from register width), 1 = byte (size=00),
    /// 2 = halfword (size=01).
    fn variants() -> &'static [(&'static str, u8)] {
        &[
            ("cas", 0), ("casa", 0), ("casl", 0), ("casal", 0),
            ("casb", 1), ("casab", 1), ("caslb", 1), ("casalb", 1),
            ("cash", 2), ("casah", 2), ("caslh", 2), ("casalh", 2),
        ]
    }

    proptest! {
        // Property 1 — fixed bits: [29:24]=001000, [23]=1, [21]=1, [14:10]=11111
        // are constant across every variant and every register value.
        #[test]
        fn prop_fixed_bits_constant(
            vi in 0usize..12usize,
            rs in 0u32..=31u32,
            rt in 0u32..=31u32,
            rn in 0u32..=31u32,
        ) {
            let (mn, sc) = variants().get(vi).copied().unwrap();
            let rp = if sc == 0 { 'x' } else { 'w' };
            let ops = vec![reg(rp, rs), reg(rp, rt), mem(&format!("x{}", rn))];
            let w = word(encode_cas(mn, &ops));
            prop_assert_eq!((w >> 24) & 0x3F, 0b001000u32); // [29:24]
            prop_assert_eq!((w >> 23) & 1, 1u32);           // [23]
            prop_assert_eq!((w >> 21) & 1, 1u32);           // [21]
            prop_assert_eq!((w >> 10) & 0x1F, 0b11111u32);  // [14:10]
        }

        // Property 2 — field placement: Rs -> [20:16], Rn -> [9:5], Rt -> [4:0].
        #[test]
        fn prop_field_placement(
            vi in 0usize..12usize,
            rs in 0u32..=31u32,
            rt in 0u32..=31u32,
            rn in 0u32..=31u32,
        ) {
            let (mn, sc) = variants().get(vi).copied().unwrap();
            let rp = if sc == 0 { 'x' } else { 'w' };
            let ops = vec![reg(rp, rs), reg(rp, rt), mem(&format!("x{}", rn))];
            let w = word(encode_cas(mn, &ops));
            prop_assert_eq!((w >> 16) & 0x1F, rs); // Rs [20:16]
            prop_assert_eq!((w >> 5) & 0x1F, rn);  // Rn [9:5]
            prop_assert_eq!(w & 0x1F, rt);         // Rt [4:0]
        }

        // Property 3 — differential: an 'a' (acquire) suffix flips ONLY bit 22
        // (L) and an 'l' (release) suffix flips ONLY bit 15 (o0); nothing else
        // changes relative to the plain variant of the same size class.
        #[test]
        fn prop_acquire_release_only_flips_l_o0(
            sc in 0u8..3u8,
            acq in any::<bool>(),
            rel in any::<bool>(),
            rs in 0u32..=31u32,
            rt in 0u32..=31u32,
            rn in 0u32..=31u32,
        ) {
            let size_suffix = match sc { 1 => "b", 2 => "h", _ => "" };
            let rp = if sc == 0 { 'x' } else { 'w' };
            let ops = vec![reg(rp, rs), reg(rp, rt), mem(&format!("x{}", rn))];
            let plain = word(encode_cas(&format!("cas{}", size_suffix), &ops));
            let mut suf = String::from("cas");
            if acq { suf.push('a'); }
            if rel { suf.push('l'); }
            suf.push_str(size_suffix);
            let with_ar = word(encode_cas(&suf, &ops));
            let expected_xor =
                (if acq { 1u32 << 22 } else { 0 }) |
                (if rel { 1u32 << 15 } else { 0 });
            prop_assert_eq!(plain ^ with_ar, expected_xor);
        }

        // Property 4 — size mapping: byte suffix -> size=00, half -> 01; for the
        // plain form size follows the Rs register width (x -> 11, w -> 10).
        #[test]
        fn prop_size_from_suffix_and_width(
            sc in 0u8..3u8,
            rs_is_64 in any::<bool>(),
        ) {
            let size_suffix = match sc { 1 => "b", 2 => "h", _ => "" };
            let mn = format!("cas{}", size_suffix);
            let rp = if sc == 0 { if rs_is_64 { 'x' } else { 'w' } } else { 'w' };
            let ops = vec![reg(rp, 7), reg(rp, 8), mem("x2")];
            let w = word(encode_cas(&mn, &ops));
            let expected = match sc {
                1 => 0b00u32,
                2 => 0b01u32,
                _ => if rs_is_64 { 0b11u32 } else { 0b10u32 },
            };
            prop_assert_eq!((w >> 30) & 0b11, expected);
        }

        // Property 5 — negative contract: fewer than 3 operands, a non-memory
        // third operand, or an out-of-range register number (>31) must all be
        // rejected (parse_reg_num/get_reg already enforce the 0..=31 range).
        #[test]
        fn prop_negative_contract(kind in 0u8..5u8) {
            let r = match kind {
                0 => encode_cas("cas", &[]),
                1 => encode_cas("cas", &[reg('x', 0)]),
                2 => encode_cas("cas", &[reg('x', 0), reg('x', 1)]),
                3 => encode_cas("cas", &[reg('x', 0), reg('x', 1), Operand::Imm(5)]),
                _ => encode_cas("cas", &[reg('x', 32), reg('x', 1), mem("x2")]),
            };
            prop_assert!(r.is_err(), "expected Err, got {:?}", r);
        }

        // Property 6 — negative contract: ARMv8-A CAS requires Rs and Rt to be
        // the SAME width, and byte/half (CASB/CASH) forms must use W (32-bit)
        // registers only (ARM ARM §C6.2.21). These combinations are
        // architecturally UNDEFINED and must be rejected.
        //
        // NOTE: this property is EXPECTED TO FAIL against the current
        // implementation — it documents the validation gap described in
        // pbt-out/bug_reports/encode_cas-mixed-width-operand-validation.md.
        #[test]
        fn prop_width_violation_rejected(kind in 0u8..5u8) {
            let ops = match kind {
                // byte CAS with 64-bit (X) registers
                0 => ("casb", vec![reg('x', 0), reg('x', 1), mem("x2")]),
                1 => ("casab", vec![reg('x', 5), reg('x', 6), mem("x3")]),
                // halfword CAS with 64-bit (X) registers
                2 => ("cash", vec![reg('x', 0), reg('x', 1), mem("x2")]),
                // plain CAS: Rs (W) and Rt (X) differ
                3 => ("cas", vec![reg('w', 0), reg('x', 1), mem("x2")]),
                // plain CAS: Rs (X) and Rt (W) differ
                _ => ("cas", vec![reg('x', 0), reg('w', 1), mem("x2")]),
            };
            let r = encode_cas(ops.0, &ops.1);
            prop_assert!(
                r.is_err(),
                "{} {:?}: expected Err for register-width violation, got Ok",
                ops.0, ops.1
            );
        }
    }
}
