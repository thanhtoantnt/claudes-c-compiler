//! Property-based tests for three AArch64 branch/address-generation encoders.
//!
//! Covers (separately from the inline `prop_*_tests` modules that already
//! exist alongside each function):
//!   * `encode_adrp`  — NOTE: this function lives in `load_store.rs`, *not*
//!                      `compare_branch.rs`. It is re-exported here via
//!                      `super::*` (encoder/mod.rs does
//!                      `pub(crate) use load_store::*;`). The user request
//!                      named `compare_branch.rs`, but `encode_adrp` is in
//!                      fact defined there. `encode_adr` is intentionally
//!                      NOT tested, as requested.
//!   * `encode_cbz`   — `compare_branch.rs` (encodes both CBZ and CBNZ via
//!                      the `is_nz` flag).
//!   * `encode_tbz`   — `compare_branch.rs` (encodes both TBZ and TBNZ via
//!                      the `is_nz` flag).
//!
//! Oracle = ARMv8-A Architecture Reference Manual field-placement, plus the
//! AArch64 ELF ABI relocation contract. Every property that *witnesses a bug*
//! is `#[ignore]`d so the default `cargo test` stays green; run them with
//! `cargo test -- --ignored`.
//!
//! # Findings (bug witnesses, all `#[ignore]`)
//!
//! ## Finding 1 — `encode_adrp` silently accepts a 32-bit (W) destination
//! ADRP materialises a 64-bit page-aligned address; bit 31 is the fixed `1`
//! of the opcode and there is no 32-bit form. Per ARM ARM (C6.2.10 "ADRP")
//! the destination is `<Xd>` only. GAS and LLVM-MC reject
//! `adrp w0, sym` ("invalid operand" / "operand size mismatch"). The encoder
//! calls `get_reg(..)` and discards the returned `is_64`, so `adrp w0, sym`
//! returns `Ok` with the *identical* word to `adrp x0, sym` (0x9000_0000).
//! - Mechanism (passes): `prop_adrp_w_register_mirrors_x`.
//! - Witness (`#[ignore]`): `prop_adrp_rejects_32bit_register`.
//!
//! ## Finding 2 — `encode_tbz` silently truncates the test-bit immediate
//! TBZ/TBNZ's bit selector is the 6-bit field `b5:b40` (valid 0..=63). The
//! encoder computes `b5 = (bit as u32 >> 5) & 1` and `b40 = (bit as u32) &
//! 0x1F`, which silently wraps any value into 0..=63. Per ARM ARM (TBZ/TBNZ)
//! the field is unsigned and in-range; GAS/LLVM-MC reject `tbz x0, #64, t`
//! and `tbz x0, #-1, t`. Two distinct gaps:
//!   - values >= 64 wrap via masking;
//!   - negative values wrap via `i64 as u32` then masking.
//! - Mechanism (passes): `prop_tbz_bit_above_63_silently_wraps`.
//! - Witnesses (`#[ignore]`): `prop_tbz_rejects_bit_above_63`,
//!   `prop_tbz_rejects_negative_bit`.
//!
//! `encode_cbz` revealed no defect: its field placement, sf/op bits, and
//! CondBr19 relocation are all architecturally correct for every valid input.

#![cfg(test)]

use super::*; // brings in encode_adrp / encode_cbz / encode_tbz + EncodeResult/Relocation/RelocType
use crate::backend::arm::assembler::parser::Operand;
use proptest::prelude::*;

// ── helpers ──────────────────────────────────────────────────────────────

fn word_of(r: Result<EncodeResult, String>) -> u32 {
    match r {
        Ok(EncodeResult::WordWithReloc { word, .. }) => word,
        Ok(EncodeResult::Word(word)) => word,
        other => panic!("expected a word, got {:?}", other),
    }
}

fn reloc_of(r: Result<EncodeResult, String>) -> Relocation {
    match r {
        Ok(EncodeResult::WordWithReloc { reloc, .. }) => reloc,
        other => panic!("expected WordWithReloc, got {:?}", other),
    }
}

