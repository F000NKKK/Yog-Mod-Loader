//! The machine the debugged process runs on (this program's own): the
//! breakpoint instruction and where a trap leaves the pc, which
//! instructions are calls (stepping over them), and the registers the
//! debugger shows.

#[cfg(target_arch = "x86_64")]
mod machine {
    use iced_x86::{Decoder, DecoderOptions, Mnemonic};

    /// `int3`.
    pub const BREAKPOINT: &[u8] = &[0xCC];
    /// A trap on `int3` leaves the pc just past it.
    pub const TRAP_ADVANCE: u64 = 1;
    /// The longest instruction there is.
    pub const LONGEST: usize = 15;
    /// The trap flag in the flags register: one instruction, then a trap
    /// (`ptrace` sets it by itself).
    #[cfg(not(target_os = "linux"))]
    pub const TRAP_FLAG: u64 = 0x100;
    const BITS: u32 = 64;

    /// The length of the instruction at `pc` (its bytes `code`) if it's a call.
    pub fn call_length(code: &[u8], pc: u64) -> Option<u64> {
        let insn = Decoder::with_ip(BITS, code, pc, DecoderOptions::NONE).decode();
        (!insn.is_invalid() && insn.mnemonic() == Mnemonic::Call).then_some(insn.len() as u64)
    }
}

#[cfg(target_arch = "aarch64")]
mod machine {
    /// `brk #0`, little-endian.
    pub const BREAKPOINT: &[u8] = &[0x00, 0x00, 0x20, 0xD4];
    /// A trap on `brk` leaves the pc on it.
    pub const TRAP_ADVANCE: u64 = 0;
    /// Every instruction is four bytes.
    pub const LONGEST: usize = 4;
    /// `bl imm26` and `blr xN`: their fixed bits, and which bits are fixed.
    const BL: (u32, u32) = (0x9400_0000, 0xFC00_0000);
    const BLR: (u32, u32) = (0xD63F_0000, 0xFFFF_FC1F);

    /// The length of the instruction at `pc` (its bytes `code`) if it's a call.
    pub fn call_length(code: &[u8], _pc: u64) -> Option<u64> {
        let word = u32::from_le_bytes(code.get(..LONGEST)?.try_into().ok()?);
        let is = |(bits, mask): (u32, u32)| word & mask == bits;
        (is(BL) || is(BLR)).then_some(LONGEST as u64)
    }
}

pub(crate) use machine::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn a_call_is_told_from_other_instructions() {
        // call rel32; mov eax, 1
        assert_eq!(call_length(&[0xE8, 0, 0, 0, 0], 0x1000), Some(5));
        assert_eq!(call_length(&[0xB8, 1, 0, 0, 0], 0x1000), None);
        assert_eq!(BREAKPOINT.len() as u64, TRAP_ADVANCE);
    }

    #[cfg(target_arch = "aarch64")]
    #[test]
    fn a_call_is_told_from_other_instructions() {
        // bl #8; blr x1; ret
        assert_eq!(call_length(&0x9400_0002u32.to_le_bytes(), 0), Some(4));
        assert_eq!(call_length(&0xD63F_0020u32.to_le_bytes(), 0), Some(4));
        assert_eq!(call_length(&0xD65F_03C0u32.to_le_bytes(), 0), None);
    }
}
