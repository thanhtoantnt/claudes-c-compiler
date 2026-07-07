use super::*;

// ── Instruction encoders ──────────────────────────────────────────────

pub(crate) fn encode_lui(operands: &[Operand]) -> Result<EncodeResult, String> {
    let rd = get_reg(operands, 0)?;
    match &operands.get(1) {
        Some(Operand::Imm(imm)) => {
            Ok(EncodeResult::Word(encode_u(OP_LUI, rd, (*imm as u32) << 12)))
        }
        Some(Operand::Symbol(s)) => {
            // %hi(symbol)
            Ok(EncodeResult::WordWithReloc {
                word: encode_u(OP_LUI, rd, 0),
                reloc: Relocation {
                    reloc_type: if s.starts_with("%tprel_hi(") {
                        RelocType::TprelHi20
                    } else {
                        RelocType::Hi20
                    },
                    symbol: extract_modifier_symbol(s),
                    addend: 0,
                },
            })
        }
        _ => Err("lui: invalid operands".to_string()),
    }
}

pub(crate) fn encode_auipc(operands: &[Operand]) -> Result<EncodeResult, String> {
    let rd = get_reg(operands, 0)?;
    match &operands.get(1) {
        Some(Operand::Imm(imm)) => {
            Ok(EncodeResult::Word(encode_u(OP_AUIPC, rd, (*imm as u32) << 12)))
        }
        Some(Operand::Symbol(s)) => {
            let (reloc_type, symbol) = parse_reloc_modifier(s);
            Ok(EncodeResult::WordWithReloc {
                word: encode_u(OP_AUIPC, rd, 0),
                reloc: Relocation {
                    reloc_type,
                    symbol,
                    addend: 0,
                },
            })
        }
        _ => Err("auipc: invalid operands".to_string()),
    }
}

pub(crate) fn encode_jal(operands: &[Operand]) -> Result<EncodeResult, String> {
    // jal rd, offset  OR  jal offset (rd = ra)
    if operands.len() == 1 {
        // jal offset (implicit rd = ra)
        match &operands[0] {
            Operand::Imm(imm) => {
                Ok(EncodeResult::Word(encode_j(OP_JAL, 1, *imm as i32)))
            }
            Operand::Symbol(s) | Operand::Label(s) | Operand::Reg(s) => {
                Ok(EncodeResult::WordWithReloc {
                    word: encode_j(OP_JAL, 1, 0),
                    reloc: Relocation {
                        reloc_type: RelocType::Jal,
                        symbol: s.clone(),
                        addend: 0,
                    },
                })
            }
            _ => Err("jal: invalid operand".to_string()),
        }
    } else {
        let rd = get_reg(operands, 0)?;
        match &operands[1] {
            Operand::Imm(imm) => {
                Ok(EncodeResult::Word(encode_j(OP_JAL, rd, *imm as i32)))
            }
            Operand::Symbol(s) | Operand::Label(s) | Operand::Reg(s) => {
                Ok(EncodeResult::WordWithReloc {
                    word: encode_j(OP_JAL, rd, 0),
                    reloc: Relocation {
                        reloc_type: RelocType::Jal,
                        symbol: s.clone(),
                        addend: 0,
                    },
                })
            }
            _ => Err("jal: invalid operand".to_string()),
        }
    }
}

pub(crate) fn encode_jalr(operands: &[Operand]) -> Result<EncodeResult, String> {
    // jalr rd, rs1, offset  OR  jalr rd, offset(rs1)  OR  jalr rs1
    match operands.len() {
        1 => {
            // jalr rs1 (rd = ra, offset = 0)
            let rs1 = get_reg(operands, 0)?;
            Ok(EncodeResult::Word(encode_i(OP_JALR, 1, 0, rs1, 0)))
        }
        2 => {
            // jalr rd, rs1  (offset = 0)
            let rd = get_reg(operands, 0)?;
            match &operands[1] {
                Operand::Reg(name) => {
                    let rs1 = reg_num(name).ok_or("invalid register")?;
                    Ok(EncodeResult::Word(encode_i(OP_JALR, rd, 0, rs1, 0)))
                }
                Operand::Mem { base, offset } => {
                    let rs1 = reg_num(base).ok_or("invalid base register")?;
                    Ok(EncodeResult::Word(encode_i(OP_JALR, rd, 0, rs1, *offset as i32)))
                }
                _ => Err("jalr: invalid operands".to_string()),
            }
        }
        3 => {
            let rd = get_reg(operands, 0)?;
            let rs1 = get_reg(operands, 1)?;
            let imm = get_imm(operands, 2)?;
            Ok(EncodeResult::Word(encode_i(OP_JALR, rd, 0, rs1, imm as i32)))
        }
        _ => Err("jalr: wrong number of operands".to_string()),
    }
}

pub(crate) fn encode_branch_instr(operands: &[Operand], funct3: u32) -> Result<EncodeResult, String> {
    let rs1 = get_reg(operands, 0)?;
    let rs2 = get_reg(operands, 1)?;

    match &operands.get(2) {
        Some(Operand::Imm(imm)) => {
            Ok(EncodeResult::Word(encode_b(OP_BRANCH, funct3, rs1, rs2, *imm as i32)))
        }
        Some(Operand::Symbol(s)) | Some(Operand::Label(s)) | Some(Operand::Reg(s)) => {
            Ok(EncodeResult::WordWithReloc {
                word: encode_b(OP_BRANCH, funct3, rs1, rs2, 0),
                reloc: Relocation {
                    reloc_type: RelocType::Branch,
                    symbol: s.clone(),
                    addend: 0,
                },
            })
        }
        _ => Err("branch: expected offset or label as 3rd operand".to_string()),
    }
}

pub(crate) fn encode_load(operands: &[Operand], funct3: u32) -> Result<EncodeResult, String> {
    let rd = get_reg(operands, 0)?;
    match &operands.get(1) {
        Some(Operand::Mem { base, offset }) => {
            let rs1 = reg_num(base).ok_or("invalid base register")?;
            Ok(EncodeResult::Word(encode_i(OP_LOAD, rd, funct3, rs1, *offset as i32)))
        }
        Some(Operand::MemSymbol { base, symbol, .. }) => {
            let rs1 = reg_num(base).ok_or("invalid base register")?;
            let (reloc_type, sym) = parse_reloc_modifier(symbol);
            // Use Lo12I for load-type relocations
            let reloc_type = match reloc_type {
                RelocType::PcrelHi20 => RelocType::PcrelLo12I,
                RelocType::Hi20 => RelocType::Lo12I,
                RelocType::TprelHi20 => RelocType::TprelLo12I,
                other => other,
            };
            Ok(EncodeResult::WordWithReloc {
                word: encode_i(OP_LOAD, rd, funct3, rs1, 0),
                reloc: Relocation {
                    reloc_type,
                    symbol: sym,
                    addend: 0,
                },
            })
        }
        // Bare symbol: "ld rd, symbol" pseudo-instruction
        // Expand to: auipc rd, %pcrel_hi(symbol) ; ld rd, 0(rd)
        // with R_RISCV_PCREL_HI20 on auipc and R_RISCV_PCREL_LO12_I on ld
        Some(Operand::Symbol(s)) | Some(Operand::Label(s)) => {
            Ok(EncodeResult::WordsWithRelocs(vec![
                (encode_u(OP_AUIPC, rd, 0), Some(Relocation {
                    reloc_type: RelocType::PcrelHi20,
                    symbol: s.clone(),
                    addend: 0,
                })),
                (encode_i(OP_LOAD, rd, funct3, rd, 0), Some(Relocation {
                    reloc_type: RelocType::PcrelLo12I,
                    symbol: s.clone(),
                    addend: 0,
                })),
            ]))
        }
        _ => Err("load: expected memory operand".to_string()),
    }
}

