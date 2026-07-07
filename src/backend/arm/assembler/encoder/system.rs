use super::*;
use crate::backend::arm::assembler::parser::Operand;

// ── System instructions ──────────────────────────────────────────────────

pub(crate) fn encode_dmb(operands: &[Operand]) -> Result<EncodeResult, String> {
    let option = match operands.first() {
        Some(Operand::Barrier(b)) | Some(Operand::Symbol(b)) => match b.to_lowercase().as_str() {
            "sy" => 0b1111u32,
            "st" => 0b1110,
            "ld" => 0b1101,
            "ish" => 0b1011,
            "ishst" => 0b1010,
            "ishld" => 0b1001,
            "nsh" => 0b0111,
            "nshst" => 0b0110,
            "nshld" => 0b0101,
            "osh" => 0b0011,
            "oshst" => 0b0010,
            "oshld" => 0b0001,
            _ => return Err(format!("unknown dmb option: {}", b)),
        },
        _ => 0b1111,
    };
    // DMB: 0xD50330BF | (CRm << 8)
    let word = 0xd50330bf | (option << 8);
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_dsb(operands: &[Operand]) -> Result<EncodeResult, String> {
    let option = match operands.first() {
        Some(Operand::Barrier(b)) | Some(Operand::Symbol(b)) => match b.to_lowercase().as_str() {
            "sy" => 0b1111u32,
            "st" => 0b1110,
            "ld" => 0b1101,
            "ish" => 0b1011,
            "ishst" => 0b1010,
            "ishld" => 0b1001,
            "nsh" => 0b0111,
            "nshst" => 0b0110,
            "nshld" => 0b0101,
            "osh" => 0b0011,
            "oshst" => 0b0010,
            "oshld" => 0b0001,
            _ => return Err(format!("unknown dsb option: {}", b)),
        },
        _ => 0b1111,
    };
    // DSB: 0xD503309F | (option << 8)
    let word = 0xd503309f | (option << 8);
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_mrs(operands: &[Operand]) -> Result<EncodeResult, String> {
    // MRS Xt, system_reg
    let (rt, _) = get_reg(operands, 0)?;
    let sysreg = match operands.get(1) {
        Some(Operand::Symbol(s)) => s.to_lowercase(),
        _ => return Err("mrs needs system register name".to_string()),
    };

    let encoding = match sysreg.as_str() {
        "sp_el0" => 0xc208u32,
        "tpidr_el0" => 0xde82,
        "tpidr_el1" => 0xc684,
        "tpidr_el2" => 0xe682,
        "tpidrro_el0" => 0xde83,
        "tcr_el1" => 0xc102,
        "ttbr0_el1" => 0xc100,
        "sctlr_el1" => 0xc080,
        "mdscr_el1" => 0x8012,
        "id_aa64mmfr0_el1" => 0xc038,
        "id_aa64mmfr1_el1" => 0xc039,
        "cpacr_el1" => 0xc082,
        "par_el1" => 0xc3a0,
        "osdlr_el1" => 0x809c,
        "currentel" => 0xc212,
        "elr_el1" => 0xc201,
        "spsr_el1" => 0xc200,
        "esr_el1" => 0xc290,
        "far_el1" => 0xc300,
        "vbar_el1" => 0xc600,
        "mpidr_el1" => 0xc005,
        "contextidr_el1" => 0xc681,
        "mair_el1" => 0xc510,
        "isr_el1" => 0xc608,
        "oslsr_el1" => 0x808c,
        "midr_el1" => 0xc000,
        "revidr_el1" => 0xc006,
        "id_aa64pfr0_el1" => 0xc020,
        "id_aa64pfr1_el1" => 0xc021,
        "id_aa64isar0_el1" => 0xc030,
        "id_aa64isar1_el1" => 0xc031,
        "id_aa64isar2_el1" => 0xc032,
        "amair_el1" => 0xc518,
        "hcr_el2" => 0xe088,
        "cptr_el2" => 0xe08a,
        "hstr_el2" => 0xe08b,
        "hacr_el2" => 0xe08f,
        "vpidr_el2" => 0xe000,
        "vmpidr_el2" => 0xe005,
        "actlr_el2" => 0xe081,
        "elr_el2" => 0xe201,
        "esr_el2" => 0xe290,
        "afsr0_el2" => 0xe288,
        "afsr1_el2" => 0xe289,
        "far_el2" => 0xe300,
        "hpfar_el2" => 0xe304,
        "spsr_el2" => 0xe200,
        "sctlr_el2" => 0xe080,
        "mdcr_el2" => 0xe089,
        "tcr_el2" => 0xe102,
        "ttbr0_el2" => 0xe100,
        "vttbr_el2" => 0xe108,
        "vtcr_el2" => 0xe10a,
        "vbar_el2" => 0xe600,
        "mair_el2" => 0xe510,
        "amair_el2" => 0xe518,
        "sp_el1" => 0xe208,
        "pmuserenr_el0" => 0xdcf0,
        "cntfrq_el0" => 0xdf00,
        "cntpct_el0" => 0xdf01,
        "cntv_ctl_el0" => 0xdf19,
        "cntp_ctl_el0" => 0xdf11,
        "cntv_cval_el0" => 0xdf1c,
        "cntp_cval_el0" => 0xdf12,
        "ctr_el0" => 0xd801,
        "ttbr1_el1" => 0xc101,
        "cntkctl_el1" => 0xc708,
        "id_aa64dfr0_el1" => 0xc028,
        "oslar_el1" => 0x8084,
        "cntvct_el0" => 0xdf02,
        "clidr_el1" => 0xc801,
        "ccsidr_el1" => 0xc800,
        "csselr_el1" => 0xd000,
        "id_aa64mmfr2_el1" => 0xc03a,
        "id_aa64dfr1_el1" => 0xc029,
        "actlr_el1" => 0xc081,
        "afsr0_el1" => 0xc288,
        "afsr1_el1" => 0xc289,
        "id_pfr0_el1" => 0xc008,
        "id_pfr1_el1" => 0xc009,
        "cnthctl_el2" => 0xe708,
        "cntvoff_el2" => 0xe703,
        "sp_el2" => 0xf208,
        "pmintenset_el1" => 0xc4f1,
        "pmintenclr_el1" => 0xc4f2,
        "pmcr_el0" => 0xdce0,
        "pmcntenset_el0" => 0xdce1,
        "pmcntenclr_el0" => 0xdce2,
        "pmovsclr_el0" => 0xdce3,
        "pmselr_el0" => 0xdce5,
        "pmceid0_el0" => 0xdce6,
        "pmceid1_el0" => 0xdce7,
        "pmccntr_el0" => 0xdce8,
        "pmxevtyper_el0" => 0xdce9,
        "pmxevcntr_el0" => 0xdcea,
        "pmccfiltr_el0" => 0xdf7f,
        "dczid_el0" => 0xd807,
        "daif" => 0xda11,
        "fpcr" => 0xda20,
        "fpsr" => 0xda21,
        "nzcv" => 0xda10,
        "spsel" => 0xc210,
        "mdccint_el1" => 0x8010,
        "fpexc32_el2" => 0xe298,
        "dbgauthstatus_el1" => 0x83f6,
        "spsr_abt" => 0xe219,
        "spsr_und" => 0xe21a,
        "spsr_irq" => 0xe218,
        "spsr_fiq" => 0xe21b,
        "ifsr32_el2" => 0xe281,
        "dacr32_el2" => 0xe180,
        _ => parse_generic_sysreg(&sysreg)?,
    };

    // MRS encoding: 0xd520_0000 has L=1 (bit 21) for read.
    // Bits [20:19] = op0, supplied entirely by the sysreg encoding field.
    let word = 0xd5200000 | (encoding << 5) | rt;
    Ok(EncodeResult::Word(word))
}

/// Compute sysreg encoding from (op0, op1, CRn, CRm, op2) fields.
pub(crate) fn sysreg_encoding(op0: u32, op1: u32, crn: u32, crm: u32, op2: u32) -> u32 {
    ((op0 & 3) << 14) | ((op1 & 7) << 11) | ((crn & 0xF) << 7) | ((crm & 0xF) << 3) | (op2 & 7)
}

/// Try to parse a numbered debug/performance register family name like
/// `dbgbcr15_el1` or `dbgwvr0_el1` into its encoding. Returns None if not matched.
pub(crate) fn parse_numbered_sysreg(name: &str) -> Option<u32> {
    // Debug breakpoint/watchpoint registers: dbg{b,w}{c,v}r<n>_el1
    // dbgbcr<n>_el1: op0=2, op1=0, CRn=0, CRm=n, op2=5
    // dbgbvr<n>_el1: op0=2, op1=0, CRn=0, CRm=n, op2=4
    // dbgwcr<n>_el1: op0=2, op1=0, CRn=0, CRm=n, op2=7
    // dbgwvr<n>_el1: op0=2, op1=0, CRn=0, CRm=n, op2=6
    let prefixes: &[(&str, &str, u32)] = &[
        ("dbgbcr", "_el1", 5),
        ("dbgbvr", "_el1", 4),
        ("dbgwcr", "_el1", 7),
        ("dbgwvr", "_el1", 6),
    ];
    for &(prefix, suffix, op2) in prefixes {
        if let Some(rest) = name.strip_prefix(prefix) {
            if let Some(num_str) = rest.strip_suffix(suffix) {
                if let Ok(n) = num_str.parse::<u32>() {
                    if n <= 15 {
                        return Some(sysreg_encoding(2, 0, 0, n, op2));
                    }
                }
            }
        }
    }

    // Performance monitor event count registers: pmevcntr<n>_el0, pmevtyper<n>_el0
    // pmevcntr<n>_el0: op0=3, op1=3, CRn=14, CRm=8+n/8, op2=n%8
    // pmevtyper<n>_el0: op0=3, op1=3, CRn=14, CRm=12+n/8, op2=n%8
    if let Some(rest) = name.strip_prefix("pmevcntr") {
        if let Some(num_str) = rest.strip_suffix("_el0") {
            if let Ok(n) = num_str.parse::<u32>() {
                if n <= 30 {
                    return Some(sysreg_encoding(3, 3, 14, 8 + n / 8, n % 8));
                }
            }
        }
    }
    if let Some(rest) = name.strip_prefix("pmevtyper") {
        if let Some(num_str) = rest.strip_suffix("_el0") {
            if let Ok(n) = num_str.parse::<u32>() {
                if n <= 30 {
                    return Some(sysreg_encoding(3, 3, 14, 12 + n / 8, n % 8));
                }
            }
        }
    }

    None
}

/// Parse generic system register name like `s3_0_c1_c0_1` into encoding bits.
/// Also handles numbered register families like `dbgbcr15_el1`.
pub(crate) fn parse_generic_sysreg(name: &str) -> Result<u32, String> {
    // Try numbered register families first
    if let Some(enc) = parse_numbered_sysreg(name) {
        return Ok(enc);
    }

    // Format: s<op0>_<op1>_c<CRn>_c<CRm>_<op2>
    let parts: Vec<&str> = name.split('_').collect();
    if parts.len() == 5 && parts[0].starts_with('s') && parts[2].starts_with('c') && parts[3].starts_with('c') {
        let op0: u32 = parts[0][1..].parse().map_err(|_| format!("unsupported system register: {}", name))?;
        let op1: u32 = parts[1].parse().map_err(|_| format!("unsupported system register: {}", name))?;
        let crn: u32 = parts[2][1..].parse().map_err(|_| format!("unsupported system register: {}", name))?;
        let crm: u32 = parts[3][1..].parse().map_err(|_| format!("unsupported system register: {}", name))?;
        let op2: u32 = parts[4].parse().map_err(|_| format!("unsupported system register: {}", name))?;
        let enc = sysreg_encoding(op0, op1, crn, crm, op2);
        Ok(enc)
    } else {
        Err(format!("unsupported system register: {}", name))
    }
}

pub(crate) fn encode_msr(operands: &[Operand]) -> Result<EncodeResult, String> {
    let sysreg = match operands.first() {
        Some(Operand::Symbol(s)) => s.to_lowercase(),
        _ => return Err("msr needs system register name".to_string()),
    };

    // MSR (immediate): msr <pstatefield>, #imm
    // Encoding: 1101_0101_0000_0 op1[18:16] 0100 CRm[11:8] op2[7:5] 11111[4:0]
    // daifset: op1=3, op2=6; daifclr: op1=3, op2=7; spsel: op1=0, op2=5
    match sysreg.as_str() {
        "daifset" => {
            let imm = get_imm(operands, 1)? as u32 & 0xF;
            let word = 0xd5034000 | (imm << 8) | (0b110 << 5) | 0x1F;
            return Ok(EncodeResult::Word(word));
        }
        "daifclr" => {
            let imm = get_imm(operands, 1)? as u32 & 0xF;
            let word = 0xd5034000 | (imm << 8) | (0b111 << 5) | 0x1F;
            return Ok(EncodeResult::Word(word));
        }
        "spsel" => {
            // SPSel: op1=0, op2=5 (MSR immediate form)
            // If the second operand is a register, fall through to MSR register form
            if let Ok(imm) = get_imm(operands, 1) {
                let imm = imm as u32 & 0xF;
                let word = 0xd5004000 | (imm << 8) | (0b101 << 5) | 0x1F;
                return Ok(EncodeResult::Word(word));
            }
        }
        _ => {}
    }

    // MSR (register): msr sysreg, Xt
    let (rt, _) = get_reg(operands, 1)?;

    let encoding = match sysreg.as_str() {
        "sp_el0" => 0xc208u32,
        "tpidr_el0" => 0xde82,
        "tpidr_el1" => 0xc684,
        "tpidr_el2" => 0xe682,
        "tpidrro_el0" => 0xde83,
        "tcr_el1" => 0xc102,
        "ttbr0_el1" => 0xc100,
        "sctlr_el1" => 0xc080,
        "mdscr_el1" => 0x8012,
        "cpacr_el1" => 0xc082,
        "par_el1" => 0xc3a0,
        "osdlr_el1" => 0x809c,
        "oslar_el1" => 0x8084,
        "oslsr_el1" => 0x808c,
        "elr_el1" => 0xc201,
        "spsr_el1" => 0xc200,
        "esr_el1" => 0xc290,
        "far_el1" => 0xc300,
        "vbar_el1" => 0xc600,
        "contextidr_el1" => 0xc681,
        "mair_el1" => 0xc510,
        "amair_el1" => 0xc518,
        "hcr_el2" => 0xe088,
        "cptr_el2" => 0xe08a,
        "hstr_el2" => 0xe08b,
        "elr_el2" => 0xe201,
        "esr_el2" => 0xe290,
        "far_el2" => 0xe300,
        "spsr_el2" => 0xe200,
        "sctlr_el2" => 0xe080,
        "mdcr_el2" => 0xe089,
        "tcr_el2" => 0xe102,
        "ttbr0_el2" => 0xe100,
        "vttbr_el2" => 0xe108,
        "vtcr_el2" => 0xe10a,
        "vbar_el2" => 0xe600,
        "mair_el2" => 0xe510,
        "sp_el1" => 0xe208,
        "csselr_el1" => 0xd000,
        "actlr_el1" => 0xc081,
        "cnthctl_el2" => 0xe708,
        "cntvoff_el2" => 0xe703,
        "sp_el2" => 0xf208,
        "vpidr_el2" => 0xe000,
        "vmpidr_el2" => 0xe005,
        "hacr_el2" => 0xe08f,
        "actlr_el2" => 0xe081,
        "afsr0_el2" => 0xe288,
        "afsr1_el2" => 0xe289,
        "amair_el2" => 0xe518,
        "hpfar_el2" => 0xe304,
        "pmintenset_el1" => 0xc4f1,
        "pmintenclr_el1" => 0xc4f2,
        "pmcr_el0" => 0xdce0,
        "pmcntenset_el0" => 0xdce1,
        "pmcntenclr_el0" => 0xdce2,
        "pmovsclr_el0" => 0xdce3,
        "pmselr_el0" => 0xdce5,
        "pmccntr_el0" => 0xdce8,
        "pmxevtyper_el0" => 0xdce9,
        "pmxevcntr_el0" => 0xdcea,
        "pmuserenr_el0" => 0xdcf0,
        "pmccfiltr_el0" => 0xdf7f,
        "cntv_ctl_el0" => 0xdf19,
        "cntp_ctl_el0" => 0xdf11,
        "cntp_cval_el0" => 0xdf12,
        "cntv_cval_el0" => 0xdf1c,
        "ttbr1_el1" => 0xc101,
        "cntkctl_el1" => 0xc708,
        "daif" => 0xda11,
        "fpcr" => 0xda20,
        "fpsr" => 0xda21,
        "nzcv" => 0xda10,
        "spsel" => 0xc210,
        "mdccint_el1" => 0x8010,
        "fpexc32_el2" => 0xe298,
        "spsr_abt" => 0xe219,
        "spsr_und" => 0xe21a,
        "spsr_irq" => 0xe218,
        "spsr_fiq" => 0xe21b,
        "ifsr32_el2" => 0xe281,
        "dacr32_el2" => 0xe180,
        _ => parse_generic_sysreg(&sysreg)?,
    };

    // MSR encoding: 0xd500_0000 has L=0 (bit 21) for write.
    // Bits [20:19] = op0, supplied entirely by the sysreg encoding field.
    let word = 0xd5000000 | (encoding << 5) | rt;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_svc(operands: &[Operand]) -> Result<EncodeResult, String> {
    let imm = get_imm(operands, 0)?;
    let word = 0xd4000001 | ((imm as u32 & 0xFFFF) << 5);
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_hvc(operands: &[Operand]) -> Result<EncodeResult, String> {
    let imm = get_imm(operands, 0)?;
    let word = 0xd4000002 | ((imm as u32 & 0xFFFF) << 5);
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_ic(raw_operands: &str) -> Result<EncodeResult, String> {
    let parts: Vec<&str> = raw_operands.splitn(2, ',').collect();
    let op_name = parts[0].trim().to_lowercase();
    let rt = if parts.len() > 1 {
        let reg_str = parts[1].trim();
        parse_reg_num(reg_str).ok_or_else(|| format!("ic: invalid register '{}'", reg_str))?
    } else {
        31 // xzr
    };
    let base = match op_name.as_str() {
        "ialluis" => 0xd508711fu32,
        "iallu"   => 0xd508751f,
        "ivau"    => 0xd50b7520,
        _ => return Err(format!("unsupported ic operation: {}", op_name)),
    };
    let word = (base & !0x1F) | rt;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_smc(operands: &[Operand]) -> Result<EncodeResult, String> {
    let imm = get_imm(operands, 0)?;
    let word = 0xd4000003 | ((imm as u32 & 0xFFFF) << 5);
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_at(_operands: &[Operand], raw_operands: &str) -> Result<EncodeResult, String> {
    let parts: Vec<&str> = raw_operands.splitn(2, ',').collect();
    let op_name = parts[0].trim().to_lowercase();
    let rt = if parts.len() > 1 {
        let reg_str = parts[1].trim();
        parse_reg_num(reg_str).ok_or_else(|| format!("at: invalid register '{}'", reg_str))?
    } else {
        31
    };
    // AT encoding: SYS instruction. Base words from GCC:
    let base = match op_name.as_str() {
        "s1e1r" => 0xd5087800u32,
        "s1e1w" => 0xd5087820,
        "s1e0r" => 0xd5087840,
        "s1e0w" => 0xd5087860,
        _ => return Err(format!("unsupported at operation: {}", op_name)),
    };
    let word = (base & !0x1F) | rt;
    Ok(EncodeResult::Word(word))
}

/// Encode `sys #op1, Cn, Cm, #op2, Xt` instruction.
pub(crate) fn encode_sys(raw_operands: &str) -> Result<EncodeResult, String> {
    let parts: Vec<&str> = raw_operands.split(',').map(|s| s.trim()).collect();
    if parts.len() < 4 {
        return Err(format!("sys needs at least 4 operands, got: {}", raw_operands));
    }
    let op1: u32 = parts[0].trim_start_matches('#').trim().parse()
        .map_err(|_| format!("sys: invalid op1: {}", parts[0]))?;
    let crn: u32 = parts[1].trim().to_lowercase().trim_start_matches('c').parse()
        .map_err(|_| format!("sys: invalid CRn: {}", parts[1]))?;
    let crm: u32 = parts[2].trim().to_lowercase().trim_start_matches('c').parse()
        .map_err(|_| format!("sys: invalid CRm: {}", parts[2]))?;
    let op2: u32 = parts[3].trim_start_matches('#').trim().parse()
        .map_err(|_| format!("sys: invalid op2: {}", parts[3]))?;
    let rt = if parts.len() >= 5 {
        let reg = parts[4].trim().to_lowercase();
        parse_reg_num(&reg).ok_or_else(|| format!("sys: invalid register: {}", parts[4]))?
    } else {
        31 // xzr if no register specified
    };
    let word = 0xd5080000 | ((op1 & 7) << 16) | ((crn & 0xF) << 12) | ((crm & 0xF) << 8) | ((op2 & 7) << 5) | rt;
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_brk(operands: &[Operand]) -> Result<EncodeResult, String> {
    let imm = get_imm(operands, 0)?;
    let word = 0xd4200000 | ((imm as u32 & 0xFFFF) << 5);
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_tlbi(_operands: &[Operand], raw_operands: &str) -> Result<EncodeResult, String> {
    let parts: Vec<&str> = raw_operands.splitn(2, ',').collect();
    let op_name = parts[0].trim().to_lowercase();
    let rt = if parts.len() > 1 {
        let reg_str = parts[1].trim();
        parse_reg_num(reg_str).ok_or_else(|| format!("tlbi: invalid register '{}'", reg_str))?
    } else {
        31 // xzr
    };
    // TLBI encoding: SYS instruction with fixed fields
    // Full word from GCC objdump for known ops (with Rt=x0):
    let base = match op_name.as_str() {
        // Standard ARMv8.0 TLBI operations
        "vmalle1is" => 0xd508831fu32,
        "vmalle1"   => 0xd508871f,
        "alle1is"   => 0xd50c839f,
        "alle1"     => 0xd50c879f,
        "alle2is"   => 0xd50c831f,
        "vale1is"   => 0xd50883a0,
        "vale1"     => 0xd50887a0,
        "vale2is"   => 0xd50c83a0,
        "vale2"     => 0xd50c87a0,
        "vaae1is"   => 0xd5088360,
        "vaae1"     => 0xd5088760,
        "vaale1is"  => 0xd50883e0,
        "vaale1"    => 0xd50887e0,
        "vae1is"    => 0xd5088320,
        "vae1"      => 0xd5088720,
        "vae2is"    => 0xd50c8320,
        "vae2"      => 0xd50c8720,
        "aside1is"  => 0xd5088340,
        "aside1"    => 0xd5088740,
        "vmalls12e1is" => 0xd50c83df,
        "vmalls12e1"   => 0xd50c87df,
        "ipas2e1is"    => 0xd50c8020,
        "ipas2e1"      => 0xd50c8420,
        "ipas2le1is"   => 0xd50c80a0,
        "ipas2le1"     => 0xd50c84a0,
        // FEAT_TLBIRANGE: range TLBI operations (ARMv8.4-A)
        "rvae1is"      => 0xd5088220,
        "rvale1is"     => 0xd50882a0,
        "rvaae1is"     => 0xd5088260,
        "rvaale1is"    => 0xd50882e0,
        "rvae1"        => 0xd5088620,
        "rvale1"       => 0xd50886a0,
        "rvaae1"       => 0xd5088660,
        "rvaale1"      => 0xd50886e0,
        "rvae1os"      => 0xd5088520,
        "rvale1os"     => 0xd50885a0,
        "rvaae1os"     => 0xd5088560,
        "rvaale1os"    => 0xd50885e0,
        "ripas2e1is"   => 0xd50c8040,
        "ripas2e1"     => 0xd50c8440,
        "ripas2e1os"   => 0xd50c8460,
        "ripas2le1is"  => 0xd50c80c0,
        "ripas2le1"    => 0xd50c84c0,
        "ripas2le1os"  => 0xd50c84e0,
        _ => return Err(format!("unsupported tlbi operation: {}", op_name)),
    };
    // Replace Rt field (bits 4:0)
    let word = (base & !0x1F) | rt;
    Ok(EncodeResult::Word(word))
}

/// Encode HINT #imm (system hint instruction)
pub(crate) fn encode_bti(raw_operands: &str) -> Result<EncodeResult, String> {
    let target = raw_operands.trim().to_lowercase();
    let word = match target.as_str() {
        "" => 0xd503241f,    // bti (no target)
        "c" => 0xd503245f,   // bti c
        "j" => 0xd503249f,   // bti j
        "jc" => 0xd50324df,  // bti jc
        _ => return Err(format!("unsupported bti target: {}", target)),
    };
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_hint(operands: &[Operand]) -> Result<EncodeResult, String> {
    let imm = get_imm(operands, 0)?;
    // HINT: 11010101 00000011 0010 CRm op2 11111
    // CRm = imm >> 3, op2 = imm & 7
    let crm = ((imm as u32) >> 3) & 0xF;
    let op2 = (imm as u32) & 0x7;
    let word = 0xd503201f | (crm << 8) | (op2 << 5);
    Ok(EncodeResult::Word(word))
}

pub(crate) fn encode_dc(operands: &[Operand], raw_operands: &str) -> Result<EncodeResult, String> {
    // Check for the operation type in the operands or raw string
    let op = match operands.first() {
        Some(Operand::Symbol(s)) => s.to_lowercase(),
        _ => raw_operands.to_lowercase(),
    };

    // Find the register operand (second operand or last operand)
    let rt = match operands.get(1) {
        Some(Operand::Reg(name)) => parse_reg_num(name).ok_or("invalid register for dc")?,
        _ => {
            if let Some(Operand::Reg(name)) = operands.last() {
                parse_reg_num(name).ok_or("invalid register for dc")?
            } else {
                0
            }
        }
    };

    if op.contains("civac") {
        // DC CIVAC: sys #3, c7, c14, #1, Xt
        let word = 0xd50b7e20 | rt;
        return Ok(EncodeResult::Word(word));
    }
    if op.contains("cvac") {
        // DC CVAC: sys #3, c7, c10, #1, Xt
        let word = 0xd50b7a20 | rt;
        return Ok(EncodeResult::Word(word));
    }
    if op.contains("cvap") {
        // DC CVAP: sys #3, c7, c12, #1, Xt
        let word = 0xd50b7c20 | rt;
        return Ok(EncodeResult::Word(word));
    }
    if op.contains("cvau") {
        let word = 0xd50b7b20 | rt;
        return Ok(EncodeResult::Word(word));
    }
    if op.contains("ivac") {
        let word = 0xd5087620 | rt;
        return Ok(EncodeResult::Word(word));
    }
    if op.contains("zva") {
        // DC ZVA: sys #3, c7, c4, #1, Xt
        let word = 0xd50b7420 | rt;
        return Ok(EncodeResult::Word(word));
    }

    Err(format!("unsupported dc variant: {}", raw_operands))
}

// ── Property-based tests for encode_svc ───────────────────────────────────
// Oracle: the AArch64 SVC encoding is
//     SVC #imm16  ->  1101 0100 000 | imm16[20:5] | 00001
//                  = 0xD4_0000_01 | (imm16 << 5)
// i.e. bits[31:21] fixed (opcode), bits[4:0] fixed = 0b00001 (SVC's LL field),
// and bits[20:5] carry the 16-bit immediate. The input immediate is masked to
// 16 bits before placement, so values outside [0, 0xFFFF] are folded by their
// low 16 bits (two's-complement for negatives).
#[cfg(test)]
mod proptest_svc {
    use super::encode_svc;
    use crate::backend::arm::assembler::encoder::EncodeResult;
    use crate::backend::arm::assembler::parser::Operand;
    use proptest::prelude::*;

    /// Helper: encode a single `#imm` SVC operand and unwrap the resulting word.
    fn encode_word(imm: i64) -> u32 {
        match encode_svc(&[Operand::Imm(imm)]).expect("Imm operand must encode") {
            EncodeResult::Word(w) => w,
            other => panic!("expected EncodeResult::Word, got {:?}", other),
        }
    }

    proptest! {
        // 1. The opcode field bits[31:21] and the SVC LL field bits[4:0]=00001
        //    are constant for every input immediate.
        #[test]
        fn svc_opcode_and_ll_fields_invariant(imm in -1_000_000i64..1_000_000) {
            let word = encode_word(imm);
            // Mask bits[31:21] (0xFFE0_0000) and bits[4:0] (0x1F).
            prop_assert_eq!(word & 0xFFE0_001F, 0xD400_0001u32);
        }

        // 2. The 16-bit immediate round-trips out of bits[20:5].
        #[test]
        fn svc_imm16_roundtrips(imm in 0u32..=0xFFFF) {
            let word = encode_word(imm as i64);
            prop_assert_eq!((word >> 5) & 0xFFFF, imm);
        }

        // 3. Exact-word oracle over the full i64 range: the low 16 bits of the
        //    (two's-complement) immediate are placed at bits[20:5] and nothing
        //    else is altered. Holds for negatives, i64::MIN/MAX, etc.
        #[test]
        fn svc_low16_bits_placed_regardless_of_sign(imm in any::<i64>()) {
            let word = encode_word(imm);
            let low16 = (imm & 0xFFFF) as u32; // i64 & mask is in [0, 0xFFFF]
            let expected = 0xD400_0001u32 | (low16 << 5);
            prop_assert_eq!(word, expected);
        }

        // 4. Injectivity within the 16-bit immediate range: distinct imm16
        //    values yield distinct encoded words.
        #[test]
        fn svc_distinct_imm16_give_distinct_words(a in 0u32..=0xFFFF, b in 0u32..=0xFFFF) {
            prop_assume!(a != b);
            let wa = encode_word(a as i64);
            let wb = encode_word(b as i64);
            prop_assert_ne!(wa, wb);
        }

        // 5. Error contract: a missing operand, or any non-Imm first operand,
        //    is rejected. The function only accepts Operand::Imm at index 0.
        #[test]
        fn svc_rejects_non_imm_first_operand(kind in 0u8..3) {
            let operands: Vec<Operand> = match kind {
                0 => vec![],
                1 => vec![Operand::Reg("x0".to_string())],
                _ => vec![Operand::Symbol("foo".to_string())],
            };
            prop_assert!(
                encode_svc(&operands).is_err(),
                "expected Err for operands: {:?}",
                operands
            );
        }
    }

    // Deterministic companion: an Imm operand always succeeds (covers MIN/MAX).
    #[test]
    fn svc_imm_always_succeeds() {
        for imm in [i64::MIN, -1i64, 0, 1, 0xFFFF, 0x1_0000, i64::MAX] {
            assert!(
                matches!(encode_svc(&[Operand::Imm(imm)]), Ok(EncodeResult::Word(_))),
                "imm {} should encode to Word",
                imm
            );
        }
    }
}

// ── Property-based tests for encode_msr ───────────────────────────────────
// encode_msr has two instruction shapes:
//
//   (A) MSR (immediate) PState fields:
//         msr daifset, #imm -> 0xD503_4000 | ((imm & 0xF) << 8) | (0b110 << 5) | 0x1F
//         msr daifclr,  #imm -> 0xD503_4000 | ((imm & 0xF) << 8) | (0b111 << 5) | 0x1F
//         msr spsel,    #imm -> 0xD500_4000 | ((imm & 0xF) << 8) | (0b101 << 5) | 0x1F
//       The 4-bit immediate occupies CRm (bits[11:8]); op2 sits at bits[7:5];
//       Rt field (bits[4:0]) is hard-wired to 0b11111.
//
//   (B) MSR (register):
//         msr <sysreg>, Xt  -> 0xD500_0000 | (sysenc << 5) | Rt
//       where <sysenc> is either a table entry (e.g. sctlr_el1 -> 0xC080) or is
//       derived generically from s<op0>_<op1>_c<CRn>_c<CRm>_<op2> via
//       sysreg_encoding(). Bit 21 (the L/read bit) must be 0 for a write.
//
// Oracle: exact-word reconstruction for every branch (reference oracle),
// plus a differential check against sysreg_encoding() for generic names.
#[cfg(test)]
mod proptest_msr {
    use super::encode_msr;
    use super::sysreg_encoding;
    use crate::backend::arm::assembler::encoder::EncodeResult;
    use crate::backend::arm::assembler::encoder::parse_reg_num;
    use crate::backend::arm::assembler::parser::Operand;
    use proptest::prelude::*;

    /// Helper: encode an operand slice for MSR and unwrap the resulting word.
    fn encode_word(operands: &[Operand]) -> u32 {
        match encode_msr(operands).expect("MSR operands must encode") {
            EncodeResult::Word(w) => w,
            other => panic!("expected EncodeResult::Word, got {:?}", other),
        }
    }

    proptest! {
        // 1. Error contract: the first operand MUST be an Operand::Symbol naming
        //    the system register. Anything else is rejected with Err.
        #[test]
        fn msr_rejects_non_symbol_first_operand(kind in 0u8..4) {
            let operands: Vec<Operand> = match kind {
                0 => vec![],
                1 => vec![Operand::Reg("x0".to_string())],
                2 => vec![Operand::Imm(7)],
                _ => vec![Operand::SymbolOffset("foo".to_string(), 4)],
            };
            prop_assert!(
                encode_msr(&operands).is_err(),
                "expected Err for first operand kind {}: {:?}",
                kind, operands
            );
        }

        // 2. daifset / daifclr immediate: exact-word oracle for any i64. The
        //    immediate is masked to its low 4 bits before placement in CRm.
        #[test]
        fn msr_daif_immediate_word(imm in any::<i64>(), field in 0u8..2) {
            let (name, op2): (&str, u32) = if field == 0 {
                ("daifset", 0b110)
            } else {
                ("daifclr", 0b111)
            };
            let word = encode_word(&[
                Operand::Symbol(name.to_string()),
                Operand::Imm(imm),
            ]);
            let crm = ((imm as u32) & 0xF) << 8;
            let expected = 0xd503_4000u32 | crm | (op2 << 5) | 0x1F;
            prop_assert_eq!(word, expected);
            // Rt field is hard-wired to 0b11111 for the immediate form.
            prop_assert_eq!(word & 0x1F, 0x1F);
        }

        // 3. spsel immediate form: distinct base 0xD500_4000 (op1=0) and
        //    op2=0b101. Same CRm masking as daifset/daifclr.
        #[test]
        fn msr_spsel_immediate_word(imm in any::<i64>()) {
            let word = encode_word(&[
                Operand::Symbol("spsel".to_string()),
                Operand::Imm(imm),
            ]);
            let crm = ((imm as u32) & 0xF) << 8;
            let expected = 0xd500_4000u32 | crm | (0b101u32 << 5) | 0x1F;
            prop_assert_eq!(word, expected);
        }

        // 4. Register form: Rt round-trips into bits[4:0] for every valid
        //    general-purpose register, and the L/read bit (bit 21) is always 0
        //    (this is a write). Uses a known table sysreg (sctlr_el1).
        #[test]
        fn msr_register_rt_roundtrips(reg_num in 0u32..=31u32) {
            let reg = format!("x{}", reg_num);
            let word = encode_word(&[
                Operand::Symbol("sctlr_el1".to_string()),
                Operand::Reg(reg.clone()),
            ]);
            let expected_rt = parse_reg_num(&reg).expect("valid gp register");
            prop_assert_eq!(word & 0x1F, expected_rt);
            // Bit 21 = 0 -> MSR write (L=0). MRS would set this bit.
            prop_assert_eq!(word & (1u32 << 21), 0);
            // High opcode bits [31:22] fixed for MSR register form.
            prop_assert_eq!(word >> 22, 0x354);
        }

        // 5. Register form: injectivity of the Rt field. Distinct registers
        //    with a fixed sysreg yield distinct encoded words.
        #[test]
        fn msr_distinct_registers_distinct_words(a in 0u32..=31u32, b in 0u32..=31u32) {
            prop_assume!(a != b);
            let wa = encode_word(&[
                Operand::Symbol("sctlr_el1".to_string()),
                Operand::Reg(format!("x{}", a)),
            ]);
            let wb = encode_word(&[
                Operand::Symbol("sctlr_el1".to_string()),
                Operand::Reg(format!("x{}", b)),
            ]);
            prop_assert_ne!(wa, wb);
        }

        // 6. Generic sysreg differential: for any in-range (op0,op1,CRn,CRm,op2),
        //    the name s<op0>_<op1>_c<CRn>_c<CRm>_<op2> is parsed and the encoded
        //    word matches the independently-computed sysreg_encoding() value.
        #[test]
        fn msr_generic_sysreg_matches_sysreg_encoding(
            op0 in 0u32..=3u32,
            op1 in 0u32..=7u32,
            crn in 0u32..=15u32,
            crm in 0u32..=15u32,
            op2 in 0u32..=7u32,
        ) {
            let name = format!("s{}_{}_c{}_c{}_{}", op0, op1, crn, crm, op2);
            let rt = 5u32;
            let word = encode_word(&[
                Operand::Symbol(name.clone()),
                Operand::Reg("x5".to_string()),
            ]);
            let sysenc = sysreg_encoding(op0, op1, crn, crm, op2);
            let expected = 0xd500_0000u32 | (sysenc << 5) | rt;
            prop_assert_eq!(word, expected);
            // The sysreg encoding lives in bits[20:5]; Rt is untouched.
            prop_assert_eq!((word >> 5) & 0xFFFF, sysenc);
        }
    }

    // Deterministic companions covering edge cases proptest may not hit.
    #[test]
    fn msr_immediate_extremes_and_register_spsel() {
        // Immediate masking at i64 extremes (low 4 bits only).
        for imm in [i64::MIN, -1i64, 0, 0xF, 0x10, 0xFF, i64::MAX] {
            let w = encode_word(&[
                Operand::Symbol("daifset".to_string()),
                Operand::Imm(imm),
            ]);
            assert_eq!(w & 0x1F00, (((imm as u32) & 0xF) << 8));
            assert_eq!(w & 0x1F, 0x1F);
        }

        // spsel with a *register* operand falls through to the register form
        // (sysreg encoding 0xC210), NOT the immediate form.
        let w = encode_word(&[
            Operand::Symbol("spsel".to_string()),
            Operand::Reg("x5".to_string()),
        ]);
        assert_eq!(w, 0xd500_0000u32 | (0xc210u32 << 5) | 5);

        // Case-insensitivity: SCTLR_EL1 == sctlr_el1.
        let upper = encode_word(&[
            Operand::Symbol("SCTLR_EL1".to_string()),
            Operand::Reg("x9".to_string()),
        ]);
        let lower = encode_word(&[
            Operand::Symbol("sctlr_el1".to_string()),
            Operand::Reg("x9".to_string()),
        ]);
        assert_eq!(upper, lower);
    }
}

// ── Property-based tests for sysreg_encoding ─────────────────────────────
// sysreg_encoding packs the five AArch64 system-register addressing fields
// into the 16-bit "system register encoding" used by MRS/MSR:
//
//     op0[15:14] | op1[13:11] | CRn[10:7] | CRm[6:3] | op2[2:0]
//
// Each field is masked to its declared width (op0&3, op1&7, CRn&0xF,
// CRm&0xF, op2&7) before being shifted into place, so the fields occupy
// pairwise-disjoint bit ranges and together span exactly bits[15:0].
//
// Oracle: field-extraction round-trip. Rather than reproducing the function
// body, we extract each field back out of the result with its declared
// (mask, shift) and assert it equals the masked input. This independently
// pins both the masking and the placement of every field.
#[cfg(test)]
mod proptest_sysreg {
    use super::sysreg_encoding;
    use proptest::prelude::*;

    proptest! {
        // 1. Bounding: the output of an AArch64 sysreg encoding is always a
        //    16-bit value (bits[15:0]), regardless of how large the inputs are.
        #[test]
        fn sysreg_output_fits_in_16_bits(
            op0 in any::<u32>(),
            op1 in any::<u32>(),
            crn in any::<u32>(),
            crm in any::<u32>(),
            op2 in any::<u32>(),
        ) {
            let enc = sysreg_encoding(op0, op1, crn, crm, op2);
            prop_assert!(enc <= 0xFFFF, "encoding must fit in 16 bits, got {:#x}", enc);
        }

        // 2. Masking invariance: the high bits of each input are ignored.
        //    Encoding the raw inputs equals encoding each input masked down to
        //    its legal width.
        #[test]
        fn sysreg_ignores_high_bits_of_inputs(
            op0 in any::<u32>(),
            op1 in any::<u32>(),
            crn in any::<u32>(),
            crm in any::<u32>(),
            op2 in any::<u32>(),
        ) {
            let raw = sysreg_encoding(op0, op1, crn, crm, op2);
            let masked = sysreg_encoding(op0 & 3, op1 & 7, crn & 0xF, crm & 0xF, op2 & 7);
            prop_assert_eq!(raw, masked);
        }

        // 3. Field-extraction oracle: every field round-trips back out of the
        //    result at its declared (shift, mask). This is the inverse of the
        //    packing and pins both placement and width for all five fields.
        #[test]
        fn sysreg_each_field_roundtrips_at_expected_position(
            op0 in any::<u32>(),
            op1 in any::<u32>(),
            crn in any::<u32>(),
            crm in any::<u32>(),
            op2 in any::<u32>(),
        ) {
            let enc = sysreg_encoding(op0, op1, crn, crm, op2);
            prop_assert_eq!((enc >> 14) & 0x3, op0 & 0x3, "op0 field at [15:14]");
            prop_assert_eq!((enc >> 11) & 0x7, op1 & 0x7, "op1 field at [13:11]");
            prop_assert_eq!((enc >> 7)  & 0xF, crn & 0xF, "CRn field at [10:7]");
            prop_assert_eq!((enc >> 3)  & 0xF, crm & 0xF, "CRm field at [6:3]");
            prop_assert_eq!(enc         & 0x7, op2 & 0x7, "op2 field at [2:0]");
        }

        // 4. Injectivity over legal ranges: distinct (op0,op1,CRn,CRm,op2)
        //    tuples — each already within its field width — yield distinct
        //    encodings. Holds precisely because the fields occupy disjoint
        //    bits; a collision would reveal an overlap bug.
        #[test]
        fn sysreg_injective_over_valid_ranges(
            a0 in 0u32..=3, a1 in 0u32..=7, acn in 0u32..=15, acm in 0u32..=15, a2 in 0u32..=7,
            b0 in 0u32..=3, b1 in 0u32..=7, bcn in 0u32..=15, bcm in 0u32..=15, b2 in 0u32..=7,
        ) {
            let a = (a0, a1, acn, acm, a2);
            let b = (b0, b1, bcn, bcm, b2);
            prop_assume!(a != b);
            let wa = sysreg_encoding(a0, a1, acn, acm, a2);
            let wb = sysreg_encoding(b0, b1, bcn, bcm, b2);
            prop_assert_ne!(wa, wb);
        }

        // 5. Bit-field isolation: mutating exactly one field changes only that
        //    field's bits in the output and leaves every other bit untouched,
        //    and the mutated bits equal the new value masked. Parametrised over
        //    which of the five fields is mutated.
        #[test]
        fn sysreg_changing_one_field_isolates_to_its_bits(
            op0 in any::<u32>(),
            op1 in any::<u32>(),
            crn in any::<u32>(),
            crm in any::<u32>(),
            op2 in any::<u32>(),
            delta in any::<u32>(),
            field in 0u8..5,
        ) {
            let base = sysreg_encoding(op0, op1, crn, crm, op2);
            let (n0, n1, ncn, ncm, n2) = match field {
                0 => (delta, op1, crn, crm, op2),
                1 => (op0, delta, crn, crm, op2),
                2 => (op0, op1, delta, crm, op2),
                3 => (op0, op1, crn, delta, op2),
                _ => (op0, op1, crn, crm, delta),
            };
            let changed = sysreg_encoding(n0, n1, ncn, ncm, n2);
            let (mask, shift): (u32, u32) = match field {
                0 => (0x3, 14),
                1 => (0x7, 11),
                2 => (0xF, 7),
                3 => (0xF, 3),
                _ => (0x7, 0),
            };
            // Bits outside the mutated field must be identical.
            prop_assert_eq!(changed & !(mask << shift), base & !(mask << shift));
            // The mutated field's bits equal the new (masked) value.
            prop_assert_eq!((changed >> shift) & mask, delta & mask);
        }
    }

    // Deterministic companion: the canonical reference encoding for SCTLR_EL1
    // is s3_0_c1_c0_0 -> sysreg_encoding(3, 0, 1, 0, 0). The ARM ARM value is
    // 0xC080 (the table entry used by encode_mrs/encode_msr for sctlr_el1):
    // bits[15:14]=op0=3, bits[10:7]=CRn=1.
    #[test]
    fn sysreg_known_canonical_values() {
        assert_eq!(sysreg_encoding(3, 0, 1, 0, 0), 0xC080, "SCTLR_EL1");
        assert_eq!(sysreg_encoding(2, 0, 0, 0, 0), 0x8000, "lowest op0 bit");
        assert_eq!(sysreg_encoding(3, 7, 0xF, 0xF, 7), 0xFFFF, "all-ones saturates to 16 bits");
        assert_eq!(sysreg_encoding(0, 0, 0, 0, 0), 0, "all-zero inputs");
    }
}

// ── Property-based tests for encode_sys ──────────────────────────────────
// encode_sys builds the AArch64 `SYS #op1, Cn, Cm, #op2 [, Xt]` instruction
// from a raw comma-separated operand string:
//
//     1101 0101 0000 1 op1[18:16] CRn[15:12] CRm[11:8] op2[7:5] Rt[4:0]
//       = 0xD508_0000 | (op1 & 7)<<16 | (CRn & 0xF)<<12 | (CRm & 0xF)<<8
//                     | (op2 & 7)<<5 | Rt
//
// Each numeric field is masked to its declared width before placement, so the
// five variable fields occupy pairwise-disjoint bit ranges within bits[18:0];
// bits[31:19] are the fixed SYS opcode. If the optional register is omitted,
// Rt defaults to 31 (xzr).
//
// Oracle: field-extraction round-trip — pull each field back out of the result
// at its declared (shift, mask) and assert equality with the masked input.
#[cfg(test)]
mod proptest_sys {
    use super::encode_sys;
    use crate::backend::arm::assembler::encoder::EncodeResult;
    use proptest::prelude::*;

    /// Helper: encode a raw `sys` operand string and unwrap the resulting word.
    fn encode_word(raw: &str) -> u32 {
        match encode_sys(raw).expect("valid sys operands must encode") {
            EncodeResult::Word(w) => w,
            other => panic!("expected EncodeResult::Word, got {:?}", other),
        }
    }

    proptest! {
        // 1. Fixed opcode: bits[31:19] are constant (0xD508_0000) for every
        //    well-formed input, regardless of field values.
        #[test]
        fn sys_high_opcode_bits_fixed(
            op1 in 0u32..=255, crn in 0u32..=255, crm in 0u32..=255,
            op2 in 0u32..=255, rt in 0u32..=31,
        ) {
            let raw = format!("#{}, c{}, c{}, #{}, x{}", op1, crn, crm, op2, rt);
            let word = encode_word(&raw);
            prop_assert_eq!(word & 0xFFF8_0000, 0xD508_0000u32);
        }

        // 2. Field-extraction oracle: within each field's legal range, every
        //    field round-trips out of the result at its declared position. This
        //    independently pins both the masking and the placement of all five
        //    fields (op1, CRn, CRm, op2, Rt).
        #[test]
        fn sys_each_field_roundtrips_in_valid_range(
            op1 in 0u32..=7, crn in 0u32..=15, crm in 0u32..=15,
            op2 in 0u32..=7, rt in 0u32..=31,
        ) {
            let raw = format!("#{}, c{}, c{}, #{}, x{}", op1, crn, crm, op2, rt);
            let word = encode_word(&raw);
            prop_assert_eq!((word >> 16) & 0x7,  op1, "op1 at [18:16]");
            prop_assert_eq!((word >> 12) & 0xF, crn, "CRn at [15:12]");
            prop_assert_eq!((word >> 8)  & 0xF, crm, "CRm at [11:8]");
            prop_assert_eq!((word >> 5)  & 0x7, op2, "op2 at [7:5]");
            prop_assert_eq!(word         & 0x1F, rt, "Rt at [4:0]");
        }

        // 3. Masking invariance: high bits of each numeric input are ignored.
        //    Encoding the raw (possibly out-of-range) values equals encoding
        //    each value masked down to its legal field width.
        #[test]
        fn sys_masks_field_inputs_to_width(
            op1 in 0u32..=0xFFFF, crn in 0u32..=0xFFFF, crm in 0u32..=0xFFFF,
            op2 in 0u32..=0xFFFF, rt in 0u32..=31,
        ) {
            let raw_big = format!("#{}, c{}, c{}, #{}, x{}", op1, crn, crm, op2, rt);
            let raw_masked = format!(
                "#{}, c{}, c{}, #{}, x{}",
                op1 & 7, crn & 0xF, crm & 0xF, op2 & 7, rt,
            );
            let big = encode_word(&raw_big);
            let masked = encode_word(&raw_masked);
            prop_assert_eq!(big, masked);
        }

        // 4. Default register: with exactly four operands (no Xt), Rt defaults
        //    to 31 (xzr).
        #[test]
        fn sys_omitted_register_defaults_to_xzr_31(
            op1 in 0u32..=7, crn in 0u32..=15, crm in 0u32..=15, op2 in 0u32..=7,
        ) {
            let raw = format!("#{}, c{}, c{}, #{}", op1, crn, crm, op2);
            let word = encode_word(&raw);
            prop_assert_eq!(word & 0x1F, 31u32);
        }

        // 5. Injectivity over legal ranges: distinct (op1, CRn, CRm, op2, Rt)
        //    tuples yield distinct words. Holds because the fields occupy
        //    disjoint bits; a collision would reveal an overlap bug.
        #[test]
        fn sys_distinct_valid_tuples_distinct_words(
            a1 in 0u32..=7, acn in 0u32..=15, acm in 0u32..=15, a2 in 0u32..=7, art in 0u32..=31,
            b1 in 0u32..=7, bcn in 0u32..=15, bcm in 0u32..=15, b2 in 0u32..=7, brt in 0u32..=31,
        ) {
            let a = (a1, acn, acm, a2, art);
            let b = (b1, bcn, bcm, b2, brt);
            prop_assume!(a != b);
            let wa = encode_word(&format!("#{}, c{}, c{}, #{}, x{}", a1, acn, acm, a2, art));
            let wb = encode_word(&format!("#{}, c{}, c{}, #{}, x{}", b1, bcn, bcm, b2, brt));
            prop_assert_ne!(wa, wb);
        }

        // 6. Error contract: malformed operand strings are rejected with Err —
        //    too few operands, non-numeric op1/op2, or an unparseable register.
        #[test]
        fn sys_rejects_malformed_operands(kind in 0u8..5) {
            let raw = match kind {
                0 => "#1, c0, c0".to_string(),           // < 4 operands
                1 => "#1, c0, c0, #0, #0".to_string(),   // 5th part not a register
                2 => "foo, c0, c0, #0".to_string(),      // non-numeric op1
                3 => "#1, c0, c0, bar".to_string(),      // non-numeric op2
                _ => "#1, c0, c0, #0, xyz".to_string(),  // unparseable register
            };
            prop_assert!(
                encode_sys(&raw).is_err(),
                "expected Err for operand string: {}", raw,
            );
        }
    }

    // Deterministic companions: canonical boundary encodings.
    #[test]
    fn sys_canonical_words() {
        // All-zero fields, no register -> Rt=31 (xzr). Only the opcode survives.
        assert_eq!(encode_word("#0, c0, c0, #0"), 0xD508_001Fu32);
        // All-zero fields with x0 -> Rt=0.
        assert_eq!(encode_word("#0, c0, c0, #0, x0"), 0xD508_0000u32);
        // DC-CIVAC-equivalent fields: op1=3, CRn=7, CRm=14, op2=1, Rt=0.
        assert_eq!(
            encode_word("#3, c7, c14, #1, x0"),
            0xD508_0000u32 | (3 << 16) | (7 << 12) | (14 << 8) | (1 << 5) | 0,
        );
        // Uppercase CRn/CRm accepted (lowercased before strip).
        assert_eq!(encode_word("#0, C7, C10, #1, x5"), encode_word("#0, c7, c10, #1, x5"));
    }
}
