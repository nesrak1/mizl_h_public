use super::{regmap_arch_amd64::RegCodeAmd64, regmap_arch_amd64::RegSrcAmd64};
use crate::debugger::registers::regmap::RegmapEntry;

#[rustfmt::skip]
#[cfg(target_arch = "x86_64")]
pub const REGMAP_LINUX: [RegmapEntry; 59] = [
    // standard registers
    RegmapEntry::new(RegCodeAmd64::R15 as i32,       8, 0x00, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::R14 as i32,       8, 0x08, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::R13 as i32,       8, 0x10, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::R12 as i32,       8, 0x18, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::Rbp as i32,       8, 0x20, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::Rbx as i32,       8, 0x28, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::R11 as i32,       8, 0x30, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::R10 as i32,       8, 0x38, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::R9 as i32,        8, 0x40, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::R8 as i32,        8, 0x48, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::Rax as i32,       8, 0x50, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::Rcx as i32,       8, 0x58, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::Rdx as i32,       8, 0x60, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::Rsi as i32,       8, 0x68, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::Rdi as i32,       8, 0x70, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::OrigRax as i32,   8, 0x78, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::Rip as i32,       8, 0x80, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::Cs as i32,        2, 0x88, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::Eflags as i32,    4, 0x90, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::Rsp as i32,       8, 0x98, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::Ss as i32,        2, 0xa0, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::FsBase as i32,    8, 0xa8, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::GsBase as i32,    8, 0xb0, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::Ds as i32,        2, 0xb8, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::Es as i32,        2, 0xc0, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::Fs as i32,        2, 0xc8, RegSrcAmd64::Standard as i32),
    RegmapEntry::new(RegCodeAmd64::Gs as i32,        2, 0xd0, RegSrcAmd64::Standard as i32),

    // floating point registers
    RegmapEntry::new(RegCodeAmd64::Cwd as i32,       2, 0x00, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Swd as i32,       2, 0x02, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Ftw as i32,       2, 0x04, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Fop as i32,       2, 0x06, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Frip as i32,      8, 0x08, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Frdp as i32,      8, 0x10, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Mxcsr as i32,     4, 0x18, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::MxcrMask as i32,  4, 0x1c, RegSrcAmd64::FloatingPoint as i32),
    //
    RegmapEntry::new(RegCodeAmd64::St0 as i32,       8, 0x20 + 0*8, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::St1 as i32,       8, 0x20 + 1*8, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::St2 as i32,       8, 0x20 + 2*8, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::St3 as i32,       8, 0x20 + 3*8, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::St4 as i32,       8, 0x20 + 4*8, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::St5 as i32,       8, 0x20 + 5*8, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::St6 as i32,       8, 0x20 + 6*8, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::St7 as i32,       8, 0x20 + 7*8, RegSrcAmd64::FloatingPoint as i32),
    //
    RegmapEntry::new(RegCodeAmd64::Xmm0 as i32,      16, 0x60 + 0*16, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Xmm1 as i32,      16, 0x60 + 1*16, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Xmm2 as i32,      16, 0x60 + 2*16, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Xmm3 as i32,      16, 0x60 + 3*16, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Xmm4 as i32,      16, 0x60 + 4*16, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Xmm5 as i32,      16, 0x60 + 5*16, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Xmm6 as i32,      16, 0x60 + 6*16, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Xmm7 as i32,      16, 0x60 + 7*16, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Xmm8 as i32,      16, 0x60 + 8*16, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Xmm9 as i32,      16, 0x60 + 9*16, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Xmm10 as i32,     16, 0x60 + 10*16, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Xmm11 as i32,     16, 0x60 + 11*16, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Xmm12 as i32,     16, 0x60 + 12*16, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Xmm13 as i32,     16, 0x60 + 13*16, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Xmm14 as i32,     16, 0x60 + 14*16, RegSrcAmd64::FloatingPoint as i32),
    RegmapEntry::new(RegCodeAmd64::Xmm15 as i32,     16, 0x60 + 15*16, RegSrcAmd64::FloatingPoint as i32),

    // todo: other registers
];

// please add the regmap list for your new architecture here
#[rustfmt::skip]
#[cfg(not(target_arch = "x86_64"))]
pub const REGMAP_LINUX: [RegmapEntry; 1] = [
    RegmapEntry::new(-1, 0, 0, -1)
];

pub fn get_regmap_entries() -> &'static [RegmapEntry] {
    if cfg!(target_os = "linux") {
        &REGMAP_LINUX
    } else {
        unimplemented!();
    }
}

pub fn find_regmap_entry(mizl_idx: i32) -> Option<&'static RegmapEntry> {
    // we should be using one giant array, but w/e for now
    if cfg!(target_os = "linux") {
        for item in &REGMAP_LINUX {
            if item.reg_idx == mizl_idx {
                return Some(&item);
            }
        }
        return None;
    } else {
        unimplemented!();
    }
}