pub(crate) fn encode_store(operands: &[Operand], funct3: u32) -> Result<EncodeResult, String> {
    let rs2 = get_reg(operands, 0)?;
    match &operands.get(1) {
        Some(Operand::Mem { base, offset }) => {
            let rs1 = reg_num(base).ok_or("invalid base register")?;
            Ok(EncodeResult::Word(encode_s(OP_STORE, funct3, rs1, rs2, *offset as i32)))
        }
        Some(Operand::MemSymbol { base, symbol, .. }) => {
            let rs1 = reg_num(base).ok_or("invalid base register")?;
            let (reloc_type, sym) = parse_reloc_modifier(symbol);
            let reloc_type = match reloc_type {
                RelocType::PcrelHi20 => RelocType::PcrelLo12S,
                RelocType::Hi20 => RelocType::Lo12S,
                RelocType::TprelHi20 => RelocType::TprelLo12S,
                other => other,
            };
            Ok(EncodeResult::WordWithReloc {
                word: encode_s(OP_STORE, funct3, rs1, rs2, 0),
                reloc: Relocation {
                    reloc_type,
                    symbol: sym,
                    addend: 0,
                },
            })
        }
        _ => Err("store: expected memory operand".to_string()),
    }
}

pub(crate) fn encode_alu_imm(operands: &[Operand], funct3: u32) -> Result<EncodeResult, String> {
    let rd = get_reg(operands, 0)?;
    let rs1 = get_reg(operands, 1)?;
    match &operands.get(2) {
        Some(Operand::Imm(imm)) => {
            Ok(EncodeResult::Word(encode_i(OP_OP_IMM, rd, funct3, rs1, *imm as i32)))
        }
        Some(Operand::Symbol(s)) => {
            let (reloc_type, sym) = parse_reloc_modifier(s);
            let reloc_type = match reloc_type {
                RelocType::PcrelHi20 => RelocType::PcrelLo12I,
                RelocType::Hi20 => RelocType::Lo12I,
                RelocType::TprelHi20 => RelocType::TprelLo12I,
                other => other,
            };
            Ok(EncodeResult::WordWithReloc {
                word: encode_i(OP_OP_IMM, rd, funct3, rs1, 0),
                reloc: Relocation {
                    reloc_type,
                    symbol: sym,
                    addend: 0,
                },
            })
        }
        _ => Err("alu_imm: expected immediate".to_string()),
    }
}

pub(crate) fn encode_shift_imm(operands: &[Operand], funct3: u32, funct6: u32) -> Result<EncodeResult, String> {
    let rd = get_reg(operands, 0)?;
    let rs1 = get_reg(operands, 1)?;
    let shamt = get_imm(operands, 2)? as u32;
    // For RV64, shift amount is 6 bits
    let imm = (funct6 << 6) | (shamt & 0x3F);
    Ok(EncodeResult::Word(encode_i(OP_OP_IMM, rd, funct3, rs1, imm as i32)))
}

pub(crate) fn encode_alu_reg(operands: &[Operand], funct3: u32, funct7: u32) -> Result<EncodeResult, String> {
    let rd = get_reg(operands, 0)?;
    let rs1 = get_reg(operands, 1)?;
    let rs2 = get_reg(operands, 2)?;
    Ok(EncodeResult::Word(encode_r(OP_OP, rd, funct3, rs1, rs2, funct7)))
}

pub(crate) fn encode_alu_imm_w(operands: &[Operand], funct3: u32) -> Result<EncodeResult, String> {
    let rd = get_reg(operands, 0)?;
    let rs1 = get_reg(operands, 1)?;
    let imm = get_imm(operands, 2)? as i32;
    Ok(EncodeResult::Word(encode_i(OP_OP_IMM_32, rd, funct3, rs1, imm)))
}

pub(crate) fn encode_shift_imm_w(operands: &[Operand], funct3: u32, funct7: u32) -> Result<EncodeResult, String> {
    let rd = get_reg(operands, 0)?;
    let rs1 = get_reg(operands, 1)?;
    let shamt = get_imm(operands, 2)? as u32;
    // For RV32/W operations, shift amount is 5 bits
    let imm = (funct7 << 5) | (shamt & 0x1F);
    Ok(EncodeResult::Word(encode_i(OP_OP_IMM_32, rd, funct3, rs1, imm as i32)))
}

pub(crate) fn encode_alu_reg_w(operands: &[Operand], funct3: u32, funct7: u32) -> Result<EncodeResult, String> {
    let rd = get_reg(operands, 0)?;
    let rs1 = get_reg(operands, 1)?;
    let rs2 = get_reg(operands, 2)?;
    Ok(EncodeResult::Word(encode_r(OP_OP_32, rd, funct3, rs1, rs2, funct7)))
}

// ── Zbb (bit manipulation) helpers ──

/// Encode a Zbb unary instruction (clz, ctz, cpop, sext.b, sext.h, rev8).
/// These are I-type with funct3=001 and the 12-bit immediate encoding the operation.
pub(crate) fn encode_zbb_unary(operands: &[Operand], imm12: u32) -> Result<EncodeResult, String> {
    let rd = get_reg(operands, 0)?;
    let rs1 = get_reg(operands, 1)?;
    Ok(EncodeResult::Word(encode_i(OP_OP_IMM, rd, 0b001, rs1, imm12 as i32)))
}

/// Encode a Zbb unary instruction with funct3=101 (rev8, orc.b).
pub(crate) fn encode_zbb_unary_f5(operands: &[Operand], imm12: u32) -> Result<EncodeResult, String> {
    let rd = get_reg(operands, 0)?;
    let rs1 = get_reg(operands, 1)?;
    Ok(EncodeResult::Word(encode_i(OP_OP_IMM, rd, 0b101, rs1, imm12 as i32)))
}

/// Encode a Zbb unary word instruction (clzw, ctzw, cpopw).
/// These are I-type on OP-IMM-32 with funct3=001.
pub(crate) fn encode_zbb_unary_w(operands: &[Operand], imm12: u32) -> Result<EncodeResult, String> {
    let rd = get_reg(operands, 0)?;
    let rs1 = get_reg(operands, 1)?;
    Ok(EncodeResult::Word(encode_i(OP_OP_IMM_32, rd, 0b001, rs1, imm12 as i32)))
}

