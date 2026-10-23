//! The machines a mod's native runs on, as DWARF numbers their registers:
//! x86-64 (System V psABI, and Windows' — the same numbers) and AArch64
//! (Apple Silicon, Linux on ARM). Both keep a frame-pointer chain laid out
//! alike — the caller's frame pointer at `[fp]`, the return address right
//! above it — so only the register numbers and where a return address
//! lives differ.

/// DWARF register numbers for x86_64.
pub mod x86_64 {
    pub const RAX: u16 = 0;
    pub const RDX: u16 = 1;
    pub const RCX: u16 = 2;
    pub const RBX: u16 = 3;
    pub const RSI: u16 = 4;
    pub const RDI: u16 = 5;
    pub const RBP: u16 = 6;
    pub const RSP: u16 = 7;
    pub const R8: u16 = 8;
    pub const R9: u16 = 9;
    pub const R10: u16 = 10;
    pub const R11: u16 = 11;
    pub const R12: u16 = 12;
    pub const R13: u16 = 13;
    pub const R14: u16 = 14;
    pub const R15: u16 = 15;
    /// Return-address column; holds the frame's pc.
    pub const RIP: u16 = 16;
    /// Registers in DWARF order.
    pub const COUNT: usize = 17;
}

/// DWARF register numbers for AArch64.
pub mod aarch64 {
    /// `x0`..`x30` are 0..=30; the first callee-saved one.
    pub const X19: u16 = 19;
    /// The frame pointer (`x29`) and the link register (`x30`): the CFI's
    /// return-address column.
    pub const FP: u16 = 29;
    pub const LR: u16 = 30;
    pub const SP: u16 = 31;
    /// The pc (not a DWARF register: the slot after `sp` holds it).
    pub const PC: u16 = 32;
    /// General registers `x0`..`x30`.
    pub const GENERAL: usize = 31;
}

/// Register slots a frame keeps: enough for either machine.
pub(crate) const SLOTS: usize = aarch64::PC as usize + 1;

/// A machine a native runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Arch {
    #[default]
    X86_64,
    Aarch64,
}

/// x86-64's callee-saved registers (both ABIs share these; Windows also
/// keeps `rsi`/`rdi`, recovered when the CFI says so).
const X86_64_SAVED: [u16; 8] = [
    x86_64::RBX,
    x86_64::RBP,
    x86_64::RSI,
    x86_64::RDI,
    x86_64::R12,
    x86_64::R13,
    x86_64::R14,
    x86_64::R15,
];

/// AArch64's: `x19`..`x28`, the frame pointer and the link register.
const AARCH64_SAVED_COUNT: usize = (aarch64::LR - aarch64::X19 + 1) as usize;
const AARCH64_SAVED: [u16; AARCH64_SAVED_COUNT] = {
    let mut regs = [0; AARCH64_SAVED_COUNT];
    let mut i = 0;
    while i < AARCH64_SAVED_COUNT {
        regs[i] = aarch64::X19 + i as u16;
        i += 1;
    }
    regs
};

impl Arch {
    /// The machine this program runs on (the one it debugs).
    #[must_use]
    pub const fn host() -> Arch {
        if cfg!(target_arch = "aarch64") {
            Arch::Aarch64
        } else {
            Arch::X86_64
        }
    }

    /// The machine object file `object` is for; none for others.
    #[must_use]
    pub fn of(object: &object::File<'_>) -> Option<Arch> {
        use object::Object as _;
        match object.architecture() {
            object::Architecture::X86_64 => Some(Arch::X86_64),
            object::Architecture::Aarch64 => Some(Arch::Aarch64),
            _ => None,
        }
    }

    /// The slots of its pc, stack pointer and frame pointer.
    #[must_use]
    pub const fn pc(self) -> u16 {
        match self {
            Arch::X86_64 => x86_64::RIP,
            Arch::Aarch64 => aarch64::PC,
        }
    }

    #[must_use]
    pub const fn sp(self) -> u16 {
        match self {
            Arch::X86_64 => x86_64::RSP,
            Arch::Aarch64 => aarch64::SP,
        }
    }

    #[must_use]
    pub const fn fp(self) -> u16 {
        match self {
            Arch::X86_64 => x86_64::RBP,
            Arch::Aarch64 => aarch64::FP,
        }
    }

    /// The CFI column a frame's return address is in.
    #[must_use]
    pub const fn return_address(self) -> u16 {
        match self {
            Arch::X86_64 => x86_64::RIP,
            Arch::Aarch64 => aarch64::LR,
        }
    }

    /// The registers a callee keeps for its caller — the only ones (with
    /// the stack pointer and pc) an outer frame's CFI can recover.
    #[must_use]
    pub fn callee_saved(self) -> &'static [u16] {
        match self {
            Arch::X86_64 => &X86_64_SAVED,
            Arch::Aarch64 => &AARCH64_SAVED,
        }
    }

    /// Whether a call leaves the return address in a register (AArch64's
    /// link register) rather than on the stack (x86-64's `call`).
    #[must_use]
    pub const fn return_in_register(self) -> bool {
        matches!(self, Arch::Aarch64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_machine_names_its_own_registers() {
        assert_eq!(
            (Arch::X86_64.pc(), Arch::X86_64.sp(), Arch::X86_64.fp()),
            (16, 7, 6)
        );
        assert_eq!(
            (Arch::Aarch64.pc(), Arch::Aarch64.sp(), Arch::Aarch64.fp()),
            (32, 31, 29)
        );
        assert_eq!(Arch::Aarch64.return_address(), aarch64::LR);
        assert!(Arch::Aarch64.callee_saved().contains(&aarch64::LR));
        const { assert!(SLOTS > x86_64::COUNT) };
    }
}
