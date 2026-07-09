use super::*;
use crate::backend::arm::assembler::parser::Operand;

// ── Floating point ───────────────────────────────────────────────────────

pub(crate) fn encode_fmov(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("fmov requires 2 operands".to_string());
    }

    let (rd_name, rm_name) = match (&operands[0], &operands[1]) {
        (Operand::Reg(a), Operand::Reg(b)) => (a.clone(), b.clone()),
        (Operand::Reg(_a), Operand::Imm(_)) => {
            // TODO: implement fmov with float immediate encoding
            return Err("fmov with immediate operand not yet supported".to_string());
        }
        _ => return Err("fmov needs register operands".to_string()),
    };

    let rd = parse_reg_num(&rd_name).ok_or("invalid rd")?;
    let rm = parse_reg_num(&rm_name).ok_or("invalid rm")?;

    let rd_is_fp = is_fp_reg(&rd_name);
    let rm_is_fp = is_fp_reg(&rm_name);
    let rd_lower = rd_name.to_lowercase();
    let rm_lower = rm_name.to_lowercase();

    if rd_is_fp && rm_is_fp {
        // FMOV between FP registers
        let is_double = rd_lower.starts_with('d') || rm_lower.starts_with('d');
        let ftype = if is_double { 0b01 } else { 0b00 };
        // 0 00 11110 ftype 1 0000 00 10000 Rn Rd
        let word = (0b00011110 << 24) | (ftype << 22) | (0b100000 << 16) | (0b10000 << 10) | (rm << 5) | rd;
        return Ok(EncodeResult::Word(word));
    }

    if rd_is_fp && !rm_is_fp {
        // FMOV from GP to FP: FMOV Dn, Xn or FMOV Sn, Wn
        let is_double = rd_lower.starts_with('d');
        if is_double {
            // FMOV Dd, Xn: 1 00 11110 01 1 00 111 000000 Rn Rd
            let word = ((0b1001111001 << 22) | (0b100111 << 16)) | (rm << 5) | rd;
            return Ok(EncodeResult::Word(word));
        } else {
            // FMOV Sd, Wn: 0 00 11110 00 1 00 111 000000 Rn Rd
            let word = ((0b0001111000 << 22) | (0b100111 << 16)) | (rm << 5) | rd;
            return Ok(EncodeResult::Word(word));
        }
    }

    if !rd_is_fp && rm_is_fp {
        // FMOV from FP to GP: FMOV Xn, Dn or FMOV Wn, Sn
        let is_double = rm_lower.starts_with('d');
        if is_double {
            // FMOV Xd, Dn: 1 00 11110 01 1 00 110 000000 Rn Rd
            let word = ((0b1001111001 << 22) | (0b100110 << 16)) | (rm << 5) | rd;
            return Ok(EncodeResult::Word(word));
        } else {
            // FMOV Wd, Sn: 0 00 11110 00 1 00 110 000000 Rn Rd
            let word = ((0b0001111000 << 22) | (0b100110 << 16)) | (rm << 5) | rd;
            return Ok(EncodeResult::Word(word));
        }
    }

    Err(format!("unsupported fmov operands: {} -> {}", rd_name, rm_name))
}

