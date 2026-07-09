use super::*;
use crate::backend::arm::assembler::parser::Operand;

// ── NEON/SIMD ────────────────────────────────────────────────────────────

/// Helper to extract register number from a RegArrangement operand
pub(crate) fn get_neon_reg(operands: &[Operand], idx: usize) -> Result<(u32, String), String> {
    match operands.get(idx) {
        Some(Operand::RegArrangement { reg, arrangement }) => {
            let num = parse_reg_num(reg)
                .ok_or_else(|| format!("invalid NEON register: {}", reg))?;
            Ok((num, arrangement.clone()))
        }
        Some(Operand::Reg(name)) => {
            let num = parse_reg_num(name)
                .ok_or_else(|| format!("invalid register: {}", name))?;
            Ok((num, String::new()))
        }
        other => Err(format!("expected NEON register at operand {}, got {:?}", idx, other)),
    }
}

pub(crate) fn encode_cnt(operands: &[Operand]) -> Result<EncodeResult, String> {
    // CNT Vd.<T>, Vn.<T>
    // Encoding: 0 Q 00 1110 size 10 0000 0101 10 Rn Rd
    // Only valid for .8b (Q=0) and .16b (Q=1)
    if operands.len() < 2 {
        return Err("cnt requires 2 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _arr_n) = get_neon_reg(operands, 1)?;

    let q: u32 = if arr_d == "16b" { 1 } else { 0 }; // .8b -> Q=0, .16b -> Q=1

    // 0 Q 00 1110 00 10 0000 0101 10 Rn Rd
    let word = ((q << 30) | (0b001110 << 24)) | (0b100000 << 16)
        | (0b010110 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON three-same register operations ──────────────────────────────────

/// Get Q bit and size from arrangement specifier.
pub(crate) fn neon_arr_to_q_size(arr: &str) -> Result<(u32, u32), String> {
    match arr {
        "8b" => Ok((0, 0b00)),
        "16b" => Ok((1, 0b00)),
        "4h" => Ok((0, 0b01)),
        "8h" => Ok((1, 0b01)),
        "2s" => Ok((0, 0b10)),
        "4s" => Ok((1, 0b10)),
        "1d" => Ok((0, 0b11)),
        "2d" => Ok((1, 0b11)),
        _ => Err(format!("unsupported NEON arrangement: {}", arr)),
    }
}

/// Encode NEON three-same-register instructions: CMEQ, UQSUB, SQSUB, CMHI, etc.
///
/// Layout: 0 Q U 01110 size 1 Rm opcode 1 Rn Rd
///         31 30 29 28-24 23-22 21 20-16 15-11 10 9-5 4-0
///
/// `u_bit`: U field (bit 29) - 0 for signed, 1 for unsigned
/// `opcode`: instruction opcode (bits 15-11)
pub(crate) fn encode_neon_three_same(operands: &[Operand], u_bit: u32, opcode: u32) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("NEON three-same requires 3 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _arr_n) = get_neon_reg(operands, 1)?;
    let (rm, _arr_m) = get_neon_reg(operands, 2)?;

    let (q, size) = neon_arr_to_q_size(&arr_d)?;

    // 0 Q U 01110 size 1 Rm opcode 1 Rn Rd
    let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22) | (1 << 21)
        | (rm << 16) | (opcode << 11) | (1 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON three-different instructions: USUBL, SSUBL, UADDL, SADDL, etc.
///
/// These instructions have wider destination than source operands.
/// Format: 0 Q U 01110 size 1 Rm opcode 00 Rn Rd
///
/// `u_bit`: 0 for signed, 1 for unsigned
/// `opcode`: 4-bit opcode (bits 15-12)
/// `is_high`: true for the "2" variant (upper half, Q=1)
pub(crate) fn encode_neon_three_diff(operands: &[Operand], u_bit: u32, opcode: u32, is_high: bool) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("NEON three-different requires 3 operands".to_string());
    }
    let (rd, _arr_d) = get_neon_reg(operands, 0)?;
    let (rn, arr_n) = get_neon_reg(operands, 1)?;
    let (rm, _arr_m) = get_neon_reg(operands, 2)?;

    // Size is determined from the source (narrow) arrangement
    let (q, size) = match arr_n.as_str() {
        "8b" => (0u32, 0b00u32),   // base
        "16b" => (1, 0b00),         // "2" variant
        "4h" => (0, 0b01),
        "8h" => (1, 0b01),
        "2s" => (0, 0b10),
        "4s" => (1, 0b10),
        _ => return Err(format!("unsupported source arrangement for three-diff: {}", arr_n)),
    };

    // For the "2" variant, override Q
    let q = if is_high { 1 } else { q };

    // 0 Q U 01110 size 1 Rm opcode 00 Rn Rd
    let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22)
        | (1 << 21) | (rm << 16) | (opcode << 12) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON SQSHRUN/SQSHRUN2: Signed saturating shift right unsigned narrow
/// Format: 0 Q 1 011110 immh immb 100011 Rn Rd
pub(crate) fn encode_neon_sqshrun(operands: &[Operand], is_rounding: bool, is_high: bool) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("sqshrun requires 3 operands".to_string());
    }
    let (rd, _arr_d) = get_neon_reg(operands, 0)?;
    let (rn, arr_n) = get_neon_reg(operands, 1)?;
    let shift = match &operands[2] {
        Operand::Imm(v) => *v as u32,
        _ => return Err("sqshrun: expected immediate shift".to_string()),
    };

    // immh:immb encode element size and shift amount
    // For source .4s (dest .4h or .8h): immh=001x, shift_amount = 32 - (immh:immb)
    // For source .8h (dest .8b or .16b): immh=0001, shift_amount = 16 - (immh:immb)
    // For source .2d (dest .2s or .4s): immh=01xx, shift_amount = 64 - (immh:immb)
    let (element_bits, immh_base) = match arr_n.as_str() {
        "8h" => (16u32, 0b0001u32),
        "4s" => (32, 0b0010),
        "2d" => (64, 0b0100),
        _ => return Err(format!("sqshrun: unsupported source arrangement: {}", arr_n)),
    };

    if shift == 0 || shift > element_bits {
        return Err(format!("sqshrun: shift {} out of range for {}-bit elements", shift, element_bits));
    }

    let immhb = (element_bits - shift) & 0x7F; // immh:immb combined
    let immh = (immhb >> 3) | immh_base;
    let immb = immhb & 0x7;

    let q = if is_high { 1u32 } else { 0 };

    // 0 Q 1 011110 immh immb opcode 1 Rn Rd
    // SQSHRUN: opcode = 100001, SQRSHRUN: opcode = 100011
    let opcode_bits: u32 = if is_rounding { 0b100011 } else { 0b100001 };
    let word = (q << 30) | (1 << 29) | (0b011110 << 23) | (immh << 19) | (immb << 16)
        | (opcode_bits << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON UXTL/SXTL (unsigned/signed extend long).
/// These are aliases for USHLL/SSHLL with shift #0.
///
/// Format: 0 Q U 011110 immh immb 10100 1 Rn Rd
pub(crate) fn encode_neon_xtl(operands: &[Operand], u_bit: u32, is_high: bool) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("NEON uxtl/sxtl requires 2 operands".to_string());
    }
    let (rd, _arr_d) = get_neon_reg(operands, 0)?;
    let (rn, arr_n) = get_neon_reg(operands, 1)?;

    // immh encodes the source element size, immb=0 (shift=0)
    let immh = match arr_n.as_str() {
        "8b" | "16b" => 0b0001u32,
        "4h" | "8h" => 0b0010,
        "2s" | "4s" => 0b0100,
        _ => return Err(format!("uxtl/sxtl: unsupported source arrangement: {}", arr_n)),
    };

    let q = if is_high { 1u32 } else { 0 };

    // 0 Q U 011110 immh immb 10100 1 Rn Rd
    let word = (q << 30) | (u_bit << 29) | (0b011110 << 23) | (immh << 19)
        | (0b101001 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON compare-to-zero: CMEQ Vd, Vn, #0, CMGE Vd, Vn, #0, etc.
///
/// Format: 0 Q U 01110 size 10000 opcode 10 Rn Rd
pub(crate) fn encode_neon_cmp_zero(operands: &[Operand], u_bit: u32, opcode: u32) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("NEON compare-zero requires at least 2 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (q, size) = neon_arr_to_q_size(&arr_d)?;

    // 0 Q U 01110 size 10000 opcode 10 Rn Rd
    let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22)
        | (0b10000 << 17) | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON two-register miscellaneous narrowing: UQXTN, SQXTN, XTN
///
/// Format: 0 Q U 01110 size 10000 opcode 10 Rn Rd
pub(crate) fn encode_neon_two_misc_narrow(operands: &[Operand], u_bit: u32, opcode: u32, is_high: bool) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("NEON two-reg narrow requires 2 operands".to_string());
    }
    let (rd, _arr_d) = get_neon_reg(operands, 0)?;
    let (rn, arr_n) = get_neon_reg(operands, 1)?;

    // Size from source (wider) arrangement
    let size = match arr_n.as_str() {
        "8h" => 0b00u32,
        "4s" => 0b01,
        "2d" => 0b10,
        _ => return Err(format!("unsupported source arrangement for narrow: {}", arr_n)),
    };

    let q = if is_high { 1u32 } else { 0 };

    // 0 Q U 01110 size 10000 opcode 10 Rn Rd
    let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22)
        | (0b10000 << 17) | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON vector-by-element long instructions: SMULL/UMULL/SMLAL/UMLAL/SMLSL/UMLSL (elem)
///
/// Format: 0 Q U 01111 size L M Rm opcode H 0 Rn Rd
///
/// These are the widening multiply-by-element forms where the third operand
/// is a register lane (e.g., v0.h[2]).
pub(crate) fn encode_neon_elem_long(operands: &[Operand], u_bit: u32, opcode: u32, is_high: bool) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("NEON elem-long requires 3 operands".to_string());
    }
    let (rd, _arr_d) = get_neon_reg(operands, 0)?;
    let (rn, arr_n) = get_neon_reg(operands, 1)?;

    // Third operand is RegLane: v0.h[2]
    let (rm, index) = match &operands[2] {
        Operand::RegLane { reg, elem_size: _, index } => {
            let rm = parse_reg_num(reg).ok_or("invalid NEON register")?;
            (rm, *index)
        }
        _ => return Err(format!("expected register lane operand, got {:?}", operands[2])),
    };

    // Determine size and Q from source arrangement
    let (q, size) = match arr_n.as_str() {
        "4h" => (0u32, 0b01u32),
        "8h" => (1, 0b01),
        "2s" => (0, 0b10),
        "4s" => (1, 0b10),
        _ => return Err(format!("unsupported source arrangement for elem-long: {}", arr_n)),
    };
    let q = if is_high { 1 } else { q };

    // Encode index into H:L:M bits depending on element size
    let (h, l, m) = match size {
        0b01 => {
            // Half-word: index = H:L:M (3 bits), Rm limited to v0-v15
            if index > 7 {
                return Err(format!("element index {} out of range for .h", index));
            }
            let h = (index >> 2) & 1;
            let l = (index >> 1) & 1;
            let m = index & 1;
            (h, l, m)
        }
        0b10 => {
            // Word: index = H:L (2 bits), M=Rm[4]
            if index > 3 {
                return Err(format!("element index {} out of range for .s", index));
            }
            let h = (index >> 1) & 1;
            let l = index & 1;
            let m = (rm >> 4) & 1; // M bit from Rm[4]
            (h, l, m)
        }
        _ => return Err("unsupported element size for by-element".to_string()),
    };

    // Limit Rm for half-word indexing (only v0-v15)
    let rm_enc = if size == 0b01 { rm & 0xF } else { rm & 0x1F };

    // 0 Q U 01111 size L M Rm opcode H 0 Rn Rd
    let word = (q << 30) | (u_bit << 29) | (0b01111 << 24) | (size << 22)
        | (l << 21) | (m << 20) | (rm_enc << 16) | (opcode << 12)
        | (h << 11) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON logical operations: ORR/AND/EOR Vd.T, Vn.T, Vm.T