/// 64-bit destination registers `encode_adrp` is expected to accept, with the
/// 5-bit encoding the field must carry. (ADRP is 64-bit-only — see Finding 1.)
fn adrp_reg_case(n: u32) -> (String, u32) {
    match n {
        0..=30 => (format!("x{}", n), n),
        31 => ("sp".to_string(), 31),
        32 => ("xzr".to_string(), 31),
        33 => ("lr".to_string(), 30),
        _ => unreachable!(),
    }
}

prop_compose! {
    fn arb_adrp_reg()(n in 0u32..34) -> (String, u32) { adrp_reg_case(n) }
}

prop_compose! {
    fn arb_gp_reg()(n in 0u32..=30u32, is_64 in any::<bool>()) -> (String, u32) {
        let name = if is_64 { format!("x{}", n) } else { format!("w{}", n) };
        (name, n)
    }
}

prop_compose! {
    fn arb_sym()(s in "[a-zA-Z_][a-zA-Z0-9_]{0,7}") -> String { s }
}

// ADRP opcode template: bit[31]=1, bits[28:24]=10000, imm fields left zero.
const ADRP_TEMPLATE: u32 = 0x9000_0000;
// CBZ/CBNZ base: bits[30:25]=011010; sf[31] and op[24] added per instance.
const CBZ_BASE: u32 = 0b011010u32 << 25; // 0x3400_0000
// TBZ/TBNZ base: bits[30:25]=011011; b5[31], op[24], b40[23:19] added per instance.
const TBZ_BASE: u32 = 0b011011u32 << 25; // 0x3600_0000