/// Encode zext.h rd, rs1 (R-type on OP-32: funct7=0000100, rs2=0, funct3=100).
pub(crate) fn encode_zbb_zexth(operands: &[Operand]) -> Result<EncodeResult, String> {
    let rd = get_reg(operands, 0)?;
    let rs1 = get_reg(operands, 1)?;
    Ok(EncodeResult::Word(encode_r(OP_OP_32, rd, 0b100, rs1, 0, 0b0000100)))
}

#[cfg(test)]
mod pbt_encode_lui {
    use super::*;
    use proptest::prelude::*;

    const OP_LUI_BITS: u32 = OP_LUI; // 0b0110111 = 0x37

    /// Strategy yielding (register_name, expected_5bit_number) pairs covering
    /// both the `xN` form and the ABI alias names accepted by `reg_num`.
    fn reg_strategy() -> impl Strategy<Value = (String, u32)> {
        let pairs: Vec<(String, u32)> = (0u32..=31)
            .flat_map(|n| {
                let mut v: Vec<(String, u32)> = vec![(format!("x{}", n), n)];
                let abi: Option<&'static str> = match n {
                    0 => Some("zero"), 1 => Some("ra"), 2 => Some("sp"), 3 => Some("gp"),
                    4 => Some("tp"), 5 => Some("t0"), 6 => Some("t1"), 7 => Some("t2"),
                    8 => Some("s0"), 9 => Some("s1"), 10 => Some("a0"), 11 => Some("a1"),
                    12 => Some("a2"), 13 => Some("a3"), 14 => Some("a4"), 15 => Some("a5"),
                    16 => Some("a6"), 17 => Some("a7"), 18 => Some("s2"), 19 => Some("s3"),
                    20 => Some("s4"), 21 => Some("s5"), 22 => Some("s6"), 23 => Some("s7"),
                    24 => Some("s8"), 25 => Some("s9"), 26 => Some("s10"), 27 => Some("s11"),
                    28 => Some("t3"), 29 => Some("t4"), 30 => Some("t5"), 31 => Some("t6"),
                    _ => None,
                };
                if let Some(a) = abi {
                    v.push((a.to_string(), n));
                }
                if n == 8 {
                    v.push(("fp".to_string(), n));
                }
                v
            })
            .collect();
        proptest::sample::select(pairs)
    }

    proptest! {
        // Oracle: Reference — `lui rd, imm` must produce a U-format word whose
        // opcode field (bits[6:0]) is LUI, whose rd field (bits[11:7]) equals
        // the destination register number, and whose imm field (bits[31:12])
        // carries exactly the low 20 bits of the immediate (the assembler
        // pre-shifts by 12; encode_u masks with 0xFFFFF000).
        #[test]
        fn lui_imm_encodes_opcode_rd_and_imm_fields(
            (rd_name, rd_num) in reg_strategy(),
            imm in any::<i64>()
        ) {
            let ops = [Operand::Reg(rd_name), Operand::Imm(imm)];
            let w = match encode_lui(&ops).expect("lui imm must encode") {
                EncodeResult::Word(w) => w,
                other => panic!("expected Word, got {:?}", other),
            };

            prop_assert_eq!(w & 0x7F, OP_LUI_BITS, "opcode bits[6:0]");
            prop_assert_eq!((w >> 7) & 0x1F, rd_num, "rd bits[11:7]");
            prop_assert_eq!(
                (w >> 12) & 0xFFFFF,
                (imm as u32) & 0xFFFFF,
                "imm[31:12] == low 20 bits of immediate"
            );
            // Cross-check against the raw U-format encoder used by the impl.
            prop_assert_eq!(w, encode_u(OP_LUI, rd_num, (imm as u32) << 12));
        }

        // Oracle: Reference — GCC-style bare register numbers (Operand::Imm
        // with value 0..=31) are accepted by get_reg as the destination.
        #[test]
        fn lui_accepts_bare_number_destination(
            rd_num in 0u32..=31,
            imm in any::<i64>()
        ) {
            let ops = [Operand::Imm(rd_num as i64), Operand::Imm(imm)];
            let w = match encode_lui(&ops).expect("bare-number rd must encode") {
                EncodeResult::Word(w) => w,
                other => panic!("expected Word, got {:?}", other),
            };
            prop_assert_eq!(w & 0x7F, OP_LUI_BITS);
            prop_assert_eq!((w >> 7) & 0x1F, rd_num);
        }

        // Oracle: Reference — `lui rd, symbol` (no %tprel_hi prefix) emits a
        // WordWithReloc with a zeroed immediate field, Hi20 reloc type, zero
        // addend, and the modifier-stripped symbol name.
        #[test]
        fn lui_symbol_emits_hi20_relocation(
            (rd_name, rd_num) in reg_strategy(),
            sym_body in "[%a-zA-Z_][%a-zA-Z0-9_]*"
        ) {
            let s = format!("%hi({})", sym_body);
            let ops = [Operand::Reg(rd_name), Operand::Symbol(s.clone())];
            let (word, reloc) = match encode_lui(&ops).expect("lui symbol must encode") {
                EncodeResult::WordWithReloc { word, reloc } => (word, reloc),
                other => panic!("expected WordWithReloc, got {:?}", other),
            };

            prop_assert_eq!(word & 0x7F, OP_LUI_BITS, "opcode");
            prop_assert_eq!((word >> 7) & 0x1F, rd_num, "rd");
            prop_assert_eq!(word & 0xFFFFF000, 0, "imm field must be zero");
            prop_assert_eq!(word, encode_u(OP_LUI, rd_num, 0));
            match reloc.reloc_type {
                RelocType::Hi20 => {}
                other => panic!("expected Hi20, got {:?}", other),
            }
            prop_assert_eq!(reloc.addend, 0);
            prop_assert_eq!(&reloc.symbol, &extract_modifier_symbol(&s));
            prop_assert_eq!(&reloc.symbol, &sym_body);
        }

        // Oracle: Reference — a `%tprel_hi(sym)` operand selects the TLS
        // TprelHi20 relocation variant while still zeroing the immediate.
        #[test]
        fn lui_tprel_hi_symbol_emits_tprel_hi20_relocation(
            (rd_name, _rd_num) in reg_strategy(),
            sym_body in "[%a-zA-Z_][%a-zA-Z0-9_]*"
        ) {
            let s = format!("%tprel_hi({})", sym_body);
            let ops = [Operand::Reg(rd_name), Operand::Symbol(s.clone())];
            let reloc = match encode_lui(&ops).expect("lui tprel must encode") {
                EncodeResult::WordWithReloc { word, reloc } => {
                    prop_assert_eq!(word & 0xFFFFF000, 0);
                    reloc
                }
                other => panic!("expected WordWithReloc, got {:?}", other),
            };
            match reloc.reloc_type {
                RelocType::TprelHi20 => {}
                other => panic!("expected TprelHi20, got {:?}", other),
            }
            prop_assert_eq!(reloc.addend, 0);
            prop_assert_eq!(reloc.symbol, sym_body);
        }

        // Oracle: Negative/error contract — lui requires exactly (rd, imm|sym).
        // A non-Imm/non-Symbol second operand, or a missing second operand,
        // must yield the specific "lui: invalid operands" error; a missing or
        // unparseable first register must also be rejected.
        #[test]
        fn lui_rejects_invalid_operands(
            bad_second in prop::sample::select(vec![
                Operand::Reg("a1".to_string()),
                Operand::Label("foo".to_string()),
                Operand::Mem { base: "sp".to_string(), offset: 0 },
                Operand::MemSymbol { base: "sp".to_string(), symbol: "s".to_string(), modifier: "%lo".to_string() },
                Operand::SymbolOffset("s".to_string(), 4),
                Operand::FenceArg("iorw".to_string()),
            ])
        ) {
            // Present but invalid second operand.
            let ops = vec![Operand::Reg("a0".to_string()), bad_second.clone()];
            let err = encode_lui(&ops).expect_err("bad 2nd operand must error");
            prop_assert!(
                err.contains("lui: invalid operands"),
                "unexpected error message: {}",
                err
            );

            // Missing second operand.
            let ops = vec![Operand::Reg("a0".to_string())];
            let err = encode_lui(&ops).expect_err("missing 2nd operand must error");
            prop_assert!(err.contains("lui: invalid operands"), "got: {}", err);

            // Missing first operand (different message, still an error).
            prop_assert!(encode_lui(&[]).is_err());

            // Invalid register name as first operand.
            let ops = vec![Operand::Reg("x32".to_string()), Operand::Imm(1)];
            prop_assert!(encode_lui(&ops).is_err());
        }
    }
}