pub(crate) fn encode_fp_arith(operands: &[Operand], opcode: u32) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;

    let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
    let is_double = rd_name.starts_with('d');
    let ftype = if is_double { 0b01 } else { 0b00 };

    // 0 00 11110 ftype 1 Rm opcode 10 Rn Rd
    let word = (0b00011110 << 24) | (ftype << 22) | (1 << 21) | (rm << 16) | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_fneg(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
    let is_double = rd_name.starts_with('d');
    let ftype = if is_double { 0b01 } else { 0b00 };
    // FNEG: 0 00 11110 ftype 1 0000 10 10000 Rn Rd
    let word = (0b00011110 << 24) | (ftype << 22) | (0b100001 << 16) | (0b10000 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_fabs(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
    let is_double = rd_name.starts_with('d');
    let ftype = if is_double { 0b01 } else { 0b00 };
    // FABS: 0 00 11110 ftype 1 0000 01 10000 Rn Rd
    let word = (0b00011110 << 24) | (ftype << 22) | (0b100000 << 16) | (0b110000 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_fsqrt(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
    let is_double = rd_name.starts_with('d');
    let ftype = if is_double { 0b01 } else { 0b00 };
    // FSQRT: 0 00 11110 ftype 1 0000 11 10000 Rn Rd
    let word = (0b00011110 << 24) | (ftype << 22) | (0b100001 << 16) | (0b110000 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode FP 1-source ops: FRINTN/P/M/Z/A/X/I
/// Format: 0 00 11110 ftype 1 opcode 10000 Rn Rd
pub(crate) fn encode_fp_1src(operands: &[Operand], opcode: u32) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
    let is_double = rd_name.starts_with('d');
    let ftype = if is_double { 0b01u32 } else { 0b00 };
    let word = (0b00011110u32 << 24) | (ftype << 22) | (1 << 21)
        | (opcode << 15) | (0b10000 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode FMADD/FMSUB: Rd = Ra +/- (Rn * Rm)
/// Format: 0 00 11111 ftype 0 Rm o1 Ra Rn Rd
pub(crate) fn encode_fmadd_fmsub(operands: &[Operand], is_sub: bool) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let (ra, _) = get_reg(operands, 3)?;
    let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
    let is_double = rd_name.starts_with('d');
    let ftype = if is_double { 0b01u32 } else { 0b00 };
    let o1 = if is_sub { 1u32 } else { 0 };
    let word = (0b00011111u32 << 24) | (ftype << 22) | (rm << 16)
        | (o1 << 15) | (ra << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

/// Encode FNMADD/FNMSUB: Rd = -Ra +/- (Rn * Rm)
/// Format: 0 00 11111 ftype 1 Rm o1 Ra Rn Rd
pub(crate) fn encode_fnmadd_fnmsub(operands: &[Operand], is_sub: bool) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let (ra, _) = get_reg(operands, 3)?;
    let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
    let is_double = rd_name.starts_with('d');
    let ftype = if is_double { 0b01u32 } else { 0b00 };
    let o1 = if is_sub { 1u32 } else { 0 };
    let word = (0b00011111u32 << 24) | (ftype << 22) | (1 << 21) | (rm << 16)
        | (o1 << 15) | (ra << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_fcmp(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rn, _) = get_reg(operands, 0)?;
    let rn_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
    let is_double = rn_name.starts_with('d');
    let ftype = if is_double { 0b01 } else { 0b00 };

    // FCMP Dn, #0.0
    if operands.len() < 2 || matches!(operands.get(1), Some(Operand::Imm(0))) {
        let word = ((0b00011110 << 24) | (ftype << 22) | (1 << 21)) | (0b001000 << 10) | (rn << 5) | 0b01000;
        return Ok(EncodeResult::Word(word));
    }

    let (rm, _) = get_reg(operands, 1)?;
    // FCMP Dn, Dm: 0 00 11110 ftype 1 Rm 00 1000 Rn 00 000
    let word = (0b00011110 << 24) | (ftype << 22) | (1 << 21) | (rm << 16) | (0b001000 << 10) | (rn << 5);
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_fcvt_rounding(operands: &[Operand], rmode: u32, opcode: u32) -> Result<EncodeResult, String> {
    // Float-to-integer conversion with specified rounding mode
    // Encoding: sf 00 11110 ftype 1 rmode opcode 000000 Rn Rd
    // sf: 0=W dest, 1=X dest
    // ftype: 00=S source, 01=D source
    // rmode+opcode: determines rounding mode and signedness
    if operands.len() < 2 {
        return Err("fcvt* requires 2 operands".to_string());
    }
    let (rd, rd_is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;

    let src_name = match &operands[1] {
        Operand::Reg(name) => name.to_lowercase(),
        _ => return Err("fcvt*: expected register source".to_string()),
    };
    let ftype: u32 = if src_name.starts_with('d') { 0b01 } else { 0b00 };
    let sf: u32 = if rd_is_64 { 1 } else { 0 };

    let word = ((sf << 31) | (0b11110 << 24) | (ftype << 22)
        | (1 << 21) | (rmode << 19) | (opcode << 16)) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_ucvtf(operands: &[Operand]) -> Result<EncodeResult, String> {
    encode_int_to_float(operands, false)
}

pub(crate) fn encode_scvtf(operands: &[Operand]) -> Result<EncodeResult, String> {
    encode_int_to_float(operands, true)
}

pub(crate) fn encode_int_to_float(operands: &[Operand], is_signed: bool) -> Result<EncodeResult, String> {
    // SCVTF/UCVTF: integer-to-float conversion
    // Encoding: sf 00 11110 ftype 1 00 opcode 000000 Rn Rd
    // sf: 0=W source, 1=X source
    // ftype: 00=S dest, 01=D dest
    // opcode: 010=signed (SCVTF), 011=unsigned (UCVTF)
    if operands.len() < 2 {
        return Err("scvtf/ucvtf requires 2 operands".to_string());
    }
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, rn_is_64) = get_reg(operands, 1)?;

    let dst_name = match &operands[0] {
        Operand::Reg(name) => name.to_lowercase(),
        _ => return Err("scvtf/ucvtf: expected register dest".to_string()),
    };
    let ftype: u32 = if dst_name.starts_with('d') { 0b01 } else { 0b00 };
    let sf: u32 = if rn_is_64 { 1 } else { 0 };
    let opcode: u32 = if is_signed { 0b010 } else { 0b011 };

    let word = (((sf << 31) | (0b11110 << 24) | (ftype << 22)
        | (1 << 21)) | (opcode << 16)) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_fcvt_precision(operands: &[Operand]) -> Result<EncodeResult, String> {
    // FCVT: float precision conversion (e.g., FCVT Dd, Sn or FCVT Sd, Dn)
    // Encoding: 0 00 11110 ftype 1 0001 opc 10000 Rn Rd
    // ftype: source precision (00=S, 01=D, 11=H)
    // opc: dest precision (00=S, 01=D, 11=H)
    if operands.len() < 2 {
        return Err("fcvt requires 2 operands".to_string());
    }
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;

    let dst_name = match &operands[0] {
        Operand::Reg(name) => name.to_lowercase(),
        _ => return Err("fcvt: expected register dest".to_string()),
    };
    let src_name = match &operands[1] {
        Operand::Reg(name) => name.to_lowercase(),
        _ => return Err("fcvt: expected register source".to_string()),
    };

    let ftype: u32 = match src_name.chars().next() {
        Some('s') => 0b00,
        Some('d') => 0b01,
        Some('h') => 0b11,
        _ => return Err(format!("fcvt: unsupported source type: {}", src_name)),
    };
    let opc: u32 = match dst_name.chars().next() {
        Some('s') => 0b00,
        Some('d') => 0b01,
        Some('h') => 0b11,
        _ => return Err(format!("fcvt: unsupported dest type: {}", dst_name)),
    };

    let word = (0b00011110 << 24) | (ftype << 22) | (1 << 21) | (0b0001 << 17)
        | (opc << 15) | (0b10000 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ── FMOV field extractors (ARMv8 encoding layout) ───────────────────────
    // All FMOV variants share: bits[4:0]=Rd, bits[9:5]=Rn(source), bits[15:10]=opcode,
    // bits[18:16]=rmode, bit[21]=1, bits[23:22]=ftype, bit[31]=sf.
    fn rd_of(w: u32) -> u32    { w & 0x1F }
    fn rn_of(w: u32) -> u32    { (w >> 5) & 0x1F }      // source register field
    fn opcode_of(w: u32) -> u32 { (w >> 10) & 0x3F }
    fn rmode_of(w: u32) -> u32 { (w >> 16) & 0x7 }
    fn ftype_of(w: u32) -> u32 { (w >> 22) & 0x3 }
    fn sf_of(w: u32) -> u32    { (w >> 31) & 1 }

    fn expect_word(r: Result<EncodeResult, String>) -> u32 {
        match r {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {:?}", other),
        }
    }

    proptest! {
        // FMOV <Sd>, <Sn>: 0 00 11110 00 1 0000 00 10000 Rn Rd  (= 0x1E204000 | src<<5 | dst)
        #[test]
        fn prop_fmov_fp_to_fp_single_places_fields(src in 0u32..32, dst in 0u32..32) {
            let ops = vec![Operand::Reg(format!("s{}", dst)), Operand::Reg(format!("s{}", src))];
            let w = expect_word(encode_fmov(&ops));
            let base = (0b00011110u32 << 24) | (0b100000 << 16) | (0b10000 << 10);
            prop_assert_eq!(w, base | (src << 5) | dst);
            // Round-trip: source lands in Rn field, dest in Rd field, no truncation.
            prop_assert_eq!(rn_of(w), src);
            prop_assert_eq!(rd_of(w), dst);
            // Single precision => ftype == 00.
            prop_assert_eq!(ftype_of(w), 0b00);
        }

        // FMOV <Dd>, <Dn>: 0 00 11110 01 1 0000 00 10000 Rn Rd  (= 0x1E604000 | src<<5 | dst)
        #[test]
        fn prop_fmov_fp_to_fp_double_sets_ftype(src in 0u32..32, dst in 0u32..32) {
            let ops = vec![Operand::Reg(format!("d{}", dst)), Operand::Reg(format!("d{}", src))];
            let w = expect_word(encode_fmov(&ops));
            let base = (0b00011110u32 << 24) | (0b01 << 22) | (0b100000 << 16) | (0b10000 << 10);
            prop_assert_eq!(w, base | (src << 5) | dst);
            // Double precision => ftype == 01, i.e. bit 22 must be set.
            prop_assert_eq!(ftype_of(w), 0b01);
            prop_assert_eq!(rn_of(w), src);
            prop_assert_eq!(rd_of(w), dst);
        }

        // FMOV (general) GP -> FP: sf 00 11110 ftype 1 00 111 000000 Rn Rd
        // Dd<-Xn (sf=1,ftype=01) and Sd<-Wn (sf=0,ftype=00); rmode==111.
        #[test]
        fn prop_fmov_gp_to_fp_uses_rmode111(src in 0u32..32, dst in 0u32..32, dbl in any::<bool>()) {
            let (dst_reg, src_reg, sf, ftype) = if dbl {
                (format!("d{}", dst), format!("x{}", src), 1u32, 0b01u32)
            } else {
                (format!("s{}", dst), format!("w{}", src), 0u32, 0b00u32)
            };
            let ops = vec![Operand::Reg(dst_reg), Operand::Reg(src_reg)];
            let w = expect_word(encode_fmov(&ops));
            prop_assert_eq!(sf_of(w), sf);
            prop_assert_eq!(ftype_of(w), ftype);
            prop_assert_eq!(rmode_of(w), 0b111);   // GP->FP conversion select
            prop_assert_eq!(opcode_of(w), 0);       // 000000
            prop_assert_eq!(rn_of(w), src);
            prop_assert_eq!(rd_of(w), dst);
        }

        // FMOV (general) FP -> GP: sf 00 11110 ftype 1 00 110 000000 Rn Rd
        // Xd<-Dn (sf=1,ftype=01) and Wd<-Sn (sf=0,ftype=00); rmode==110.
        #[test]
        fn prop_fmov_fp_to_gp_uses_rmode110(src in 0u32..32, dst in 0u32..32, dbl in any::<bool>()) {
            let (dst_reg, src_reg, sf, ftype) = if dbl {
                (format!("x{}", dst), format!("d{}", src), 1u32, 0b01u32)
            } else {
                (format!("w{}", dst), format!("s{}", src), 0u32, 0b00u32)
            };
            let ops = vec![Operand::Reg(dst_reg), Operand::Reg(src_reg)];
            let w = expect_word(encode_fmov(&ops));
            prop_assert_eq!(sf_of(w), sf);
            prop_assert_eq!(ftype_of(w), ftype);
            prop_assert_eq!(rmode_of(w), 0b110);   // FP->GP conversion select
            prop_assert_eq!(opcode_of(w), 0);
            prop_assert_eq!(rn_of(w), src); // FP source in Rn field
            prop_assert_eq!(rd_of(w), dst); // GP dest in Rd field
        }

        // Negative / arity / range contract: immediates, too-few operands, and
        // out-of-range register numbers must all be rejected (no silent truncation).
        #[test]
        fn prop_fmov_rejects_immediate_arity_and_out_of_range(
            imm in any::<i64>(), n in 0u32..200u32
        ) {
            // Immediate operand: code documents this as unsupported -> Err.
            let imm_ops = vec![Operand::Reg("s0".into()), Operand::Imm(imm)];
            prop_assert!(encode_fmov(&imm_ops).is_err());

            // Arity: 0 or 1 operands -> Err ("requires 2 operands").
            prop_assert!(encode_fmov(&[]).is_err());
            prop_assert!(encode_fmov(&[Operand::Reg("s0".into())]).is_err());

            // Out-of-range register numbers must NOT be truncated into the 5-bit field.
            if n > 31 {
                let ops = vec![Operand::Reg(format!("s{}", n)), Operand::Reg("s0".into())];
                prop_assert!(
                    encode_fmov(&ops).is_err(),
                    "register s{} should be rejected, not masked into 5 bits", n
                );
            }
        }
        // Width/precision negative contract: FMOV's GP/FP transfer forms only
        // allow W<->S and X<->D, and FP<->FP requires matching precision.
        #[test]
        fn prop_fmov_rejects_width_precision_mismatch(n in 0u32..32) {
            let cases = [
                vec![Operand::Reg(format!("d{}", n)), Operand::Reg(format!("w{}", n))],
                vec![Operand::Reg(format!("s{}", n)), Operand::Reg(format!("x{}", n))],
                vec![Operand::Reg(format!("w{}", n)), Operand::Reg(format!("d{}", n))],
                vec![Operand::Reg(format!("x{}", n)), Operand::Reg(format!("s{}", n))],
                vec![Operand::Reg(format!("d{}", n)), Operand::Reg(format!("s{}", n))],
                vec![Operand::Reg(format!("s{}", n)), Operand::Reg(format!("d{}", n))],
            ];
            for ops in cases {
                prop_assert!(encode_fmov(&ops).is_err(), "mismatched FMOV operands should be Err, got {:?}", encode_fmov(&ops));
            }
        }
    }

    // ── encode_fp_arith (FP data-processing, 2-source) =====================
    // Layout: 0 00 11110 ftype 1 Rm opcode 10 Rn Rd
    //   bits[4:0]=Rd, [9:5]=Rn, [11:10]=0b10, [15:12]=opcode(4-bit),
    //   [20:16]=Rm, [21]=1, [23:22]=ftype, [30:24]=0b00011110, [31]=0.
    fn fp_rm_of(w: u32) -> u32   { (w >> 16) & 0x1F }
    fn fp_opc_of(w: u32) -> u32  { (w >> 12) & 0xF }
    fn fp_fixed_of(w: u32) -> u32 { (w >> 10) & 0b11 }

    proptest! {
        // Oracle: reference / field layout. Homogeneous-precision FP
        // operands + valid 4-bit opcode => every field at its canonical bit
        // position with no truncation.
        #[test]
        fn prop_fp_arith_places_fields(
            rd in 0u32..32, rn in 0u32..32, rm in 0u32..32,
            opcode in 0u32..16, dbl in any::<bool>(),
        ) {
            let p = if dbl { "d" } else { "s" };
            let ops = vec![
                Operand::Reg(format!("{}{}", p, rd)),
                Operand::Reg(format!("{}{}", p, rn)),
                Operand::Reg(format!("{}{}", p, rm)),
            ];
            let w = expect_word(encode_fp_arith(&ops, opcode));

            // Fixed bits of the scalar FP data-processing (2-source) encoding.
            prop_assert_eq!(w >> 24, 0b0001_1110u32); // [31:24] = 0x1E (sf=0 + 00011110)
            prop_assert_eq!((w >> 21) & 1, 1u32);      // bit 21 = 1
            prop_assert_eq!(fp_fixed_of(w), 0b10u32); // [11:10] = 0b10

            // Register fields round-trip exactly into their 5-bit slots.
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(fp_rm_of(w), rm);

            // Opcode round-trips into the 4-bit [15:12] slot.
            prop_assert_eq!(fp_opc_of(w), opcode);
        }

        // Oracle: reference / precision. ftype is derived solely from the
        // destination register: 'd' => 01 (double), else => 00 (single).
        #[test]
        fn prop_fp_arith_ftype_from_dest(
            rd in 0u32..32, rn in 0u32..32, rm in 0u32..32, dbl in any::<bool>(),
        ) {
            let p = if dbl { "d" } else { "s" };
            let ops = vec![
                Operand::Reg(format!("{}{}", p, rd)),
                Operand::Reg(format!("{}{}", p, rn)),
                Operand::Reg(format!("{}{}", p, rm)),
            ];
            let w = expect_word(encode_fp_arith(&ops, 0b0010));
            prop_assert_eq!(ftype_of(w), if dbl { 0b01 } else { 0b00 });
            prop_assert_eq!(sf_of(w), 0); // scalar FP, never sf=1
        }

        // Oracle: determinism. Same operands + opcode => identical word.
        #[test]
        fn prop_fp_arith_is_deterministic(
            rd in 0u32..32, rn in 0u32..32, rm in 0u32..32, opcode in 0u32..16,
        ) {
            let ops = vec![
                Operand::Reg(format!("d{}", rd)),
                Operand::Reg(format!("d{}", rn)),
                Operand::Reg(format!("d{}", rm)),
            ];
            let w1 = expect_word(encode_fp_arith(&ops, opcode));
            let w2 = expect_word(encode_fp_arith(&ops, opcode));
            prop_assert_eq!(w1, w2);
        }

        // Negative contract (validated): out-of-range FP register numbers
        // (>= 32) MUST be rejected, not masked into the 5-bit field.
        #[test]
        fn prop_fp_arith_rejects_out_of_range_reg(n in 32u32..256u32, pos in 0u32..3u32) {
            let mut names = vec!["d0".to_string(), "d0".to_string(), "d0".to_string()];
            names[pos as usize] = format!("d{}", n);
            let ops: Vec<Operand> = names.into_iter().map(Operand::Reg).collect();
            prop_assert!(
                encode_fp_arith(&ops, 0b0010).is_err(),
                "register d{} must be rejected (5-bit field), not silently masked", n
            );
        }

        // Negative contract (FINDING — see BUGS_fp_arith.md): FP arithmetic
        // must reject non-FP (GP) operands, mixed-precision operands, and
        // opcodes that overflow the 4-bit opcode field. Currently NONE of
        // these are checked, so this property FAILS by design.
        #[test]
        fn prop_fp_arith_rejects_wrong_banks_precision_and_oversized_opcode(
            n in 0u32..32, bad_opcode in 16u32..256u32,
        ) {
            // 1. GP-register operands are not valid for FP arithmetic.
            let gp = vec![
                Operand::Reg(format!("x{}", n)),
                Operand::Reg(format!("x{}", n)),
                Operand::Reg(format!("x{}", n)),
            ];
            prop_assert!(
                encode_fp_arith(&gp, 0b0010).is_err(),
                "GP registers (x{}) are not valid FP operands", n
            );

            // 2. Mixed precision across dest/source must be rejected.
            let mix = vec![
                Operand::Reg(format!("d{}", n)),
                Operand::Reg(format!("s{}", n)),
                Operand::Reg(format!("s{}", n)),
            ];
            prop_assert!(
                encode_fp_arith(&mix, 0b0010).is_err(),
                "mixed precision (Dd, Sn, Sm) must be rejected"
            );

            // 3. opcode must fit 4 bits; >= 16 corrupts the Rm field.
            let ok = vec![
                Operand::Reg("d0".into()),
                Operand::Reg("d0".into()),
                Operand::Reg("d0".into()),
            ];
            prop_assert!(
                encode_fp_arith(&ok, bad_opcode).is_err(),
                "opcode {} must be rejected (4-bit field), not OR'd into Rm", bad_opcode
            );
        }
    }

    // ── encode_fneg (FP data-processing, 1 source: FNEG) ===================
    // ARMv8-A "Floating-point data-processing (1 source)" layout:
    //   0 00 11110 ftype 1 opcode 10000 Rn Rd
    //   bits[31]=0, bits[30:24]=0011110 (0x1E), bits[23:22]=ftype,
    //   bit[21]=1, bits[20:15]=opcode (6-bit, FNEG=000010), bits[14:10]=10000,
    //   bits[9:5]=Rn (src), bits[4:0]=Rd (dst).
    //   Cross-checked: FNEG S0,S0 = 0x1E214000, FNEG D0,D0 = 0x1E614000.
    fn fneg_opcode_of(w: u32) -> u32 { (w >> 15) & 0x3F }
    fn fneg_fixed_of(w: u32) -> u32  { (w >> 10) & 0x1F }

    proptest! {
        // Oracle: reference / field layout. Homogeneous-precision FP operands
        // => every field at its canonical ARMv8 bit position, opcode == FNEG.
        #[test]
        fn prop_fneg_places_fields(
            rd in 0u32..32, rn in 0u32..32, dbl in any::<bool>(),
        ) {
            let p = if dbl { "d" } else { "s" };
            let ops = vec![
                Operand::Reg(format!("{}{}", p, rd)),
                Operand::Reg(format!("{}{}", p, rn)),
            ];
            let w = expect_word(encode_fneg(&ops));
            let ftype = if dbl { 0b01u32 } else { 0b00u32 };

            // Reference word: 0x1E214000 (single) / 0x1E614000 (double),
            // OR'd with the two 5-bit register fields.
            prop_assert_eq!(w, 0x1E214000u32 | (ftype << 22) | (rn << 5) | rd);
            // Fixed bits of the scalar FP 1-source encoding.
            prop_assert_eq!(w >> 24, 0x1Eu32);             // [31:24] = 0x1E
            prop_assert_eq!((w >> 21) & 1, 1u32);          // bit 21 = 1
            prop_assert_eq!(fneg_fixed_of(w), 0b10000u32);// [14:10] = 10000
            // FNEG opcode occupies bits[20:15] and must equal 000010.
            prop_assert_eq!(fneg_opcode_of(w), 0b000010u32);
            // Register fields round-trip exactly into their 5-bit slots.
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(rn_of(w), rn);
        }

        // Oracle: reference / precision. ftype derived solely from dest prefix:
        // 'd' => 01 (double), else => 00 (single); sf (bit 31) always 0.
        #[test]
        fn prop_fneg_ftype_from_dest(
            rd in 0u32..32, rn in 0u32..32, dbl in any::<bool>(),
        ) {
            let p = if dbl { "d" } else { "s" };
            let ops = vec![
                Operand::Reg(format!("{}{}", p, rd)),
                Operand::Reg(format!("{}{}", p, rn)),
            ];
            let w = expect_word(encode_fneg(&ops));
            prop_assert_eq!(ftype_of(w), if dbl { 0b01 } else { 0b00 });
            prop_assert_eq!(sf_of(w), 0); // scalar FP, never sf=1
        }

        // Oracle: determinism. Same operands => identical word.
        #[test]
        fn prop_fneg_is_deterministic(
            rd in 0u32..32, rn in 0u32..32, dbl in any::<bool>(),
        ) {
            let p = if dbl { "d" } else { "s" };
            let ops = vec![
                Operand::Reg(format!("{}{}", p, rd)),
                Operand::Reg(format!("{}{}", p, rn)),
            ];
            let w1 = expect_word(encode_fneg(&ops));
            let w2 = expect_word(encode_fneg(&ops));
            prop_assert_eq!(w1, w2);
        }

        // Negative contract (validated, PASSES): out-of-range FP register
        // numbers (>= 32) MUST be rejected by get_reg/parse_reg_num, not
        // masked into the 5-bit field.
        #[test]
        fn prop_fneg_rejects_out_of_range_reg(n in 32u32..256u32, pos in 0u32..2u32) {
            let mut names = vec!["d0".to_string(), "d0".to_string()];
            names[pos as usize] = format!("d{}", n);
            let ops: Vec<Operand> = names.into_iter().map(Operand::Reg).collect();
            prop_assert!(
                encode_fneg(&ops).is_err(),
                "register d{} must be rejected (5-bit field), not silently masked", n
            );
        }

        // Negative contract (FINDING — FAILS): FNEG requires homogeneous
        // FP-register operands. Mixed precision (Dd, Sn) and GP-bank operands
        // (Xd, Xn) must be rejected, but encode_fneg derives ftype ONLY from
        // the dest prefix and never validates either operand's bank or the
        // source's precision, so it silently accepts illegal operands.
        #[test]
        fn prop_fneg_rejects_mismatched_precision_and_bank(n in 0u32..32) {
            // Mixed precision: D dest, S source.
            let mix = vec![
                Operand::Reg(format!("d{}", n)),
                Operand::Reg(format!("s{}", n)),
            ];
            prop_assert!(
                encode_fneg(&mix).is_err(),
                "mixed precision (Dd, Sn) must be rejected; got {:?}",
                encode_fneg(&mix)
            );
            // GP-bank operands are not valid for FNEG.
            let gp = vec![
                Operand::Reg(format!("x{}", n)),
                Operand::Reg(format!("x{}", n)),
            ];
            prop_assert!(
                encode_fneg(&gp).is_err(),
                "GP registers (x{}) are not valid FNEG operands; got {:?}",
                n, encode_fneg(&gp)
            );
        }
    }

    // ── encode_fabs (FP data-processing, 1 source: FABS) ===================
    // ARMv8-A "Floating-point data-processing (1 source)" layout:
    //   0 00 11110 ftype 1 opcode 10000 Rn Rd
    //   bits[31]=0, bits[30:24]=0011110 (0x1E), bits[23:22]=ftype,
    //   bit[21]=1, bits[20:15]=opcode (6-bit, FABS=000001), bits[14:10]=10000,
    //   bits[9:5]=Rn (src), bits[4:0]=Rd (dst).
    //   Cross-checked: FABS S0,S0 = 0x1E20C000, FABS D0,D0 = 0x1E60C000.
    fn fabs_opcode_of(w: u32) -> u32 { (w >> 15) & 0x3F }
    fn fabs_fixed_of(w: u32) -> u32  { (w >> 10) & 0x1F }

    proptest! {
        // Oracle: reference / field layout. Homogeneous-precision FP operands
        // => every field at its canonical ARMv8 bit position, opcode == FABS.
        #[test]
        fn prop_fabs_places_fields(
            rd in 0u32..32, rn in 0u32..32, dbl in any::<bool>(),
        ) {
            let p = if dbl { "d" } else { "s" };
            let ops = vec![
                Operand::Reg(format!("{}{}", p, rd)),
                Operand::Reg(format!("{}{}", p, rn)),
            ];
            let w = expect_word(encode_fabs(&ops));
            let ftype = if dbl { 0b01u32 } else { 0b00u32 };

            // Reference word: 0x1E20C000 (single) / 0x1E60C000 (double),
            // OR'd with the two 5-bit register fields.
            prop_assert_eq!(w, 0x1E20C000u32 | (ftype << 22) | (rn << 5) | rd);
            // Fixed bits of the scalar FP 1-source encoding.
            prop_assert_eq!(w >> 24, 0x1Eu32);               // [31:24] = 0x1E
            prop_assert_eq!((w >> 21) & 1, 1u32);            // bit 21 = 1
            prop_assert_eq!(fabs_fixed_of(w), 0b10000u32);   // [14:10] = 10000
            // FABS opcode occupies bits[20:15] and must equal 000001.
            prop_assert_eq!(fabs_opcode_of(w), 0b000001u32);
            // Register fields round-trip exactly into their 5-bit slots.
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(rn_of(w), rn);
        }

        // Oracle: reference / precision. ftype derived solely from dest prefix:
        // 'd' => 01 (double), else => 00 (single); sf (bit 31) always 0.
        #[test]
        fn prop_fabs_ftype_from_dest(
            rd in 0u32..32, rn in 0u32..32, dbl in any::<bool>(),
        ) {
            let p = if dbl { "d" } else { "s" };
            let ops = vec![
                Operand::Reg(format!("{}{}", p, rd)),
                Operand::Reg(format!("{}{}", p, rn)),
            ];
            let w = expect_word(encode_fabs(&ops));
            prop_assert_eq!(ftype_of(w), if dbl { 0b01 } else { 0b00 });
            prop_assert_eq!(sf_of(w), 0); // scalar FP, never sf=1
        }

        // Oracle: determinism. Same operands => identical word.
        #[test]
        fn prop_fabs_is_deterministic(
            rd in 0u32..32, rn in 0u32..32, dbl in any::<bool>(),
        ) {
            let p = if dbl { "d" } else { "s" };
            let ops = vec![
                Operand::Reg(format!("{}{}", p, rd)),
                Operand::Reg(format!("{}{}", p, rn)),
            ];
            let w1 = expect_word(encode_fabs(&ops));
            let w2 = expect_word(encode_fabs(&ops));
            prop_assert_eq!(w1, w2);
        }

        // Negative contract (validated, PASSES): out-of-range FP register
        // numbers (>= 32) MUST be rejected by get_reg/parse_reg_num, not
        // masked into the 5-bit field.
        #[test]
        fn prop_fabs_rejects_out_of_range_reg(n in 32u32..256u32, pos in 0u32..2u32) {
            let mut names = vec!["d0".to_string(), "d0".to_string()];
            names[pos as usize] = format!("d{}", n);
            let ops: Vec<Operand> = names.into_iter().map(Operand::Reg).collect();
            prop_assert!(
                encode_fabs(&ops).is_err(),
                "register d{} must be rejected (5-bit field), not silently masked", n
            );
        }

        // Negative contract (FINDING — FAILS): FABS requires homogeneous
        // FP-register operands. Mixed precision (Dd, Sn) and GP-bank operands
        // (Xd, Xn) must be rejected, but encode_fabs derives ftype ONLY from
        // the dest prefix and never validates either operand's bank or the
        // source's precision, so it silently accepts illegal operands.
        #[test]
        fn prop_fabs_rejects_mismatched_precision_and_bank(n in 0u32..32) {
            // Mixed precision: D dest, S source.
            let mix = vec![
                Operand::Reg(format!("d{}", n)),
                Operand::Reg(format!("s{}", n)),
            ];
            prop_assert!(
                encode_fabs(&mix).is_err(),
                "mixed precision (Dd, Sn) must be rejected; got {:?}",
                encode_fabs(&mix)
            );
            // GP-bank operands are not valid for FABS.
            let gp = vec![
                Operand::Reg(format!("x{}", n)),
                Operand::Reg(format!("x{}", n)),
            ];
            prop_assert!(
                encode_fabs(&gp).is_err(),
                "GP registers (x{}) are not valid FABS operands; got {:?}",
                n, encode_fabs(&gp)
            );
        }
    }

    // ── encode_fp_1src (FP data-processing, 1 source: FRINTN/P/M/Z/A/X/I) ===
    // ARMv8-A layout: 0 00 11110 ftype 1 opcode 10000 Rn Rd
    //   bits[31:24]=0x1E, bits[23:22]=ftype, bit[21]=1,
    //   bits[20:15]=opcode (6-bit), bits[14:10]=10000, bits[9:5]=Rn, bits[4:0]=Rd.
    fn fp1_opc_of(w: u32) -> u32   { (w >> 15) & 0x3F }
    fn fp1_fixed_of(w: u32) -> u32 { (w >> 10) & 0x1F }

    proptest! {
        // Oracle: reference / field layout. Homogeneous-precision FP operands
        // + a 6-bit opcode => every field at its canonical bit position.
        #[test]
        fn prop_fp_1src_places_fields(
            rd in 0u32..32, rn in 0u32..32, opcode in 0u32..64, dbl in any::<bool>(),
        ) {
            let p = if dbl { "d" } else { "s" };
            let ops = vec![
                Operand::Reg(format!("{}{}", p, rd)),
                Operand::Reg(format!("{}{}", p, rn)),
            ];
            let w = expect_word(encode_fp_1src(&ops, opcode));

            prop_assert_eq!(w >> 24, 0x1Eu32);            // [31:24] = 0x1E
            prop_assert_eq!((w >> 21) & 1, 1u32);         // bit 21 = 1
            prop_assert_eq!(fp1_fixed_of(w), 0b10000u32); // [14:10] = 10000
            prop_assert_eq!(rd_of(w), rd);                // [4:0]   = Rd
            prop_assert_eq!(rn_of(w), rn);                // [9:5]   = Rn
            prop_assert_eq!(fp1_opc_of(w), opcode);       // [20:15] = opcode
        }

        // Oracle: reference / precision. ftype derived solely from dest prefix:
        // 'd' => 01 (double), else => 00 (single); sf (bit 31) always 0.
        #[test]
        fn prop_fp_1src_ftype_from_dest(
            rd in 0u32..32, rn in 0u32..32, dbl in any::<bool>(),
        ) {
            let p = if dbl { "d" } else { "s" };
            let ops = vec![
                Operand::Reg(format!("{}{}", p, rd)),
                Operand::Reg(format!("{}{}", p, rn)),
            ];
            let w = expect_word(encode_fp_1src(&ops, 0b001000));
            prop_assert_eq!(ftype_of(w), if dbl { 0b01 } else { 0b00 });
            prop_assert_eq!(sf_of(w), 0);
        }

        // Negative contract (validated, PASSES): out-of-range FP register
        // numbers (>= 32) MUST be rejected by get_reg, not masked into 5 bits.
        #[test]
        fn prop_fp_1src_rejects_out_of_range_reg(n in 32u32..256u32, pos in 0u32..2u32) {
            let mut names = vec!["d0".to_string(), "d0".to_string()];
            names[pos as usize] = format!("d{}", n);
            let ops: Vec<Operand> = names.into_iter().map(Operand::Reg).collect();
            prop_assert!(
                encode_fp_1src(&ops, 0b001000).is_err(),
                "register d{} must be rejected (5-bit field), not silently masked", n
            );
        }

        // Negative contract (FINDING — FAILS): opcode is a 6-bit field at
        // [20:15]. Values >= 64 leak past bit 20 (bit 21, then ftype[23:22])
        // and silently corrupt the word, yet encode_fp_1src ORs any u32 opcode
        // into the field with NO range check.
        #[test]
        fn prop_fp_1src_rejects_out_of_range_opcode(bad in 64u32..4096u32) {
            let ops = vec![Operand::Reg("d0".into()), Operand::Reg("d0".into())];
            prop_assert!(
                encode_fp_1src(&ops, bad).is_err(),
                "opcode {} must be rejected (6-bit field [20:15]); it is OR'd in and corrupts bit21/ftype", bad
            );
        }

        // Negative contract (FINDING — FAILS): FRINT* require homogeneous
        // FP-register operands. Mixed precision (Dd, Sn) and GP-bank operands
        // (Xd, Xn) must be rejected, but the function only inspects the dest.
        #[test]
        fn prop_fp_1src_rejects_mismatched_precision_and_bank(n in 0u32..32) {
            let mix = vec![
                Operand::Reg(format!("d{}", n)),
                Operand::Reg(format!("s{}", n)),
            ];
            prop_assert!(
                encode_fp_1src(&mix, 0b001000).is_err(),
                "mixed precision (Dd, Sn) must be rejected"
            );
            let gp = vec![
                Operand::Reg(format!("x{}", n)),
                Operand::Reg(format!("x{}", n)),
            ];
            prop_assert!(
                encode_fp_1src(&gp, 0b001000).is_err(),
                "GP registers (x{}) are not valid FP 1-source operands", n
            );
        }
    }

    // ── encode_fcmp (FP compare) ===========================================
    // ARMv8-A layout:
    //   FCMP <Pn>, <Pm>: 0 00 11110 ftype 1 Rm 00 1000 Rn 00 000
    //     bits[31:24]=0x1E, [23:22]=ftype, [21]=1, [20:16]=Rm,
    //     [15:10]=001000, [9:5]=Rn, [4:0]=00000.
    //   FCMP <Pn>, #0.0: 0 00 11110 ftype 1 0000 00 1000 Rn 00 1000
    //     same fixed fields but [4:0]=01000 (bit 3 = compare-to-zero marker).
    fn fcmp_rm_of(w: u32) -> u32 { (w >> 16) & 0x1F }
    // bits[15:10] fixed field shared by both forms (== 0b001000 == 8).
    fn fcmp_fixed_of(w: u32) -> u32 { (w >> 10) & 0x3F }

    proptest! {
        // Oracle: reference / field layout (register form). Homogeneous-
        // precision FP operands => every field at its canonical bit position.
        #[test]
        fn prop_fcmp_reg_places_fields(
            rn in 0u32..32, rm in 0u32..32, dbl in any::<bool>(),
        ) {
            let p = if dbl { "d" } else { "s" };
            let ops = vec![
                Operand::Reg(format!("{}{}", p, rn)),
                Operand::Reg(format!("{}{}", p, rm)),
            ];
            let w = expect_word(encode_fcmp(&ops));
            let ftype = if dbl { 0b01u32 } else { 0b00u32 };

            // Reference word: 0x1E202000 | ftype<<22 | rm<<16 | rn<<5.
            prop_assert_eq!(w, 0x1E202000u32 | (ftype << 22) | (rm << 16) | (rn << 5));
            // Fixed bits of the scalar FP compare encoding.
            prop_assert_eq!(w >> 24, 0x1Eu32);            // [31:24] = 0x1E
            prop_assert_eq!((w >> 21) & 1, 1u32);          // bit 21 = 1
            prop_assert_eq!(fcmp_fixed_of(w), 0b001000u32);// [15:10] = 001000
            prop_assert_eq!(ftype_of(w), ftype);           // precision field
            prop_assert_eq!(sf_of(w), 0);                   // scalar FP, sf always 0
            // Register fields round-trip exactly into their 5-bit slots.
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(fcmp_rm_of(w), rm);
            prop_assert_eq!(w & 0x1F, 0u32);                // [4:0] = 0 (register form)
        }

        // Oracle: reference / field layout (compare-to-zero form). Both the
        // 1-operand form ("FCMP Pn") and the explicit "FCMP Pn, #0" must yield
        // the #0.0 encoding with bit 3 set and no Rm field.
        #[test]
        fn prop_fcmp_zero_form_places_fields(
            rn in 0u32..32, dbl in any::<bool>(), explicit in any::<bool>(),
        ) {
            let p = if dbl { "d" } else { "s" };
            let ops = if explicit {
                vec![
                    Operand::Reg(format!("{}{}", p, rn)),
                    Operand::Imm(0),
                ]
            } else {
                vec![Operand::Reg(format!("{}{}", p, rn))]
            };
            let w = expect_word(encode_fcmp(&ops));
            let ftype = if dbl { 0b01u32 } else { 0b00u32 };

            // Reference word: 0x1E202008 | ftype<<22 | rn<<5 (bit 3 set).
            prop_assert_eq!(w, 0x1E202008u32 | (ftype << 22) | (rn << 5));
            prop_assert_eq!(w >> 24, 0x1Eu32);
            prop_assert_eq!((w >> 21) & 1, 1u32);
            prop_assert_eq!(fcmp_fixed_of(w), 0b001000u32);
            prop_assert_eq!(ftype_of(w), ftype);
            prop_assert_eq!(rn_of(w), rn);
            // The compare-to-zero marker: bits[4:0] == 01000 (bit 3 set),
            // and no Rm field is encoded (bits[20:16] == 0).
            prop_assert_eq!(w & 0x1F, 0b01000u32);
            prop_assert_eq!(fcmp_rm_of(w), 0u32);
        }

        // Oracle: precision. ftype is derived solely from operand[0]'s prefix:
        // 'd' => 01 (double), else => 00 (single).
        #[test]
        fn prop_fcmp_ftype_from_first_operand(
            rn in 0u32..32, rm in 0u32..32, dbl in any::<bool>(),
        ) {
            let p = if dbl { "d" } else { "s" };
            let ops = vec![
                Operand::Reg(format!("{}{}", p, rn)),
                Operand::Reg(format!("{}{}", p, rm)),
            ];
            let w = expect_word(encode_fcmp(&ops));
            prop_assert_eq!(ftype_of(w), if dbl { 0b01 } else { 0b00 });
        }

        // Oracle: determinism. Same operands => identical word, both forms.
        #[test]
        fn prop_fcmp_is_deterministic(
            rn in 0u32..32, rm in 0u32..32, dbl in any::<bool>(),
        ) {
            let p = if dbl { "d" } else { "s" };
            let ops = vec![
                Operand::Reg(format!("{}{}", p, rn)),
                Operand::Reg(format!("{}{}", p, rm)),
            ];
            let w1 = expect_word(encode_fcmp(&ops));
            let w2 = expect_word(encode_fcmp(&ops));
            prop_assert_eq!(w1, w2);
        }

        // Negative contract (validated, PASSES): out-of-range FP register
        // numbers (>= 32) MUST be rejected by get_reg, not masked into 5 bits;
        // and non-zero immediates are invalid (FCMP only supports #0.0).
        #[test]
        fn prop_fcmp_rejects_out_of_range_reg_and_nonzero_imm(
            n in 32u32..256u32, bad_imm in 1i64..1000i64,
        ) {
            // Out-of-range source register.
            let ops = vec![Operand::Reg(format!("d{}", n)), Operand::Reg("d0".into())];
            prop_assert!(
                encode_fcmp(&ops).is_err(),
                "register d{} must be rejected (5-bit field), not silently masked", n
            );
            // Out-of-range second register.
            let ops = vec![Operand::Reg("d0".into()), Operand::Reg(format!("d{}", n))];
            prop_assert!(
                encode_fcmp(&ops).is_err(),
                "register d{} must be rejected (5-bit field), not silently masked", n
            );
            // Non-zero immediate: only #0.0 is encodable; #<other> is invalid.
            let ops = vec![Operand::Reg("d0".into()), Operand::Imm(bad_imm)];
            prop_assert!(
                encode_fcmp(&ops).is_err(),
                "FCMP with immediate #{} must be rejected (only #0.0 allowed)", bad_imm
            );
        }

        // Negative contract (FINDING — FAILS): FCMP requires homogeneous-
        // precision FP-register operands. Mixed precision (Dn, Sm) and
        // GP-bank operands (Wn/Xn) must be rejected, but encode_fcmp derives
        // ftype ONLY from operand[0] and never validates operand[1]'s bank or
        // precision, so it silently accepts illegal operands.
        #[test]
        fn prop_fcmp_rejects_mismatched_precision_and_bank(n in 0u32..32) {
            // Mixed precision: D then S.
            let mix1 = vec![
                Operand::Reg(format!("d{}", n)),
                Operand::Reg(format!("s{}", n)),
            ];
            prop_assert!(
                encode_fcmp(&mix1).is_err(),
                "mixed precision (Dn, Sm) must be rejected; got {:?}",
                encode_fcmp(&mix1)
            );
            // Mixed precision: S then D.
            let mix2 = vec![
                Operand::Reg(format!("s{}", n)),
                Operand::Reg(format!("d{}", n)),
            ];
            prop_assert!(
                encode_fcmp(&mix2).is_err(),
                "mixed precision (Sn, Dm) must be rejected; got {:?}",
                encode_fcmp(&mix2)
            );
            // GP-bank operands are not valid for FCMP.
            let gp = vec![
                Operand::Reg(format!("x{}", n)),
                Operand::Reg(format!("x{}", n)),
            ];
            prop_assert!(
                encode_fcmp(&gp).is_err(),
                "GP registers (x{}) are not valid FCMP operands; got {:?}",
                n, encode_fcmp(&gp)
            );
        }
    }

    // ── encode_fcvt_rounding (FP<->integer conversion with rounding mode) ===
    // ARMv8-A "Floating-point<->integer conversions" layout:
    //   sf 0 0 1 1 1 1 0 ftype 1 rmode opcode 0 0 0 0 0 0 Rn Rd
    //   bit31=sf, bits[30:24]=0011110 (0x1E), bits[23:22]=ftype,
    //   bit21=1, bits[20:19]=rmode (2-bit), bits[18:16]=opcode (3-bit),
    //   bits[15:10]=000000, bits[9:5]=Rn, bits[4:0]=Rd.
    //   (Cross-checked: FCVTZS <Xd>,<Dn> = 0x9E780000 => sf=1,ftype=01,
    //    bit21=1,rmode=11,opcode=000,000000,Rn,Rd.)
    fn fcvt_rmode_of(w: u32) -> u32 { (w >> 19) & 0x3 }
    fn fcvt_opcode_of(w: u32) -> u32 { (w >> 16) & 0x7 }

    proptest! {
        // Oracle: reference / field layout. A valid GP destination (Wd/Xd)
        // and FP source (Sn/Dn) with in-range rmode/opcode => every field
        // lands at its canonical ARMv8 bit position with no truncation.
        #[test]
        fn prop_fcvt_rounding_places_fields(
            rd in 0u32..32, rn in 0u32..32,
            dbl_src in any::<bool>(), dbl_dst in any::<bool>(),
            rmode in 0u32..4u32, opcode in 0u32..8u32,
        ) {
            let (dst_reg, sf) = if dbl_dst {
                (format!("x{}", rd), 1u32)
            } else {
                (format!("w{}", rd), 0u32)
            };
            let src_reg = if dbl_src { format!("d{}", rn) } else { format!("s{}", rn) };
            let ftype = if dbl_src { 0b01u32 } else { 0b00u32 };
            let ops = vec![Operand::Reg(dst_reg), Operand::Reg(src_reg)];
            let w = expect_word(encode_fcvt_rounding(&ops, rmode, opcode));

            // Fixed bits of the FP<->int conversion encoding.
            prop_assert_eq!((w >> 24) & 0x7F, 0x1Eu32); // bits[30:24] = 0011110
            prop_assert_eq!((w >> 21) & 1, 1u32);       // bit 21 = 1
            prop_assert_eq!((w >> 10) & 0x3F, 0u32);    // bits[15:10] = 000000

            // sf (bit31), ftype[23:22], rmode[20:19], opcode[18:16].
            prop_assert_eq!(sf_of(w), sf);
            prop_assert_eq!(ftype_of(w), ftype);
            prop_assert_eq!(fcvt_rmode_of(w), rmode);
            prop_assert_eq!(fcvt_opcode_of(w), opcode);

            // Register fields round-trip exactly into their 5-bit slots.
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(rn_of(w), rn);

            // Reference reconstruction.
            let expected = (sf << 31) | (0x1Eu32 << 24) | (ftype << 22)
                | (1u32 << 21) | (rmode << 19) | (opcode << 16) | (rn << 5) | rd;
            prop_assert_eq!(w, expected);
        }

        // Oracle: precision / sf derivation. sf comes from the destination's
        // GP width (X=>1, W=>0); ftype comes solely from the source prefix
        // ('d'=>01 double, else=>00 single). The two are independent.
        #[test]
        fn prop_fcvt_rounding_sf_and_ftype_derivation(
            rd in 0u32..32, rn in 0u32..32,
            dbl_src in any::<bool>(), dbl_dst in any::<bool>(),
        ) {
            let dst_reg = if dbl_dst { format!("x{}", rd) } else { format!("w{}", rd) };
            let src_reg = if dbl_src { format!("d{}", rn) } else { format!("s{}", rn) };
            let ops = vec![Operand::Reg(dst_reg), Operand::Reg(src_reg)];
            let w = expect_word(encode_fcvt_rounding(&ops, 0, 0));
            prop_assert_eq!(sf_of(w), if dbl_dst { 1 } else { 0 });
            prop_assert_eq!(ftype_of(w), if dbl_src { 0b01 } else { 0b00 });
        }

        // Oracle: determinism. Same operands+rmode+opcode => identical word.
        #[test]
        fn prop_fcvt_rounding_is_deterministic(
            rd in 0u32..32, rn in 0u32..32, rmode in 0u32..4u32, opcode in 0u32..8u32,
        ) {
            let ops = vec![Operand::Reg(format!("w{}", rd)), Operand::Reg(format!("d{}", rn))];
            let w1 = expect_word(encode_fcvt_rounding(&ops, rmode, opcode));
            let w2 = expect_word(encode_fcvt_rounding(&ops, rmode, opcode));
            prop_assert_eq!(w1, w2);
        }

        // Negative contract (validated, PASSES): out-of-range register numbers
        // (>= 32) MUST be rejected by get_reg, not masked into 5 bits; and
        // non-register operands / too-few operands must be rejected.
        #[test]
        fn prop_fcvt_rounding_rejects_bad_regs_and_arity(
            n in 32u32..256u32, pos in 0u32..2u32, bad_imm in any::<i64>(),
        ) {
            let prefix = if pos == 0 { "w" } else { "d" };
            let mut names = vec!["w0".to_string(), "d0".to_string()];
            names[pos as usize] = format!("{}{}", prefix, n);
            let ops: Vec<Operand> = names.into_iter().map(Operand::Reg).collect();
            prop_assert!(
                encode_fcvt_rounding(&ops, 0, 0).is_err(),
                "register {}{} must be rejected (5-bit field), not silently masked", prefix, n
            );
            // Too few operands.
            prop_assert!(encode_fcvt_rounding(&[], 0, 0).is_err());
            prop_assert!(encode_fcvt_rounding(&[Operand::Reg("w0".into())], 0, 0).is_err());
            // Non-register source operand.
            let bad = vec![Operand::Reg("w0".into()), Operand::Imm(bad_imm)];
            prop_assert!(encode_fcvt_rounding(&bad, 0, 0).is_err());
        }

        // Negative contract (FINDING — FAILS): rmode is a 2-bit field [20:19]
        // and opcode is a 3-bit field [18:16]. Values outside these ranges are
        // OR'd into the word with NO range check, silently corrupting the
        // fixed bit 21, the ftype field, and the neighbouring rmode/opcode
        // bits. They MUST be rejected.
        #[test]
        fn prop_fcvt_rounding_rejects_oversized_rmode_and_opcode(
            bad_rmode in 4u32..32u32, bad_opcode in 8u32..128u32,
        ) {
            let ops = vec![Operand::Reg("w0".into()), Operand::Reg("d0".into())];
            prop_assert!(
                encode_fcvt_rounding(&ops, bad_rmode, 0).is_err(),
                "rmode {} must be rejected (2-bit field [20:19]); it overflows into bit21/ftype", bad_rmode
            );
            prop_assert!(
                encode_fcvt_rounding(&ops, 0, bad_opcode).is_err(),
                "opcode {} must be rejected (3-bit field [18:16]); it overflows into the rmode field", bad_opcode
            );
        }
    }

    // ── encode_int_to_float (SCVTF/UCVTF: integer→float conversion) =========
    // ARMv8-A "Floating-point<->integer conversions" layout:
    //   sf 0 0 1 1 1 1 0 ftype 1 00 opcode 000000 Rn Rd
    //   bit31=sf (0=W source, 1=X source), bits[30:24]=0011110 (0x1E),
    //   bits[23:22]=ftype (00=S dest, 01=D dest), bit[21]=1,
    //   bits[20:19]=00, bits[18:16]=opcode (010=SCVTF signed, 011=UCVTF unsigned),
    //   bits[15:10]=000000, bits[9:5]=Rn (GP source), bits[4:0]=Rd (FP dest).
    //   (Cross-checked: SCVTF <Sd>,<Wn> = 0x1E220000, SCVTF <Dd>,<Xn> = 0x9E260000,
    //    UCVTF <Dd>,<Wn> = 0x1E270000.)
    fn itf_opcode_of(w: u32) -> u32 { (w >> 16) & 0x7 }
    fn itf_fixed_of(w: u32) -> u32 { (w >> 10) & 0x3F }

    proptest! {
        // Oracle: reference / field layout. A valid FP destination (Sd/Dd) and
        // GP source (Wn/Xn) with in-range register numbers => every field lands
        // at its canonical ARMv8 bit position with no truncation.
        #[test]
        fn prop_int_to_float_places_fields(
            rd in 0u32..32, rn in 0u32..32,
            dbl_dst in any::<bool>(), src64 in any::<bool>(), signed in any::<bool>(),
        ) {
            let dst_reg = if dbl_dst { format!("d{}", rd) } else { format!("s{}", rd) };
            let src_reg = if src64  { format!("x{}", rn) } else { format!("w{}", rn) };
            let ops = vec![Operand::Reg(dst_reg), Operand::Reg(src_reg)];
            let sf     = if src64  { 1u32 } else { 0u32 };
            let ftype  = if dbl_dst { 0b01u32 } else { 0b00u32 };
            let opcode = if signed  { 0b010u32 } else { 0b011u32 };
            let w = expect_word(encode_int_to_float(&ops, signed));

            // Fixed bits of the FP<->int conversion encoding.
            prop_assert_eq!((w >> 24) & 0x7F, 0x1Eu32); // bits[30:24] = 0011110
            prop_assert_eq!((w >> 21) & 1, 1u32);      // bit 21 = 1
            prop_assert_eq!((w >> 19) & 0x3, 0u32);    // bits[20:19] = 00
            prop_assert_eq!(itf_fixed_of(w), 0u32);    // bits[15:10] = 000000

            // Field derivation: sf (bit31) from source GP width, ftype[23:22]
            // from dest prefix, opcode[18:16] from signedness.
            prop_assert_eq!(sf_of(w), sf);
            prop_assert_eq!(ftype_of(w), ftype);
            prop_assert_eq!(itf_opcode_of(w), opcode);

            // Register fields round-trip exactly into their 5-bit slots.
            prop_assert_eq!(rd_of(w), rd);
            prop_assert_eq!(rn_of(w), rn);

            // Reference reconstruction.
            let expected = (sf << 31) | (0x1Eu32 << 24) | (ftype << 22)
                | (1u32 << 21) | (opcode << 16) | (rn << 5) | rd;
            prop_assert_eq!(w, expected);
        }

        // Oracle: sf/ftype independence. sf comes solely from the GP source
        // width (X=>1, W=>0); ftype comes solely from the FP dest prefix
        // ('d'=>01, else=>00). The two vary independently.
        #[test]
        fn prop_int_to_float_sf_ftype_derivation(
            rd in 0u32..32, rn in 0u32..32,
            dbl_dst in any::<bool>(), src64 in any::<bool>(),
        ) {
            let dst_reg = if dbl_dst { format!("d{}", rd) } else { format!("s{}", rd) };
            let src_reg = if src64  { format!("x{}", rn) } else { format!("w{}", rn) };
            let ops = vec![Operand::Reg(dst_reg), Operand::Reg(src_reg)];
            let w = expect_word(encode_int_to_float(&ops, true));
            prop_assert_eq!(sf_of(w), if src64 { 1 } else { 0 });
            prop_assert_eq!(ftype_of(w), if dbl_dst { 0b01 } else { 0b00 });
        }

        // Oracle: signedness derivation. is_signed selects opcode 010 (SCVTF)
        // in [18:16] vs 011 (UCVTF); only bit 16 differs between the two.
        #[test]
        fn prop_int_to_float_signedness_selects_opcode(
            rd in 0u32..32, rn in 0u32..32,
        ) {
            let ops = vec![Operand::Reg(format!("d{}", rd)), Operand::Reg(format!("x{}", rn))];
            let w_signed   = expect_word(encode_int_to_float(&ops, true));
            let w_unsigned = expect_word(encode_int_to_float(&ops, false));
            prop_assert_eq!(w_signed ^ w_unsigned, 1u32 << 16);
            prop_assert_eq!(itf_opcode_of(w_signed), 0b010u32);
            prop_assert_eq!(itf_opcode_of(w_unsigned), 0b011u32);
        }

        // Negative contract (validated, PASSES): out-of-range register numbers
        // (>= 32) MUST be rejected by get_reg (parse_reg_num caps at 31), not
        // masked into 5 bits; too-few operands and non-register dests rejected.
        #[test]
        fn prop_int_to_float_rejects_bad_regs_and_arity(
            n in 32u32..256u32, pos in 0u32..2u32, bad_imm in any::<i64>(),
        ) {
            let prefix = if pos == 0 { "s" } else { "w" };
            let mut names = vec!["s0".to_string(), "w0".to_string()];
            names[pos as usize] = format!("{}{}", prefix, n);
            let ops: Vec<Operand> = names.into_iter().map(Operand::Reg).collect();
            prop_assert!(
                encode_int_to_float(&ops, true).is_err(),
                "register {}{} must be rejected (5-bit field), not silently masked", prefix, n
            );
            // Too few operands.
            prop_assert!(encode_int_to_float(&[], true).is_err());
            prop_assert!(encode_int_to_float(&[Operand::Reg("s0".into())], true).is_err());
            // Non-register dest operand.
            let bad = vec![Operand::Imm(bad_imm), Operand::Reg("w0".into())];
            prop_assert!(encode_int_to_float(&bad, true).is_err());
        }

        // Negative contract (FINDING — FAILS): SCVTF/UCVTF convert a GP
        // integer source (Wn/Xn) to an FP destination (Sd/Dd). The source MUST
        // be a GP register and the dest MUST be an FP register; any other bank
        // combination is an illegal operand. But encode_int_to_float never
        // validates operand banks: it accepts an FP source (mis-reading its
        // width as sf=0) and a GP dest (mis-reading it as ftype=00 single),
        // silently producing a word with wrong register-class semantics.
        #[test]
        fn prop_int_to_float_rejects_wrong_operand_banks(n in 0u32..32) {
            // GP destination: SCVTF Wd, Wn is not a valid instruction (dest
            // must be FP). Currently ftype is mis-derived as 00 from 'w'.
            let gp_dst = vec![Operand::Reg(format!("w{}", n)), Operand::Reg(format!("w{}", n))];
            prop_assert!(
                encode_int_to_float(&gp_dst, true).is_err(),
                "GP destination (w{}) must be rejected; SCVTF/UCVTF dest must be FP, got {:?}",
                n, encode_int_to_float(&gp_dst, true)
            );
            // FP source: SCVTF Dd, Dn is not a valid instruction (source must
            // be GP). Currently sf is mis-derived as 0 from the 'd' prefix.
            let fp_src = vec![Operand::Reg(format!("d{}", n)), Operand::Reg(format!("d{}", n))];
            prop_assert!(
                encode_int_to_float(&fp_src, true).is_err(),
                "FP source (d{}) must be rejected; SCVTF/UCVTF source must be GP, got {:?}",
                n, encode_int_to_float(&fp_src, true)
            );
        }
    }

    // ── encode_fcvt_precision (FCVT: float precision conversion) ============
    // ARMv8-A "Floating-point data-processing (1 source)" layout:
    //   0 00 11110 ftype 1 0001 opc 10000 Rn Rd
    //   bits[31]=0, bits[30:24]=0011110 (0x1E), bits[23:22]=ftype
    //     (source: S=00, D=01, H=11), bit[21]=1, bits[20:17]=0001,
    //   bits[16:15]=opc (dest: S=00, D=01, H=11), bits[14:10]=10000,
    //   bits[9:5]=Rn (src reg), bits[4:0]=Rd (dst reg).
    //   Cross-checked: FCVT D0,S0 = 0x1E22C000, FCVT S0,D0 = 0x1E624000,
    //   FCVT H0,S0 = 0x1E23C000, FCVT D0,H0 = 0x1EE2C000.
    fn fcvt_opc_of(w: u32) -> u32    { (w >> 15) & 0x3 }
    fn fcvt_fixed_hi(w: u32) -> u32  { (w >> 17) & 0xF }
    fn fcvt_fixed_lo(w: u32) -> u32  { (w >> 10) & 0x1F }

    proptest! {
        // Oracle: reference / field layout. Distinct-precision FP operands
        // (source precision != dest precision) => every field lands at its
        // canonical ARMv8 bit position with no truncation.
        #[test]
        fn prop_fcvt_precision_places_fields(
            rd in 0u32..32, rn in 0u32..32,
            src_kind in 0u32..3u32, dst_kind in 0u32..3u32,
        ) {
            // kind: 0=S, 1=D, 2=H. Skip same-precision combos (UNALLOCATED).
            prop_assume!(src_kind != dst_kind);
            let pre = ["s", "d", "h"];
            let map = [0b00u32, 0b01u32, 0b11u32];
            let src_pre = pre[src_kind as usize];
            let dst_pre = pre[dst_kind as usize];
            let ftype = map[src_kind as usize];
            let opc   = map[dst_kind as usize];
            let ops = vec![
                Operand::Reg(format!("{}{}", dst_pre, rd)),
                Operand::Reg(format!("{}{}", src_pre, rn)),
            ];
            let w = expect_word(encode_fcvt_precision(&ops));

            // Fixed bits of the FCVT encoding.
            prop_assert_eq!(w >> 24, 0x1Eu32);             // [31:24] = 0x1E (sf=0 + 0011110)
            prop_assert_eq!((w >> 21) & 1, 1u32);          // bit 21 = 1
            prop_assert_eq!(fcvt_fixed_hi(w), 0b0001u32);  // [20:17] = 0001
            prop_assert_eq!(fcvt_fixed_lo(w), 0b10000u32); // [14:10] = 10000
            prop_assert_eq!(sf_of(w), 0u32);               // scalar FP, sf always 0

            // Precision fields: ftype from source prefix, opc from dest prefix.
            prop_assert_eq!(ftype_of(w), ftype);
            prop_assert_eq!(fcvt_opc_of(w), opc);

            // Register fields round-trip exactly into their 5-bit slots.
            prop_assert_eq!(rn_of(w), rn);
            prop_assert_eq!(rd_of(w), rd);

            // Reference reconstruction.
            let expected = (0b00011110u32 << 24) | (ftype << 22) | (1u32 << 21)
                | (0b0001u32 << 17) | (opc << 15) | (0b10000u32 << 10)
                | (rn << 5) | rd;
            prop_assert_eq!(w, expected);
        }

        // Oracle: precision derivation. ftype encodes the SOURCE precision,
        // opc encodes the DEST precision; both map S->00, D->01, H->11. They
        // are derived independently from the two operands.
        #[test]
        fn prop_fcvt_precision_ftype_and_opc_derivation(
            rd in 0u32..32, rn in 0u32..32,
            src_kind in 0u32..3u32, dst_kind in 0u32..3u32,
        ) {
            prop_assume!(src_kind != dst_kind);
            let pre = ["s", "d", "h"];
            let map = [0b00u32, 0b01u32, 0b11u32];
            let ops = vec![
                Operand::Reg(format!("{}{}", pre[dst_kind as usize], rd)),
                Operand::Reg(format!("{}{}", pre[src_kind as usize], rn)),
            ];
            let w = expect_word(encode_fcvt_precision(&ops));
            prop_assert_eq!(ftype_of(w), map[src_kind as usize]);
            prop_assert_eq!(fcvt_opc_of(w), map[dst_kind as usize]);
        }

        // Oracle: determinism. Same operands => identical word.
        #[test]
        fn prop_fcvt_precision_is_deterministic(rd in 0u32..32, rn in 0u32..32) {
            let ops = vec![Operand::Reg(format!("d{}", rd)), Operand::Reg(format!("s{}", rn))];
            let w1 = expect_word(encode_fcvt_precision(&ops));
            let w2 = expect_word(encode_fcvt_precision(&ops));
            prop_assert_eq!(w1, w2);
        }

        // Negative contract (validated, PASSES): out-of-range register numbers
        // (>= 32) MUST be rejected by get_reg, not masked into 5 bits; too-few
        // operands, non-register operands, and unsupported type prefixes
        // (GP registers W/X, or any non-{s,d,h} prefix) must be Err.
        #[test]
        fn prop_fcvt_precision_rejects_bad_operands(
            n in 32u32..256u32, pos in 0u32..2u32, imm in any::<i64>(),
            in_range in 0u32..32u32,
        ) {
            // Out-of-range register in either position.
            let prefix = if pos == 0 { "d" } else { "s" };
            let mut names = vec!["d0".to_string(), "s0".to_string()];
            names[pos as usize] = format!("{}{}", prefix, n);
            let ops: Vec<Operand> = names.into_iter().map(Operand::Reg).collect();
            prop_assert!(
                encode_fcvt_precision(&ops).is_err(),
                "register {}{} must be rejected (5-bit field), not silently masked", prefix, n
            );
            // Too few operands.
            prop_assert!(encode_fcvt_precision(&[]).is_err());
            prop_assert!(encode_fcvt_precision(&[Operand::Reg("d0".into())]).is_err());
            // Non-register operand.
            let bad = vec![Operand::Reg("d0".into()), Operand::Imm(imm)];
            prop_assert!(encode_fcvt_precision(&bad).is_err());
            // Unsupported type prefix: GP registers (w/x) are not valid FP types.
            let gp = vec![
                Operand::Reg(format!("x{}", in_range)),
                Operand::Reg(format!("w{}", in_range)),
            ];
            prop_assert!(
                encode_fcvt_precision(&gp).is_err(),
                "GP registers (x{}/w{}) are not valid FCVT operands", in_range, in_range
            );
        }

        // Negative contract (FINDING — FAILS): FCVT converts precision; the
        // destination precision MUST differ from the source precision.
        // Per ARMv8-A, the combos ftype==opc (S->S, D->D, H->H) are
        // UNALLOCATED encodings — there is no FCVT Sd,Sn / Dd,Dn / Hd,Hn.
        // But encode_fcvt_precision never checks this and emits the
        // unallocated word (e.g. FCVT S0,S0 -> 0x1E604000, which is actually
        // the FMOV Sd,Sn encoding, not FCVT).
        #[test]
        fn prop_fcvt_precision_rejects_same_precision(kind in 0u32..3u32, n in 0u32..32) {
            let pre = ["s", "d", "h"][kind as usize];
            let ops = vec![
                Operand::Reg(format!("{}{}", pre, n)),
                Operand::Reg(format!("{}{}", pre, n)),
            ];
            prop_assert!(
                encode_fcvt_precision(&ops).is_err(),
                "FCVT {}{},{}{} is UNALLOCATED (same precision); must be rejected, got {:?}",
                pre, n, pre, n, encode_fcvt_precision(&ops)
            );
        }
    }

    // ── encode_fmadd_fmsub (FMADD/FMSUB: Rd = Ra +/- (Rn * Rm)) ==============
    // ARMv8-A "Floating-point data-processing (3 source)" layout:
    //   0 00 11111 ftype 0 Rm o1 Ra Rn Rd
    //   bits[31:24]=00011111 (0x1F), bits[23:22]=ftype (00=S, 01=D),
    //   bit[21]=0, bits[20:16]=Rm, bit[15]=o1 (0=FMADD, 1=FMSUB),
    //   bits[14:10]=Ra, bits[9:5]=Rn, bits[4:0]=Rd.
    //   Cross-checked: FMADD D0,D0,D0,D0 = 0x1F400000, FMADD S0,S0,S0,S0 = 0x1F000000,
    //   FMSUB D0,D0,D0,D0 = 0x1F408000 (only bit 15 differs from FMADD).
    fn fms_rm_of(w: u32) -> u32 { (w >> 16) & 0x1F }
    fn fms_ra_of(w: u32) -> u32 { (w >> 10) & 0x1F }
    fn fms_o1_of(w: u32) -> u32 { (w >> 15) & 1 }

    proptest! {
        // Oracle: reference / field layout. Homogeneous-precision FP operands
        // (all S or all D) with in-range register numbers + is_sub => every
        // field lands at its canonical ARMv8 bit position with no truncation.
        #[test]
        fn prop_fmadd_places_fields(
            rd in 0u32..32, rn in 0u32..32, rm in 0u32..32, ra in 0u32..32,
            dbl in any::<bool>(), is_sub in any::<bool>(),
        ) {
            let p = if dbl { "d" } else { "s" };
            let ops = vec![
                Operand::Reg(format!("{}{}", p, rd)),
                Operand::Reg(format!("{}{}", p, rn)),
                Operand::Reg(format!("{}{}", p, rm)),
                Operand::Reg(format!("{}{}", p, ra)),
            ];
            let w = expect_word(encode_fmadd_fmsub(&ops, is_sub));
            let ftype = if dbl { 0b01u32 } else { 0b00u32 };
            let o1 = if is_sub { 1u32 } else { 0u32 };

            // Fixed bits of the scalar FP 3-source encoding.
            prop_assert_eq!(w >> 24, 0x1Fu32);      // bits[31:24] = 00011111
            prop_assert_eq!((w >> 21) & 1, 0u32);   // bit 21 = 0 (not FNMADD/FNMSUB)
            prop_assert_eq!(sf_of(w), 0u32);        // scalar FP, sf always 0

            // ftype, o1, and all four register fields round-trip exactly.
            prop_assert_eq!(ftype_of(w), ftype);
            prop_assert_eq!(fms_o1_of(w), o1);
            prop_assert_eq!(rd_of(w), rd);          // [4:0]
            prop_assert_eq!(rn_of(w), rn);          // [9:5]
            prop_assert_eq!(fms_rm_of(w), rm);      // [20:16]
            prop_assert_eq!(fms_ra_of(w), ra);      // [14:10]

            // Reference reconstruction.
            let expected = (0b00011111u32 << 24) | (ftype << 22) | (rm << 16)
                | (o1 << 15) | (ra << 10) | (rn << 5) | rd;
            prop_assert_eq!(w, expected);
        }

        // Oracle: precision + opcode derivation. ftype comes solely from the
        // dest prefix ('d' => 01 double, else => 00 single); is_sub selects
        // o1 (bit 15). FMADD vs FMSUB on identical operands differ ONLY in
        // bit 15 — no other field is perturbed.
        #[test]
        fn prop_fmadd_ftype_and_o1_derivation(
            rd in 0u32..32, rn in 0u32..32, rm in 0u32..32, ra in 0u32..32,
            dbl in any::<bool>(),
        ) {
            let p = if dbl { "d" } else { "s" };
            let ops = vec![
                Operand::Reg(format!("{}{}", p, rd)),
                Operand::Reg(format!("{}{}", p, rn)),
                Operand::Reg(format!("{}{}", p, rm)),
                Operand::Reg(format!("{}{}", p, ra)),
            ];
            let w_add = expect_word(encode_fmadd_fmsub(&ops, false));
            let w_sub = expect_word(encode_fmadd_fmsub(&ops, true));

            prop_assert_eq!(ftype_of(w_add), if dbl { 0b01 } else { 0b00 });
            prop_assert_eq!(fms_o1_of(w_add), 0u32); // FMADD
            prop_assert_eq!(fms_o1_of(w_sub), 1u32); // FMSUB
            // The ONLY difference between FMADD and FMSUB is bit 15.
            prop_assert_eq!(w_add ^ w_sub, 1u32 << 15);
        }

        // Oracle: determinism. Same operands + is_sub => identical word.
        #[test]
        fn prop_fmadd_is_deterministic(
            rd in 0u32..32, rn in 0u32..32, rm in 0u32..32, ra in 0u32..32,
            is_sub in any::<bool>(),
        ) {
            let ops = vec![
                Operand::Reg(format!("d{}", rd)),
                Operand::Reg(format!("d{}", rn)),
                Operand::Reg(format!("d{}", rm)),
                Operand::Reg(format!("d{}", ra)),
            ];
            let w1 = expect_word(encode_fmadd_fmsub(&ops, is_sub));
            let w2 = expect_word(encode_fmadd_fmsub(&ops, is_sub));
            prop_assert_eq!(w1, w2);
        }

        // Negative contract (validated, PASSES): out-of-range register numbers
        // (>= 32) MUST be rejected by get_reg (parse_reg_num caps at 31), not
        // masked into 5 bits; too-few operands (< 4) and non-register operands
        // must be rejected.
        #[test]
        fn prop_fmadd_rejects_bad_regs_arity(
            n in 32u32..256u32, pos in 0u32..4u32, imm in any::<i64>(),
        ) {
            // Out-of-range register in any of the four operand positions.
            let mut names = vec!["d0".to_string(), "d0".to_string(),
                                 "d0".to_string(), "d0".to_string()];
            names[pos as usize] = format!("d{}", n);
            let ops: Vec<Operand> = names.into_iter().map(Operand::Reg).collect();
            prop_assert!(
                encode_fmadd_fmsub(&ops, false).is_err(),
                "register d{} must be rejected (5-bit field), not silently masked", n
            );
            // Too few operands (< 4).
            prop_assert!(encode_fmadd_fmsub(&[], false).is_err());
            prop_assert!(encode_fmadd_fmsub(&[Operand::Reg("d0".into())], false).is_err());
            prop_assert!(encode_fmadd_fmsub(&[
                Operand::Reg("d0".into()), Operand::Reg("d0".into()), Operand::Reg("d0".into())
            ], false).is_err());
            // Non-register operand.
            let bad = vec![
                Operand::Reg("d0".into()), Operand::Reg("d0".into()),
                Operand::Reg("d0".into()), Operand::Imm(imm),
            ];
            prop_assert!(encode_fmadd_fmsub(&bad, false).is_err());
        }

        // Negative contract (FINDING — FAILS): FMADD/FMSUB operate ONLY on
        // floating-point registers, and all four operands MUST be the same
        // precision (all S or all D). But encode_fmadd_fmsub derives ftype
        // solely from operands[0] and never validates operand banks or
        // precision homogeneity, so it silently accepts illegal operands:
        //   - GP-bank operands (W/X) are mis-encoded as single-precision FP;
        //   - mixed-precision FP operands (Dd, Sn, Sm, Sa) corrupt the encoding.
        #[test]
        fn prop_fmadd_rejects_mixed_precision_and_gp_bank(n in 0u32..32) {
            // Mixed precision: dest double, sources single.
            let mix = vec![
                Operand::Reg(format!("d{}", n)),
                Operand::Reg(format!("s{}", n)),
                Operand::Reg(format!("s{}", n)),
                Operand::Reg(format!("s{}", n)),
            ];
            prop_assert!(
                encode_fmadd_fmsub(&mix, false).is_err(),
                "mixed precision (Dd, Sn, Sm, Sa) must be rejected; got {:?}",
                encode_fmadd_fmsub(&mix, false)
            );
            // GP-bank operands are not valid FP 3-source operands.
            let gp = vec![
                Operand::Reg(format!("x{}", n)),
                Operand::Reg(format!("x{}", n)),
                Operand::Reg(format!("x{}", n)),
                Operand::Reg(format!("x{}", n)),
            ];
            prop_assert!(
                encode_fmadd_fmsub(&gp, false).is_err(),
                "GP registers (x{}) are not valid FMADD/FMSUB operands; got {:?}",
                n, encode_fmadd_fmsub(&gp, false)
            );
        }
    }
}