proptest! {
    // ════════════════════════════════════════════════════════════════════════
    // encode_adrp  (load_store.rs)
    // ════════════════════════════════════════════════════════════════════════

    // Property 1 — opcode template + relocation type for a plain symbol.
    // ADRP Rd, sym  =>  word == 0x9000_0000 | Rd, reloc AdrpPage21,
    // symbol copied verbatim, addend 0.
    #[test]
    fn prop_adrp_template_and_reloc((rd_name, rd_num) in arb_adrp_reg(), sym in arb_sym()) {
        let ops = vec![Operand::Reg(rd_name), Operand::Symbol(sym.clone())];
        let word = word_of(encode_adrp(&ops));
        let rel = reloc_of(encode_adrp(&ops));
        prop_assert_eq!(word, ADRP_TEMPLATE | rd_num);
        prop_assert!(matches!(rel.reloc_type, RelocType::AdrpPage21));
        prop_assert_eq!(rel.symbol, sym);
        prop_assert_eq!(rel.addend, 0);
    }

    // Property 2 — Rd occupies exactly bits [4:0]; everything above is the
    // fixed template (op=1, [28:24]=10000, immhi/immlo zero).
    #[test]
    fn prop_adrp_rd_low5_field((rd_name, rd_num) in arb_adrp_reg(), sym in arb_sym()) {
        let ops = vec![Operand::Reg(rd_name), Operand::Symbol(sym)];
        let word = word_of(encode_adrp(&ops));
        prop_assert_eq!(word & 0x1F, rd_num);
        prop_assert_eq!(word & !0x1F, ADRP_TEMPLATE);
    }

    // Property 3 — SymbolOffset addend is forwarded VERBATIM (no masking).
    // R_AARCH64_ADR_PREL_PG_HI21 is page-relative: the *linker* discards the
    // low 12 bits of S+A, so the encoder must forward the full addend —
    // including negative, non-page-aligned, and full-range i64 values.
    // Per the AArch64 ELF ABI this delegation is intentional, so this is a
    // positive passthrough oracle, not a missing-range bug.
    #[test]
    fn prop_adrp_symboloffset_addend_verbatim(
        (rd_name, _) in arb_adrp_reg(),
        sym in arb_sym(),
        addend in any::<i64>(),
    ) {
        let ops = vec![Operand::Reg(rd_name), Operand::SymbolOffset(sym.clone(), addend)];
        let word = word_of(encode_adrp(&ops));
        let rel = reloc_of(encode_adrp(&ops));
        prop_assert_eq!(word & !0x1F, ADRP_TEMPLATE); // template unaffected by addend
        prop_assert!(matches!(rel.reloc_type, RelocType::AdrpPage21));
        prop_assert_eq!(rel.symbol, sym);
        prop_assert_eq!(rel.addend, addend); // verbatim, no truncation/mask
    }

    // Property 4 — `:got:` selects a distinct relocation; a non-"got" modifier
    // is not an ADRP operand and must be rejected.
    #[test]
    fn prop_adrp_got_modifier_distinct((rd_name, _) in arb_adrp_reg(), sym in arb_sym()) {
        let got = vec![
            Operand::Reg(rd_name.clone()),
            Operand::Modifier { kind: "got".to_string(), symbol: sym.clone() },
        ];
        let rel = reloc_of(encode_adrp(&got));
        prop_assert!(matches!(rel.reloc_type, RelocType::AdrGotPage21));
        prop_assert_eq!((rel.symbol, rel.addend), (sym.clone(), 0));

        // plain symbol -> AdrpPage21 (distinct from GOT)
        let plain = reloc_of(encode_adrp(&[
            Operand::Reg(rd_name.clone()), Operand::Symbol(sym.clone()),
        ]));
        prop_assert!(matches!(plain.reloc_type, RelocType::AdrpPage21));

        // a "lo12" modifier is rejected with Err
        let bad = encode_adrp(&[
            Operand::Reg(rd_name),
            Operand::Modifier { kind: "lo12".to_string(), symbol: sym },
        ]);
        prop_assert!(bad.is_err());
    }

    // Property 5 (MECHANISM for Finding 1, passes) — a 32-bit W register is
    // silently accepted and encodes to the *identical* word as the X form,
    // because `encode_adrp` discards `get_reg`'s `is_64` flag. This documents
    // the bug; the contract violation is the `#[ignore]`d Property 6.
    #[test]
    fn prop_adrp_w_register_mirrors_x(n in 0u32..=30u32, sym in arb_sym()) {
        let w_ops = vec![Operand::Reg(format!("w{}", n)), Operand::Symbol(sym.clone())];
        let x_ops = vec![Operand::Reg(format!("x{}", n)), Operand::Symbol(sym)];
        // The encoder must NOT accept the W form — but it does today.
        prop_assert!(encode_adrp(&w_ops).is_ok());
        prop_assert_eq!(word_of(encode_adrp(&w_ops)), word_of(encode_adrp(&x_ops)));
        // Both produce the 64-bit-only template with no distinguishing bit.
        prop_assert_eq!(word_of(encode_adrp(&w_ops)), ADRP_TEMPLATE | n);
    }

    // Property 6 (WITNESS for Finding 1, `#[ignore]`) — ADRP is 64-bit-only.
    // Per ARM ARM C6.2.10 the destination is `<Xd>`; GAS/LLVM-MC reject
    // `adrp w0, sym`. The encoder must return Err for a W register, but
    // instead returns Ok (see Property 5). Expected to FAIL today.
    #[test]
    #[ignore = "documented bug: encode_adrp silently accepts a 32-bit W destination (ADRP is Xd-only)"]
    fn prop_adrp_rejects_32bit_register(n in 0u32..=30u32, sym in arb_sym()) {
        let ops = vec![Operand::Reg(format!("w{}", n)), Operand::Symbol(sym)];
        prop_assert!(
            encode_adrp(&ops).is_err(),
            "adrp w{}, sym must be rejected (ADRP is 64-bit-only), got {:?}",
            n, encode_adrp(&ops)
        );
    }

    // ════════════════════════════════════════════════════════════════════════
    // encode_cbz  (compare_branch.rs) — also covers CBNZ via is_nz
    // ════════════════════════════════════════════════════════════════════════

    // Property 7 — CBZ/CBNZ structure: sf[31] 011010[30:25] op[24] imm19=0[23:5] Rt[4:0],
    // relocation CondBr19 with verbatim symbol + 0 addend.
    #[test]
    fn prop_cbz_structure_and_reloc(
        (rt_name, rt_num) in arb_gp_reg(),
        sym in arb_sym(),
        is_nz in any::<bool>(),
    ) {
        let ops = vec![Operand::Reg(rt_name.clone()), Operand::Symbol(sym.clone())];
        let word = word_of(encode_cbz(&ops, is_nz));
        let rel = reloc_of(encode_cbz(&ops, is_nz));

        let expected_sf = u32::from(rt_name.starts_with('x'));
        let op = u32::from(is_nz);
        prop_assert_eq!(word, (expected_sf << 31) | CBZ_BASE | (op << 24) | rt_num);
        // imm19 branch-offset field [23:5] left zero for the linker.
        prop_assert_eq!(word & 0x00FF_FFE0, 0);
        prop_assert!(matches!(rel.reloc_type, RelocType::CondBr19));
        prop_assert_eq!((rel.symbol, rel.addend), (sym, 0));
    }

    // Property 8 — CBZ and CBNZ differ ONLY in the op bit [24].
    #[test]
    fn prop_cbz_vs_cbnz_differ_op24(
        (rt_name, _) in arb_gp_reg(),
        sym in arb_sym(),
    ) {
        let ops = vec![Operand::Reg(rt_name), Operand::Symbol(sym)];
        prop_assert_eq!(
            word_of(encode_cbz(&ops, false)) ^ word_of(encode_cbz(&ops, true)),
            1u32 << 24
        );
    }

    // Property 9 — register width flips ONLY bit 31 (sf): x{N} vs w{N}.
    #[test]
    fn prop_cbz_width_sf31(n in 0u32..=30u32, sym in arb_sym(), is_nz in any::<bool>()) {
        let x = vec![Operand::Reg(format!("x{}", n)), Operand::Symbol(sym.clone())];
        let w = vec![Operand::Reg(format!("w{}", n)), Operand::Symbol(sym)];
        prop_assert_eq!(word_of(encode_cbz(&x, is_nz)) ^ word_of(encode_cbz(&w, is_nz)), 1u32 << 31);
    }

    // Property 10 — SymbolOffset addend forwarded verbatim into a CondBr19 reloc.
    #[test]
    fn prop_cbz_reloc_condbr19_addend(
        (rt_name, _) in arb_gp_reg(),
        sym in arb_sym(),
        off in any::<i64>(),
        is_nz in any::<bool>(),
    ) {
        let ops = vec![Operand::Reg(rt_name), Operand::SymbolOffset(sym.clone(), off)];
        let rel = reloc_of(encode_cbz(&ops, is_nz));
        prop_assert!(matches!(rel.reloc_type, RelocType::CondBr19));
        prop_assert_eq!((rel.symbol, rel.addend), (sym, off));
    }

    // ════════════════════════════════════════════════════════════════════════
    // encode_tbz  (compare_branch.rs) — also covers TBNZ via is_nz
    // ════════════════════════════════════════════════════════════════════════

    // Property 11 — TBZ/TBNZ structure + b5:b40 round-trip:
    //   b5[31] 011011[30:25] op[24] b40[23:19] imm14=0[18:5] Rt[4:0],
    // relocation TstBr14 with verbatim symbol + 0 addend.
    #[test]
    fn prop_tbz_structure_and_bit_roundtrip(
        (rt_name, rt_num) in arb_gp_reg(),
        bit in 0u32..=63u32,
        sym in arb_sym(),
        is_nz in any::<bool>(),
    ) {
        let ops = vec![Operand::Reg(rt_name), Operand::Imm(bit as i64), Operand::Symbol(sym.clone())];
        let word = word_of(encode_tbz(&ops, is_nz));
        let rel = reloc_of(encode_tbz(&ops, is_nz));

        let b5 = (bit >> 5) & 1;
        let b40 = bit & 0x1F;
        let op = u32::from(is_nz);
        prop_assert_eq!(word, (b5 << 31) | TBZ_BASE | (op << 24) | (b40 << 19) | rt_num);
        // imm14 branch-offset field [18:5] left zero for the linker.
        prop_assert_eq!(word & 0x0007_FFE0, 0);
        // round-trip: the split fields reconstruct the original bit.
        let got_b5 = (word >> 31) & 1;
        let got_b40 = (word >> 19) & 0x1F;
        prop_assert_eq!((got_b5 << 5) | got_b40, bit);
        prop_assert!(matches!(rel.reloc_type, RelocType::TstBr14));
        prop_assert_eq!((rel.symbol, rel.addend), (sym, 0));
    }

    // Property 12 — TBZ and TBNZ differ ONLY in the op bit [24].
    #[test]
    fn prop_tbz_vs_tbnz_differ_op24(
        (rt_name, _) in arb_gp_reg(),
        bit in 0u32..=63u32,
        sym in arb_sym(),
    ) {
        let ops = vec![Operand::Reg(rt_name), Operand::Imm(bit as i64), Operand::Symbol(sym)];
        prop_assert_eq!(
            word_of(encode_tbz(&ops, false)) ^ word_of(encode_tbz(&ops, true)),
            1u32 << 24
        );
    }

    // Property 13 — register width is irrelevant: bit 31 carries b5, not sf,
    // so x{N} and w{N} encode identically for the same test bit.
    #[test]
    fn prop_tbz_width_independent(
        n in 0u32..=30u32,
        bit in 0u32..=63u32,
        sym in arb_sym(),
        is_nz in any::<bool>(),
    ) {
        let x = vec![Operand::Reg(format!("x{}", n)), Operand::Imm(bit as i64), Operand::Symbol(sym.clone())];
        let w = vec![Operand::Reg(format!("w{}", n)), Operand::Imm(bit as i64), Operand::Symbol(sym)];
        prop_assert_eq!(word_of(encode_tbz(&x, is_nz)), word_of(encode_tbz(&w, is_nz)));
    }

    // Property 14 (MECHANISM for Finding 2, passes) — a bit >= 64 is silently
    // wrapped into 0..=63 by the masking (`& 0x1F`, `>>5 &1`) rather than
    // rejected. The reconstructed b5:b40 equals `bit & 0x3F`, and the encoder
    // returns Ok. Documents the bug; the contract violation is Property 15.
    #[test]
    fn prop_tbz_bit_above_63_silently_wraps(
        (rt_name, _) in arb_gp_reg(),
        bit in 64u32..=127u32,
        sym in arb_sym(),
        is_nz in any::<bool>(),
    ) {
        let ops = vec![Operand::Reg(rt_name), Operand::Imm(bit as i64), Operand::Symbol(sym)];
        // The encoder must NOT accept this — but it does today.
        prop_assert!(encode_tbz(&ops, is_nz).is_ok());
        let word = word_of(encode_tbz(&ops, is_nz));
        let b5 = (word >> 31) & 1;
        let b40 = (word >> 19) & 0x1F;
        prop_assert_eq!((b5 << 5) | b40, bit & 0x3F);
    }

    // Property 15 (WITNESS for Finding 2, `#[ignore]`) — the test-bit field is
    // a 6-bit UNSIGNED value (0..=63). Per ARM ARM (TBZ/TBNZ) and GAS/LLVM-MC,
    // `tbz x0, #64, t` is invalid and must be Err. Expected to FAIL today.
    #[test]
    #[ignore = "documented bug: encode_tbz silently truncates bit >= 64 into 0..=63 (field is 6-bit unsigned)"]
    fn prop_tbz_rejects_bit_above_63(
        (rt_name, _) in arb_gp_reg(),
        bit in 64u32..=4096u32,
        is_nz in any::<bool>(),
    ) {
        let ops = vec![Operand::Reg(rt_name), Operand::Imm(bit as i64), Operand::Symbol("t".into())];
        prop_assert!(
            encode_tbz(&ops, is_nz).is_err(),
            "tbz #{} (valid 0..=63) must be rejected, got {:?}",
            bit, encode_tbz(&ops, is_nz)
        );
    }

    // Property 16 (WITNESS for Finding 2, `#[ignore]`) — the field is unsigned,
    // so a negative immediate must be Err. The implementation does
    // `bit as u32` (i64 -> u32 truncation) then masks, silently wrapping
    // e.g. `tbz x0, #-1, t` into `tbz x0, #63, t`. Expected to FAIL today.
    #[test]
    #[ignore = "documented bug: encode_tbz silently wraps a negative bit via `i64 as u32` then masking (field is unsigned)"]
    fn prop_tbz_rejects_negative_bit(
        (rt_name, _) in arb_gp_reg(),
        bit in -4096i64..=-1i64,
        is_nz in any::<bool>(),
    ) {
        let ops = vec![Operand::Reg(rt_name), Operand::Imm(bit), Operand::Symbol("t".into())];
        prop_assert!(
            encode_tbz(&ops, is_nz).is_err(),
            "tbz #{} (negative) must be rejected (field is unsigned 0..=63)",
            bit
        );
    }

    // Property 17 (MECHANISM for Finding 3, passes) — for a 32-bit W register
    // the valid test-bit range is 0..=31, yet the encoder discards `get_reg`'s
    // `is_64` and accepts bits 32..=63 unchanged. Documents the bug; the
    // contract violation is the `#[ignore]`d Property 18.
    #[test]
    fn prop_tbz_w_register_accepts_bit_above_31(
        n in 0u32..=30u32,
        bit in 32u32..=63u32,
        is_nz in any::<bool>(),
    ) {
        let ops = vec![
            Operand::Reg(format!("w{}", n)),
            Operand::Imm(bit as i64),
            Operand::Symbol("t".into()),
        ];
        // The encoder must NOT accept this — but it does today.
        prop_assert!(encode_tbz(&ops, is_nz).is_ok());
        let word = word_of(encode_tbz(&ops, is_nz));
        let b5 = (word >> 31) & 1;
        let b40 = (word >> 19) & 0x1F;
        // The out-of-range-for-W bit is encoded verbatim (b5:b40 == bit).
        prop_assert_eq!((b5 << 5) | b40, bit);
    }

    // Property 18 (WITNESS for Finding 3, `#[ignore]`) — a 32-bit W register is
    // only 32 bits wide, so the test bit must be in 0..=31. The encoder
    // discards `get_reg`'s `is_64`, so bits 32..=63 are silently accepted on a
    // W destination, testing bits beyond the register width. GAS/LLVM-MC reject
    // `tbz w0, #40, t` ("immediate must be an integer in range [0, 31]").
    // Expected to FAIL today.
    #[test]
    #[ignore = "documented bug: encode_tbz accepts bit 32..=63 on a 32-bit W destination (width not validated)"]
    fn prop_tbz_rejects_bit_beyond_w_width(
        n in 0u32..=30u32,
        bit in 32u32..=63u32,
        is_nz in any::<bool>(),
    ) {
        let ops = vec![
            Operand::Reg(format!("w{}", n)),
            Operand::Imm(bit as i64),
            Operand::Symbol("t".into()),
        ];
        prop_assert!(
            encode_tbz(&ops, is_nz).is_err(),
            "tbz w{}, #{} must be rejected (W is 32-bit, valid 0..=31), got {:?}",
            n, bit, encode_tbz(&ops, is_nz)
        );
    }
}