pub(crate) fn encode_neon_logical(operands: &[Operand], opc: u32) -> Result<EncodeResult, String> {
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _arr_n) = get_neon_reg(operands, 1)?;
    let (rm, _arr_m) = get_neon_reg(operands, 2)?;

    let q: u32 = if arr_d == "16b" { 1 } else { 0 };

    // NEON logical three-same:
    // ORR: 0 Q 0 01110 10 1 Rm 000111 Rn Rd  (opc=0b01 -> size=10)
    // AND: 0 Q 0 01110 00 1 Rm 000111 Rn Rd  (opc=0b00 -> size=00)
    // EOR: 0 Q 1 01110 00 1 Rm 000111 Rn Rd  (opc=0b10 -> size=00, U=1)
    // BIC: 0 Q 0 01110 01 1 Rm 000111 Rn Rd  (would be opc=0b01 with N=1... but not needed)
    let (u_bit, size_bits): (u32, u32) = match opc {
        0b00 => (0, 0b00),  // AND
        0b01 => (0, 0b10),  // ORR
        0b10 => (1, 0b00),  // EOR
        0b11 => (1, 0b00),  // ANDS - not valid for NEON, fall back
        _ => return Err("unsupported NEON logical opc".to_string()),
    };

    let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size_bits << 22)
        | (1 << 21) | (rm << 16) | (0b000111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON MUL Vd.T, Vn.T, Vm.T
pub(crate) fn encode_neon_mul(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;
    let (q, size) = neon_arr_to_q_size(&arr_d)?;

    // MUL (vector): 0 Q 0 01110 size 1 Rm 10011 1 Rn Rd
    let word = (q << 30) | (0b001110 << 24) | (size << 22) | (1 << 21)
        | (rm << 16) | (0b100111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON PMUL Vd.T, Vn.T, Vm.T (polynomial multiply, bytes only)
pub(crate) fn encode_neon_pmul(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;
    let q: u32 = if arr_d == "16b" { 1 } else { 0 };
    // PMUL: 0 Q 1 01110 00 1 Rm 10011 1 Rn Rd (size=00 for bytes, U=1)
    // PMUL encoding: size=00 (bytes) is implicit (zero bits at [23:22])
    let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (1 << 21)
        | (rm << 16) | (0b100111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON MLA Vd.T, Vn.T, Vm.T (multiply-accumulate)
pub(crate) fn encode_neon_mla(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;
    let (q, size) = neon_arr_to_q_size(&arr_d)?;
    // MLA: 0 Q 0 01110 size 1 Rm 10010 1 Rn Rd
    let word = (q << 30) | (0b001110 << 24) | (size << 22) | (1 << 21)
        | (rm << 16) | (0b100101 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON MLS Vd.T, Vn.T, Vm.T (multiply-subtract)
pub(crate) fn encode_neon_mls(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;
    let (q, size) = neon_arr_to_q_size(&arr_d)?;
    // MLS: 0 Q 1 01110 size 1 Rm 10010 1 Rn Rd (U=1)
    let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (size << 22) | (1 << 21)
        | (rm << 16) | (0b100101 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON USHR Vd.T, Vn.T, #shift (unsigned shift right immediate)
pub(crate) fn encode_neon_shift_imm(operands: &[Operand], _is_unsigned: bool) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("ushr requires 3 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let shift = get_imm(operands, 2)?;

    let (q, _size) = neon_arr_to_q_size(&arr_d)?;

    // USHR: 0 Q 1 011110 immh:immb 00000 1 Rn Rd
    // For .16b (bytes, size=8): immh = 0001, immb = 8-shift (3 bits)
    // For .8h (halfwords, size=16): immh = 001x
    // For .4s (words, size=32): immh = 01xx
    // For .2d (doublewords, size=64): immh = 1xxx
    // immh:immb = (element_size * 2 - shift)
    let (elem_bits, immh_immb) = match arr_d.as_str() {
        "8b" | "16b" => (8u32, (16 - shift as u32) & 0xF),   // immh:immb is 4 bits for 8-bit elems
        "4h" | "8h" => (16, (32 - shift as u32) & 0x1F),
        "2s" | "4s" => (32, (64 - shift as u32) & 0x3F),
        "2d" => (64, (128 - shift as u32) & 0x7F),
        _ => return Err(format!("unsupported USHR arrangement: {}", arr_d)),
    };
    let _ = elem_bits;

    // Full encoding: 0 Q 1 011110 immh:immb 000001 Rn Rd
    let word = (q << 30) | (1 << 29) | (0b011110 << 23) | (immh_immb << 16)
        | (0b000001 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON EXT Vd.T, Vn.T, Vm.T, #index
pub(crate) fn encode_neon_ext(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 4 {
        return Err("ext requires 4 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;
    let index = get_imm(operands, 3)? as u32;

    let q: u32 = if arr_d == "16b" { 1 } else { 0 };

    // EXT Vd.T, Vn.T, Vm.T, #index
    // Encoding: 0 Q 10 1110 00 0 Rm 0 imm4 0 Rn Rd
    let word = ((((q << 30) | (0b101110 << 24))
        | (rm << 16)) | ((index & 0xF) << 11)) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON ADDV: add across vector lanes
pub(crate) fn encode_neon_addv(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("addv requires 2 operands".to_string());
    }
    let (rd, _) = get_neon_reg(operands, 0)?;
    let (rn, arr_n) = get_neon_reg(operands, 1)?;

    let (q, size) = neon_arr_to_q_size(&arr_n)?;

    // ADDV: 0 Q 0 01110 size 11000 11011 10 Rn Rd
    let word = (q << 30) | (0b001110 << 24) | (size << 22) | (0b11000 << 17)
        | (0b110111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON across-vector instructions: UMAXV, UMINV, SMAXV, SMINV
///
/// Format: 0 Q U 01110 size 11000 opcode 10 Rn Rd
///
/// `u_bit`: 0 for signed, 1 for unsigned
/// `opcode`: 5-bit opcode (bits 16-12)
pub(crate) fn encode_neon_across(operands: &[Operand], u_bit: u32, opcode: u32) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("NEON across-vector requires 2 operands".to_string());
    }
    let (rd, _) = get_neon_reg(operands, 0)?;
    let (rn, arr_n) = get_neon_reg(operands, 1)?;

    let (q, size) = neon_arr_to_q_size(&arr_n)?;

    // 0 Q U 01110 size 11000 opcode 10 Rn Rd
    let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22) | (0b11000 << 17)
        | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON UMOV: move element to GP register
pub(crate) fn encode_neon_umov(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("umov requires 2 operands".to_string());
    }
    let (rd, is_64) = get_reg(operands, 0)?;

    // Second operand should be a RegLane (v0.b[0])
    match operands.get(1) {
        Some(Operand::RegLane { reg, elem_size, index }) => {
            let rn = parse_reg_num(reg).ok_or("invalid NEON register")?;
            let q = if is_64 { 1u32 } else { 0 };

            let imm5 = match elem_size.as_str() {
                "b" => ((*index & 0xF) << 1) | 0b00001,
                "h" => ((*index & 0x7) << 2) | 0b00010,
                "s" => ((*index & 0x3) << 3) | 0b00100,
                "d" => ((*index & 0x1) << 4) | 0b01000,
                _ => return Err(format!("unsupported umov element size: {}", elem_size)),
            };

            // UMOV Rd, Vn.Ts[index]: 0 Q 0 01110 000 imm5 0 0111 1 Rn Rd
            let word = (q << 30) | (0b001110000u32 << 21) | (imm5 << 16)
                | (0b001111 << 10) | (rn << 5) | rd;
            Ok(EncodeResult::Word(word))
        }
        _ => Err("umov: expected register lane operand".to_string()),
    }
}

/// Encode NEON DUP: broadcast GP register to all vector lanes
pub(crate) fn encode_neon_dup(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("dup requires 2 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;

    // DUP Vd.T, Rn (general form - broadcast GP reg to vector)
    if let Some(Operand::Reg(rn_name)) = operands.get(1) {
        let rn = parse_reg_num(rn_name).ok_or("invalid rn")?;
        let (q, _) = neon_arr_to_q_size(&arr_d)?;

        // imm5 encoding for element size:
        // .8b/.16b: imm5 = 00001
        // .4h/.8h:  imm5 = 00010
        // .2s/.4s:  imm5 = 00100
        // .2d:      imm5 = 01000
        let imm5 = match arr_d.as_str() {
            "8b" | "16b" => 0b00001u32,
            "4h" | "8h" => 0b00010,
            "2s" | "4s" => 0b00100,
            "2d" => 0b01000,
            _ => return Err(format!("unsupported dup arrangement: {}", arr_d)),
        };

        // DUP Vd.T, Rn: 0 Q 0 01110 000 imm5 0 0001 1 Rn Rd
        let word = (q << 30) | (0b001110000u32 << 21) | (imm5 << 16)
            | (0b000011 << 10) | (rn << 5) | rd;
        return Ok(EncodeResult::Word(word));
    }

    // DUP Vd.T, Vn.Ts[index] (broadcast element to all lanes)
    if let Some(Operand::RegLane { reg, elem_size, index }) = operands.get(1) {
        let rn = parse_reg_num(reg).ok_or("invalid NEON register")?;
        let (q, _) = neon_arr_to_q_size(&arr_d)?;

        // imm5 encodes both element size and index:
        // .b[i]: imm5 = (i << 1) | 0b00001
        // .h[i]: imm5 = (i << 2) | 0b00010
        // .s[i]: imm5 = (i << 3) | 0b00100
        // .d[i]: imm5 = (i << 4) | 0b01000
        let imm5 = match elem_size.as_str() {
            "b" => ((*index & 0xF) << 1) | 0b00001,
            "h" => ((*index & 0x7) << 2) | 0b00010,
            "s" => ((*index & 0x3) << 3) | 0b00100,
            "d" => ((*index & 0x1) << 4) | 0b01000,
            _ => return Err(format!("unsupported dup element size: {}", elem_size)),
        };

        // DUP Vd.T, Vn.Ts[i]: 0 Q 0 01110 000 imm5 0 0000 1 Rn Rd
        let word = (q << 30) | (0b001110000u32 << 21) | (imm5 << 16)
            | (0b000001 << 10) | (rn << 5) | rd;
        return Ok(EncodeResult::Word(word));
    }

    Err("unsupported dup operands".to_string())
}

/// Encode NEON INS (insert element from GP register): INS Vd.Ts[index], Xn
pub(crate) fn encode_neon_ins(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("ins requires 2 operands".to_string());
    }
    match (&operands[0], &operands[1]) {
        // INS Vd.Ts[dst_idx], Xn (general register to element)
        (Operand::RegLane { reg, elem_size, index }, Operand::Reg(rn_name)) => {
            let rd = parse_reg_num(reg).ok_or("invalid NEON register")?;
            let rn = parse_reg_num(rn_name).ok_or("invalid register")?;

            let imm5 = match elem_size.as_str() {
                "b" => ((*index & 0xF) << 1) | 0b00001,
                "h" => ((*index & 0x7) << 2) | 0b00010,
                "s" => ((*index & 0x3) << 3) | 0b00100,
                "d" => ((*index & 0x1) << 4) | 0b01000,
                _ => return Err(format!("unsupported ins element size: {}", elem_size)),
            };

            // INS Vd.Ts[i], Xn: 0 1 0 01110 000 imm5 0 0011 1 Rn Rd
            let word = (0b01001110000u32 << 21) | (imm5 << 16)
                | (0b000111 << 10) | (rn << 5) | rd;
            Ok(EncodeResult::Word(word))
        }
        // INS Vd.Ts[dst_idx], Vn.Ts[src_idx] (element to element)
        (Operand::RegLane { reg: rd_name, elem_size: dst_size, index: dst_idx },
         Operand::RegLane { reg: rn_name, elem_size: _src_size, index: src_idx }) => {
            let rd = parse_reg_num(rd_name).ok_or("invalid NEON rd")?;
            let rn = parse_reg_num(rn_name).ok_or("invalid NEON rn")?;

            let (imm5, imm4) = match dst_size.as_str() {
                "b" => (
                    ((*dst_idx & 0xF) << 1) | 0b00001,
                    *src_idx & 0xF,
                ),
                "h" => (
                    ((*dst_idx & 0x7) << 2) | 0b00010,
                    (*src_idx & 0x7) << 1,
                ),
                "s" => (
                    ((*dst_idx & 0x3) << 3) | 0b00100,
                    (*src_idx & 0x3) << 2,
                ),
                "d" => (
                    ((*dst_idx & 0x1) << 4) | 0b01000,
                    (*src_idx & 0x1) << 3,
                ),
                _ => return Err(format!("unsupported ins element size: {}", dst_size)),
            };

            // INS Vd.Ts[dst], Vn.Ts[src]: 0 1 1 01110 000 imm5 0 imm4 1 Rn Rd
            let word = (0b01101110000u32 << 21) | (imm5 << 16)
                | (imm4 << 11) | (1 << 10) | (rn << 5) | rd;
            Ok(EncodeResult::Word(word))
        }
        _ => Err("ins: expected (RegLane, Reg) or (RegLane, RegLane) operands".to_string()),
    }
}

/// Encode NEON NOT (bitwise NOT): NOT Vd.T, Vn.T
pub(crate) fn encode_neon_not(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("not requires 2 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;

    let q: u32 = if arr_d == "16b" { 1 } else { 0 };

    // NOT Vd.T, Vn.T (alias of MVN): 0 Q 1 01110 00 10000 00101 10 Rn Rd
    let word = ((q << 30) | (1 << 29) | (0b01110 << 24))
        | (0b10000 << 17) | (0b00101 << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON MOVI (move immediate to vector)
pub(crate) fn encode_neon_movi(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("movi requires 2 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let imm = get_imm(operands, 1)?;

    match arr_d.as_str() {
        "16b" | "8b" => {
            // MOVI Vd.16b, #imm8
            // Encoding: 0 Q 00 1111 00000 abc 1110 01 defgh Rd
            // where imm8 = abc:defgh
            let q: u32 = if arr_d == "16b" { 1 } else { 0 };
            let imm8 = imm as u32 & 0xFF;
            let abc = (imm8 >> 5) & 0x7;
            let defgh = imm8 & 0x1F;
            // 0 Q op 0 1111 0 a b c cmode(1110) o2(0) 1 defgh Rd
            let word = (q << 30) | (0b0011110 << 23) | ((abc >> 2) << 18) | (((abc >> 1) & 1) << 17)
                | ((abc & 1) << 16) | (0b1110 << 12) | (0b01 << 10) | (defgh << 5) | rd;
            Ok(EncodeResult::Word(word))
        }
        "2d" => {
            // MOVI Vd.2d, #imm
            // The 64-bit immediate is encoded as 8 bits, where each bit expands
            // to 8 bits (0x00 or 0xFF) in the result.
            // Convert the 64-bit value to the 8-bit encoding.
            let imm64 = imm as u64;
            let mut imm8 = 0u32;
            for i in 0..8 {
                let byte_val = (imm64 >> (i * 8)) & 0xFF;
                if byte_val == 0xFF {
                    imm8 |= 1 << i;
                } else if byte_val != 0 {
                    return Err(format!("movi .2d: each byte of immediate must be 0x00 or 0xFF, got 0x{:02x} at byte {}", byte_val, i));
                }
            }
            let abc = (imm8 >> 5) & 0x7;
            let defgh = imm8 & 0x1F;
            // MOVI Vd.2d, #imm: 0 1 1 0 1111 00 abc 1110 01 defgh Rd  (op=1, Q=1)
            let word = (0b01101111 << 24) | ((abc >> 2) << 18) | (((abc >> 1) & 1) << 17)
                | ((abc & 1) << 16) | (0b111001 << 10) | (defgh << 5) | rd;
            Ok(EncodeResult::Word(word))
        }
        "2s" | "4s" => {
            // MOVI Vd.2s/4s, #imm8 (32-bit element, no shift)
            // Encoding: 0 Q op(0) 0 1111 0 abc cmode(0000) o2(0) 1 defgh Rd
            let q: u32 = if arr_d == "4s" { 1 } else { 0 };
            let imm8 = imm as u32 & 0xFF;
            let abc = (imm8 >> 5) & 0x7;
            let defgh = imm8 & 0x1F;

            // Check for optional LSL shift operand
            let (cmode, shift_val) = if operands.len() > 2 {
                if let Some(Operand::Shift { kind, amount }) = operands.get(2) {
                    if kind == "lsl" {
                        match amount {
                            0 => (0b0000u32, 0),
                            8 => (0b0010, 8),
                            16 => (0b0100, 16),
                            24 => (0b0110, 24),
                            _ => return Err(format!("movi: unsupported shift amount: {}", amount)),
                        }
                    } else {
                        (0b0000, 0)
                    }
                } else {
                    (0b0000, 0)
                }
            } else {
                (0b0000, 0)
            };
            let _ = shift_val;

            let word = (q << 30) | (0b0011110 << 23) | ((abc >> 2) << 18) | (((abc >> 1) & 1) << 17)
                | ((abc & 1) << 16) | (cmode << 12) | (0b01 << 10) | (defgh << 5) | rd;
            Ok(EncodeResult::Word(word))
        }
        "4h" | "8h" => {
            // MOVI Vd.4h/8h, #imm8
            let q: u32 = if arr_d == "8h" { 1 } else { 0 };
            let imm8 = imm as u32 & 0xFF;
            let abc = (imm8 >> 5) & 0x7;
            let defgh = imm8 & 0x1F;
            // cmode=1000 for .4h/.8h with no shift
            let word = (q << 30) | (0b0011110 << 23) | ((abc >> 2) << 18) | (((abc >> 1) & 1) << 17)
                | ((abc & 1) << 16) | (0b1000 << 12) | (0b01 << 10) | (defgh << 5) | rd;
            Ok(EncodeResult::Word(word))
        }
        _ => Err(format!("movi: unsupported arrangement: {}", arr_d)),
    }
}


/// Encode NEON BIC (bitwise clear vector): BIC Vd.T, Vn.T, Vm.T
pub(crate) fn encode_neon_bic(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("bic requires 3 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;

    let q: u32 = if arr_d == "16b" { 1 } else { 0 };

    // BIC Vd.T, Vn.T, Vm.T: 0 Q 0 01110 01 1 Rm 000111 Rn Rd
    let word = (q << 30) | (0b001110 << 24) | (0b01 << 22) | (1 << 21)
        | (rm << 16) | (0b000111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON BSL (bitwise select): BSL Vd.T, Vn.T, Vm.T
pub(crate) fn encode_neon_bsl(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("bsl requires 3 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;

    let q: u32 = if arr_d == "16b" { 1 } else { 0 };

    // BSL Vd.T, Vn.T, Vm.T: 0 Q 1 01110 01 1 Rm 000111 Rn Rd
    let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (0b01 << 22) | (1 << 21)
        | (rm << 16) | (0b000111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON REV64: reverse elements within 64-bit doublewords
pub(crate) fn encode_neon_rev64(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("rev64 requires 2 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;

    let (q, size) = neon_arr_to_q_size(&arr_d)?;

    // REV64 Vd.T, Vn.T: 0 Q 0 01110 size 10 0000 0000 10 Rn Rd
    let word = (q << 30) | (0b001110 << 24) | (size << 22)
        | (0b100000 << 16) | (0b000010 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON TBL: table vector lookup
pub(crate) fn encode_neon_tbl(operands: &[Operand]) -> Result<EncodeResult, String> {
    // TBL Vd.T, {Vn.T}, Vm.T  (single register table)
    // TBL Vd.T, {Vn.T, Vn+1.T}, Vm.T  (two register table)
    // etc.
    if operands.len() < 3 {
        return Err("tbl requires 3 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let q: u32 = if arr_d == "16b" { 1 } else { 0 };

    // The second operand is a register list
    let (rn, num_regs) = match &operands[1] {
        Operand::RegList(regs) => {
            let first_reg = match &regs[0] {
                Operand::RegArrangement { reg, .. } => parse_reg_num(reg).ok_or("invalid reg")?,
                Operand::Reg(name) => parse_reg_num(name).ok_or("invalid reg")?,
                _ => return Err("tbl: expected register in list".to_string()),
            };
            (first_reg, regs.len() as u32)
        }
        _ => return Err("tbl: expected register list as second operand".to_string()),
    };

    let (rm, _) = get_neon_reg(operands, 2)?;

    // len field: 1 reg -> 00, 2 -> 01, 3 -> 10, 4 -> 11
    let len = (num_regs - 1) & 0x3;

    // TBL: 0 Q 00 1110 000 Rm 0 len 0 00 Rn Rd
    let word = ((((q << 30) | (0b001110 << 24))
        | (rm << 16)) | (len << 13)) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON TBX: table vector lookup with insert (preserves out-of-range lanes)
pub(crate) fn encode_neon_tbx(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("tbx requires 3 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let q: u32 = if arr_d == "16b" { 1 } else { 0 };

    let (rn, num_regs) = match &operands[1] {
        Operand::RegList(regs) => {
            let first_reg = match &regs[0] {
                Operand::RegArrangement { reg, .. } => parse_reg_num(reg).ok_or("invalid reg")?,
                Operand::Reg(name) => parse_reg_num(name).ok_or("invalid reg")?,
                _ => return Err("tbx: expected register in list".to_string()),
            };
            (first_reg, regs.len() as u32)
        }
        _ => return Err("tbx: expected register list as second operand".to_string()),
    };

    let (rm, _) = get_neon_reg(operands, 2)?;
    let len = (num_regs - 1) & 0x3;

    // TBX: 0 Q 00 1110 000 Rm 0 len 1 00 Rn Rd (op=1 for TBX vs op=0 for TBL)
    let word = (q << 30) | (0b001110 << 24) | (rm << 16) | (len << 13)
        | (1 << 12) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON LD1R: load single structure and replicate to all lanes
pub(crate) fn encode_neon_ld1r(operands: &[Operand]) -> Result<EncodeResult, String> {
    // LD1R {Vt.T}, [Xn]
    if operands.len() < 2 {
        return Err("ld1r requires 2 operands".to_string());
    }

    let (rt, arr) = match &operands[0] {
        Operand::RegList(regs) => {
            if regs.len() != 1 {
                return Err("ld1r expects exactly one register in list".to_string());
            }
            match &regs[0] {
                Operand::RegArrangement { reg, arrangement } => {
                    let num = parse_reg_num(reg).ok_or("invalid reg")?;
                    (num, arrangement.clone())
                }
                _ => return Err("ld1r: expected register with arrangement".to_string()),
            }
        }
        _ => return Err("ld1r: expected register list as first operand".to_string()),
    };

    let (q, size) = match arr.as_str() {
        "8b"  => (0u32, 0b00u32),
        "16b" => (1, 0b00),
        "4h"  => (0, 0b01),
        "8h"  => (1, 0b01),
        "2s"  => (0, 0b10),
        "4s"  => (1, 0b10),
        "1d"  => (0, 0b11),
        "2d"  => (1, 0b11),
        _ => return Err(format!("ld1r: unsupported arrangement: {}", arr)),
    };

    match &operands[1] {
        Operand::Mem { base, offset: 0 } => {
            let rn = parse_reg_num(base).ok_or("invalid base reg")?;
            // LD1R: 0 Q 0 01101 0 1 0 00000 110 0 size Rn Rt (no post-index)
            let word = (q << 30) | (0b001101 << 24) | (1 << 22) | (0b110 << 13)
                | (size << 10) | (rn << 5) | rt;
            Ok(EncodeResult::Word(word))
        }
        Operand::MemPostIndex { base, offset } => {
            let rn = parse_reg_num(base).ok_or("invalid base reg")?;
            // LD1R post-index (immediate): 0 Q 0 01101 1 1 0 11111 110 0 size Rn Rt
            // Rm=11111 means post-index by element size
            let _ = offset; // offset must match element size, not encoded separately
            let word = (q << 30) | (0b001101 << 24) | (1 << 23) | (1 << 22)
                | (0b11111 << 16) | (0b110 << 13) | (size << 10) | (rn << 5) | rt;
            Ok(EncodeResult::Word(word))
        }
        _ => Err("ld1r: expected [Xn] or [Xn], #imm memory operand".to_string()),
    }
}

/// Encode NEON LD1 (vector load, multiple structures)
/// Dispatch LD/ST1-4: choose between "multiple structures" and "single structure (element)" encoding.
pub(crate) fn encode_neon_ld_st_dispatch(operands: &[Operand], is_load: bool, num_structs: u32) -> Result<EncodeResult, String> {
    // If the first operand is a RegListIndexed, use single-element encoding
    if let Some(Operand::RegListIndexed { .. }) = operands.first() {
        return encode_neon_ld_st_single(operands, is_load, num_structs);
    }
    // Multiple-structures encoding for ld1-4/st1-4
    encode_neon_ld_st_multi(operands, is_load, num_structs)
}

/// Encode NEON LD/ST single structure (element):
/// st1 {v0.s}[0], [x3]
/// st2 {v0.s, v1.s}[0], [x3]
/// st4 {v0.s, v1.s, v2.s, v3.s}[0], [x3]
/// ld2 {v0.s, v1.s}[0], [x3]
// TODO: add post-index form [Xn], #imm
pub(crate) fn encode_neon_ld_st_single(operands: &[Operand], is_load: bool, num_structs: u32) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err(format!("ld/st{} single element requires at least 2 operands", num_structs));
    }

    let (regs, index) = match &operands[0] {
        Operand::RegListIndexed { regs, index } => (regs, *index),
        _ => return Err("expected register list with index".to_string()),
    };

    if regs.len() as u32 != num_structs {
        return Err(format!("expected {} registers in list, got {}", num_structs, regs.len()));
    }
    // TODO: validate that registers in the list are consecutive (ARM ISA requirement)

    // Get element size and first register from the list
    let (rt, elem_size) = match &regs[0] {
        Operand::RegArrangement { reg, arrangement } => {
            (parse_reg_num(reg).ok_or("invalid register in list")?, arrangement.clone())
        }
        _ => return Err("expected register with arrangement in list".to_string()),
    };

    // Get base register and check for post-index
    let (rn, post_index) = match &operands[1] {
        Operand::Mem { base, offset: 0 } => {
            let rn = parse_reg_num(base).ok_or_else(|| format!("invalid base register: {}", base))?;
            // Check for post-index immediate: operands[2] is the post-index offset
            let pi = if operands.len() > 2 {
                match &operands[2] {
                    Operand::Imm(off) => Some(*off),
                    _ => None,
                }
            } else {
                None
            };
            (rn, pi)
        }
        Operand::MemPostIndex { base, offset } => {
            let rn = parse_reg_num(base).ok_or_else(|| format!("invalid base register: {}", base))?;
            (rn, Some(*offset))
        }
        _ => return Err("expected [Xn] memory operand".to_string()),
    };

    let l_bit = if is_load { 1u32 } else { 0u32 };

    // R bit: 0 for 1,3 registers; 1 for 2,4 registers
    let r_bit = match num_structs {
        1 | 3 => 0u32,
        2 | 4 => 1u32,
        _ => return Err(format!("unsupported struct count: {}", num_structs)),
    };

    // Compute opcode, S, Q, size based on element size and index
    let (opcode, s_bit, q_bit, size_field) = match elem_size.as_str() {
        "b" => {
            // opcode = 000 (1,2 regs) or 001 (3,4 regs)
            let base_opc = if num_structs <= 2 { 0b000u32 } else { 0b001u32 };
            // index bits: Q:S:size[1]:size[0] = 4 bits for 0-15
            let q = (index >> 3) & 1;
            let s = (index >> 2) & 1;
            let sz = index & 3;
            (base_opc, s, q, sz)
        }
        "h" => {
            let base_opc = if num_structs <= 2 { 0b010u32 } else { 0b011u32 };
            // index bits: Q:S:size[1] = 3 bits for 0-7, size[0]=0
            let q = (index >> 2) & 1;
            let s = (index >> 1) & 1;
            let sz = (index & 1) << 1;
            (base_opc, s, q, sz)
        }
        "s" => {
            let base_opc = if num_structs <= 2 { 0b100u32 } else { 0b101u32 };
            // index bits: Q:S = 2 bits for 0-3, size=00
            let q = (index >> 1) & 1;
            let s = index & 1;
            (base_opc, s, q, 0b00u32)
        }
        "d" => {
            let base_opc = if num_structs <= 2 { 0b100u32 } else { 0b101u32 };
            // index bits: Q = 1 bit for 0-1, S=0, size=01
            let q = index & 1;
            (base_opc, 0u32, q, 0b01u32)
        }
        _ => return Err(format!("unsupported element size for ld/st single: {}", elem_size)),
    };

    if let Some(_offset) = post_index {
        // Post-index form: Q 0011011 L R 11111 opcode S size Rn Rt
        // (Rm=11111 means immediate post-index, the amount is implicit from element size)
        let word = (q_bit << 30) | (0b0011011 << 23) | (l_bit << 22) | (r_bit << 21)
            | (0b11111 << 16) | (opcode << 13) | (s_bit << 12) | (size_field << 10) | (rn << 5) | rt;
        Ok(EncodeResult::Word(word))
    } else {
        // No post-index: Q 0011010 L R 00000 opcode S size Rn Rt
        let word = (q_bit << 30) | (0b0011010 << 23) | (l_bit << 22) | (r_bit << 21)
            | (opcode << 13) | (s_bit << 12) | (size_field << 10) | (rn << 5) | rt;
        Ok(EncodeResult::Word(word))
    }
}

/// Common encoder for LD1/ST1 (multiple structures)
pub(crate) fn encode_neon_ld_st_multi(operands: &[Operand], is_load: bool, num_structs: u32) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err(format!("ld{}/st{} requires at least 2 operands", num_structs, num_structs));
    }

    // First operand: register list {Vt.T} or {Vt.T, Vt+1.T, ...}
    let (rt, arr, num_regs) = match &operands[0] {
        Operand::RegList(regs) => {
            let (first_reg, arrangement) = match &regs[0] {
                Operand::RegArrangement { reg, arrangement } => {
                    (parse_reg_num(reg).ok_or("invalid reg")?, arrangement.clone())
                }
                _ => return Err(format!("ld{}/st{}: expected RegArrangement in list", num_structs, num_structs)),
            };
            (first_reg, arrangement, regs.len() as u32)
        }
        _ => return Err(format!("ld{}/st{}: expected register list", num_structs, num_structs)),
    };

    let (q, size) = neon_arr_to_q_size(&arr)?;

    // Second operand: [Xn] memory base or [Xn], #imm (post-index, merged by parser)
    let (rn, post_index) = match &operands[1] {
        Operand::Mem { base, offset: 0 } => {
            let r = parse_reg_num(base).ok_or_else(|| format!("invalid base register: {}", base))?;
            (r, None)
        }
        Operand::MemPostIndex { base, offset } => {
            let r = parse_reg_num(base).ok_or_else(|| format!("invalid base register: {}", base))?;
            (r, Some(*offset))
        }
        _ => return Err(format!("ld{}/st{}: expected [Xn] memory operand", num_structs, num_structs)),
    };

    // opcode field based on structure count and number of registers:
    // LD1/ST1: 1 reg=0111, 2 reg=1010, 3 reg=0110, 4 reg=0010
    // LD2/ST2: 2 reg=1000
    // LD3/ST3: 3 reg=0100
    // LD4/ST4: 4 reg=0000
    let opcode = match num_structs {
        1 => match num_regs {
            1 => 0b0111u32,
            2 => 0b1010,
            3 => 0b0110,
            4 => 0b0010,
            _ => return Err(format!("ld1/st1: unsupported register count: {}", num_regs)),
        },
        2 => 0b1000u32,
        3 => 0b0100,
        4 => 0b0000,
        _ => return Err(format!("unsupported structure count: {}", num_structs)),
    };

    let l_bit = if is_load { 1u32 } else { 0u32 };

    // Handle post-index form from merged MemPostIndex
    if let Some(_imm) = post_index {
        // Post-index with immediate: use Rm=11111 (0x1F)
        let word = ((q << 30) | (0b001100 << 24) | (1 << 23) | (l_bit << 22)) | (0b11111 << 16) | (opcode << 12) | (size << 10) | (rn << 5) | rt;
        return Ok(EncodeResult::Word(word));
    }

    // Check for post-index form via separate operands: [Xn], Xm
    if operands.len() > 2 {
        match &operands[2] {
            Operand::Imm(_) => {
                // Post-index with immediate: use Rm=11111
                let word = ((q << 30) | (0b001100 << 24) | (1 << 23) | (l_bit << 22)) | (0b11111 << 16) | (opcode << 12) | (size << 10) | (rn << 5) | rt;
                return Ok(EncodeResult::Word(word));
            }
            Operand::Reg(rm_name) => {
                let rm = parse_reg_num(rm_name).ok_or("invalid rm")?;
                let word = ((q << 30) | (0b001100 << 24) | (1 << 23) | (l_bit << 22)) | (rm << 16) | (opcode << 12) | (size << 10) | (rn << 5) | rt;
                return Ok(EncodeResult::Word(word));
            }
            _ => {}
        }
    }

    // No post-index: LD1/ST1 {Vt.T...}, [Xn]
    // 0 Q 001100 0 L 0 00000 opcode size Rn Rt
    let word = (((q << 30) | (0b001100 << 24)) | (l_bit << 22)) | (opcode << 12) | (size << 10) | (rn << 5) | rt;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON UZP1/UZP2/ZIP1/ZIP2
pub(crate) fn encode_neon_zip_uzp(operands: &[Operand], op_bits: u32, _is_zip: bool) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("uzp/zip requires 3 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;
    let (q, size) = neon_arr_to_q_size(&arr_d)?;

    // UZP1: 0 Q 0 01110 size 0 Rm 0 001 10 Rn Rd  (op_bits=001)
    // UZP2: 0 Q 0 01110 size 0 Rm 0 101 10 Rn Rd  (op_bits=101)
    // ZIP1: 0 Q 0 01110 size 0 Rm 0 011 10 Rn Rd  (op_bits=011)
    // ZIP2: 0 Q 0 01110 size 0 Rm 0 111 10 Rn Rd  (op_bits=111)
    let word = (((q << 30) | (0b001110 << 24) | (size << 22)) | (rm << 16)) | (op_bits << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON EOR3 (three-way XOR, SHA3 extension): EOR3 Vd.16b, Vn.16b, Vm.16b, Vk.16b
pub(crate) fn encode_neon_eor3(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 4 {
        return Err("eor3 requires 4 operands".to_string());
    }
    let (rd, _) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;
    let (rk, _) = get_neon_reg(operands, 3)?;

    // EOR3 Vd.16b, Vn.16b, Vm.16b, Vk.16b
    // Encoding: 11001110 000 Rm 0 Rk(4:0) 00 Rn Rd
    let word = ((0b11001110u32 << 24) | (rm << 16)) | (rk << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON PMULL/PMULL2 (polynomial multiply long)
pub(crate) fn encode_neon_pmull(operands: &[Operand], is_pmull2: bool) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("pmull requires 3 operands".to_string());
    }
    let (rd, _) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;

    let q = if is_pmull2 { 1u32 } else { 0 };

    // PMULL  Vd.1q, Vn.1d, Vm.1d: 0 0 00 1110 11 1 Rm 11100 0 Rn Rd  (size=11)
    // PMULL2 Vd.1q, Vn.2d, Vm.2d: 0 1 00 1110 11 1 Rm 11100 0 Rn Rd
    let word = ((q << 30) | (0b001110 << 24) | (0b11 << 22) | (1 << 21)
        | (rm << 16) | (0b11100 << 11)) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON AES instructions (AESE, AESD, AESMC, AESIMC)
pub(crate) fn encode_neon_aes(operands: &[Operand], opcode: u32) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("aes instruction requires 2 operands".to_string());
    }
    let (rd, _) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;

    // AES instructions: 0100 1110 0010 1000 opcode 10 Rn Rd
    // AESE:  opcode = 00100 (0x4)
    // AESD:  opcode = 00101 (0x5)
    // AESMC: opcode = 00110 (0x6)
    // AESIMC:opcode = 00111 (0x7)
    let word = (0b01001110 << 24) | (0b0010100 << 17) | (opcode << 12)
        | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON ADD/SUB (vector integer): ADD/SUB Vd.T, Vn.T, Vm.T
pub(crate) fn encode_neon_add_sub(operands: &[Operand], is_sub: bool) -> Result<EncodeResult, String> {
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;
    let (q, size) = neon_arr_to_q_size(&arr_d)?;
    let u = if is_sub { 1u32 } else { 0u32 };

    // ADD: 0 Q 0 01110 size 1 Rm 10000 1 Rn Rd
    // SUB: 0 Q 1 01110 size 1 Rm 10000 1 Rn Rd
    let word = (q << 30) | (u << 29) | (0b01110 << 24) | (size << 22) | (1 << 21)
        | (rm << 16) | (0b10000 << 11) | (1 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON USHR (unsigned shift right immediate)
pub(crate) fn encode_neon_ushr(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("ushr requires 3 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let shift = get_imm(operands, 2)? as u32;

    let (q, _) = neon_arr_to_q_size(&arr_d)?;

    // USHR Vd.T, Vn.T, #shift
    // 0 Q 1 0 11110 immh:immb 000001 Rn Rd
    let immh_immb = match arr_d.as_str() {
        "8b" | "16b" => (16 - shift) & 0xF,
        "4h" | "8h" => (32 - shift) & 0x1F,
        "2s" | "4s" => (64 - shift) & 0x3F,
        "2d" => (128 - shift) & 0x7F,
        _ => return Err(format!("unsupported ushr arrangement: {}", arr_d)),
    };

    let word = (q << 30) | (1 << 29) | (0b011110 << 23) | (immh_immb << 16)
        | (0b000001 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON SSHR (signed shift right immediate)
pub(crate) fn encode_neon_sshr(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("sshr requires 3 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let shift = get_imm(operands, 2)? as u32;

    let (q, _) = neon_arr_to_q_size(&arr_d)?;

    // SSHR Vd.T, Vn.T, #shift
    // 0 Q 0 0 11110 immh:immb 000001 Rn Rd  (U=0)
    let immh_immb = match arr_d.as_str() {
        "8b" | "16b" => (16 - shift) & 0xF,
        "4h" | "8h" => (32 - shift) & 0x1F,
        "2s" | "4s" => (64 - shift) & 0x3F,
        "2d" => (128 - shift) & 0x7F,
        _ => return Err(format!("unsupported sshr arrangement: {}", arr_d)),
    };

    let word = (q << 30) | (0b011110 << 23) | (immh_immb << 16)
        | (0b000001 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON SHL (shift left immediate)
pub(crate) fn encode_neon_shl(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("shl requires 3 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let shift = get_imm(operands, 2)? as u32;

    let (q, _) = neon_arr_to_q_size(&arr_d)?;

    // SHL Vd.T, Vn.T, #shift
    // 0 Q 0 0 11110 immh:immb 010101 Rn Rd
    // immh:immb = element_size + shift
    let immh_immb = match arr_d.as_str() {
        "8b" | "16b" => (8 + shift) & 0xF,
        "4h" | "8h" => (16 + shift) & 0x1F,
        "2s" | "4s" => (32 + shift) & 0x3F,
        "2d" => (64 + shift) & 0x7F,
        _ => return Err(format!("unsupported shl arrangement: {}", arr_d)),
    };

    let word = (q << 30) | (0b011110 << 23) | (immh_immb << 16)
        | (0b010101 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode NEON SLI (shift left and insert)
pub(crate) fn encode_neon_sli(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("sli requires 3 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let shift = get_imm(operands, 2)? as u32;

    let (q, _) = neon_arr_to_q_size(&arr_d)?;

    // SLI Vd.T, Vn.T, #shift
    // 0 Q 1 0 11110 immh:immb 010101 Rn Rd  (U=1)
    let immh_immb = match arr_d.as_str() {
        "8b" | "16b" => (8 + shift) & 0xF,
        "4h" | "8h" => (16 + shift) & 0x1F,
        "2s" | "4s" => (32 + shift) & 0x3F,
        "2d" => (64 + shift) & 0x7F,
        _ => return Err(format!("unsupported sli arrangement: {}", arr_d)),
    };

    let word = (q << 30) | (1 << 29) | (0b011110 << 23) | (immh_immb << 16)
        | (0b010101 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode SRI (Shift Right and Insert) immediate.
/// SRI Vd.T, Vn.T, #shift: 0 Q 1 0 11110 immh:immb 010001 Rn Rd  (U=1)
pub(crate) fn encode_neon_sri(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("sri requires 3 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let shift = get_imm(operands, 2)? as u32;

    let (q, _) = neon_arr_to_q_size(&arr_d)?;

    // immh:immb = (2*esize - shift) for right shift
    let immh_immb = match arr_d.as_str() {
        "8b" | "16b" => (16 - shift) & 0xF,
        "4h" | "8h" => (32 - shift) & 0x1F,
        "2s" | "4s" => (64 - shift) & 0x3F,
        "2d" => (128 - shift) & 0x7F,
        _ => return Err(format!("unsupported sri arrangement: {}", arr_d)),
    };

    let word = (q << 30) | (1 << 29) | (0b011110 << 23) | (immh_immb << 16)
        | (0b010001 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON RBIT (vector bit reverse) ───────────────────────────────────────

/// Encode NEON RBIT Vd.T, Vn.T (per-byte bit reversal in each element).
pub(crate) fn encode_neon_rbit(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("neon rbit requires 2 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;

    // Only .8b and .16b arrangements are valid for NEON RBIT
    if arr_d != "8b" && arr_d != "16b" {
        return Err(format!("neon rbit: unsupported arrangement .{}, expected .8b or .16b", arr_d));
    }
    let q: u32 = if arr_d == "16b" { 1 } else { 0 };
    // RBIT Vd.T, Vn.T: 0 Q 1 01110 01 10000 00101 10 Rn Rd
    let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (0b01 << 22)
        | (0b10000 << 17) | (0b00101 << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON MVNI (move NOT immediate) ───────────────────────────────────────

/// Encode NEON MVNI Vd.T, #imm (move bitwise NOT immediate to vector).
pub(crate) fn encode_neon_mvni(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("mvni requires 2 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let imm = get_imm(operands, 1)?;
    let imm8 = imm as u32 & 0xFF;

    // Extract abc:defgh for encoding
    let abc = (imm8 >> 5) & 0x7;
    let defgh = imm8 & 0x1f;

    match arr_d.as_str() {
        "2s" | "4s" => {
            let q: u32 = if arr_d == "4s" { 1 } else { 0 };
            // Check for optional shift
            let cmode = if let Some(Operand::Shift { kind, amount }) = operands.get(2) {
                if kind.to_lowercase() == "lsl" {
                    match *amount {
                        0 => 0b0000u32,
                        8 => 0b0010,
                        16 => 0b0100,
                        24 => 0b0110,
                        _ => return Err(format!("mvni: unsupported shift amount: {}", amount)),
                    }
                } else if kind.to_lowercase() == "msl" {
                    match *amount {
                        8 => 0b1100u32,
                        16 => 0b1101,
                        _ => return Err(format!("mvni: unsupported MSL shift: {}", amount)),
                    }
                } else {
                    0b0000
                }
            } else {
                0b0000
            };
            // MVNI: 0 Q 1 0 1111 00 abc cmode 01 defgh Rd  (op=1)
            let word = (q << 30) | (1 << 29) | (0b0111100 << 22)
                | (abc << 16) | (cmode << 12) | (0b01 << 10) | (defgh << 5) | rd;
            Ok(EncodeResult::Word(word))
        }
        "4h" | "8h" => {
            let q: u32 = if arr_d == "8h" { 1 } else { 0 };
            // MVNI 16-bit: cmode=1000, op=1
            let word = (q << 30) | (1 << 29) | (0b0111100 << 22)
                | (abc << 16) | (0b1000 << 12) | (0b01 << 10) | (defgh << 5) | rd;
            Ok(EncodeResult::Word(word))
        }
        _ => Err(format!("mvni: unsupported arrangement: {}", arr_d)),
    }
}

// ── NEON float three-same ────────────────────────────────────────────────
/// Encode NEON float three-same: FADD, FSUB, FMUL, FDIV, FMLA, FMLS, etc.
/// Format: 0 Q U 01110 size 1 Rm opcode 1 Rn Rd
/// size[1]=size_hi (0 or 1), size[0]=sz (0=single, 1=double)
pub(crate) fn encode_neon_float_three_same(operands: &[Operand], u_bit: u32, size_hi: u32, opcode: u32) -> Result<EncodeResult, String> {
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;
    let (q, sz) = match arr_d.as_str() {
        "2s" => (0u32, 0u32), "4s" => (1, 0), "2d" => (1, 1),
        _ => return Err(format!("float three-same: unsupported arrangement: {}", arr_d)),
    };
    let size = (size_hi << 1) | sz;
    let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22) | (1 << 21)
        | (rm << 16) | (opcode << 11) | (1 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON two-register misc (integer) ─────────────────────────────────────
/// Encode NEON two-reg misc: ABS, NEG, CLS, CLZ, etc.
/// Format: 0 Q U 01110 size 10000 opcode 10 Rn Rd
pub(crate) fn encode_neon_two_misc(operands: &[Operand], u_bit: u32, opcode: u32) -> Result<EncodeResult, String> {
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (q, size) = neon_arr_to_q_size(&arr_d)?;
    let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22)
        | (0b10000 << 17) | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON float two-register misc ─────────────────────────────────────────
/// Encode NEON float two-reg misc: UCVTF, SCVTF, FCVTZS, FCVTZU, FNEG, FABS, etc. (vector)
/// Format: 0 Q U 01110 size 10000 opcode 10 Rn Rd
/// size[1]=size_hi, size[0]=sz (0=single, 1=double)
pub(crate) fn encode_neon_float_two_misc(operands: &[Operand], u_bit: u32, size_hi: u32, opcode: u32) -> Result<EncodeResult, String> {
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (q, sz) = match arr_d.as_str() {
        "2s" => (0u32, 0u32), "4s" => (1, 0), "2d" => (1, 1),
        _ => return Err(format!("float two-misc: unsupported arrangement: {}", arr_d)),
    };
    let size = (size_hi << 1) | sz;
    let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22)
        | (0b10000 << 17) | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON shift right narrow (SHRN/RSHRN) ─────────────────────────────────
/// Format: 0 Q 0 01111 0 immh immb opcode 1 Rn Rd
/// SHRN opcode=10000, RSHRN opcode=10001
pub(crate) fn encode_neon_shrn(operands: &[Operand], opcode: u32, is_high: bool) -> Result<EncodeResult, String> {
    if operands.len() < 3 { return Err("shrn/rshrn requires 3 operands".to_string()); }
    let (rd, _) = get_neon_reg(operands, 0)?;
    let (rn, arr_n) = get_neon_reg(operands, 1)?;
    let shift = get_imm(operands, 2)? as u32;
    let element_bits = match arr_n.as_str() { "8h" => 16u32, "4s" => 32, "2d" => 64,
        _ => return Err(format!("shrn: unsupported source: {}", arr_n)), };
    let half_bits = element_bits / 2;
    if shift == 0 || shift > half_bits { return Err(format!("shrn: shift {} out of range", shift)); }
    let immhb = element_bits - shift;
    let q = if is_high { 1u32 } else { 0 };
    let word = (q << 30) | (0b011110 << 23) | ((immhb >> 3) << 19) | ((immhb & 7) << 16)
        | (opcode << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON shift right accumulate (SSRA/USRA/SRSHR/URSHR) ─────────────────
/// Format: 0 Q U 01111 0 immh immb opcode 1 Rn Rd
pub(crate) fn encode_neon_shift_right(operands: &[Operand], u_bit: u32, opcode: u32) -> Result<EncodeResult, String> {
    if operands.len() < 3 { return Err("shift-right requires 3 operands".to_string()); }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let shift = get_imm(operands, 2)? as u32;
    let (q, _) = neon_arr_to_q_size(&arr_d)?;
    let element_bits: u32 = match arr_d.as_str() {
        "8b" | "16b" => 8, "4h" | "8h" => 16, "2s" | "4s" => 32, "2d" => 64,
        _ => return Err(format!("shift-right: unsupported: {}", arr_d)), };
    if shift == 0 || shift > element_bits { return Err(format!("shift {} out of range", shift)); }
    let immhb = (element_bits * 2) - shift;
    let word = (q << 30) | (u_bit << 29) | (0b011110 << 23) | ((immhb >> 3) << 19) | ((immhb & 7) << 16)
        | (opcode << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON SSHLL/USHLL (shift left long) ───────────────────────────────────
/// Format: 0 Q U 011110 immh immb 10100 1 Rn Rd
pub(crate) fn encode_neon_shll(operands: &[Operand], u_bit: u32, is_high: bool) -> Result<EncodeResult, String> {
    if operands.len() < 3 { return Err("sshll/ushll requires 3 operands".to_string()); }
    let (rd, _) = get_neon_reg(operands, 0)?;
    let (rn, arr_n) = get_neon_reg(operands, 1)?;
    let shift = get_imm(operands, 2)? as u32;
    let base_val = match arr_n.as_str() {
        "8b" | "16b" => 8u32, "4h" | "8h" => 16, "2s" | "4s" => 32,
        _ => return Err(format!("sshll/ushll: unsupported source: {}", arr_n)), };
    let immhb = base_val + shift;
    let q = if is_high { 1u32 } else { 0 };
    let word = (q << 30) | (u_bit << 29) | (0b011110 << 23) | ((immhb >> 3) << 19) | ((immhb & 7) << 16)
        | (0b101001 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON pairwise add (UADDLP/SADDLP/UADALP/SADALP) ────────────────────

// ── NEON three-different extras: UABAL/SABAL/ADDHN/RADDHN/SUBHN/RSUBHN ──
// Already have encode_neon_three_diff which handles these opcodes.

// ── NEON SQXTUN ──────────────────────────────────────────────────────────
// Two-reg misc with U=1, opcode=10010. Reuse encode_neon_two_misc_narrow.

// ── NEON shift right narrow saturating (SQSHRN/UQSHRN/SQRSHRN/UQRSHRN) ─
pub(crate) fn encode_neon_qshrn(operands: &[Operand], u_bit: u32, is_rounding: bool, is_high: bool) -> Result<EncodeResult, String> {
    if operands.len() < 3 { return Err("qshrn requires 3 operands".to_string()); }
    let (rd, _) = get_neon_reg(operands, 0)?;
    let (rn, arr_n) = get_neon_reg(operands, 1)?;
    let shift = get_imm(operands, 2)? as u32;
    let element_bits = match arr_n.as_str() { "8h" => 16u32, "4s" => 32, "2d" => 64,
        _ => return Err(format!("qshrn: unsupported source: {}", arr_n)), };
    if shift == 0 || shift > element_bits { return Err(format!("qshrn: shift {} out of range for {}-bit elements", shift, element_bits)); }
    let immhb = element_bits - shift;
    let q = if is_high { 1u32 } else { 0 };
    let opcode_bits: u32 = if is_rounding { 0b100111 } else { 0b100101 };
    let word = (q << 30) | (u_bit << 29) | (0b011110 << 23) | ((immhb >> 3) << 19) | ((immhb & 7) << 16)
        | (opcode_bits << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON ADDHN/RADDHN/SUBHN/RSUBHN ──────────────────────────────────────
/// Three-different narrowing high: Format: 0 Q U 01110 size 1 Rm opcode 00 Rn Rd
pub(crate) fn encode_neon_three_diff_narrow(operands: &[Operand], u_bit: u32, opcode: u32, is_high: bool) -> Result<EncodeResult, String> {
    if operands.len() < 3 { return Err("addhn/subhn requires 3 operands".to_string()); }
    let (rd, _) = get_neon_reg(operands, 0)?;
    let (rn, arr_n) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;
    let size = match arr_n.as_str() { "8h" => 0b00u32, "4s" => 0b01, "2d" => 0b10,
        _ => return Err(format!("addhn: unsupported source: {}", arr_n)), };
    let q = if is_high { 1u32 } else { 0 };
    let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22) | (1 << 21)
        | (rm << 16) | (opcode << 12) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON LD2R/LD3R/LD4R ──────────────────────────────────────────────────
pub(crate) fn encode_neon_ldnr(operands: &[Operand], num_structs: u32) -> Result<EncodeResult, String> {
    if operands.len() < 2 { return Err(format!("ld{}r requires 2 operands", num_structs)); }
    let (rt, arr, num_regs) = match &operands[0] {
        Operand::RegList(regs) => {
            let (first_reg, arrangement) = match &regs[0] {
                Operand::RegArrangement { reg, arrangement } =>
                    (parse_reg_num(reg).ok_or("invalid reg")?, arrangement.clone()),
                _ => return Err("expected RegArrangement in list".to_string()),
            };
            (first_reg, arrangement, regs.len() as u32)
        }
        _ => return Err("expected register list".to_string()),
    };
    if num_regs != num_structs { return Err(format!("ld{}r: expected {} regs, got {}", num_structs, num_structs, num_regs)); }
    let (q, size) = match arr.as_str() {
        "8b" => (0u32, 0b00u32), "16b" => (1, 0b00),
        "4h" => (0, 0b01), "8h" => (1, 0b01),
        "2s" => (0, 0b10), "4s" => (1, 0b10),
        "1d" => (0, 0b11), "2d" => (1, 0b11),
        _ => return Err(format!("ld{}r: unsupported arrangement: {}", num_structs, arr)),
    };
    // opcode: ld1r=110, ld2r=110(S=1), ld3r=111, ld4r=111(S=1)
    let (opcode, s_bit) = match num_structs {
        1 => (0b110u32, 0u32),
        2 => (0b110, 1),
        3 => (0b111, 0),
        4 => (0b111, 1),
        _ => return Err(format!("unsupported: ld{}r", num_structs)),
    };
    let base = match &operands[1] {
        Operand::Mem { base, .. } => parse_reg_num(base).ok_or("invalid base")?,
        Operand::MemPostIndex { base, .. } => parse_reg_num(base).ok_or("invalid base")?,
        _ => return Err("expected memory operand".to_string()),
    };
    // check for post-index
    let rm = match &operands[1] {
        Operand::MemPostIndex { .. } => 0b11111u32, // immediate post-index
        _ => 0u32,
    };
    let has_post = rm != 0;
    let word = (q << 30) | (0b001101 << 24) | (if has_post { 1u32 } else { 0 } << 23)
        | (1 << 22) | (if has_post { rm } else { 0 } << 16) | (opcode << 13) | (s_bit << 12) | (size << 10) | (base << 5) | rt;
    Ok(EncodeResult::Word(word))
}

// ── NEON float compare-to-zero ───────────────────────────────────────────
/// FCMEQ/FCMLE/FCMLT/FCMGE/FCMGT to zero
/// Format: 0 Q U 01110 size 10000 opcode 10 Rn Rd (float, size = 0sz)
pub(crate) fn encode_neon_float_cmp_zero(operands: &[Operand], u_bit: u32, size_hi: u32, opcode: u32) -> Result<EncodeResult, String> {
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (q, sz) = match arr_d.as_str() {
        "2s" => (0u32, 0u32), "4s" => (1, 0), "2d" => (1, 1),
        _ => return Err(format!("float cmp zero: unsupported: {}", arr_d)),
    };
    let size = (size_hi << 1) | sz;
    let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22)
        | (0b10000 << 17) | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON by-element (non-long) ───────────────────────────────────────────
/// MUL/MLA/MLS by element: 0 Q U 01111 size L M Rm opcode H 0 Rn Rd
pub(crate) fn encode_neon_elem(operands: &[Operand], u_bit: u32, opcode: u32) -> Result<EncodeResult, String> {
    if operands.len() < 3 { return Err("NEON by-element requires 3 operands".to_string()); }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, index) = match &operands[2] {
        Operand::RegLane { reg, index, .. } => (parse_reg_num(reg).ok_or("invalid reg")?, *index),
        _ => return Err(format!("expected register lane, got {:?}", operands[2])),
    };
    let (q, size) = neon_arr_to_q_size(&arr_d)?;
    let (h, l, m_bit) = match size {
        0b01 => ((index >> 2) & 1, (index >> 1) & 1, index & 1),
        0b10 => ((index >> 1) & 1, index & 1, (rm >> 4) & 1),
        _ => return Err("unsupported element size for by-element".to_string()),
    };
    let rm_enc = if size == 0b01 { rm & 0xF } else { rm & 0x1F };
    let word = (q << 30) | (u_bit << 29) | (0b01111 << 24) | (size << 22)
        | (l << 21) | (m_bit << 20) | (rm_enc << 16) | (opcode << 12)
        | (h << 11) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON float by-element ────────────────────────────────────────────────
pub(crate) fn encode_neon_float_elem(operands: &[Operand], u_bit: u32, opcode: u32) -> Result<EncodeResult, String> {
    if operands.len() < 3 { return Err("NEON float by-element requires 3 operands".to_string()); }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, index) = match &operands[2] {
        Operand::RegLane { reg, index, .. } => (parse_reg_num(reg).ok_or("invalid reg")?, *index),
        _ => return Err(format!("expected register lane, got {:?}", operands[2])),
    };
    let (q, sz) = match arr_d.as_str() {
        "2s" => (0u32, 0u32), "4s" => (1, 0), "2d" => (1, 1),
        _ => return Err(format!("float by-element: unsupported: {}", arr_d)),
    };
    let (h, l, m_bit) = if sz == 0 {
        ((index >> 1) & 1, index & 1, (rm >> 4) & 1)
    } else {
        (index & 1, 0u32, (rm >> 4) & 1)
    };
    let rm_enc = rm & 0x1F;
    let word = (q << 30) | (u_bit << 29) | (0b01111 << 24) | (sz << 22)
        | (l << 21) | (m_bit << 20) | (rm_enc << 16) | (opcode << 12)
        | (h << 11) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON FCVTL/FCVTN ────────────────────────────────────────────────────
/// FCVTL: half→single or single→double widening float convert
/// Format: 0 Q 0 01110 0 sz 10000 10111 10 Rn Rd
pub(crate) fn encode_neon_fcvtl(operands: &[Operand], is_high: bool) -> Result<EncodeResult, String> {
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let sz = match arr_d.as_str() { "4s" | "2s" => 0u32, "2d" => 1,
        _ => return Err(format!("fcvtl: unsupported dest: {}", arr_d)), };
    let q = if is_high { 1u32 } else { 0 };
    let word = (q << 30) | (0b01110 << 24) | (sz << 22) | (0b10000 << 17)
        | (0b10111 << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// FCVTN: single→half or double→single narrowing float convert
pub(crate) fn encode_neon_fcvtn(operands: &[Operand], is_high: bool) -> Result<EncodeResult, String> {
    let (rd, _) = get_neon_reg(operands, 0)?;
    let (rn, arr_n) = get_neon_reg(operands, 1)?;
    let sz = match arr_n.as_str() { "4s" | "2s" => 0u32, "2d" => 1,
        _ => return Err(format!("fcvtn: unsupported source: {}", arr_n)), };
    let q = if is_high { 1u32 } else { 0 };
    let word = (q << 30) | (0b01110 << 24) | (sz << 22) | (0b10000 << 17)
        | (0b10110 << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── BIT/BIF (bitwise insert if true/false) ──────────────────────────────
/// Encodes BIT (size=10) and BIF (size=11) instructions.
/// Same format as BSL but with different size field.
/// Format: 0 Q 1 01110 ss 1 Rm 000111 Rn Rd
pub(crate) fn encode_neon_bitwise_insert(operands: &[Operand], size: u32) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("bit/bif requires 3 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;
    let q: u32 = if arr_d == "16b" { 1 } else { 0 };
    let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (size << 22) | (1 << 21)
        | (rm << 16) | (0b000111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── FADDP (float pairwise add) ──────────────────────────────────────────
/// FADDP — float add pairwise
/// Vector form: FADDP Vd.T, Vn.T, Vm.T
///   Format: 0 Q 1 01110 0 sz 1 Rm 110101 Rn Rd
/// Scalar form: FADDP Sd, Vn.2S  or FADDP Dd, Vn.2D
///   Format: 01 1 11110 0 sz 11000 01101 10 Rn Rd
pub(crate) fn encode_neon_faddp(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() >= 3 {
        // Vector form: 3 operands
        let (rd, arr_d) = get_neon_reg(operands, 0)?;
        let (rn, _) = get_neon_reg(operands, 1)?;
        let (rm, _) = get_neon_reg(operands, 2)?;
        let (q, sz) = match arr_d.as_str() {
            "2s" => (0u32, 0u32),
            "4s" => (1, 0),
            "2d" => (1, 1),
            _ => return Err(format!("faddp: unsupported arrangement: {}", arr_d)),
        };
        let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (sz << 22) | (1 << 21)
            | (rm << 16) | (0b110101 << 10) | (rn << 5) | rd;
        Ok(EncodeResult::Word(word))
    } else if operands.len() == 2 {
        // Scalar form: FADDP Sd, Vn.2S or FADDP Dd, Vn.2D
        let rd = match &operands[0] {
            Operand::Reg(r) => parse_reg_num(r).ok_or("invalid dest reg")?,
            _ => return Err("faddp scalar: expected register".to_string()),
        };
        let (rn, arr_n) = get_neon_reg(operands, 1)?;
        let sz = match arr_n.as_str() {
            "2s" => 0u32,
            "2d" => 1,
            _ => return Err(format!("faddp scalar: unsupported source: {}", arr_n)),
        };
        // 01 1 11110 0 sz 11000 01101 10 Rn Rd
        let word = (0b01 << 30) | (1 << 29) | (0b11110 << 24) | (sz << 22)
            | (0b11000 << 17) | (0b01101 << 12) | (0b10 << 10) | (rn << 5) | rd;
        Ok(EncodeResult::Word(word))
    } else {
        Err("faddp requires 2 or 3 operands".to_string())
    }
}

// ── SADDLV/UADDLV (signed/unsigned add long across vector) ─────────────
/// Format: 0 Q U 01110 size 11000 00011 10 Rn Rd
pub(crate) fn encode_neon_across_long(operands: &[Operand], u: u32, opcode: u32) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("saddlv/uaddlv requires 2 operands".to_string());
    }
    // Destination is a scalar register (e.g., s16), source is a vector arrangement
    let rd = match &operands[0] {
        Operand::Reg(r) => parse_reg_num(r).ok_or("invalid dest reg")?,
        Operand::RegArrangement { reg, .. } => parse_reg_num(reg).ok_or("invalid dest reg")?,
        _ => return Err("saddlv: expected register".to_string()),
    };
    let (rn, arr_n) = get_neon_reg(operands, 1)?;
    let (q, size) = neon_arr_to_q_size(&arr_n)?;
    let word = (q << 30) | (u << 29) | (0b01110 << 24) | (size << 22)
        | (0b11000 << 17) | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON shift left by immediate (SQSHL, UQSHL, SHL, etc.) ─────────────
/// Format: 0 Q U 011110 immh:immb opcode 1 Rn Rd
/// immh:immb encodes both the element size and the shift amount.
pub(crate) fn encode_neon_shift_left_imm(operands: &[Operand], u: u32, opcode: u32) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("shift left immediate requires 3 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let shift = get_imm(operands, 2)? as u32;

    let (q, _immh_base, esize) = match arr_d.as_str() {
        "8b" => (0u32, 0b0001u32, 8u32),
        "16b" => (1, 0b0001, 8),
        "4h" => (0, 0b0010, 16),
        "8h" => (1, 0b0010, 16),
        "2s" => (0, 0b0100, 32),
        "4s" => (1, 0b0100, 32),
        "2d" => (1, 0b1000, 64),
        _ => return Err(format!("shift left imm: unsupported arrangement: {}", arr_d)),
    };

    // immh:immb = esize + shift_amount
    // For 8-bit: immh=0001, shift in 0..7 => immh:immb = 8 + shift
    // For 16-bit: immh=001x, shift in 0..15 => immh:immb = 16 + shift
    // For 32-bit: immh=01xx, shift in 0..31 => immh:immb = 32 + shift
    // For 64-bit: immh=1xxx, shift in 0..63 => immh:immb = 64 + shift
    let immhb = esize + shift;
    let immh = (immhb >> 3) & 0xF;
    let immb = immhb & 0x7;

    let word = (q << 30) | (u << 29) | (0b011110 << 23) | (immh << 19) | (immb << 16)
        | (opcode << 11) | (1 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── Helper: detect scalar d-register 3-operand NEON operations ──────────────
pub(crate) fn is_neon_scalar_d_reg_op(operands: &[Operand]) -> bool {
    if operands.len() < 3 { return false; }
    match &operands[0] {
        Operand::Reg(r) => {
            let r = r.to_lowercase();
            r.starts_with('d') && r[1..].parse::<u32>().is_ok()
        }
        _ => false,
    }
}

// ── NEON scalar three-same: ADD/SUB Dd, Dn, Dm ────────────────────────────
/// Encode scalar NEON three-same: 01 U 11110 size 1 Rm opcode 1 Rn Rd
pub(crate) fn encode_neon_scalar_three_same(operands: &[Operand], u_bit: u32, opcode: u32, size: u32) -> Result<EncodeResult, String> {
    if operands.len() < 3 { return Err("scalar three-same requires 3 operands".to_string()); }
    let rd = match &operands[0] { Operand::Reg(r) => parse_reg_num(r).ok_or("invalid reg")?, _ => return Err("expected register".to_string()) };
    let rn = match &operands[1] { Operand::Reg(r) => parse_reg_num(r).ok_or("invalid reg")?, _ => return Err("expected register".to_string()) };
    let rm = match &operands[2] { Operand::Reg(r) => parse_reg_num(r).ok_or("invalid reg")?, _ => return Err("expected register".to_string()) };
    let word = (0b01 << 30) | (u_bit << 29) | (0b11110 << 24) | (size << 22) | (1 << 21)
        | (rm << 16) | (opcode << 11) | (1 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON scalar ADDP: addp Dd, Vn.2d ──────────────────────────────────────
pub(crate) fn encode_neon_scalar_addp(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 2 { return Err("scalar addp requires 2 operands".to_string()); }
    let rd = match &operands[0] { Operand::Reg(r) => parse_reg_num(r).ok_or("invalid reg")?, _ => return Err("expected d register".to_string()) };
    let rn = match &operands[1] {
        Operand::RegArrangement { reg, arrangement } => {
            if arrangement != "2d" { return Err(format!("scalar addp requires .2d source, got .{}", arrangement)); }
            parse_reg_num(reg).ok_or("invalid reg")?
        }
        _ => return Err("scalar addp: expected Vn.2d source".to_string()),
    };
    // Scalar ADDP: 01 0 11110 11 11000 11011 10 Rn Rd
    let word = (0b01 << 30) | (0b011110 << 24) | (0b11 << 22) | (0b11000 << 17)
        | (0b11011 << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON scalar two-reg misc: SQABS/SQNEG Hd,Hn / Sd,Sn / Dd,Dn ──────────
pub(crate) fn encode_neon_scalar_two_misc(operands: &[Operand], u_bit: u32, opcode: u32) -> Result<EncodeResult, String> {
    if operands.len() < 2 { return Err("scalar two-misc requires 2 operands".to_string()); }
    let (rd, rd_name) = match &operands[0] { Operand::Reg(r) => (parse_reg_num(r).ok_or("invalid reg")?, r.to_lowercase()), _ => return Err("expected register".to_string()) };
    let rn = match &operands[1] { Operand::Reg(r) => parse_reg_num(r).ok_or("invalid reg")?, _ => return Err("expected register".to_string()) };
    let size = if rd_name.starts_with('b') { 0b00u32 }
        else if rd_name.starts_with('h') { 0b01 }
        else if rd_name.starts_with('s') { 0b10 }
        else if rd_name.starts_with('d') { 0b11 }
        else { return Err(format!("scalar two-misc: unsupported register type: {}", rd_name)); };
    // 01 U 11110 size 10000 opcode 10 Rn Rd
    let word = (0b01 << 30) | (u_bit << 29) | (0b11110 << 24) | (size << 22)
        | (0b10000 << 17) | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON scalar SQSHRN: sqshrn Hd,Sn,#shift / sqshrn Sd,Dn,#shift ────────
pub(crate) fn encode_neon_scalar_qshrn(operands: &[Operand], u_bit: u32, is_rounding: bool) -> Result<EncodeResult, String> {
    if operands.len() < 3 { return Err("scalar qshrn requires 3 operands".to_string()); }
    let (rd, rd_name) = match &operands[0] { Operand::Reg(r) => (parse_reg_num(r).ok_or("invalid reg")?, r.to_lowercase()), _ => return Err("expected register".to_string()) };
    let rn = match &operands[1] { Operand::Reg(r) => parse_reg_num(r).ok_or("invalid reg")?, _ => return Err("expected register".to_string()) };
    let shift = get_imm(operands, 2)? as u32;
    // Determine element bits from destination register type
    let element_bits = if rd_name.starts_with('b') { 8u32 }  // b <- h (narrow from 16-bit)
        else if rd_name.starts_with('h') { 16 }  // h <- s (narrow from 32-bit), immh base = 16
        else if rd_name.starts_with('s') { 32 }  // s <- d (narrow from 64-bit), immh base = 32
        else { return Err(format!("scalar qshrn: unsupported dest: {}", rd_name)); };
    if shift == 0 || shift > element_bits { return Err(format!("scalar qshrn: shift {} out of range", shift)); }
    let immhb = (element_bits * 2) - shift;  // source element bits - shift
    let opcode_bits: u32 = if is_rounding { 0b100111 } else { 0b100101 };
    // 01 U 11110 immh:immb opcode 1 Rn Rd
    let word = (0b01 << 30) | (u_bit << 29) | (0b011110 << 23) | ((immhb >> 3) << 19) | ((immhb & 7) << 16)
        | (opcode_bits << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

// ── NEON addp (integer pairwise add) — already handled in three-same as addp ──

#[cfg(test)]
mod scalar_qshrn_pbt_tests {
    use super::*;
    use proptest::prelude::*;
    use std::io::Write;
    use std::process::{Command, Stdio};

    /// (u_bit, is_rounding) -> reference mnemonic accepted by llvm-mc.
    fn mnemonic(u_bit: u32, is_rounding: bool) -> &'static str {
        match (u_bit, is_rounding) {
            (0, false) => "sqshrn",
            (0, true) => "sqrshrn",
            (1, false) => "uqshrn",
            (1, true) => "uqrshrn",
            _ => unreachable!(),
        }
    }

    /// dest scalar letter -> (dest element bits, source scalar letter, valid shift max).
    /// b<-h (narrow 16->8), h<-s (32->16), s<-d (64->32).
    fn dest_info(dest: &str) -> (u32, char, u32) {
        match dest {
            "b" => (8, 'h', 8),
            "h" => (16, 's', 16),
            "s" => (32, 'd', 32),
            _ => unreachable!(),
        }
    }

    fn run(u_bit: u32, is_rounding: bool, dest: &str, dest_num: u32, src_num: u32, shift: i64) -> Result<u32, String> {
        let (_, src_letter, _) = dest_info(dest);
        let ops = vec![
            Operand::Reg(format!("{}{}", dest, dest_num)),
            Operand::Reg(format!("{}{}", src_letter, src_num)),
            Operand::Imm(shift),
        ];
        match encode_neon_scalar_qshrn(&ops, u_bit, is_rounding) {
            Ok(EncodeResult::Word(w)) => Ok(w),
            Ok(other) => Err(format!("unexpected non-Word result: {:?}", other)),
            Err(e) => Err(e),
        }
    }

    /// Differential oracle: assemble a single instruction with llvm-mc-18 and
    /// return its little-endian 32-bit encoding, or None if unavailable/rejected.
    fn llvm_mc_encode(mnem: &str, dest: &str, dest_num: u32, src_letter: char, src_num: u32, shift: u32) -> Option<u32> {
        let text = format!("{} {}{}, {}{}, #{}\n", mnem, dest, dest_num, src_letter, src_num, shift);
        let mut child = Command::new("llvm-mc-18")
            .args(["--triple=aarch64", "--assemble", "--show-encoding"])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().ok()?;
        {
            let mut stdin = child.stdin.take()?;
            stdin.write_all(text.as_bytes()).ok()?;
        }
        let output = child.wait_with_output().ok()?;
        if !output.status.success() {
            return None; // llvm rejected the operand combination
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout.lines().find(|l| l.contains("encoding:"))?;
        let bytes_str = line.split('[').nth(1)?.split(']').next()?;
        let bytes: Vec<u8> = bytes_str
            .split(',')
            .map(|s| s.trim().trim_start_matches("0x"))
            .filter_map(|s| u8::from_str_radix(s, 16).ok())
            .collect();
        if bytes.len() != 4 {
            return None;
        }
        Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(96))]

        // 1. Differential oracle against llvm-mc. Catches any encoding deviation
        //    from the reference ISA assembler for valid operand combinations.
        #[test]
        #[ignore = "documented bug: scalar qshrn misses fixed bit 28"]
        fn prop_matches_llvm_mc(
            u_bit in 0u32..2u32,
            is_rounding in any::<bool>(),
            dest_idx in 0usize..3usize,
            dest_num in 0u32..32u32,
            src_num in 0u32..32u32,
            shift in 1u32..=32u32,
        ) {
            let dests = ["b", "h", "s"];
            let dest = dests[dest_idx];
            let (_, src_letter, max_shift) = dest_info(dest);
            let shift = (shift % max_shift) + 1; // clamp into [1, max_shift]

            let word = run(u_bit, is_rounding, dest, dest_num, src_num, shift as i64)
                .expect("valid input must encode");

            if let Some(refw) = llvm_mc_encode(mnemonic(u_bit, is_rounding), dest, dest_num, src_letter, src_num, shift) {
                prop_assert_eq!(
                    word, refw,
                    "mismatch {} {}{}, {}{}, #{}",
                    mnemonic(u_bit, is_rounding), dest, dest_num, src_letter, src_num, shift
                );
            }
        }

        // 2. ISA-fixed high bits for scalar shift-by-immediate.
        //    bits 31-30 == 0b01 and bit 28 == 1 are hard-wired in the encoding.
        #[test]
        #[ignore = "documented bug: scalar qshrn misses fixed bit 28"]
        fn prop_fixed_high_bits(
            dest_idx in 0usize..3usize,
            dest_num in 0u32..32u32,
            src_num in 0u32..32u32,
            shift in 1u32..=32u32,
            u_bit in 0u32..2u32,
            is_rounding in any::<bool>(),
        ) {
            let dests = ["b", "h", "s"];
            let dest = dests[dest_idx];
            let (_, _, max_shift) = dest_info(dest);
            let shift = (shift % max_shift) + 1;
            let word = run(u_bit, is_rounding, dest, dest_num, src_num, shift as i64).unwrap();
            prop_assert_eq!((word >> 30) & 0b11, 0b01u32, "bits 31-30 (scalar)");
            // Bit 28 is a fixed '1' for Advanced SIMD scalar shift by immediate.
            prop_assert_eq!((word >> 28) & 1, 1u32, "bit 28 must be 1; word=0x{:08x}", word);
        }

        // 3. Rd (bits 4-0) and Rn (bits 9-5) fields preserve the register numbers.
        #[test]
        fn prop_reg_fields_preserved(
            dest_idx in 0usize..3usize,
            dest_num in 0u32..32u32,
            src_num in 0u32..32u32,
            shift in 1u32..=32u32,
            u_bit in 0u32..2u32,
            is_rounding in any::<bool>(),
        ) {
            let dests = ["b", "h", "s"];
            let dest = dests[dest_idx];
            let (_, _, max_shift) = dest_info(dest);
            let shift = (shift % max_shift) + 1;
            let word = run(u_bit, is_rounding, dest, dest_num, src_num, shift as i64).unwrap();
            prop_assert_eq!(word & 0x1F, dest_num, "Rd field");
            prop_assert_eq!((word >> 5) & 0x1F, src_num, "Rn field");
        }

        // 4. immh:immb (bits 22-16) encodes (source_element_bits - shift).
        #[test]
        fn prop_immh_immb_value(
            dest_idx in 0usize..3usize,
            shift in 1u32..=32u32,
            u_bit in 0u32..2u32,
            is_rounding in any::<bool>(),
        ) {
            let dests = ["b", "h", "s"];
            let dest = dests[dest_idx];
            let (ebits, _, max_shift) = dest_info(dest);
            let shift = (shift % max_shift) + 1;
            let word = run(u_bit, is_rounding, dest, 0, 0, shift as i64).unwrap();
            let immh_immb = (word >> 16) & 0x7F;
            prop_assert_eq!(immh_immb, ebits * 2 - shift, "immh:immb for {} shift {}", dest, shift);
        }

        // 5. U bit (bit 29) and opcode (bits 15-11) reflect the flags; bit 10 == 1.
        #[test]
        fn prop_u_bit_and_opcode(
            dest_idx in 0usize..3usize,
            shift in 1u32..=32u32,
            u_bit in 0u32..2u32,
            is_rounding in any::<bool>(),
        ) {
            let dests = ["b", "h", "s"];
            let dest = dests[dest_idx];
            let (_, _, max_shift) = dest_info(dest);
            let shift = (shift % max_shift) + 1;
            let word = run(u_bit, is_rounding, dest, 0, 0, shift as i64).unwrap();
            prop_assert_eq!((word >> 29) & 1, u_bit, "U bit");
            prop_assert_eq!((word >> 10) & 1, 1u32, "fixed bit 10");
            let opcode = (word >> 11) & 0x1F;
            let expected = if is_rounding { 0b10011u32 } else { 0b10010u32 };
            prop_assert_eq!(opcode, expected, "opcode (rounding={})", is_rounding);
        }

        // 6. Negative contract: shift==0 or shift>dest_element_bits is rejected;
        //    shifts inside the valid window are accepted.
        #[test]
        fn prop_rejects_out_of_range_shift(
            dest_idx in 0usize..3usize,
            shift in proptest::sample::select(vec![0u32, 1u32, 8u32, 9u32, 16u32, 17u32, 32u32, 33u32, 64u32]),
            u_bit in 0u32..2u32,
            is_rounding in any::<bool>(),
        ) {
            let dests = ["b", "h", "s"];
            let dest = dests[dest_idx];
            let (ebits, src_letter, _) = dest_info(dest);
            let ops = vec![
                Operand::Reg(format!("{}0", dest)),
                Operand::Reg(format!("{}0", src_letter)),
                Operand::Imm(shift as i64),
            ];
            let res = encode_neon_scalar_qshrn(&ops, u_bit, is_rounding);
            let should_err = shift == 0 || shift > ebits;
            prop_assert_eq!(res.is_err(), should_err, "dest={} shift={}", dest, shift);
        }

        // 7. Negative contract: unsupported destination scalar type is rejected.
        #[test]
        fn prop_rejects_bad_dest_type(
            bad_dest in proptest::sample::select(vec!["d0", "q0", "v0", "x0", "w0"]),
            shift in 1u32..=8u32,
        ) {
            let ops = vec![
                Operand::Reg(bad_dest.to_string()),
                Operand::Reg("h0".to_string()),
                Operand::Imm(shift as i64),
            ];
            let res = encode_neon_scalar_qshrn(&ops, 0, false);
            prop_assert!(res.is_err(), "expected Err for dest {}", bad_dest);
        }
    }
}

#[cfg(test)]
mod movi_pbt_tests {
    use super::*;
    use proptest::prelude::*;

    // Arrangements where MOVI accepts any 8-bit immediate.
    const SIMPLE_ARRS: &[&str] = &["8b", "16b", "4h", "8h", "2s", "4s"];
    const WIDE_SIMPLE: &[&str] = &["16b", "4s", "8h"];

    fn movi_ops(rd: u32, arr: &str, imm: i64) -> Vec<Operand> {
        vec![
            Operand::RegArrangement { reg: format!("v{}", rd), arrangement: arr.to_string() },
            Operand::Imm(imm),
        ]
    }

    fn encode(rd: u32, arr: &str, imm: i64) -> Result<u32, String> {
        match encode_neon_movi(&movi_ops(rd, arr, imm)) {
            Ok(EncodeResult::Word(w)) => Ok(w),
            Ok(other) => Err(format!("unexpected non-Word result: {:?}", other)),
            Err(e) => Err(e),
        }
    }

    // Reconstruct the encoded 8-bit immediate: abc at bits 18-16, defgh at bits 9-5.
    fn reconstruct_imm8(word: u32) -> u32 {
        let abc = (word >> 16) & 0x7;
        let defgh = (word >> 5) & 0x1F;
        (abc << 5) | defgh
    }

    proptest! {
        // 1. The Rd field (bits 4-0) always equals the source register number.
        #[test]
        fn prop_rd_field_preserved(rd in 0u32..32u32, imm8 in 0u32..256u32) {
            for &arr in SIMPLE_ARRS {
                let word = encode(rd, arr, imm8 as i64).expect("encode should succeed");
                prop_assert_eq!(word & 0x1F, rd, "Rd mismatch for arr {}", arr);
            }
        }

        // 2. The 8-bit immediate round-trips through abc/defgh for every simple form.
        #[test]
        fn prop_imm8_roundtrip(rd in 0u32..32u32, imm8 in 0u32..256u32) {
            for &arr in SIMPLE_ARRS {
                let word = encode(rd, arr, imm8 as i64).expect("encode should succeed");
                prop_assert_eq!(reconstruct_imm8(word), imm8, "imm8 roundtrip for arr {}", arr);
            }
        }

        // 3. Fixed ISA fields are correct per arrangement: Q bit, bit31==0, cmode.
        #[test]
        fn prop_fixed_fields_per_arrangement(rd in 0u32..32u32, imm8 in 0u32..256u32) {
            for &arr in SIMPLE_ARRS {
                let word = encode(rd, arr, imm8 as i64).expect("encode should succeed");
                // bit 31 is always 0 for AArch64 MOVI.
                prop_assert_eq!((word >> 31) & 1, 0u32, "bit31 for arr {}", arr);
                // Q (bit 30) selects the wide register arrangement.
                let expected_q = if WIDE_SIMPLE.contains(&arr) { 1u32 } else { 0u32 };
                prop_assert_eq!((word >> 30) & 1, expected_q, "Q for arr {}", arr);
                // cmode (bits 15-12) is arrangement-dependent.
                let expected_cmode: u32 = match arr {
                    "8b" | "16b" => 0b1110,
                    "4h" | "8h" => 0b1000,
                    "2s" | "4s" => 0b0000,
                    _ => unreachable!(),
                };
                prop_assert_eq!((word >> 12) & 0xF, expected_cmode, "cmode for arr {}", arr);
            }
        }

        // 4. .2d byte-pattern contract (differential oracle):
        //    Ok iff every byte of the 64-bit immediate is 0x00 or 0xFF;
        //    when Ok, the reconstructed imm8 equals the byte mask, and Q=op=1 (bits31-28=0110).
        #[test]
        fn prop_2d_byte_pattern_contract(v in any::<u64>(), rd in 0u32..32u32) {
            // Independent re-implementation of the validity predicate.
            let is_valid = {
                let mut x = v;
                let mut ok = true;
                while x != 0 {
                    let b = x & 0xFF;
                    if b != 0 && b != 0xFF { ok = false; break; }
                    x >>= 8;
                }
                ok
            };

            let res = encode(rd, "2d", v as i64);
            prop_assert_eq!(res.is_ok(), is_valid);

            if let Ok(word) = res {
                let mut mask = 0u32;
                for i in 0..8 {
                    if (v >> (i * 8)) & 0xFF == 0xFF {
                        mask |= 1 << i;
                    }
                }
                prop_assert_eq!(reconstruct_imm8(word), mask, "2d imm8 mask");
                // .2d is Q=1, op=1: bits 31-28 == 0110.
                prop_assert_eq!((word >> 28) & 0xF, 0b0110u32, "2d top nibble");
            }
        }

        // 5. Error contracts: too few operands, unsupported arrangements, invalid LSL shift.
        #[test]
        fn prop_error_contracts(rd in 0u32..32u32, imm8 in 0u32..256u32, amt in 1u32..32u32) {
            // Too few operands (destination only, no immediate).
            let dest_only = encode_neon_movi(&[Operand::RegArrangement {
                reg: format!("v{}", rd),
                arrangement: "8b".to_string(),
            }]);
            prop_assert!(dest_only.is_err(), "missing immediate must error");

            // Unsupported arrangements (not in the match arms).
            for &arr in &["1d", "2h", "1q"] {
                let r = encode_neon_movi(&movi_ops(rd, arr, imm8 as i64));
                prop_assert!(r.is_err(), "arrangement {} must be rejected", arr);
            }

            // .2s/.4s only accept LSL shift amounts in {0,8,16,24}; anything else errors.
            prop_assume!(!matches!(amt, 0 | 8 | 16 | 24));
            let ops = vec![
                Operand::RegArrangement { reg: format!("v{}", rd), arrangement: "2s".to_string() },
                Operand::Imm(imm8 as i64),
                Operand::Shift { kind: "lsl".to_string(), amount: amt },
            ];
            let r = encode_neon_movi(&ops);
            prop_assert!(r.is_err(), "unsupported LSL #{} must error", amt);
        }
    }
}

#[cfg(test)]
mod dup_pbt_tests {
    use super::*;
    use proptest::prelude::*;

    // Valid destination arrangements for the GP-register form of DUP
    // (these drive both the Q bit and the size imm5). NOTE: "1d" is intentionally
    // absent — DUP (general) has no .1d variant, so the encoder rejects it.
    const GP_ARRS: &[&str] = &["8b", "16b", "4h", "8h", "2s", "4s", "2d"];
    // Destination arrangements accepted for the element form (Q derived from these).
    const ELEM_DEST_ARRS: &[&str] = &["8b", "16b", "4h", "8h", "2s", "4s", "1d", "2d"];

    // Expected Q bit for a destination arrangement.
    fn expected_q(arr: &str) -> u32 {
        match arr {
            "16b" | "8h" | "4s" | "2d" => 1,
            _ => 0,
        }
    }

    // Expected imm5 size-code for the GP form per arrangement.
    fn gp_imm5_code(arr: &str) -> u32 {
        match arr {
            "8b" | "16b" => 0b00001,
            "4h" | "8h" => 0b00010,
            "2s" | "4s" => 0b00100,
            "2d" => 0b01000,
            _ => unreachable!("gp_imm5_code on {}", arr),
        }
    }

    // Build DUP Vd.T, Rn  (general register form)
    fn gp_ops(rd: u32, arr: &str, rn: u32) -> Vec<Operand> {
        vec![
            Operand::RegArrangement { reg: format!("v{}", rd), arrangement: arr.to_string() },
            Operand::Reg(format!("x{}", rn)),
        ]
    }

    // Build DUP Vd.T, Vn.Ss[index]  (element form)
    fn elem_ops(rd: u32, dest_arr: &str, rn: u32, elem_size: &str, index: u32) -> Vec<Operand> {
        vec![
            Operand::RegArrangement {
                reg: format!("v{}", rd),
                arrangement: dest_arr.to_string(),
            },
            Operand::RegLane {
                reg: format!("v{}", rn),
                elem_size: elem_size.to_string(),
                index,
            },
        ]
    }

    fn encode_word(ops: &[Operand]) -> Result<u32, String> {
        match encode_neon_dup(ops) {
            Ok(EncodeResult::Word(w)) => Ok(w),
            Ok(other) => Err(format!("unexpected non-Word: {:?}", other)),
            Err(e) => Err(e),
        }
    }

    proptest! {
        // 1. Constant ISA fields hold for every valid encoding of both forms.
        #[test]
        fn prop_fixed_fields(rd in 0u32..32u32, rn in 0u32..32u32, idx in 0u32..16u32) {
            // General-register form
            for &arr in GP_ARRS {
                let w = encode_word(&gp_ops(rd, arr, rn)).expect("gp encodes");
                prop_assert_eq!((w >> 31) & 1, 0u32, "bit31 gp {}", arr);
                prop_assert_eq!((w >> 24) & 0x1F, 0b01110u32, "opcode[28:24] gp {}", arr);
                prop_assert_eq!((w >> 21) & 0x7, 0b000u32, "bits[23:21] gp {}", arr);
                prop_assert_eq!((w >> 15) & 1, 0u32, "bit15 gp {}", arr);
                prop_assert_eq!((w >> 10) & 1, 1u32, "bit10 gp {}", arr);
            }
            // Element form
            for &dest in ELEM_DEST_ARRS {
                for &(es, max) in &[("b", 15u32), ("h", 7u32), ("s", 3u32), ("d", 1u32)] {
                    let index = idx & max;
                    let w = encode_word(&elem_ops(rd, dest, rn, es, index)).expect("elem encodes");
                    prop_assert_eq!((w >> 31) & 1, 0u32, "bit31 elem {} {}", dest, es);
                    prop_assert_eq!((w >> 24) & 0x1F, 0b01110u32, "opcode[28:24] elem {} {}", dest, es);
                    prop_assert_eq!((w >> 21) & 0x7, 0b000u32, "bits[23:21] elem {} {}", dest, es);
                    prop_assert_eq!((w >> 15) & 1, 0u32, "bit15 elem {} {}", dest, es);
                    prop_assert_eq!((w >> 10) & 1, 1u32, "bit10 elem {} {}", dest, es);
                }
            }
        }

        // 2. Rd (bits[4:0]) and Rn (bits[9:5]) always equal the source register numbers.
        #[test]
        fn prop_reg_fields(rd in 0u32..32u32, rn in 0u32..32u32, idx in 0u32..16u32) {
            for &arr in GP_ARRS {
                let w = encode_word(&gp_ops(rd, arr, rn)).expect("gp encodes");
                prop_assert_eq!(w & 0x1F, rd, "Rd gp {}", arr);
                prop_assert_eq!((w >> 5) & 0x1F, rn, "Rn gp {}", arr);
            }
            for &dest in ELEM_DEST_ARRS {
                for &(es, max) in &[("b", 15u32), ("h", 7u32), ("s", 3u32), ("d", 1u32)] {
                    let index = idx & max;
                    let w = encode_word(&elem_ops(rd, dest, rn, es, index)).expect("elem encodes");
                    prop_assert_eq!(w & 0x1F, rd, "Rd elem {} {}", dest, es);
                    prop_assert_eq!((w >> 5) & 0x1F, rn, "Rn elem {} {}", dest, es);
                }
            }
        }

        // 3. Q bit (bit30) reflects the destination arrangement for both forms.
        #[test]
        fn prop_q_bit(rd in 0u32..32u32, rn in 0u32..32u32, idx in 0u32..16u32) {
            for &arr in GP_ARRS {
                let w = encode_word(&gp_ops(rd, arr, rn)).expect("gp encodes");
                prop_assert_eq!((w >> 30) & 1, expected_q(arr), "Q gp {}", arr);
            }
            for &dest in ELEM_DEST_ARRS {
                for &(es, max) in &[("b", 15u32), ("h", 7u32), ("s", 3u32), ("d", 1u32)] {
                    let index = idx & max;
                    let w = encode_word(&elem_ops(rd, dest, rn, es, index)).expect("elem encodes");
                    prop_assert_eq!((w >> 30) & 1, expected_q(dest), "Q elem {} {}", dest, es);
                }
            }
        }

        // 4. imm5 round-trips and bit 11 distinguishes the two forms.
        #[test]
        fn prop_imm5_and_form_opcode(rd in 0u32..32u32, rn in 0u32..32u32, idx in 0u32..16u32) {
            // General-register form: opcode bits[15:10] == 0b000011, imm5 == size code.
            for &arr in GP_ARRS {
                let w = encode_word(&gp_ops(rd, arr, rn)).expect("gp encodes");
                prop_assert_eq!((w >> 10) & 0x3F, 0b000011u32, "form-opcode gp {}", arr);
                prop_assert_eq!((w >> 16) & 0x1F, gp_imm5_code(arr), "imm5 gp {}", arr);
            }
            // Element form: opcode bits[15:10] == 0b000001,
            //   imm5 == (index << sh) | size_code, and the masked index round-trips.
            for &(es, max, sh, code) in &[
                ("b", 15u32, 1u32, 0b00001u32),
                ("h", 7u32, 2u32, 0b00010u32),
                ("s", 3u32, 3u32, 0b00100u32),
                ("d", 1u32, 4u32, 0b01000u32),
            ] {
                let index = idx & max;
                let w = encode_word(&elem_ops(rd, "8b", rn, es, index)).expect("elem encodes");
                prop_assert_eq!((w >> 10) & 0x3F, 0b000001u32, "form-opcode elem {}", es);
                let imm5 = (w >> 16) & 0x1F;
                prop_assert_eq!(imm5, (index << sh) | code, "imm5 elem {} idx {}", es, index);
                // reconstruct the index from imm5 and verify round-trip
                prop_assert_eq!((imm5 - code) >> sh, index, "idx roundtrip elem {}", es);
            }
        }

        // 5. Error contracts: too few operands, bogus dest arrangement,
        //    the GP-only rejection of .1d, and unsupported element size.
        #[test]
        fn prop_error_contracts(rd in 0u32..32u32, rn in 0u32..32u32, idx in 0u32..16u32) {
            // <2 operands -> Err
            let dest_only = vec![Operand::RegArrangement {
                reg: format!("v{}", rd),
                arrangement: "8b".to_string(),
            }];
            prop_assert!(encode_word(&dest_only).is_err(), "dest-only must error");

            // GP form with unsupported dest arrangement -> Err (neon_arr_to_q_size fails)
            for &arr in &["1q", "2h", "bogus"] {
                let r = encode_word(&gp_ops(rd, arr, rn));
                prop_assert!(r.is_err(), "gp bogus arr {} must error", arr);
            }

            // GP form rejects .1d: neon_arr_to_q_size(1d) is Ok but the imm5 match has no arm.
            // DUP (general) defines no .1d variant.
            let r = encode_word(&gp_ops(rd, "1d", rn));
            prop_assert!(r.is_err(), "gp .1d must error");

            // Element form: valid dest but unsupported element size -> Err
            let r = encode_word(&elem_ops(rd, "8b", rn, "q", idx));
            prop_assert!(r.is_err(), "elem bogus elem_size must error");

            // Element form: bogus dest arrangement -> Err
            let r = encode_word(&elem_ops(rd, "1q", rn, "b", idx));
            prop_assert!(r.is_err(), "elem bogus dest arr must error");
        }
    }
}

#[cfg(test)]
mod tbl_pbt_tests {
    use super::*;
    use proptest::prelude::*;

    // Destination arrangements valid for TBL: only .8b (Q=0) and .16b (Q=1).
    const DEST_ARRS: &[&str] = &["8b", "16b"];

    // Build TBL Vd.<dest_arr>, { Vn0.<t>, Vn1.<t>, ... }, Vm.<t>.
    // The table register list is constructed from a slice of register numbers;
    // `rn_first+i` is wrapped to the 0..31 range so callers can pass large bases.
    fn tbl_ops(rd: u32, dest_arr: &str, table_regs: &[u32], t_arr: &str, rm: u32) -> Vec<Operand> {
        let regs: Vec<Operand> = table_regs
            .iter()
            .map(|r| Operand::RegArrangement {
                reg: format!("v{}", r),
                arrangement: t_arr.to_string(),
            })
            .collect();
        vec![
            Operand::RegArrangement { reg: format!("v{}", rd), arrangement: dest_arr.to_string() },
            Operand::RegList(regs),
            Operand::RegArrangement { reg: format!("v{}", rm), arrangement: t_arr.to_string() },
        ]
    }

    fn encode_word(ops: &[Operand]) -> Result<u32, String> {
        match encode_neon_tbl(ops) {
            Ok(EncodeResult::Word(w)) => Ok(w),
            Ok(other) => Err(format!("unexpected non-Word: {:?}", other)),
            Err(e) => Err(e),
        }
    }

    // ORACLE: reference encoding of TBL Vd.T, {Vn0..}, Vm.T.
    //   0 Q 00 1110 000 Rm 0 len 0 00 Rn Rd   (op bit12 = 0 for TBL)
    fn reference_word(rd: u32, dest_arr: &str, table_regs: &[u32], rm: u32) -> u32 {
        let q: u32 = if dest_arr == "16b" { 1 } else { 0 };
        let rn = table_regs[0];
        let len = ((table_regs.len() as u32) - 1) & 0x3;
        (q << 30) | (0b001110u32 << 24) | (rm << 16) | (len << 13) | (rn << 5) | rd
    }

    proptest! {
        // 1. Constant ISA fields are correct for every valid encoding,
        //    independent of registers / arrangement / table width.
        #[test]
        fn prop_fixed_fields(
            rd in 0u32..32u32,
            rm in 0u32..32u32,
            rn_first in 0u32..32u32,
            nregs in 1u32..5u32,
            dest_q in 0u32..2u32,
        ) {
            let dest_arr = if dest_q == 1 { "16b" } else { "8b" };
            let table_regs: Vec<u32> = (0..nregs).map(|i| (rn_first + i) & 0x1F).collect();
            let w = encode_word(&tbl_ops(rd, dest_arr, &table_regs, "8b", rm)).expect("encodes");
            prop_assert_eq!((w >> 31) & 1, 0u32, "bit31 must be 0");
            prop_assert_eq!((w >> 24) & 0x3F, 0b001110u32, "bits[29:24] must be 001110");
            prop_assert_eq!((w >> 21) & 0x7, 0u32, "bits[23:21] must be 000");
            prop_assert_eq!((w >> 15) & 1, 0u32, "bit15 must be 0");
            prop_assert_eq!((w >> 12) & 1, 0u32, "op bit12 must be 0 for TBL");
            prop_assert_eq!((w >> 10) & 0x3, 0u32, "bits[11:10] must be 00");
        }

        // 2. Differential oracle: the encoder output exactly matches an
        //    independent re-implementation of the TBL bit layout.
        #[test]
        fn prop_matches_reference_encoding(
            rd in 0u32..32u32,
            rm in 0u32..32u32,
            rn_first in 0u32..32u32,
            nregs in 1u32..5u32,
            dest_arr_idx in 0usize..DEST_ARRS.len(),
        ) {
            let dest_arr = DEST_ARRS[dest_arr_idx];
            let table_regs: Vec<u32> = (0..nregs).map(|i| (rn_first + i) & 0x1F).collect();
            let ops = tbl_ops(rd, dest_arr, &table_regs, "8b", rm);
            let w = encode_word(&ops).expect("encodes");
            prop_assert_eq!(w, reference_word(rd, dest_arr, &table_regs, rm));
        }

        // 3. Rd (bits[4:0]), Rn = first table reg (bits[9:5]), and Rm (bits[20:16])
        //    survive verbatim in their fields for table widths 1..=4.
        #[test]
        fn prop_register_fields_preserved(
            rd in 0u32..32u32,
            rm in 0u32..32u32,
            rn_first in 0u32..32u32,
            nregs in 1u32..5u32,
        ) {
            let table_regs: Vec<u32> = (0..nregs).map(|i| (rn_first + i) & 0x1F).collect();
            let w = encode_word(&tbl_ops(rd, "8b", &table_regs, "8b", rm)).expect("encodes");
            prop_assert_eq!(w & 0x1F, rd, "Rd");
            prop_assert_eq!((w >> 5) & 0x1F, table_regs[0], "Rn (first table reg)");
            prop_assert_eq!((w >> 16) & 0x1F, rm, "Rm");
        }

        // 4. The `len` field (bits[14:13]) encodes (num_regs - 1) for tables of 1..=4 registers.
        #[test]
        fn prop_len_field(
            nregs in 1u32..5u32,
            rd in 0u32..32u32,
            rm in 0u32..32u32,
            rn_first in 0u32..32u32,
        ) {
            let table_regs: Vec<u32> = (0..nregs).map(|i| (rn_first + i) & 0x1F).collect();
            let w = encode_word(&tbl_ops(rd, "8b", &table_regs, "8b", rm)).expect("encodes");
            prop_assert_eq!((w >> 13) & 0x3, (nregs - 1) & 0x3, "len field");
        }

        // 5. The Q bit (bit30) is 1 iff the destination arrangement is .16b.
        #[test]
        fn prop_q_bit(rd in 0u32..32u32, rm in 0u32..32u32, rn in 0u32..32u32) {
            for &(arr, expected_q) in &[("8b", 0u32), ("16b", 1u32)] {
                let w = encode_word(&tbl_ops(rd, arr, &[rn], "8b", rm)).expect("encodes");
                prop_assert_eq!((w >> 30) & 1, expected_q, "Q for arr {}", arr);
            }
        }

        // 6. Error contracts. Too-few-operands, a non-RegList second operand,
        //    and an invalid register name inside the list all return Err.
        #[test]
        fn prop_error_contracts(rd in 0u32..32u32, rm in 0u32..32u32) {
            // < 3 operands.
            let too_few = encode_neon_tbl(&[Operand::RegArrangement {
                reg: format!("v{}", rd),
                arrangement: "8b".to_string(),
            }]);
            prop_assert!(too_few.is_err(), "<3 operands must error");

            // Second operand is not a RegList.
            let not_list = vec![
                Operand::RegArrangement { reg: format!("v{}", rd), arrangement: "8b".to_string() },
                Operand::RegArrangement { reg: format!("v{}", rm), arrangement: "8b".to_string() },
                Operand::RegArrangement { reg: format!("v{}", rm), arrangement: "8b".to_string() },
            ];
            prop_assert!(encode_neon_tbl(&not_list).is_err(), "non-RegList 2nd operand must error");

            // Invalid register name inside the list (parse_reg_num returns None).
            let bad_reg = vec![
                Operand::RegArrangement { reg: format!("v{}", rd), arrangement: "8b".to_string() },
                Operand::RegList(vec![Operand::RegArrangement {
                    reg: "bogus".to_string(),
                    arrangement: "8b".to_string(),
                }]),
                Operand::RegArrangement { reg: format!("v{}", rm), arrangement: "8b".to_string() },
            ];
            prop_assert!(encode_neon_tbl(&bad_reg).is_err(), "invalid reg in list must error");
        }

        // 7. BUG: an empty table register list should return Err, but the current
        //    implementation indexes `regs[0]` and panics with index-out-of-bounds.
        //    We assert the graceful contract (no panic, returns Err).
        #[test]
        fn prop_empty_list_does_not_panic(rd in 0u32..32u32, rm in 0u32..32u32) {
            let ops = vec![
                Operand::RegArrangement { reg: format!("v{}", rd), arrangement: "8b".to_string() },
                Operand::RegList(vec![]),
                Operand::RegArrangement { reg: format!("v{}", rm), arrangement: "8b".to_string() },
            ];
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| encode_neon_tbl(&ops)));
            prop_assert!(result.is_ok(), "empty table list must not panic");
            if let Ok(res) = result {
                prop_assert!(res.is_err(), "empty table list must yield Err");
            }
        }
    }
}


#[cfg(test)]
mod ext_pbt_tests {
    use super::*;
    use proptest::prelude::*;

    // EXT Vd.T, Vn.T, Vm.T, #index
    // ARMv8 encoding: 0 Q 10 1110 00 0 Rm 0 imm4 0 Rn Rd
    //   bit 31      : 0
    //   bit 30      : Q (1 iff arrangement == "16b")
    //   bits 29-24  : 101110  (fixed)
    //   bits 23-22  : 00
    //   bit 21      : 0
    //   bits 20-16  : Rm
    //   bit 15      : 0
    //   bits 14-11  : imm4  (== index & 0xF)
    //   bit 10      : 0
    //   bits 9-5    : Rn
    //   bits 4-0    : Rd

    // Both .8b and .16b are valid; any other string silently maps to Q=0 (no validation).
    const ARRANGEMENTS: &[&str] = &["8b", "16b"];

    fn ext_ops(rd: u32, rn: u32, rm: u32, arr: &str, index: i64) -> Vec<Operand> {
        vec![
            Operand::RegArrangement { reg: format!("v{}", rd), arrangement: arr.to_string() },
            Operand::RegArrangement { reg: format!("v{}", rn), arrangement: arr.to_string() },
            Operand::RegArrangement { reg: format!("v{}", rm), arrangement: arr.to_string() },
            Operand::Imm(index),
        ]
    }

    fn encode(rd: u32, rn: u32, rm: u32, arr: &str, index: i64) -> Result<u32, String> {
        match encode_neon_ext(&ext_ops(rd, rn, rm, arr, index)) {
            Ok(EncodeResult::Word(w)) => Ok(w),
            Ok(other) => Err(format!("unexpected non-Word result: {:?}", other)),
            Err(e) => Err(e),
        }
    }

    // Independent reference oracle from the ARM ARM (C7.2.96 EXT).
    fn ref_word(rd: u32, rn: u32, rm: u32, q: u32, index: u32) -> u32 {
        let imm4 = index & 0xF;
        (q << 30) | (0b101110u32 << 24) | (rm << 16) | (imm4 << 11) | (rn << 5) | rd
    }

    proptest! {
        // 1. Differential oracle: encoder output equals an independent reference
        //    reconstruction of the EXT word for all valid register numbers and
        //    in-range byte indices (0..=15), for both .8b and .16b.
        #[test]
        fn prop_matches_reference(
            rd in 0u32..32u32,
            rn in 0u32..32u32,
            rm in 0u32..32u32,
            index in 0u32..16u32,
        ) {
            for &arr in ARRANGEMENTS {
                let q = if arr == "16b" { 1u32 } else { 0u32 };
                let word = encode(rd, rn, rm, arr, index as i64)
                    .expect("valid EXT must encode");
                prop_assert_eq!(word, ref_word(rd, rn, rm, q, index), "diff for arr {}", arr);
            }
        }

        // 2. Fixed ISA opcode fields are invariant: bit31==0, bits29-24==101110,
        //    and the always-zero filler bits (10, 15, 21, 22, 23) are clear.
        #[test]
        fn prop_fixed_opcode_fields(
            rd in 0u32..32u32,
            rn in 0u32..32u32,
            rm in 0u32..32u32,
            index in 0u32..16u32,
        ) {
            for &arr in ARRANGEMENTS {
                let word = encode(rd, rn, rm, arr, index as i64)
                    .expect("valid EXT must encode");
                prop_assert_eq!((word >> 31) & 1, 0u32, "bit31 must be 0");
                prop_assert_eq!((word >> 24) & 0x3F, 0b101110u32, "opcode bits 29-24");
                for (bit, name) in [(10u32, "10"), (15u32, "15"), (21u32, "21"), (22u32, "22"), (23u32, "23")] {
                    prop_assert_eq!((word >> bit) & 1, 0u32, "bit {} must be 0", name);
                }
            }
        }

        // 3. Q bit (bit 30) is 1 iff the destination arrangement is "16b".
        #[test]
        fn prop_q_bit_selects_16b(
            rd in 0u32..32u32,
            rn in 0u32..32u32,
            rm in 0u32..32u32,
            index in 0u32..16u32,
        ) {
            for &arr in ARRANGEMENTS {
                let word = encode(rd, rn, rm, arr, index as i64)
                    .expect("valid EXT must encode");
                let expected_q = if arr == "16b" { 1u32 } else { 0u32 };
                prop_assert_eq!((word >> 30) & 1, expected_q, "Q for arr {}", arr);
            }
        }

        // 4. Register operand fields are preserved verbatim:
        //    Rd (bits 4-0) == rd, Rn (bits 9-5) == rn, Rm (bits 20-16) == rm.
        #[test]
        fn prop_register_fields_preserved(
            rd in 0u32..32u32,
            rn in 0u32..32u32,
            rm in 0u32..32u32,
            index in 0u32..16u32,
        ) {
            for &arr in ARRANGEMENTS {
                let word = encode(rd, rn, rm, arr, index as i64)
                    .expect("valid EXT must encode");
                prop_assert_eq!(word & 0x1F, rd, "Rd for arr {}", arr);
                prop_assert_eq!((word >> 5) & 0x1F, rn, "Rn for arr {}", arr);
                prop_assert_eq!((word >> 16) & 0x1F, rm, "Rm for arr {}", arr);
            }
        }

        // 5. imm4 field round-trips: bits 14-11 == index & 0xF. For out-of-range
        //    indices (>=16) only the low nibble is kept (index is masked with 0xF),
        //    documenting the absence of range validation.
        #[test]
        fn prop_imm4_field_masks(
            rd in 0u32..32u32,
            rn in 0u32..32u32,
            rm in 0u32..32u32,
            index in 0u32..256u32,
        ) {
            for &arr in ARRANGEMENTS {
                let word = encode(rd, rn, rm, arr, index as i64)
                    .expect("any index must encode (no range check)");
                let imm4 = (word >> 11) & 0xF;
                prop_assert_eq!(imm4, index & 0xF, "imm4 == index & 0xF for arr {}", arr);
                // For in-range indices, imm4 equals the index exactly.
                if index < 16 {
                    prop_assert_eq!(imm4, index, "imm4 == index (in range) for arr {}", arr);
                }
            }
        }

        // 6. Error contracts: fewer than 4 operands must be rejected, and a
        //    non-immediate 4th operand must be rejected (get_imm contract).
        #[test]
        fn prop_error_contracts(rd in 0u32..32u32, rn in 0u32..32u32, rm in 0u32..32u32) {
            // 0, 1, 2, 3 operands -> all must error.
            for n in 0..4 {
                let mut ops: Vec<Operand> = vec![
                    Operand::RegArrangement { reg: format!("v{}", rd), arrangement: "8b".to_string() },
                    Operand::RegArrangement { reg: format!("v{}", rn), arrangement: "8b".to_string() },
                    Operand::RegArrangement { reg: format!("v{}", rm), arrangement: "8b".to_string() },
                    Operand::Imm(7),
                ];
                ops.truncate(n);
                prop_assert!(
                    encode_neon_ext(&ops).is_err(),
                    "{} operands must be rejected", n
                );
            }

            // 4th operand is not an Imm -> must error.
            let bad_imm = vec![
                Operand::RegArrangement { reg: format!("v{}", rd), arrangement: "8b".to_string() },
                Operand::RegArrangement { reg: format!("v{}", rn), arrangement: "8b".to_string() },
                Operand::RegArrangement { reg: format!("v{}", rm), arrangement: "8b".to_string() },
                Operand::Reg(format!("v{}", rm)),
            ];
            prop_assert!(encode_neon_ext(&bad_imm).is_err(), "non-Imm index must error");
        }
    }
}

// ── PBT for encode_neon_shift_imm (USHR-family immediate shift) ──────────
//
// Oracle: USHR (vector, immediate) is encoded as
//     0 Q 1 0 11110 immh:immb 000001 Rn Rd
// where  immh:immb = (2 * element_bits) - shift   (for a valid shift in 1..=element_bits).
// element_bits is derived from the arrangement: 8b/16b->8, 4h/8h->16, 2s/4s->32, 2d->64.
//
// Notable behaviors under test:
//   * the `_is_unsigned` parameter is IGNORED — the U bit (29) is always 1.
//   * shift is NOT range-checked (shift=0 yields immh=0, which is UNALLOCATED in ARMv8).
#[cfg(test)]
mod shift_imm_pbt_tests {
    use super::*;
    use proptest::prelude::*;

    // (arrangement, element_bits, valid-mask-width-for-immh:immb)
    const ARRAYS: &[(&str, u32)] = &[
        ("8b", 8),
        ("16b", 8),
        ("4h", 16),
        ("8h", 16),
        ("2s", 32),
        ("4s", 32),
        ("2d", 64),
    ];

    fn expected_q(arr: &str) -> u32 {
        match arr {
            "16b" | "8h" | "4s" | "2d" => 1,
            _ => 0,
        }
    }

    fn shift_ops(rd: u32, arr: &str, rn: u32, shift: i64) -> Vec<Operand> {
        vec![
            Operand::RegArrangement { reg: format!("v{}", rd), arrangement: arr.to_string() },
            Operand::RegArrangement { reg: format!("v{}", rn), arrangement: arr.to_string() },
            Operand::Imm(shift),
        ]
    }

    fn encode_word(ops: &[Operand], is_unsigned: bool) -> Result<u32, String> {
        match encode_neon_shift_imm(ops, is_unsigned) {
            Ok(EncodeResult::Word(w)) => Ok(w),
            Ok(other) => Err(format!("unexpected non-Word result: {:?}", other)),
            Err(e) => Err(e),
        }
    }

    proptest! {
        // 1. Constant ISA fields for every valid encoding.
        //    bit31=0, bits[28:23]=0b011110, bits[15:10]=0b000001, U(bit29)=1.
        #[test]
        fn prop_fixed_fields(rd in 0u32..32u32, rn in 0u32..32u32, s in 1u32..64u32) {
            for &(arr, elem_bits) in ARRAYS {
                let shift = ((s - 1) % elem_bits) + 1; // valid shift in [1, elem_bits]
                let w = encode_word(&shift_ops(rd, arr, rn, shift as i64), true)
                    .expect("valid shift must encode");
                prop_assert_eq!((w >> 31) & 1, 0u32, "bit31 {}", arr);
                prop_assert_eq!((w >> 23) & 0x3F, 0b011110u32, "opcode[28:23] {}", arr);
                prop_assert_eq!((w >> 10) & 0x3F, 0b000001u32, "fixed[15:10] {}", arr);
                prop_assert_eq!((w >> 29) & 1, 1u32, "U bit must always be 1 ({})", arr);
            }
        }

        // 2. Rd (bits[4:0]) and Rn (bits[9:5]) always equal the source register numbers.
        #[test]
        fn prop_reg_fields_preserved(rd in 0u32..32u32, rn in 0u32..32u32, s in 1u32..64u32) {
            for &(arr, elem_bits) in ARRAYS {
                let shift = ((s - 1) % elem_bits) + 1;
                let w = encode_word(&shift_ops(rd, arr, rn, shift as i64), true)
                    .expect("valid shift must encode");
                prop_assert_eq!(w & 0x1F, rd, "Rd {}", arr);
                prop_assert_eq!((w >> 5) & 0x1F, rn, "Rn {}", arr);
            }
        }

        // 3. Q bit (30) tracks the wide/narrow arrangement.
        #[test]
        fn prop_q_bit_per_arrangement(rd in 0u32..32u32, s in 1u32..64u32) {
            for &(arr, elem_bits) in ARRAYS {
                let shift = ((s - 1) % elem_bits) + 1;
                let w = encode_word(&shift_ops(rd, arr, 0, shift as i64), true)
                    .expect("valid shift must encode");
                prop_assert_eq!((w >> 30) & 1, expected_q(arr), "Q for {}", arr);
            }
        }

        // 4. immh:immb oracle + round-trip (differential oracle).
        //    encoded immh:immb (bits[22:16]) == 2*elem_bits - shift,
        //    and the shift is fully reconstructable from the word.
        #[test]
        fn prop_immh_immb_oracle(rd in 0u32..32u32, s in 1u32..64u32) {
            for &(arr, elem_bits) in ARRAYS {
                let shift = ((s - 1) % elem_bits) + 1;
                let w = encode_word(&shift_ops(rd, arr, 0, shift as i64), true)
                    .expect("valid shift must encode");
                let field = (w >> 16) & 0x7F;
                let expected = 2 * elem_bits - shift; // = elem_bits*2 - shift
                prop_assert_eq!(field, expected, "immh:immb for {} shift {}", arr, shift);
                // Round-trip: shift recovered from the encoding equals the input.
                let recovered = 2 * elem_bits - field;
                prop_assert_eq!(recovered, shift, "shift round-trip for {}", arr);
            }
        }

        // 5. The `_is_unsigned` parameter is ignored: both signs produce identical words.
        #[test]
        fn prop_is_unsigned_ignored(rd in 0u32..32u32, rn in 0u32..32u32, s in 1u32..64u32) {
            for &(arr, elem_bits) in ARRAYS {
                let shift = ((s - 1) % elem_bits) + 1;
                let ops = shift_ops(rd, arr, rn, shift as i64);
                let w_u = encode_word(&ops, true).expect("unsigned encodes");
                let w_s = encode_word(&ops, false).expect("signed path encodes");
                prop_assert_eq!(w_u, w_s, "is_unsigned must not change encoding for {}", arr);
                // And both hardcode U=1 (i.e. neither produces an SSHR U=0 encoding).
                prop_assert_eq!((w_u >> 29) & 1, 1u32, "U hardcoded to 1 ({})", arr);
            }
        }

        // 6. Error contracts.
        #[test]
        fn prop_error_contracts(rd in 0u32..32u32) {
            // (a) fewer than 3 operands -> Err for every arity 0..=2.
            for n in 0..=2usize {
                let mut ops: Vec<Operand> = vec![
                    Operand::RegArrangement { reg: format!("v{}", rd), arrangement: "8b".to_string() },
                    Operand::RegArrangement { reg: format!("v{}", rd), arrangement: "8b".to_string() },
                    Operand::Imm(1),
                ];
                ops.truncate(n);
                prop_assert!(
                    encode_neon_shift_imm(&ops, true).is_err(),
                    "{} operands must error", n
                );
            }

            // (b) arrangements not accepted by the shift-imm match arm -> Err.
            //     "4h" is intentionally absent: it is a valid narrow halfword form.
            for &bad in &["1d", "1q", "2h", "3s", ""] {
                let ops = shift_ops(rd, bad, rd, 4);
                prop_assert!(
                    encode_neon_shift_imm(&ops, true).is_err(),
                    "arrangement {:?} must be rejected", bad
                );
            }

            // (c) third operand not an immediate -> Err.
            let bad_imm = vec![
                Operand::RegArrangement { reg: format!("v{}", rd), arrangement: "8b".to_string() },
                Operand::RegArrangement { reg: format!("v{}", rd), arrangement: "8b".to_string() },
                Operand::Reg(format!("v{}", rd)),
            ];
            prop_assert!(encode_neon_shift_imm(&bad_imm, true).is_err(), "non-Imm shift must error");
        }

        // 7. Characterization of the missing shift-range check.
        //    shift == 0 is UNALLOCATED in ARMv8 (immh:immb would need immh != 0),
        //    yet the encoder silently returns Ok with immh == 0. This documents that gap.
        #[test]
        fn prop_shift_zero_accepted_but_unallocated(rd in 0u32..32u32) {
            for &(arr, _elem_bits) in ARRAYS {
                let res = encode_word(&shift_ops(rd, arr, 0, 0), true);
                prop_assert!(res.is_ok(), "shift=0 is accepted (no range check) for {}", arr);
                let w = res.unwrap();
                let immh = (w >> 19) & 0xF; // top 4 bits of immh:immb
                prop_assert_eq!(immh, 0u32, "shift=0 yields immh=0 (UNALLOCATED) for {}", arr);
            }
        }
    }
}

// ── encode_neon_logical (ORR/AND/EOR vector) ───────────────────────────
//
// ARMv8 three-same logical encoding:
//   0 Q U 01110 size 1 Rm 000111 Rn Rd
//   31 30 29 28:24 23:22 21 20:16 15:11(+bit10) 9:5 4:0
//
// The implementation writes `0b000111 << 10`, which is the 5-bit opcode
// 0b00011 at bits[15:11] OR'd with the fixed `1` at bit[10].
#[cfg(test)]
mod neon_logical_tests {
    use super::*;
    use proptest::prelude::*;

    // Field extractors.
    fn q_of(w: u32) -> u32        { (w >> 30) & 1 }
    fn u_of(w: u32) -> u32        { (w >> 29) & 1 }
    fn class_of(w: u32) -> u32    { (w >> 24) & 0x1F } // bits 28:24
    fn size_of(w: u32) -> u32     { (w >> 22) & 0x3 }
    fn bit21_of(w: u32) -> u32    { (w >> 21) & 1 }
    fn rm_of(w: u32) -> u32       { (w >> 16) & 0x1F }
    fn opcode_of(w: u32) -> u32   { (w >> 11) & 0x1F } // bits 15:11
    fn bit10_of(w: u32) -> u32    { (w >> 10) & 1 }
    fn rn_of(w: u32) -> u32       { (w >> 5) & 0x1F }
    fn rd_of(w: u32) -> u32       { w & 0x1F }

    fn vreg_arr(n: u32, arr: &str) -> Operand {
        Operand::RegArrangement { reg: format!("v{}", n), arrangement: arr.to_string() }
    }

    fn ops(rd: u32, arr: &str, rn: u32, rm: u32) -> Vec<Operand> {
        vec![vreg_arr(rd, arr), vreg_arr(rn, arr), vreg_arr(rm, arr)]
    }

    fn encode(operands: &[Operand], opc: u32) -> Result<u32, String> {
        match encode_neon_logical(operands, opc) {
            Ok(EncodeResult::Word(w)) => Ok(w),
            Ok(other) => Err(format!("expected Word, got {:?}", other)),
            Err(e) => Err(e),
        }
    }

    /// Reference oracle: reconstruct the ARMv8 word from its fields.
    fn ref_word(q: u32, u_bit: u32, size: u32, rm: u32, rn: u32, rd: u32) -> u32 {
        (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22) | (1 << 21)
            | (rm << 16) | (0b00011 << 11) | (1 << 10) | (rn << 5) | rd
    }

    /// opc -> (U bit, size field) per the ARMv8 logical table.
    /// NOTE: opc=0b11 (ANDS) is marked "not valid for NEON, fall back" in the
    /// source and emits the *same* U/size as EOR (opc=0b10). This aliasing is a
    /// known quirk; we assert it explicitly rather than treat it as a bug.
    fn opc_to_u_size(opc: u32) -> Option<(u32, u32)> {
        match opc {
            0b00 => Some((0, 0b00)), // AND
            0b01 => Some((0, 0b10)), // ORR
            0b10 => Some((1, 0b00)), // EOR
            0b11 => Some((1, 0b00)), // ANDS -> aliased to EOR
            _ => None,
        }
    }

    const ARRANGEMENTS: &[&str] = &["8b", "16b"];
    const OPCS: &[u32] = &[0b00, 0b01, 0b10, 0b11];

    proptest! {
        // 1. Differential oracle: the encoder's word must equal an independent
        //    reconstruction from (Q, U, size, Rm, Rn, Rd) for every valid input.
        #[test]
        fn prop_matches_reference_encoding(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            arr_idx in 0usize..ARRANGEMENTS.len(),
            opc_idx in 0usize..OPCS.len(),
        ) {
            let arr = ARRANGEMENTS[arr_idx];
            let opc = OPCS[opc_idx];
            let w = encode(&ops(rd, arr, rn, rm), opc).expect("valid logical ops");
            let q = if arr == "16b" { 1 } else { 0 };
            let (u_bit, size) = opc_to_u_size(opc).unwrap();
            prop_assert_eq!(w, ref_word(q, u_bit, size, rm, rn, rd));
        }

        // 2. Every register field survives into its own 5-bit slice, untouched,
        //    across the full v0..v31 range.
        #[test]
        fn prop_register_fields_preserved(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            arr_idx in 0usize..ARRANGEMENTS.len(),
            opc_idx in 0usize..OPCS.len(),
        ) {
            let arr = ARRANGEMENTS[arr_idx];
            let opc = OPCS[opc_idx];
            let w = encode(&ops(rd, arr, rn, rm), opc).expect("valid logical ops");
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rm_of(w), rm);
        }

        // 3. Q is set *only* for the .16b arrangement. Every other arrangement
        //    (including non-byte ones like .4s) collapses to Q=0; the encoder
        //    performs no arrangement validation, so we characterize that.
        #[test]
        fn prop_q_bit_only_for_16b(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            opc_idx in 0usize..OPCS.len(),
        ) {
            let opc = OPCS[opc_idx];
            for &arr in &["8b", "16b", "4s", "2d", "4h"] {
                let w = encode(&ops(rd, arr, rn, rm), opc).expect("valid logical ops");
                let want = if arr == "16b" { 1 } else { 0 };
                prop_assert_eq!(q_of(w), want, "arr={}", arr);
            }
        }

        // 4. U bit and size field are a pure function of `opc` (Q/regs irrelevant).
        #[test]
        fn prop_u_and_size_per_opc(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            arr_idx in 0usize..ARRANGEMENTS.len(),
        ) {
            let arr = ARRANGEMENTS[arr_idx];
            for &opc in OPCS {
                let w = encode(&ops(rd, arr, rn, rm), opc).expect("valid logical ops");
                let (want_u, want_size) = opc_to_u_size(opc).unwrap();
                prop_assert_eq!(u_of(w), want_u, "opc={:b}", opc);
                prop_assert_eq!(size_of(w), want_size, "opc={:b}", opc);
            }
            // opc=0b11 must alias opc=0b10 (the documented fall-back quirk).
            let w11 = encode(&ops(rd, arr, rn, rm), 0b11).expect("valid");
            let w10 = encode(&ops(rd, arr, rn, rm), 0b10).expect("valid");
            prop_assert_eq!(w11, w10);
        }

        // 5. The fixed opcode/class fields are invariant for every input.
        #[test]
        fn prop_fixed_opcode_fields_constant(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            arr_idx in 0usize..ARRANGEMENTS.len(),
            opc_idx in 0usize..OPCS.len(),
        ) {
            let arr = ARRANGEMENTS[arr_idx];
            let opc = OPCS[opc_idx];
            let w = encode(&ops(rd, arr, rn, rm), opc).expect("valid logical ops");
            prop_assert_eq!(w >> 31, 0u32,        // bit 31 reserved = 0
                "bit31 must be 0");
            prop_assert_eq!(class_of(w), 0b01110, // bits 28:24
                "class must be 01110");
            prop_assert_eq!(bit21_of(w), 1,       // bit 21 fixed = 1
                "bit21 must be 1");
            prop_assert_eq!(opcode_of(w), 0b00011,// bits 15:11 logical opcode
                "opcode must be 00011");
            prop_assert_eq!(bit10_of(w), 1,       // bit 10 fixed = 1
                "bit10 must be 1");
        }

        // 6. Only operand 0's arrangement feeds Q; operands 1 and 2 are parsed
        //    for their register number only — their arrangements are discarded.
        #[test]
        fn prop_source_arrangements_ignored(
            rd in 0u32..=31,
            rn in 0u32..=31,
            rm in 0u32..=31,
            opc_idx in 0usize..OPCS.len(),
        ) {
            let opc = OPCS[opc_idx];
            let base = vec![
                vreg_arr(rd, "16b"),
                vreg_arr(rn, "16b"),
                vreg_arr(rm, "16b"),
            ];
            let w0 = encode(&base, opc).expect("valid base");
            // Mutate operand 1 / 2 arrangements: word must be identical.
            let mut mixed = base.clone();
            mixed[1] = vreg_arr(rn, "4s");
            mixed[2] = vreg_arr(rm, "2d");
            prop_assert_eq!(encode(&mixed, opc).expect("valid mixed"), w0);
        }

        // 7. Error contract: missing operands and out-of-range opc must Err.
        #[test]
        fn prop_error_contracts(opc in any::<u32>()) {
            // Fewer than 3 operands -> Err for every valid opc.
            for &valid_opc in OPCS {
                let one = vec![vreg_arr(0, "16b")];
                let two = vec![vreg_arr(0, "16b"), vreg_arr(1, "16b")];
                prop_assert!(encode_neon_logical(&one, valid_opc).is_err(),
                    "1 operand should error for opc={:b}", valid_opc);
                prop_assert!(encode_neon_logical(&two, valid_opc).is_err(),
                    "2 operands should error for opc={:b}", valid_opc);
            }
            // opc outside {0,1,2,3} -> Err, regardless of valid operands.
            prop_assume!(opc >= 4);
            let ok_ops = ops(0, "16b", 1, 2);
            prop_assert!(encode_neon_logical(&ok_ops, opc).is_err(),
                "opc={} (>=4) should error", opc);
        }
    }
}

#[cfg(test)]
mod neon_movi_props {
    use super::{encode_neon_movi, EncodeResult};
    use crate::backend::arm::assembler::parser::Operand;
    use proptest::prelude::*;

    fn movi_ops(rd: u32, arr: &str, imm: i64) -> Vec<Operand> {
        vec![
            Operand::RegArrangement { reg: format!("v{}", rd), arrangement: arr.to_string() },
            Operand::Imm(imm),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(512))]

        // Oracle: reference layout (AdvSIMD modified immediate)
        // .8b/.16b byte-mask form: 0 Q 0 0 1111 0 0000 abc 1110 0 1 defgh Rd
        // where imm8 = abc:defgh. Every field of the word must match the spec.
        #[test]
        fn byte_form_field_layout(arr in prop_oneof![Just("8b"), Just("16b")],
                                  imm8 in 0u32..=255u32, rd in 0u32..=31u32) {
            let w = match encode_neon_movi(&movi_ops(rd, arr, imm8 as i64)) {
                Ok(EncodeResult::Word(w)) => w,
                other => panic!("expected Word, got {:?}", other),
            };
            prop_assert_eq!(w >> 31, 0, "bit31 must be 0");
            prop_assert_eq!((w >> 30) & 1, u32::from(arr == "16b"), "Q bit");
            prop_assert_eq!((w >> 23) & 0x7F, 0b0011110, "fixed bits[29:23]");
            prop_assert_eq!((w >> 19) & 0xF, 0, "bits[22:19] reserved 0");
            prop_assert_eq!((w >> 16) & 0x7, (imm8 >> 5) & 0x7, "abc=imm8[7:5]@[18:16]");
            prop_assert_eq!((w >> 12) & 0xF, 0b1110, "cmode=1110");
            prop_assert_eq!((w >> 11) & 1, 0, "o2=0");
            prop_assert_eq!((w >> 10) & 1, 1, "fixed 1@10");
            prop_assert_eq!((w >> 5) & 0x1F, imm8 & 0x1F, "defgh=imm8[4:0]@[9:5]");
            prop_assert_eq!(w & 0x1F, rd, "Rd@[4:0]");
        }

        // Oracle: algebraic field independence. Varying only Rd touches bits[4:0];
        // varying only the immediate touches only bits[18:16] and bits[9:5].
        #[test]
        fn field_independence(imm8 in 0u32..=255u32, r1 in 0u32..=31u32, r2 in 0u32..=31u32) {
            prop_assume!(r1 != r2);
            let wr = |rd| match encode_neon_movi(&movi_ops(rd, "16b", imm8 as i64)) {
                Ok(EncodeResult::Word(w)) => w, other => panic!("{:?}", other) };
            let w1 = wr(r1);
            let w2 = wr(r2);
            // Only the Rd field differs.
            prop_assert_eq!(w1 & !0x1Fu32, w2 & !0x1Fu32, "Rd leaked into other bits");

            let i1 = match encode_neon_movi(&movi_ops(7, "16b", 0x5A)) {
                Ok(EncodeResult::Word(w)) => w, other => panic!("{:?}", other) };
            let i2 = match encode_neon_movi(&movi_ops(7, "16b", 0xA5)) {
                Ok(EncodeResult::Word(w)) => w, other => panic!("{:?}", other) };
            let imm_mask = (0x7u32 << 16) | (0x1Fu32 << 5);
            prop_assert_eq!((i1 ^ i2) & !imm_mask, 0, "immediate leaked into non-imm bits");
        }

        // Oracle: cmode selection per arrangement / shift.
        #[test]
        fn cmode_per_form(
            arr in prop_oneof![Just("4h"), Just("8h"), Just("2s"), Just("4s")],
            imm8 in 0u32..=255u32, rd in 0u32..=31u32
        ) {
            let w = match encode_neon_movi(&movi_ops(rd, arr, imm8 as i64)) {
                Ok(EncodeResult::Word(w)) => w, other => panic!("{:?}", other) };
            let want_cmode = match arr {
                "4h" | "8h" => 0b1000u32,
                _ => 0b0000u32,
            };
            prop_assert_eq!((w >> 12) & 0xF, want_cmode, "cmode for {}", arr);
            prop_assert_eq!((w >> 16) & 0x7, (imm8 >> 5) & 0x7, "abc");
            prop_assert_eq!((w >> 5) & 0x1F, imm8 & 0x1F, "defgh");
            prop_assert_eq!(w & 0x1F, rd, "Rd");
            prop_assert_eq!((w >> 30) & 1, u32::from(arr == "8h" || arr == "4s"), "Q");
        }

        // Oracle: .2s/.4s LSL shift selects cmode 0000/0010/0100/0110.
        #[test]
        fn shift_selects_cmode(
            arr in prop_oneof![Just("2s"), Just("4s")],
            imm8 in 0u32..=255u32, rd in 0u32..=31u32,
            shift in prop_oneof![Just(0u32), Just(8), Just(16), Just(24)]
        ) {
            let ops = vec![
                Operand::RegArrangement { reg: format!("v{}", rd), arrangement: arr.to_string() },
                Operand::Imm(imm8 as i64),
                Operand::Shift { kind: "lsl".to_string(), amount: shift },
            ];
            let w = match encode_neon_movi(&ops) {
                Ok(EncodeResult::Word(w)) => w, other => panic!("{:?}", other) };
            let want = match shift { 0 => 0b0000u32, 8 => 0b0010, 16 => 0b0100, _ => 0b0110 };
            prop_assert_eq!((w >> 12) & 0xF, want, "cmode for lsl #{}", shift);
        }

        // Negative contract: arrangements with no MOVI encoding are rejected.
        #[test]
        fn unsupported_arrangement_rejected(
            arr in prop_oneof![Just("1d"), Just("1q"), Just("2h"), Just("16s"), Just("b"), Just("")]
        ) {
            prop_assert!(encode_neon_movi(&movi_ops(0, arr, 0)).is_err(),
                "arrangement {:?} has no MOVI encoding", arr);
        }

        // Negative contract: unsupported LSL shift amounts for .2s/.4s error.
        #[test]
        fn unsupported_shift_rejected(
            arr in prop_oneof![Just("2s"), Just("4s")],
            shift in (1u32..32u32).prop_filter(
                "not canonical", |s| !(*s == 0 || *s == 8 || *s == 16 || *s == 24))
        ) {
            let ops = vec![
                Operand::RegArrangement { reg: "v0".to_string(), arrangement: arr.to_string() },
                Operand::Imm(1),
                Operand::Shift { kind: "lsl".to_string(), amount: shift },
            ];
            prop_assert!(encode_neon_movi(&ops).is_err(), "shift={} must error", shift);
        }

        // Oracle: .2d strict byte validation (each byte must be 0x00 or 0xFF).
        #[test]
        fn d2_form_strict_byte_validation(bits in 0u64..=255u64) {
            let mut imm: u64 = 0;
            for i in 0..8 { if (bits >> i) & 1 == 1 { imm |= 0xFFu64 << (i * 8); } }
            prop_assert!(matches!(encode_neon_movi(&movi_ops(0, "2d", imm as i64)),
                Ok(EncodeResult::Word(_))), "valid 2d imm 0x{:x}", imm);
            // A byte that is neither 0x00 nor 0xFF must be rejected.
            prop_assert!(encode_neon_movi(&movi_ops(0, "2d", 1)).is_err(),
                "imm=1 has byte 0x01 and must error");
        }

        // Negative contract (EXPECTED TO FAIL — documents a bug):
        // ARM MOVI byte/word/halfword immediates occupy an 8-bit field
        // (0..=255). GAS/LLVM reject out-of-range values ("immediate must be
        // an integer in range [0, 255]"). This encoder masks with `& 0xFF`
        // and silently truncates (#256 -> #0) instead of returning Err,
        // inconsistent with the strictly-validated .2d path.
        #[test]
        fn out_of_range_immediate_must_be_rejected(
            arr in prop_oneof![Just("8b"), Just("16b"), Just("2s"), Just("4s"), Just("4h"), Just("8h")],
            imm in 256i64..=65535i64
        ) {
            let res = encode_neon_movi(&movi_ops(0, arr, imm));
            prop_assert!(res.is_err(),
                "MOVI {:?} #{} is out of 8-bit range and must be rejected, got {:?}",
                arr, imm, res);
        }
    }
}

#[cfg(test)]
mod ext_range_pbt_tests {
    use super::*;
    use proptest::prelude::*;

    // Companion to `ext_pbt_tests` above. The existing module documents the
    // missing validation with *passing* masking/round-trip properties. These
    // properties instead assert the spec-mandated *error* contracts directly;
    // each is EXPECTED TO FAIL until range/arrangement validation is added.
    //
    // Oracle (ARM ARM, EXT):
    //   EXT Vd.T, Vn.T, Vm.T, #index   (T = 8B or 16B only)
    //   - imm4 is a 4-bit field (0..=15); out-of-range index is unrepresentable.
    //   - EXT 8B:  Q=0 and imm4<3> == 0, i.e. index in 0..=7 (8..=15 UNDEFINED).
    //   - EXT 16B: Q=1, index in 0..=15.
    //   - Only 8B/16B arrangements are valid; any other arrangement is rejected
    //     by GAS ("operand mismatch") / LLVM.

    fn ext_ops(rd: u32, rn: u32, rm: u32, arr: &str, index: i64) -> Vec<Operand> {
        vec![
            Operand::RegArrangement { reg: format!("v{}", rd), arrangement: arr.to_string() },
            Operand::RegArrangement { reg: format!("v{}", rn), arrangement: arr.to_string() },
            Operand::RegArrangement { reg: format!("v{}", rm), arrangement: arr.to_string() },
            Operand::Imm(index),
        ]
    }

    proptest! {
        // 1. Negative contract (EXPECTED TO FAIL — documents a bug):
        //    imm4 is a 4-bit field (0..=15); an index outside that range cannot be
        //    encoded and must be rejected (GAS/LLVM: "immediate must be an integer
        //    in range [0, 15]"). The encoder masks with `index & 0xF` and silently
        //    truncates (e.g. #16 -> #0, #17 -> #1) instead of returning Err.
        #[test]
        fn out_of_range_imm4_must_be_rejected(
            arr in prop_oneof![Just("8b"), Just("16b")],
            index in 16i64..=65535i64
        ) {
            let res = encode_neon_ext(&ext_ops(0, 1, 2, arr, index));
            prop_assert!(res.is_err(),
                "EXT {:?} #{} is outside the 4-bit imm4 range and must be rejected, got {:?}",
                arr, index, res);
        }

        // 2. Negative contract (EXPECTED TO FAIL — documents a bug):
        //    EXT 8B requires imm4<3> == 0, i.e. index in 0..=7. An index of 8..=15
        //    is architecturally UNDEFINED for Q=0 and must be rejected. The encoder
        //    accepts it (masking into the imm4 field), producing an invalid word.
        #[test]
        fn ext_8b_high_index_must_be_rejected(index in 8u32..16u32) {
            let res = encode_neon_ext(&ext_ops(0, 1, 2, "8b", index as i64));
            prop_assert!(res.is_err(),
                "EXT 8B #{} is UNDEFINED (imm4<3> must be 0) and must be rejected, got {:?}",
                index, res);
        }

        // 3. Negative contract (EXPECTED TO FAIL — documents a bug):
        //    EXT is defined only for the 8B/16B arrangements. Any other arrangement
        //    (4s, 8h, 2d, ...) is invalid and must be rejected. The encoder only
        //    tests for "16b" and silently treats everything else as 8B (Q=0).
        #[test]
        fn invalid_arrangement_must_be_rejected(
            arr in prop_oneof![Just("4s"), Just("8h"), Just("4h"), Just("2d"), Just("2s")]
        ) {
            let res = encode_neon_ext(&ext_ops(0, 1, 2, arr, 1));
            prop_assert!(res.is_err(),
                "EXT {:?} is not a valid EXT arrangement (only 8b/16b), must be rejected, got {:?}",
                arr, res);
        }
    }
}

/// Complementary property-based tests for `encode_neon_dup`.
///
/// The sibling `dup_pbt_tests` module checks per-field invariants. This module
/// adds: (1) full-word reference/golden oracles that reconstruct the entire
/// 32-bit encoding from the ARM ARM bit layout independently of the encoder's
/// own shift expression, (2) a non-aliasing invariant between the general and
/// element forms, and (3) a negative contract that out-of-range lane indices
/// must be rejected (this currently FAILS and documents a real bug — see the
/// accompanying bug report).
#[cfg(test)]
mod dup_pbt_extra_tests {
    use super::*;
    use proptest::prelude::*;

    // Destination arrangements accepted by the general form. Note DUP(general)
    // has no `.1d` variant.
    const GP_ARRS: &[&str] = &["8b", "16b", "4h", "8h", "2s", "4s", "2d"];
    // Destination arrangements accepted by the element form.
    const ELEM_DEST_ARRS: &[&str] = &["8b", "16b", "4h", "8h", "2s", "4s", "1d", "2d"];

    fn expected_q(arr: &str) -> u32 {
        match arr {
            "16b" | "8h" | "4s" | "2d" => 1,
            _ => 0,
        }
    }

    // imm5 size-code for the GP form (no index).
    fn gp_imm5_code(arr: &str) -> u32 {
        match arr {
            "8b" | "16b" => 0b00001,
            "4h" | "8h" => 0b00010,
            "2s" | "4s" => 0b00100,
            "2d" => 0b01000,
            _ => unreachable!("gp_imm5_code on {}", arr),
        }
    }

    fn gp_ops(rd: u32, arr: &str, rn: u32) -> Vec<Operand> {
        vec![
            Operand::RegArrangement { reg: format!("v{}", rd), arrangement: arr.to_string() },
            Operand::Reg(format!("x{}", rn)),
        ]
    }

    fn elem_ops(rd: u32, dest_arr: &str, rn: u32, elem_size: &str, index: u32) -> Vec<Operand> {
        vec![
            Operand::RegArrangement {
                reg: format!("v{}", rd),
                arrangement: dest_arr.to_string(),
            },
            Operand::RegLane {
                reg: format!("v{}", rn),
                elem_size: elem_size.to_string(),
                index,
            },
        ]
    }

    fn word(ops: &[Operand]) -> u32 {
        match encode_neon_dup(ops) {
            Ok(EncodeResult::Word(w)) => w,
            Ok(other) => panic!("expected Word, got {:?}", other),
            Err(e) => panic!("expected Ok, got Err: {}", e),
        }
    }

    // Independent full-word reconstruction of the general form, built field by
    // field from the ARM ARM layout `0 Q 0 01110 000 imm5 0 0001 1 Rn Rd`.
    // This decomposition differs structurally from the encoder's single
    // `(0b001110000u32 << 21)` expression, so agreement is meaningful.
    fn ref_gp_word(rd: u32, arr: &str, rn: u32) -> u32 {
        let q = expected_q(arr);
        let imm5 = gp_imm5_code(arr);
        let mut w = 0u32;
        w |= 0u32 << 31; // bit31
        w |= q << 30; // bit30 = Q
        w |= 0u32 << 29; // bit29
        w |= 0b01110u32 << 24; // bits[28:24]
        w |= 0b000u32 << 21; // bits[23:21]
        w |= imm5 << 16; // bits[20:16]
        w |= 0u32 << 15; // bit15
        w |= 0b0001u32 << 11; // bits[14:11]
        w |= 1u32 << 10; // bit10
        w |= rn << 5; // bits[9:5]
        w |= rd; // bits[4:0]
        w
    }

    // Independent full-word reconstruction of the element form, built from
    // `0 Q 0 01110 000 imm5 0 0000 1 Rn Rd` where imm5 packs the lane index.
    fn ref_elem_word(rd: u32, dest_arr: &str, rn: u32, elem_size: &str, index: u32) -> u32 {
        let q = expected_q(dest_arr);
        let imm5 = match elem_size {
            "b" => (index << 1) | 0b00001,
            "h" => (index << 2) | 0b00010,
            "s" => (index << 3) | 0b00100,
            "d" => (index << 4) | 0b01000,
            _ => unreachable!(),
        };
        let mut w = 0u32;
        w |= 0u32 << 31;
        w |= q << 30;
        w |= 0b01110u32 << 24;
        w |= 0b000u32 << 21;
        w |= imm5 << 16;
        w |= 0u32 << 15;
        w |= 0b0000u32 << 11; // bits[14:11]
        w |= 1u32 << 10; // bit10
        w |= rn << 5;
        w |= rd;
        w
    }

    proptest! {
        // 1. Golden full-word anchors: a few canonical encodings must match the
        //    exact machine word mandated by the ARM ARM. These pin the entire
        //    instruction, not just individual fields, so a single misplaced bit
        //    is caught. Register operands are swept to confirm they slot in.
        #[test]
        fn prop_golden_gp_words(rn in 0u32..32u32, rd in 0u32..32u32) {
            // DUP V0.16b, X0 == 0x4E010C00
            prop_assert_eq!(word(&gp_ops(rd, "16b", rn)), 0x4E010C00u32 | (rn << 5) | rd);
            // DUP V0.8b, X0 == 0x0E010C00  (Q=0)
            prop_assert_eq!(word(&gp_ops(rd, "8b", rn)), 0x0E010C00u32 | (rn << 5) | rd);
            // DUP V0.2d, X0 == 0x4E080C00  (imm5=01000)
            prop_assert_eq!(word(&gp_ops(rd, "2d", rn)), 0x4E080C00u32 | (rn << 5) | rd);
            // DUP V0.4s, X0 == 0x4E040C00  (Q=1 for the 128-bit .4s form, imm5=00100)
            prop_assert_eq!(word(&gp_ops(rd, "4s", rn)), 0x4E040C00u32 | (rn << 5) | rd);
        }

        // 2. Golden full-word anchors for the element form.
        #[test]
        fn prop_golden_elem_words(rn in 1u32..32u32, rd in 0u32..32u32) {
            // DUP V0.16b, Vn.b[0] == 0x4E010400  (base with Rn=0)
            prop_assert_eq!(word(&elem_ops(rd, "16b", rn, "b", 0)), 0x4E010400u32 | (rn << 5) | rd);
            // DUP V0.8b, Vn.h[0] == 0x0E020400  (Q=0)
            prop_assert_eq!(word(&elem_ops(rd, "8b", rn, "h", 0)), 0x0E020400u32 | (rn << 5) | rd);
            // DUP V0.4s, Vn.s[3] == 0x4E1C0400  (imm5 = (3<<3)|00100 = 11100)
            prop_assert_eq!(word(&elem_ops(rd, "4s", rn, "s", 3)), 0x4E1C0400u32 | (rn << 5) | rd);
            // DUP V0.2d, Vn.d[1] == 0x4E180400  (imm5 = (1<<4)|01000 = 11000)
            prop_assert_eq!(word(&elem_ops(rd, "2d", rn, "d", 1)), 0x4E180400u32 | (rn << 5) | rd);
        }

        // 3. Full-word differential reference: across ALL valid arrangements and
        //    (for the element form) all in-range lane indices, the encoder's
        //    output equals the independent bit-by-bit reconstruction.
        #[test]
        fn prop_full_word_reference(rd in 0u32..32u32, rn in 0u32..32u32, idx in 0u32..16u32) {
            for &arr in GP_ARRS {
                prop_assert_eq!(word(&gp_ops(rd, arr, rn)), ref_gp_word(rd, arr, rn),
                    "gp full-word mismatch arr={}", arr);
            }
            for &dest in ELEM_DEST_ARRS {
                for &(es, max) in &[("b", 15u32), ("h", 7u32), ("s", 3u32), ("d", 1u32)] {
                    let index = idx & max;
                    prop_assert_eq!(
                        word(&elem_ops(rd, dest, rn, es, index)),
                        ref_elem_word(rd, dest, rn, es, index),
                        "elem full-word mismatch dest={} es={} idx={}", dest, es, index
                    );
                }
            }
        }

        // 4. Non-aliasing: the general and element forms must never emit the same
        //    opcode. They differ precisely in bits[15:10] (0b000011 vs 0b000001),
        //    so the two outputs for identical rd/rn/arrangement must be unequal.
        //    This guards against a regression that collapses the two forms.
        #[test]
        fn prop_gp_and_elem_forms_differ(rd in 0u32..32u32, rn in 0u32..32u32) {
            for &arr in &["8b", "16b"] {
                let g = word(&gp_ops(rd, arr, rn));
                let e = word(&elem_ops(rd, arr, rn, "b", 0));
                prop_assert_ne!(g, e, "gp/elem collide for arr={}", arr);
                // Specifically, the form discriminator bit 11 must be set in the
                // general form and clear in the element form.
                prop_assert_eq!((g >> 11) & 1, 1u32, "gp must set bit11 arr={}", arr);
                prop_assert_eq!((e >> 11) & 1, 0u32, "elem must clear bit11 arr={}", arr);
            }
        }

        // 5. NEGATIVE CONTRACT (CURRENTLY FAILS — documents a bug).
        //    For the element form the lane index is packed into imm5, whose
        //    width is finite per element size (b:4 bits, h:3, s:2, d:1). An
        //    index that exceeds this field width is unrepresentable for ANY
        //    arrangement and is reserved/UNDEFINED in the ARM ARM — it MUST be
        //    rejected. The encoder instead silently truncates with
        //    `index & 0xF` / `& 0x7` / `& 0x3` / `& 0x1`, aliasing e.g.
        //    `.b[16]` onto `.b[0]` and `.d[2]` onto `.d[0]` with no error.
        #[test]
        fn prop_out_of_range_lane_index_must_error(
            oob in 1u32..16u32
        ) {
            // Use the widest arrangement per size so the field-width limit
            // coincides with the architectural limit.
            let cases: &[(u32, &str, &str)] = &[
                (15 + oob, "16b", "b"), // > 15
                (7 + oob,  "8h",  "h"), // > 7
                (3 + oob,  "4s",  "s"), // > 3
                (1 + oob,  "2d",  "d"), // > 1
            ];
            for &(index, dest, es) in cases {
                let res = encode_neon_dup(&elem_ops(0, dest, 1, es, index));
                prop_assert!(res.is_err(),
                    "out-of-range lane index {} for .{} (field max exceeded) must be rejected, got {:?}",
                    index, es, res);
            }
        }
    }
}

#[cfg(test)]
mod prop_encode_neon_three_same_tests {
    use super::*;
    use proptest::prelude::*;

    // ORACLE: field-placement / bit-layout, anchored to ARMv8-A ARM
    // "Advanced SIMD three register, same" encoding:
    //   0 Q U 0 1 1 1 0 size 1 Rm opcode 1 Rn Rd
    //    31 30 29 28----24 23-22 21 20-16 15-11 10 9-5 4-0
    // Field widths: Q=1b, U=1b, size=2b, Rm/Rn/Rd=5b, opcode=5b.
    // The all-zero-fields golden (Q=U=size=Rm=opcode=Rn=Rd=0) is the fixed
    // template: (0b01110<<24)|(1<<21)|(1<<10) = 0x0E200400.
    const GOLDEN_TEMPLATE: u32 = 0x0E200400;
    const ARRANGEMENTS: &[&str] = &["8b", "16b", "4h", "8h", "2s", "4s", "1d", "2d"];

    fn vreg(num: u32, arr: &str) -> Operand {
        Operand::RegArrangement {
            reg: format!("v{}", num),
            arrangement: arr.to_string(),
        }
    }

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    // Independent arrangement -> (Q, size) table (NOT neon_arr_to_q_size).
    fn ref_q_size(arr: &str) -> Option<(u32, u32)> {
        match arr {
            "8b" => Some((0, 0b00)),
            "16b" => Some((1, 0b00)),
            "4h" => Some((0, 0b01)),
            "8h" => Some((1, 0b01)),
            "2s" => Some((0, 0b10)),
            "4s" => Some((1, 0b10)),
            "1d" => Some((0, 0b11)),
            "2d" => Some((1, 0b11)),
            _ => None,
        }
    }

    prop_compose! {
        fn arb_vreg_num()(n in 0u32..=31u32) -> u32 { n }
    }

    proptest! {
        // Property 1 — constant bitfield template is always present.
        // Bits: [31]=0, [28:24]=01110, [21]=1, [10]=1, for any valid
        // operands, u_bit, and opcode.
        #[test]
        fn prop_fixed_template(
            rd in arb_vreg_num(), rn in arb_vreg_num(), rm in arb_vreg_num(),
            arr in prop::sample::select(ARRANGEMENTS),
            u_bit in 0u32..=1u32, opcode in 0u32..=0x1Fu32,
        ) {
            let _ = GOLDEN_TEMPLATE;
            let ops = vec![vreg(rd, arr), vreg(rn, arr), vreg(rm, arr)];
            let w = word(encode_neon_three_same(&ops, u_bit, opcode));
            prop_assert_eq!(w >> 31, 0, "bit31 must be 0");
            prop_assert_eq!((w >> 24) & 0b11111, 0b01110, "[28:24] must be 01110");
            prop_assert_eq!((w >> 21) & 1, 1, "bit21 must be 1");
            prop_assert_eq!((w >> 10) & 1, 1, "bit10 must be 1");
        }

        // Property 2 — arrangement drives Q[30] and size[23:22], per the
        // independent reference table (not neon_arr_to_q_size).
        #[test]
        fn prop_arrangement_qsize(
            rd in arb_vreg_num(), rn in arb_vreg_num(), rm in arb_vreg_num(),
            arr in prop::sample::select(ARRANGEMENTS),
        ) {
            let (q, size) = ref_q_size(arr).unwrap();
            let ops = vec![vreg(rd, arr), vreg(rn, arr), vreg(rm, arr)];
            let w = word(encode_neon_three_same(&ops, 0, 0));
            prop_assert_eq!((w >> 30) & 1, q, "Q bit for {}", arr);
            prop_assert_eq!((w >> 22) & 0b11, size, "size field for {}", arr);
        }

        // Property 3 — register fields are independent and non-overlapping:
        // Rd -> [4:0], Rn -> [9:5], Rm -> [20:16]; all other bits constant.
        #[test]
        fn prop_register_field_independence(
            rd in arb_vreg_num(), rn in arb_vreg_num(), rm in arb_vreg_num(),
            u_bit in 0u32..=1u32, opcode in 0u32..=0x1Fu32,
        ) {
            let arr = "4s"; // fixed arrangement
            let base = word(encode_neon_three_same(
                &[vreg(0, arr), vreg(0, arr), vreg(0, arr)], u_bit, opcode));

            let w_rd = word(encode_neon_three_same(
                &[vreg(rd, arr), vreg(0, arr), vreg(0, arr)], u_bit, opcode));
            prop_assert_eq!(w_rd & !0x1F_u32, base & !0x1F_u32, "Rd must only touch bits 4:0");
            prop_assert_eq!(w_rd & 0x1F, rd & 0x1F);

            let w_rn = word(encode_neon_three_same(
                &[vreg(0, arr), vreg(rn, arr), vreg(0, arr)], u_bit, opcode));
            prop_assert_eq!(w_rn & !(0x1F_u32 << 5), base & !(0x1F_u32 << 5), "Rn must only touch bits 9:5");
            prop_assert_eq!((w_rn >> 5) & 0x1F, rn & 0x1F);

            let w_rm = word(encode_neon_three_same(
                &[vreg(0, arr), vreg(0, arr), vreg(rm, arr)], u_bit, opcode));
            prop_assert_eq!(w_rm & !(0x1F_u32 << 16), base & !(0x1F_u32 << 16), "Rm must only touch bits 20:16");
            prop_assert_eq!((w_rm >> 16) & 0x1F, rm & 0x1F);
        }

        // Property 4 — error contract: fewer than 3 operands errors; an
        // unsupported arrangement with 3 operands errors.
        #[test]
        fn prop_error_contract(
            arr in prop::sample::select(&["8b", "4s"]),
            n in 0usize..=2usize,
        ) {
            let ops: Vec<Operand> = (0..n).map(|_| vreg(0, arr)).collect();
            let res = encode_neon_three_same(&ops, 0, 0);
            prop_assert!(res.is_err(), "{} operands must error, got {:?}", n, res);

            let bad = vec![vreg(0, "12b"), vreg(1, "12b"), vreg(2, "12b")];
            let res2 = encode_neon_three_same(&bad, 0, 0);
            prop_assert!(res2.is_err(), "unsupported arrangement must error, got {:?}", res2);
        }

        // Property 5 — NEGATIVE CONTRACT: out-of-range u_bit / opcode must
        // error. u_bit occupies the single bit [29]; opcode the 5-bit field
        // [15:11]. Larger values cannot be encoded and would silently
        // corrupt adjacent fields (Q[30], Rm[20:16]). The encoder must
        // reject them rather than emit a malformed word.
        #[test]
        fn prop_out_of_range_u_bit_and_opcode_must_error(extra in 1u32..=4u32) {
            let ops = || vec![vreg(0, "4s"), vreg(1, "4s"), vreg(2, "4s")];

            let res_u = encode_neon_three_same(&ops(), 1 + extra, 0);
            prop_assert!(res_u.is_err(),
                "out-of-range u_bit {} (field is 1 bit) must error, got {:?}",
                1 + extra, res_u);

            let res_o = encode_neon_three_same(&ops(), 0, 0x1F + extra);
            prop_assert!(res_o.is_err(),
                "out-of-range opcode 0x{:x} (field is 5 bits) must error, got {:?}",
                0x1F + extra, res_o);
        }
    }
}


#[cfg(test)]
mod prop_encode_neon_aes_tests {
    use super::*;
    use proptest::prelude::*;

    // ORACLE: ARMv8-A ARM, AES cryptographic instructions.
    //   AESE/AESD/AESMC/AESIMC <Vd>.16B, <Vn>.16B
    //   0100 1110 0010 1000 opcode 10 Rn Rd
    //    31----------------24 23----16 15--12 11-10 9-5 4-0
    // Field masks (derived from the four canonical words below):
    //   Rd -> [4:0],  Rn -> [9:5],  opcode -> [16:12] (only 00100..00111 allocated),
    //   fixed "10" at [11:10],  prefix bits [31:17] constant.
    // Golden words (Vd = Vn = V0.16B), taken straight from the ARM ARM examples:
    //   AESE 0x4E284800  AESD 0x4E285800  AESMC 0x4E286800  AESIMC 0x4E287800
    const AESE: u32 = 0x4E284800;
    const AESD: u32 = 0x4E285800;
    const AESMC: u32 = 0x4E286800;
    const AESIMC: u32 = 0x4E287800;
    // Everything except opcode[16:12], Rn[9:5], Rd[4:0]:
    const FIXED_TEMPLATE: u32 = 0x4E28_0800;
    const FIXED_PREFIX: u32 = 0x4E284800 >> 17; // == 0x2714, bits [31:17]
    const NON_16B: &[&str] = &["8b", "4h", "8h", "2s", "4s", "1d", "2d"];

    fn vreg(num: u32, arr: &str) -> Operand {
        Operand::RegArrangement {
            reg: format!("v{}", num),
            arrangement: arr.to_string(),
        }
    }

    fn word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    // Independent spec oracle: place opcode at [16:12], Rn at [9:5], Rd at [4:0].
    fn ref_word(rd: u32, rn: u32, opcode: u32) -> u32 {
        FIXED_TEMPLATE | (opcode << 12) | (rn << 5) | rd
    }

    prop_compose! {
        fn vreg_num()(n in 0u32..=31u32) -> u32 { n }
    }

    proptest! {
        // Property 1 — Known-answer against the ARM ARM golden words for all
        // four mnemonics (Vd = Vn = V0.16B). These constants come directly from
        // the architecture reference, independent of the encoder's logic.
        #[test]
        fn prop_golden_known_answer(opcode in prop::sample::select(&[4u32, 5, 6, 7])) {
            let expected = match opcode {
                4 => AESE, 5 => AESD, 6 => AESMC, 7 => AESIMC, _ => unreachable!(),
            };
            let ops = vec![vreg(0, "16b"), vreg(0, "16b")];
            prop_assert_eq!(word(encode_neon_aes(&ops, opcode)), expected);
        }

        // Property 2 — Differential against the independent ARM ARM field
        // oracle, across all allocated opcodes and the full register range.
        #[test]
        fn prop_matches_spec_oracle(
            opcode in prop::sample::select(&[4u32, 5, 6, 7]),
            rd in vreg_num(), rn in vreg_num(),
        ) {
            let ops = vec![vreg(rd, "16b"), vreg(rn, "16b")];
            prop_assert_eq!(word(encode_neon_aes(&ops, opcode)), ref_word(rd, rn, opcode));
        }

        // Property 3 — Layout invariants: prefix bits [31:17] are constant;
        // Rd maps to [4:0] and Rn to [9:5] independently over 0..=31; the
        // opcode field [16:12] is untouched by the register operands.
        #[test]
        fn prop_register_and_prefix_layout(
            opcode in prop::sample::select(&[4u32, 5, 6, 7]),
            rd in vreg_num(), rn in vreg_num(),
        ) {
            let ops = vec![vreg(rd, "16b"), vreg(rn, "16b")];
            let w = word(encode_neon_aes(&ops, opcode));
            prop_assert_eq!(w >> 17, FIXED_PREFIX, "prefix bits [31:17] must be constant");
            prop_assert_eq!(w & 0x1F, rd, "Rd must map to bits [4:0]");
            prop_assert_eq!((w >> 5) & 0x1F, rn, "Rn must map to bits [9:5]");
            prop_assert_eq!((w >> 12) & 0x1F, opcode, "opcode must map to bits [16:12]");
        }

        // Property 4 — Arity contract: fewer than 2 operands must error.
        #[test]
        fn prop_arity_contract(n in 0usize..=1) {
            let ops: Vec<Operand> = (0..n).map(|_| vreg(0, "16b")).collect();
            let res = encode_neon_aes(&ops, 0b00100);
            prop_assert!(res.is_err(), "{} operands must error, got {:?}", n, res);
        }

        // Property 5 — NEGATIVE CONTRACT (findings). The AES encoding space
        // allocates ONLY opcodes 00100..00111; any other value is UNDEF at
        // runtime (ARM ARM). Likewise AESE/AESD/AESMC/AESIMC require the .16B
        // arrangement exclusively. The encoder must reject (a) unallocated
        // opcodes and (b) non-.16B arrangements rather than silently emit a
        // malformed / unallocated word. EXPECTED TO FAIL: the encoder performs
        // neither check today.
        #[test]
        #[ignore = "documented bug: AES accepts unallocated opcodes and non-.16B arrangements"]
        fn prop_unallocated_opcode_and_arrangement_must_error(
            bad_opcode in (0u32..32u32)
                .prop_filter("unallocated opcode", |o| !(*o >= 4 && *o <= 7)),
            bad_arr in prop::sample::select(NON_16B),
        ) {
            let good = vec![vreg(0, "16b"), vreg(1, "16b")];
            let res_op = encode_neon_aes(&good, bad_opcode);
            prop_assert!(res_op.is_err(),
                "unallocated AES opcode {:#07b} must error (only 00100..00111 valid), got {:?}",
                bad_opcode, res_op);

            let bad = vec![vreg(0, bad_arr), vreg(1, bad_arr)];
            let res_arr = encode_neon_aes(&bad, 0b00100);
            prop_assert!(res_arr.is_err(),
                "AES requires .16B; arrangement {} must error, got {:?}",
                bad_arr, res_arr);
        }
    }
}

// ── encode_neon_shift_right: property-based tests ────────────────────────
//
// Target: `encode_neon_shift_right(operands, u_bit, opcode)` encodes the
// AArch64 Advanced-SIMD "shift right by immediate" family dispatched in
// mod.rs: SRSHR/URSHR, SSRA/USRA, SRSRA/URSRA.
//
// ARM ARM layout (Advanced SIMD shift by immediate):
//   31  30  29  28-23   22-19  18-16   15-11   10   9-5  4-0
//   0   Q   U   011110  immh   immb    opcode   1    Rn   Rd
//
// Callers pass a *6-bit* opcode value whose low bit is the fixed '1' at
// bit 10 (SRSHR=0b001001, SSRA=0b000101, SRSRA=0b001101), so
// `(opcode << 10)` puts opcode[5:1] at bits 15-11 and opcode[0]=1 at bit 10.
// immh:immb (7 bits, 22-16) = (esize*2) - shift, shift in 1..=esize.
#[cfg(test)]
mod shift_right_pbt_tests {
    use super::*;
    use proptest::prelude::*;

    // (arrangement, element_bits, Q) for every arrangement the encoder accepts.
    const ARRANGEMENTS: &[(&str, u32, u32)] = &[
        ("8b", 8, 0), ("16b", 8, 1),
        ("4h", 16, 0), ("8h", 16, 1),
        ("2s", 32, 0), ("4s", 32, 1),
        ("2d", 64, 1),
    ];
    // Opcode values from the real dispatch table in mod.rs (6-bit, low bit=1).
    const OPCODES: &[u32] = &[0b001001, 0b000101, 0b001101];

    fn vreg(num: u32, arr: &str) -> Operand {
        Operand::RegArrangement { reg: format!("v{}", num), arrangement: arr.to_string() }
    }

    fn encode(rd: u32, rn: u32, arr: &str, shift: i64, u_bit: u32, opcode: u32) -> Result<u32, String> {
        let ops = vec![vreg(rd, arr), vreg(rn, arr), Operand::Imm(shift)];
        match encode_neon_shift_right(&ops, u_bit, opcode) {
            Ok(EncodeResult::Word(w)) => Ok(w),
            Ok(other) => Err(format!("unexpected non-Word result: {:?}", other)),
            Err(e) => Err(e),
        }
    }

    // Independent reconstruction straight from the ARM ARM field layout.
    // `(immhb << 16)` places the 7-bit immh:immb at bits 22-16 — written as a
    // single shift rather than the encoder's split `(>>3)<<19 | (&7)<<16` form.
    fn ref_word(rd: u32, rn: u32, q: u32, element_bits: u32, shift: u32, u_bit: u32, opcode: u32) -> u32 {
        let immhb = (element_bits * 2) - shift; // bits 22-16
        (q << 30) | (u_bit << 29) | (0b011110u32 << 23) | (immhb << 16)
            | (opcode << 10) | (rn << 5) | rd
    }

    proptest! {
        // 1. Differential oracle: for every accepted arrangement, every register
        //    pair, both U values, all dispatched opcodes, and every in-range
        //    shift, the encoded word equals the independent reference.
        #[test]
        fn prop_matches_reference(
            rd in 0u32..32u32,
            rn in 0u32..32u32,
            u_bit in 0u32..2u32,
            shift_factor in 1u32..1000u32,
        ) {
            for &(arr, element_bits, q) in ARRANGEMENTS {
                let shift = (shift_factor % element_bits) + 1; // 1..=element_bits
                for &opcode in OPCODES {
                    let word = encode(rd, rn, arr, shift as i64, u_bit, opcode)
                        .expect("valid shift-right must encode");
                    prop_assert_eq!(word, ref_word(rd, rn, q, element_bits, shift, u_bit, opcode));
                }
            }
        }

        // 2. immh:immb invariant: bits 22-16 must equal (esize*2) - shift, i.e.
        //    the shift amount is recoverable from the encoded word.
        #[test]
        fn prop_immhb_field(
            u_bit in 0u32..2u32,
            shift_factor in 1u32..1000u32,
        ) {
            for &(arr, element_bits, _q) in ARRANGEMENTS {
                let shift = (shift_factor % element_bits) + 1;
                for &opcode in OPCODES {
                    let word = encode(3, 5, arr, shift as i64, u_bit, opcode).unwrap();
                    let immhb = (word >> 16) & 0x7F;
                    prop_assert_eq!(immhb, element_bits * 2 - shift,
                        "arr={} shift={} immh:immb mismatch", arr, shift);
                }
            }
        }

        // 3. Field independence: Rd (4-0), Rn (9-5), opcode (15-10), the fixed
        //    011110 (28-23) and bit-31=0 must not collide with each other or with
        //    the size/shift fields for any register numbering.
        #[test]
        fn prop_fields_isolated(
            rd in 0u32..32u32,
            rn in 0u32..32u32,
            u_bit in 0u32..2u32,
        ) {
            for &(arr, element_bits, _q) in ARRANGEMENTS {
                let shift = element_bits; // max valid shift
                for &opcode in OPCODES {
                    let word = encode(rd, rn, arr, shift as i64, u_bit, opcode).unwrap();
                    prop_assert_eq!(word & 0x1F, rd, "Rd field");
                    prop_assert_eq!((word >> 5) & 0x1F, rn, "Rn field");
                    prop_assert_eq!((word >> 10) & 0x3F, opcode, "opcode field (15-10)");
                    prop_assert_eq!((word >> 23) & 0x3F, 0b011110u32, "fixed bits 28-23");
                    prop_assert_eq!(word >> 31, 0u32, "bit 31 must be 0");
                }
            }
        }

        // 4. Negative contract: natural out-of-range shifts and malformed
        //    operands are rejected for every element size. (All pass — confirms
        //    the shift==0 / shift>esize / bad-arrangement / arity guards.)
        #[test]
        fn prop_out_of_range_rejected(
            over in 1u32..1024u32,
            neg in (-1000i64)..=(-1i64),
        ) {
            for &(arr, element_bits, _q) in ARRANGEMENTS {
                prop_assert!(encode(0, 1, arr, 0, 0, 0b001001).is_err(),
                    "shift 0 must be rejected for {}", arr);
                let big = (element_bits + over) as i64;
                prop_assert!(encode(0, 1, arr, big, 0, 0b001001).is_err(),
                    "shift {} (>{}) must be rejected for {}", big, element_bits, arr);
                prop_assert!(encode(0, 1, arr, neg, 0, 0b001001).is_err(),
                    "negative shift {} must be rejected for {}", neg, arr);
            }
            // .1d is not a valid shift-right source arrangement
            prop_assert!(encode(0, 1, "1d", 1, 0, 0b001001).is_err());
            // too few operands
            let short = vec![vreg(0, "4s"), vreg(1, "4s")];
            prop_assert!(encode_neon_shift_right(&short, 0, 0b001001).is_err());
            // shift slot is not an immediate
            let wrong = vec![vreg(0, "4s"), vreg(1, "4s"), vreg(2, "4s")];
            prop_assert!(encode_neon_shift_right(&wrong, 0, 0b001001).is_err());
        }

        // 5. Silent-truncation gap: the immediate is read as i64 and cast to
        //    u32 *before* the range check (`let shift = get_imm(...)? as u32`).
        //    An i64 immediate whose low 32 bits land in 1..=esize is accepted
        //    even though the true value is far out of range. A conforming
        //    assembler rejects such immediates. This property documents the gap
        //    (it currently FAILS).
        #[test]
        #[ignore = "documented bug: shift-right casts large i64 immediates to u32 before validation"]
        fn prop_huge_immediate_not_truncated(
            k in 1u64..=4u64,
            base in 1u32..64u32,
        ) {
            for &(arr, element_bits, _q) in ARRANGEMENTS {
                let in_range_base = ((base - 1) % element_bits) + 1; // 1..=esize
                let huge = ((k << 32) as i64) + in_range_base as i64; // >= 2^32
                let res = encode(0, 1, arr, huge, 0, 0b001001);
                prop_assert!(res.is_err(),
                    "i64 shift {} (truncates to u32 {}) must be rejected for {}-bit elems, got {:?}",
                    huge, in_range_base, element_bits, res);
            }

        }
    }
}