#[cfg(test)]
mod pbt_encode_shift_imm {
    use super::*;
    use proptest::prelude::*;

    const OP_OP_IMM_BITS: u32 = OP_OP_IMM; // 0b0010011 == 0x13

    /// Strategy yielding (register_name, expected_5bit_number) pairs covering
    /// both the `xN` form and the ABI alias names accepted by `reg_num`.
    fn reg_strategy() -> impl Strategy<Value = (String, u32)> {
        let pairs: Vec<(String, u32)> = (0u32..=31)
            .flat_map(|n| {
                let mut v: Vec<(String, u32)> = vec![(format!("x{}", n), n)];
                let abi: Option<&'static str> = match n {
                    0 => Some("zero"), 1 => Some("ra"), 2 => Some("sp"), 3 => Some("gp"),
                    4 => Some("tp"), 5 => Some("t0"), 6 => Some("t1"), 7 => Some("t2"),
                    8 => Some("s0"), 9 => Some("s1"), 10 => Some("a0"), 11 => Some("a1"),
                    12 => Some("a2"), 13 => Some("a3"), 14 => Some("a4"), 15 => Some("a5"),
                    16 => Some("a6"), 17 => Some("a7"), 18 => Some("s2"), 19 => Some("s3"),
                    20 => Some("s4"), 21 => Some("s5"), 22 => Some("s6"), 23 => Some("s7"),
                    24 => Some("s8"), 25 => Some("s9"), 26 => Some("s10"), 27 => Some("s11"),
                    28 => Some("t3"), 29 => Some("t4"), 30 => Some("t5"), 31 => Some("t6"),
                    _ => None,
                };
                if let Some(a) = abi {
                    v.push((a.to_string(), n));
                }
                if n == 8 {
                    v.push(("fp".to_string(), n));
                }
                v
            })
            .collect();
        proptest::sample::select(pairs)
    }

    proptest! {
        // Oracle: Reference — encode_shift_imm builds an I-format OP-IMM word
        // whose every field is exactly determined by its inputs:
        //   bits[6:0]   = OP_OP_IMM (0x13)
        //   bits[11:7]  = rd register number
        //   bits[14:12] = funct3 argument
        //   bits[19:15] = rs1 register number
        //   bits[31:20] = (funct6 & 0x3F) << 6 | (shamt & 0x3F)
        #[test]
        fn shift_imm_encodes_all_fields(
            (rd_name, rd_num) in reg_strategy(),
            (rs1_name, rs1_num) in reg_strategy(),
            funct3 in 0u32..8,
            funct6 in 0u32..64,
            shamt in 0i64..64,
        ) {
            let ops = [Operand::Reg(rd_name), Operand::Reg(rs1_name), Operand::Imm(shamt)];
            let w = match encode_shift_imm(&ops, funct3, funct6).expect("valid operands must encode") {
                EncodeResult::Word(w) => w,
                other => panic!("expected Word, got {:?}", other),
            };

            prop_assert_eq!(w & 0x7F, OP_OP_IMM_BITS, "opcode bits[6:0]");
            prop_assert_eq!((w >> 7) & 0x1F, rd_num, "rd bits[11:7]");
            prop_assert_eq!((w >> 12) & 0x7, funct3, "funct3 bits[14:12]");
            prop_assert_eq!((w >> 15) & 0x1F, rs1_num, "rs1 bits[19:15]");
            let expected_imm_field = ((funct6 & 0x3F) << 6) | ((shamt as u32) & 0x3F);
            prop_assert_eq!((w >> 20) & 0xFFF, expected_imm_field, "imm bits[31:20]");
        }

        // Oracle: Reference — the shift amount is masked to its low 6 bits
        // (`shamt & 0x3F`), so a shamt and its low-6-bits value must produce
        // the identical word. This also holds for negative immediates, which
        // re-interpret to large u32 values before masking.
        #[test]
        fn shift_amt_is_masked_to_six_bits(
            (rd_name, _rd) in reg_strategy(),
            (rs1_name, _rs1) in reg_strategy(),
            funct3 in 0u32..8,
            funct6 in 0u32..64,
            shamt in any::<i64>(),
        ) {
            let ops_full = [Operand::Reg(rd_name.clone()), Operand::Reg(rs1_name.clone()), Operand::Imm(shamt)];
            let w_full = match encode_shift_imm(&ops_full, funct3, funct6).expect("must encode") {
                EncodeResult::Word(w) => w,
                other => panic!("expected Word, got {:?}", other),
            };

            let masked = (shamt as u32) & 0x3F;
            let ops_masked = [Operand::Reg(rd_name), Operand::Reg(rs1_name), Operand::Imm(masked as i64)];
            let w_masked = match encode_shift_imm(&ops_masked, funct3, funct6).expect("must encode") {
                EncodeResult::Word(w) => w,
                other => panic!("expected Word, got {:?}", other),
            };

            prop_assert_eq!(w_full, w_masked, "shamt must be masked to low 6 bits");
        }

        // Oracle: Reference — the output exactly equals the I-format encoder
        // applied to the computed immediate, i.e. encode_shift_imm is a thin
        // wrapper over encode_i with imm = (funct6<<6)|(shamt&0x3F).
        #[test]
        fn shift_imm_matches_encode_i_reference(
            (rd_name, rd_num) in reg_strategy(),
            (rs1_name, rs1_num) in reg_strategy(),
            funct3 in 0u32..8,
            funct6 in 0u32..64,
            shamt in any::<i64>(),
        ) {
            let ops = [Operand::Reg(rd_name), Operand::Reg(rs1_name), Operand::Imm(shamt)];
            let w = match encode_shift_imm(&ops, funct3, funct6).expect("must encode") {
                EncodeResult::Word(w) => w,
                other => panic!("expected Word, got {:?}", other),
            };
            let imm = (funct6 << 6) | ((shamt as u32) & 0x3F);
            prop_assert_eq!(w, encode_i(OP_OP_IMM, rd_num, funct3, rs1_num, imm as i32));
        }

        // Oracle: Reference — the three real base-ISA shift-immediate mnemonics
        // (slli/srli/srai) encoded through encode_shift_imm yield canonical
        // RISC-V words: opcode 0x13, funct6 in bits[31:26], shamt in bits[25:20].
        #[test]
        fn real_shift_mnemonics_decode_to_canonical_fields(
            shamt in 0i64..64,
            rd_num in 0u32..32,
            rs1_num in 0u32..32,
        ) {
            for (mnem, f3, f6) in [
                ("slli", 0b001u32, 0b000000u32),
                ("srli", 0b101u32, 0b000000u32),
                ("srai", 0b101u32, 0b010000u32),
            ] {
                let ops = vec![
                    Operand::Reg(format!("x{}", rd_num)),
                    Operand::Reg(format!("x{}", rs1_num)),
                    Operand::Imm(shamt),
                ];
                let w = match encode_shift_imm(&ops, f3, f6).expect("must encode") {
                    EncodeResult::Word(w) => w,
                    other => panic!("expected Word, got {:?}", other),
                };
                prop_assert_eq!(w & 0x7F, 0x13, "opcode for {}", mnem);
                prop_assert_eq!((w >> 26) & 0x3F, f6, "funct6 for {}", mnem);
                prop_assert_eq!((w >> 20) & 0x3F, shamt as u32, "shamt for {}", mnem);
                prop_assert_eq!((w >> 12) & 0x7, f3, "funct3 for {}", mnem);
            }
        }

        // Oracle: Negative/error contract — encode_shift_imm requires three
        // operands with the third being an immediate; fewer operands, a
        // non-immediate third operand, or an unparseable register must error.
        #[test]
        fn shift_imm_rejects_invalid_operands(
            bad_third in prop::sample::select(vec![
                Operand::Reg("a1".to_string()),
                Operand::Label("foo".to_string()),
                Operand::Symbol("bar".to_string()),
                Operand::Mem { base: "sp".to_string(), offset: 0 },
            ])
        ) {
            // Non-immediate third operand.
            let ops = vec![Operand::Reg("a0".to_string()), Operand::Reg("a1".to_string()), bad_third.clone()];
            let err = encode_shift_imm(&ops, 0b001, 0b000000)
                .expect_err("non-imm 3rd operand must error");
            prop_assert!(err.contains("expected immediate"), "got: {}", err);

            // Missing third operand.
            let ops = vec![Operand::Reg("a0".to_string()), Operand::Reg("a1".to_string())];
            let err = encode_shift_imm(&ops, 0b001, 0b000000)
                .expect_err("missing 3rd operand must error");
            prop_assert!(err.contains("expected immediate"), "got: {}", err);

            // Missing second operand.
            prop_assert!(encode_shift_imm(&[Operand::Reg("a0".to_string())], 0b001, 0b000000).is_err());

            // Empty operands.
            prop_assert!(encode_shift_imm(&[], 0b001, 0b000000).is_err());

            // Invalid register name as first operand.
            let ops = vec![Operand::Reg("x32".to_string()), Operand::Reg("a1".to_string()), Operand::Imm(1)];
            let err = encode_shift_imm(&ops, 0b001, 0b000000)
                .expect_err("invalid register must error");
            prop_assert!(err.contains("invalid integer register"), "got: {}", err);
        }
    }
}

#[cfg(test)]
mod pbt_encode_jal {
    use super::*;
    use proptest::prelude::*;

    const OP_JAL_BITS: u32 = OP_JAL; // 0b1101111 == 0x6F

    /// Strategy yielding (register_name, expected_5bit_number) pairs covering
    /// both the `xN` form and the ABI alias names accepted by `reg_num`.
    fn reg_strategy() -> impl Strategy<Value = (String, u32)> {
        let pairs: Vec<(String, u32)> = (0u32..=31)
            .flat_map(|n| {
                let mut v: Vec<(String, u32)> = vec![(format!("x{}", n), n)];
                let abi: Option<&'static str> = match n {
                    0 => Some("zero"), 1 => Some("ra"), 2 => Some("sp"), 3 => Some("gp"),
                    4 => Some("tp"), 5 => Some("t0"), 6 => Some("t1"), 7 => Some("t2"),
                    8 => Some("s0"), 9 => Some("s1"), 10 => Some("a0"), 11 => Some("a1"),
                    12 => Some("a2"), 13 => Some("a3"), 14 => Some("a4"), 15 => Some("a5"),
                    16 => Some("a6"), 17 => Some("a7"), 18 => Some("s2"), 19 => Some("s3"),
                    20 => Some("s4"), 21 => Some("s5"), 22 => Some("s6"), 23 => Some("s7"),
                    24 => Some("s8"), 25 => Some("s9"), 26 => Some("s10"), 27 => Some("s11"),
                    28 => Some("t3"), 29 => Some("t4"), 30 => Some("t5"), 31 => Some("t6"),
                    _ => None,
                };
                if let Some(a) = abi {
                    v.push((a.to_string(), n));
                }
                if n == 8 {
                    v.push(("fp".to_string(), n));
                }
                v
            })
            .collect();
        proptest::sample::select(pairs)
    }

    /// Reference decoder for the J-type immediate field. Given a full 32-bit
    /// instruction word, reconstructs the signed 21-bit jump offset (in bytes).
    /// This is the inverse of `encode_j`'s bit-scattering for the J-format.
    fn decode_j_imm(w: u32) -> i32 {
        let bit20 = (w >> 31) & 1;
        let bits10_1 = (w >> 21) & 0x3FF;
        let bit11 = (w >> 20) & 1;
        let bits19_12 = (w >> 12) & 0xFF;
        // Reassemble the 21-bit immediate (bit 0 is implicitly 0).
        let imm21 = (bit20 << 20) | (bits19_12 << 12) | (bit11 << 11) | (bits10_1 << 1);
        // Sign-extend from bit 20 of the 21-bit quantity.
        ((imm21 << 11) as i32) >> 11
    }

    proptest! {
        // Oracle: Reference — `jal rd, offset` with an immediate must produce
        // a J-format word whose opcode (bits[6:0]) is JAL, whose rd field
        // (bits[11:7]) equals the destination register number, and whose
        // value is exactly the raw J-format encoder applied to the same args.
        #[test]
        fn jal_rd_imm_encodes_opcode_and_rd(
            (rd_name, rd_num) in reg_strategy(),
            imm in any::<i32>(),
        ) {
            let ops = [Operand::Reg(rd_name), Operand::Imm(imm as i64)];
            let w = match encode_jal(&ops).expect("jal rd,imm must encode") {
                EncodeResult::Word(w) => w,
                other => panic!("expected Word, got {:?}", other),
            };

            prop_assert_eq!(w & 0x7F, OP_JAL_BITS, "opcode bits[6:0]");
            prop_assert_eq!((w >> 7) & 0x1F, rd_num, "rd bits[11:7]");
            // Cross-check against the raw J-format encoder used by the impl.
            prop_assert_eq!(w, encode_j(OP_JAL, rd_num, imm));
        }

        // Oracle: Reference (round-trip decode) — for any offset that fits the
        // J-format range and is a multiple of 2, decoding the encoded word's
        // immediate field must reproduce the original offset byte-for-byte.
        // This is the strongest correctness oracle for the bit-scatter layout
        // imm[20|10:1|11|19:12].
        #[test]
        fn jal_imm_decodes_roundtrip(
            rd_num in 0u32..=31,
            imm_raw in (-0x100000i32)..=(0x0FFFFE), // [-2^20, 2^20-2]
        ) {
            let imm = imm_raw & !1; // force even (bit 0 is implicit)
            let ops = [
                Operand::Reg(format!("x{}", rd_num)),
                Operand::Imm(imm as i64),
            ];
            let w = match encode_jal(&ops).expect("jal must encode") {
                EncodeResult::Word(w) => w,
                other => panic!("expected Word, got {:?}", other),
            };

            prop_assert_eq!(decode_j_imm(w), imm, "decoded immediate must match input");
            prop_assert_eq!((w >> 7) & 0x1F, rd_num, "rd preserved");
            prop_assert_eq!(w & 0x7F, OP_JAL_BITS, "opcode preserved");
        }

        // Oracle: Algebraic (parity invariance) — `encode_j` never reads
        // immediate bit 0, so `jal rd, n` and `jal rd, n & !1` must produce
        // identical words. This matches the RISC-V spec: J offsets are
        // implicitly multiples of two (half-word aligned).
        #[test]
        fn jal_imm_parity_invariant(
            (rd_name, rd_num) in reg_strategy(),
            imm in any::<i64>(),
        ) {
            let w_odd = match encode_jal(&[Operand::Reg(rd_name.clone()), Operand::Imm(imm)]).expect("must encode") {
                EncodeResult::Word(w) => w,
                other => panic!("expected Word, got {:?}", other),
            };
            let w_even = match encode_jal(&[Operand::Reg(rd_name), Operand::Imm(imm & !1)]).expect("must encode") {
                EncodeResult::Word(w) => w,
                other => panic!("expected Word, got {:?}", other),
            };
            prop_assert_eq!(w_odd, w_even, "bit 0 of immediate must be ignored");
            prop_assert_eq!(w_odd, encode_j(OP_JAL, rd_num, (imm & !1) as i32));
        }

        // Oracle: Reference — the one-operand form `jal offset` must default
        // the destination to ra (x1) per the RISC-V pseudo-instruction rule,
        // while still encoding the immediate identically to `jal ra, offset`.
        #[test]
        fn jal_single_imm_implicit_ra(
            imm in any::<i32>(),
        ) {
            let w_implicit = match encode_jal(&[Operand::Imm(imm as i64)]).expect("jal imm must encode") {
                EncodeResult::Word(w) => w,
                other => panic!("expected Word, got {:?}", other),
            };
            let w_explicit = match encode_jal(&[Operand::Reg("ra".to_string()), Operand::Imm(imm as i64)])
                .expect("jal ra,imm must encode") {
                EncodeResult::Word(w) => w,
                other => panic!("expected Word, got {:?}", other),
            };
            prop_assert_eq!((w_implicit >> 7) & 0x1F, 1u32, "implicit rd must be ra (x1)");
            prop_assert_eq!(w_implicit, w_explicit, "jal off == jal ra, off");
        }

        // Oracle: Reference — any non-immediate second operand (Symbol, Label,
        // or Reg-as-symbol) in `jal rd, X`, and the one-operand form `jal X`,
        // must defer the offset to link time: emit WordWithReloc with
        // RelocType::Jal, a zeroed offset field (word == encode_j(JAL, rd, 0)),
        // a zero addend, and the symbol carried verbatim.
        #[test]
        fn jal_symbol_emits_jal_relocation(
            (rd_name, rd_num) in reg_strategy(),
            sym in "[a-zA-Z_][a-zA-Z0-9_]*",
            kind in prop::sample::select(vec![0u8, 1, 2]), // Symbol / Label / Reg
        ) {
            let sym_operand = match kind {
                0 => Operand::Symbol(sym.clone()),
                1 => Operand::Label(sym.clone()),
                _ => Operand::Reg(sym.clone()),
            };

            // Two-operand form: jal rd, sym
            let (word, reloc) = match encode_jal(&[Operand::Reg(rd_name.clone()), sym_operand.clone()])
                .expect("jal rd,sym must encode") {
                EncodeResult::WordWithReloc { word, reloc } => (word, reloc),
                other => panic!("expected WordWithReloc, got {:?}", other),
            };
            prop_assert_eq!(word, encode_j(OP_JAL, rd_num, 0), "offset field zeroed for reloc");
            prop_assert_eq!(word & 0x7F, OP_JAL_BITS, "opcode");
            prop_assert_eq!((word >> 7) & 0x1F, rd_num, "rd");
            match reloc.reloc_type {
                RelocType::Jal => {}
                other => panic!("expected Jal, got {:?}", other),
            }
            prop_assert_eq!(reloc.addend, 0, "addend must be zero");
            prop_assert_eq!(reloc.symbol, sym.clone(), "symbol carried verbatim");

            // One-operand form: jal sym  -> implicit rd = ra (x1)
            let (word1, reloc1) = match encode_jal(&[sym_operand]).expect("jal sym must encode") {
                EncodeResult::WordWithReloc { word, reloc } => (word, reloc),
                other => panic!("expected WordWithReloc, got {:?}", other),
            };
            prop_assert_eq!(word1, encode_j(OP_JAL, 1, 0), "1-op form zeroes offset, rd=1");
            match reloc1.reloc_type {
                RelocType::Jal => {}
                other => panic!("expected Jal, got {:?}", other),
            }
            prop_assert_eq!(reloc1.addend, 0);
            prop_assert_eq!(reloc1.symbol, sym);
        }

        // Oracle: Negative/error contract — jal requires (offset) or
        // (rd, offset) where offset is Imm/Symbol/Label/Reg. Any operand shape
        // that is not one of these, a missing operand, or an unparseable
        // register must be rejected with a non-empty error.
        #[test]
        fn jal_rejects_invalid_operands(
            bad in prop::sample::select(vec![
                Operand::Mem { base: "sp".to_string(), offset: 0 },
                Operand::MemSymbol { base: "sp".to_string(), symbol: "s".to_string(), modifier: "%lo".to_string() },
                Operand::SymbolOffset("s".to_string(), 4),
                Operand::FenceArg("iorw".to_string()),
                Operand::Csr("cycle".to_string()),
                Operand::RoundingMode("rne".to_string()),
            ])
        ) {
            // One-operand form with an unsupported operand shape.
            let err = encode_jal(&[bad.clone()]).expect_err("1-op unsupported operand must error");
            prop_assert!(err.contains("jal: invalid operand"), "got: {}", err);

            // Two-operand form with an unsupported second operand.
            let ops = vec![Operand::Reg("a0".to_string()), bad];
            let err = encode_jal(&ops).expect_err("2-op unsupported operand must error");
            prop_assert!(err.contains("jal: invalid operand"), "got: {}", err);

            // Empty operands (no operands at all).
            prop_assert!(encode_jal(&[]).is_err(), "empty operands must error");

            // Invalid register name as first operand.
            let ops = vec![Operand::Reg("x32".to_string()), Operand::Imm(0)];
            let err = encode_jal(&ops).expect_err("invalid register must error");
            prop_assert!(err.contains("invalid integer register"), "got: {}", err);
        }
    }
}

#[cfg(test)]
mod pbt_encode_alu_imm {
    use super::*;
    use proptest::prelude::*;

    const OP_OP_IMM_BITS: u32 = OP_OP_IMM; // 0b0010011 == 0x13

    /// Strategy yielding (register_name, expected_5bit_number) pairs covering
    /// both the `xN` form and the ABI alias names accepted by `reg_num`.
    fn reg_strategy() -> impl Strategy<Value = (String, u32)> {
        let pairs: Vec<(String, u32)> = (0u32..=31)
            .flat_map(|n| {
                let mut v: Vec<(String, u32)> = vec![(format!("x{}", n), n)];
                let abi: Option<&'static str> = match n {
                    0 => Some("zero"), 1 => Some("ra"), 2 => Some("sp"), 3 => Some("gp"),
                    4 => Some("tp"), 5 => Some("t0"), 6 => Some("t1"), 7 => Some("t2"),
                    8 => Some("s0"), 9 => Some("s1"), 10 => Some("a0"), 11 => Some("a1"),
                    12 => Some("a2"), 13 => Some("a3"), 14 => Some("a4"), 15 => Some("a5"),
                    16 => Some("a6"), 17 => Some("a7"), 18 => Some("s2"), 19 => Some("s3"),
                    20 => Some("s4"), 21 => Some("s5"), 22 => Some("s6"), 23 => Some("s7"),
                    24 => Some("s8"), 25 => Some("s9"), 26 => Some("s10"), 27 => Some("s11"),
                    28 => Some("t3"), 29 => Some("t4"), 30 => Some("t5"), 31 => Some("t6"),
                    _ => None,
                };
                if let Some(a) = abi {
                    v.push((a.to_string(), n));
                }
                if n == 8 {
                    v.push(("fp".to_string(), n));
                }
                v
            })
            .collect();
        proptest::sample::select(pairs)
    }

    proptest! {
        // Oracle: Reference — `encode_alu_imm` with an immediate operand builds
        // an I-format OP-IMM word whose every field is exactly determined by
        // its inputs:
        //   bits[6:0]   = OP_OP_IMM (0x13)
        //   bits[11:7]  = rd register number
        //   bits[14:12] = funct3 argument
        //   bits[19:15] = rs1 register number
        //   bits[31:20] = low 12 bits of (imm cast to i32)
        // The whole word must also equal the raw I-format encoder applied to
        // the same arguments.
        #[test]
        fn alu_imm_encodes_all_fields(
            (rd_name, rd_num) in reg_strategy(),
            (rs1_name, rs1_num) in reg_strategy(),
            funct3 in 0u32..8,
            imm in any::<i64>(),
        ) {
            let ops = [Operand::Reg(rd_name), Operand::Reg(rs1_name), Operand::Imm(imm)];
            let w = match encode_alu_imm(&ops, funct3).expect("valid operands must encode") {
                EncodeResult::Word(w) => w,
                other => panic!("expected Word, got {:?}", other),
            };

            prop_assert_eq!(w & 0x7F, OP_OP_IMM_BITS, "opcode bits[6:0]");
            prop_assert_eq!((w >> 7) & 0x1F, rd_num, "rd bits[11:7]");
            prop_assert_eq!((w >> 12) & 0x7, funct3, "funct3 bits[14:12]");
            prop_assert_eq!((w >> 15) & 0x1F, rs1_num, "rs1 bits[19:15]");
            // Cross-check against the raw I-format encoder used by the impl.
            prop_assert_eq!(w, encode_i(OP_OP_IMM, rd_num, funct3, rs1_num, imm as i32));
        }

        // Oracle: Reference (truncation semantics) — the assembler casts the
        // i64 immediate to i32 before encoding, and encode_i masks to the low
        // 12 bits. The imm field must therefore equal
        // `((imm as i32) as u32) & 0xFFF` for ANY i64 input, including values
        // outside the i32 range (which silently truncate).
        #[test]
        fn alu_imm_imm_field_is_low_12_bits_of_i32_truncation(
            (rd_name, _rd) in reg_strategy(),
            (rs1_name, _rs1) in reg_strategy(),
            funct3 in 0u32..8,
            imm in any::<i64>(),
        ) {
            let ops = [Operand::Reg(rd_name), Operand::Reg(rs1_name), Operand::Imm(imm)];
            let w = match encode_alu_imm(&ops, funct3).expect("must encode") {
                EncodeResult::Word(w) => w,
                other => panic!("expected Word, got {:?}", other),
            };

            let expected_imm_field = ((imm as i32) as u32) & 0xFFF;
            prop_assert_eq!(
                (w >> 20) & 0xFFF,
                expected_imm_field,
                "imm[31:20] == low 12 bits of i32-truncated immediate"
            );
        }

        // Oracle: Reference — a symbol third operand must defer the value to
        // link time: emit a WordWithReloc whose word is the I-format encoder
        // with a ZERO immediate, whose reloc has a zero addend and the
        // modifier-stripped symbol, and whose reloc type is the *load-style*
        // (Lo12*) variant of whatever parse_reloc_modifier produced:
        //   %hi        -> Hi20        -> Lo12I
        //   %pcrel_hi  -> PcrelHi20   -> PcrelLo12I
        //   %tprel_hi  -> TprelHi20   -> TprelLo12I
        //   %lo / %pcrel_lo / %tprel_lo  -> already Lo12*, pass through unchanged
        //   plain symbol               -> PcrelHi20 -> PcrelLo12I
        #[test]
        fn alu_imm_symbol_maps_reloc_and_zeros_imm(
            (rd_name, rd_num) in reg_strategy(),
            (rs1_name, rs1_num) in reg_strategy(),
            funct3 in 0u32..8,
            sym_body in "[%a-zA-Z_][%a-zA-Z0-9_]*",
            case in prop::sample::select(vec![0u8, 1, 2, 3, 4, 5, 6]),
        ) {
            // (symbol_operand_string, expected reloc variant)
            // 0=%hi 1=%pcrel_hi 2=%tprel_hi 3=%lo 4=%pcrel_lo 5=%tprel_lo 6=plain
            let (sym_str, expected): (String, RelocType) = match case {
                0 => (format!("%hi({})", sym_body), RelocType::Lo12I),
                1 => (format!("%pcrel_hi({})", sym_body), RelocType::PcrelLo12I),
                2 => (format!("%tprel_hi({})", sym_body), RelocType::TprelLo12I),
                3 => (format!("%lo({})", sym_body), RelocType::Lo12I),
                4 => (format!("%pcrel_lo({})", sym_body), RelocType::PcrelLo12I),
                5 => (format!("%tprel_lo({})", sym_body), RelocType::TprelLo12I),
                _ => (sym_body.clone(), RelocType::PcrelLo12I),
            };

            let ops = [
                Operand::Reg(rd_name),
                Operand::Reg(rs1_name),
                Operand::Symbol(sym_str.clone()),
            ];
            let (word, reloc) = match encode_alu_imm(&ops, funct3).expect("symbol must encode") {
                EncodeResult::WordWithReloc { word, reloc } => (word, reloc),
                other => panic!("expected WordWithReloc, got {:?}", other),
            };

            // Word is the I-format encoder with a zeroed immediate.
            prop_assert_eq!(word, encode_i(OP_OP_IMM, rd_num, funct3, rs1_num, 0), "imm field zeroed");
            prop_assert_eq!((word >> 20) & 0xFFF, 0, "imm[31:20] must be zero");
            prop_assert_eq!(word & 0x7F, OP_OP_IMM_BITS, "opcode");

            // Reloc type matches the expected mapping.
            let types_match = matches!((&reloc.reloc_type, &expected),
                (RelocType::Lo12I, RelocType::Lo12I) |
                (RelocType::PcrelLo12I, RelocType::PcrelLo12I) |
                (RelocType::TprelLo12I, RelocType::TprelLo12I));
            prop_assert!(types_match,
                "reloc type {:?} != expected {:?} for symbol {}",
                reloc.reloc_type, expected, sym_str);

            // Addend is zero and symbol is the modifier-stripped body.
            prop_assert_eq!(reloc.addend, 0, "addend must be zero");
            prop_assert_eq!(reloc.symbol, sym_body, "symbol must be modifier-stripped");
        }

        // Oracle: Reference — the real RV64I ALU-immediate mnemonics
        // (addi/slti/sltiu/xori/ori/andi) flow through encode_alu_imm with
        // their canonical funct3 and decode back to opcode 0x13 with the
        // correct funct3 and a zero rd/rs1 baseline.
        #[test]
        fn alu_imm_real_mnemonics_decode_to_canonical_fields(
            rd_num in 0u32..32,
            rs1_num in 0u32..32,
            imm in any::<i64>(),
        ) {
            // (mnemonic, funct3)
            for (mnem, f3) in [
                ("addi",  0b000u32),
                ("slti",  0b010u32),
                ("sltiu", 0b011u32),
                ("xori",  0b100u32),
                ("ori",   0b110u32),
                ("andi",  0b111u32),
            ] {
                let ops = vec![
                    Operand::Reg(format!("x{}", rd_num)),
                    Operand::Reg(format!("x{}", rs1_num)),
                    Operand::Imm(imm),
                ];
                let w = match encode_alu_imm(&ops, f3).expect("must encode") {
                    EncodeResult::Word(w) => w,
                    other => panic!("expected Word for {}, got {:?}", mnem, other),
                };
                prop_assert_eq!(w & 0x7F, 0x13, "opcode for {}", mnem);
                prop_assert_eq!((w >> 12) & 0x7, f3, "funct3 for {}", mnem);
                prop_assert_eq!((w >> 7) & 0x1F, rd_num & 0x1F, "rd for {}", mnem);
                prop_assert_eq!((w >> 15) & 0x1F, rs1_num & 0x1F, "rs1 for {}", mnem);
                prop_assert_eq!((w >> 20) & 0xFFF, ((imm as i32) as u32) & 0xFFF, "imm for {}", mnem);
            }
        }

        // Oracle: Negative/error contract — encode_alu_imm requires exactly
        // (rd, rs1, imm|symbol). A missing or non-imm/non-symbol third
        // operand must yield the specific "alu_imm: expected immediate"
        // error; a missing/unparseable register must also be rejected.
        #[test]
        fn alu_imm_rejects_invalid_operands(
            bad_third in prop::sample::select(vec![
                Operand::Reg("a1".to_string()),
                Operand::Label("foo".to_string()),
                Operand::Mem { base: "sp".to_string(), offset: 0 },
                Operand::MemSymbol { base: "sp".to_string(), symbol: "s".to_string(), modifier: "%lo".to_string() },
                Operand::SymbolOffset("s".to_string(), 4),
                Operand::FenceArg("iorw".to_string()),
                Operand::Csr("cycle".to_string()),
                Operand::RoundingMode("rne".to_string()),
            ])
        ) {
            // Present but invalid third operand.
            let ops = vec![Operand::Reg("a0".to_string()), Operand::Reg("a1".to_string()), bad_third.clone()];
            let err = encode_alu_imm(&ops, 0b000).expect_err("bad 3rd operand must error");
            prop_assert!(err.contains("alu_imm: expected immediate"), "got: {}", err);

            // Missing third operand.
            let ops = vec![Operand::Reg("a0".to_string()), Operand::Reg("a1".to_string())];
            let err = encode_alu_imm(&ops, 0b000).expect_err("missing 3rd operand must error");
            prop_assert!(err.contains("alu_imm: expected immediate"), "got: {}", err);

            // Missing second operand (different message, still an error).
            prop_assert!(encode_alu_imm(&[Operand::Reg("a0".to_string())], 0b000).is_err());

            // Empty operands.
            prop_assert!(encode_alu_imm(&[], 0b000).is_err());

            // Invalid register name as first operand.
            let ops = vec![Operand::Reg("x32".to_string()), Operand::Reg("a1".to_string()), Operand::Imm(1)];
            let err = encode_alu_imm(&ops, 0b000).expect_err("invalid register must error");
            prop_assert!(err.contains("invalid integer register"), "got: {}", err);
        }
    }
}
