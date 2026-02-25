// remember: only include registers that are used in other reg files.
// don't include smaller registers like eax if you only need rax.
// it's okay to include eflags since linux only writes eflags for example.

use super::regmap_os_natreg::get_regmap_entries;
use crate::{
    debugger::registers::{
        registers::{NativeRegisterInfo, RegisterInfo, RegisterKind, RegisterRole},
        regmap::RegmapEntry,
    },
    sleigh::sla_file::{Sleigh, SymbolInner},
};
use num::FromPrimitive;
use smallvec::SmallVec;
use std::collections::HashMap;

#[repr(i32)]
#[derive(FromPrimitive, Copy, Clone)]
pub enum RegCodeAmd64 {
    // gpr
    Rax,
    Rcx,
    Rdx,
    Rbx,
    Rsp,
    Rbp,
    Rsi,
    Rdi,
    R8,
    R9,
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
    Rip,
    OrigRax,

    // flags
    Eflags,
    Rflags,

    // segment
    Es,
    Cs,
    Ss,
    Ds,
    Fs,
    Gs,
    FsBase,
    GsBase,

    // floating point
    St0,
    St1,
    St2,
    St3,
    St4,
    St5,
    St6,
    St7,

    Cwd,
    Swd,
    Ftw,
    Fop,
    Frip,
    Frdp,
    Mxcsr,
    MxcrMask,

    // control
    Cr0,
    Cr1,
    Cr3,
    Cr4,
    Cr5,
    Cr6,
    Cr7,
    Cr8,
    Cr9,
    Cr10,
    Cr11,
    Cr12,
    Cr13,
    Cr14,
    Cr15,

    // sse registers
    Xmm0,
    Xmm1,
    Xmm2,
    Xmm3,
    Xmm4,
    Xmm5,
    Xmm6,
    Xmm7,
    Xmm8,
    Xmm9,
    Xmm10,
    Xmm11,
    Xmm12,
    Xmm13,
    Xmm14,
    Xmm15,

    // debug
    Dr0,
    Dr1,
    Dr2,
    Dr3,
    Dr4,
    Dr5,
    Dr6,
    Dr7,
}

pub enum RegSrcAmd64 {
    Standard,      // user_regs_struct
    FloatingPoint, // user_fpregs_struct
}

pub struct Amd64NativeRegisterInfo {
    infos: Vec<RegisterInfo>,

    // use this to lookup a register string to an info.
    // this map contains all register infos, so you can
    // iterate the values if you only care about infos.
    reg_infos_lookup: HashMap<String, usize>,

    // use this to lookup a mizl register index to an
    // info. since mizl register indices only contain
    // registers directly readable from the host, it
    // will not contain smaller overlapping registers.
    host_infos_lookup: Vec<Option<usize>>,
}

impl Amd64NativeRegisterInfo {
    pub fn new(sleigh: &Sleigh) -> Amd64NativeRegisterInfo {
        let off2sla_map = sleigh.get_varnodes_by_offset();
        let sla_symbols = &sleigh.symbol_table.symbols;

        let mut infos: Vec<RegisterInfo> = Vec::new();
        let mut reg_infos_lookup: HashMap<String, usize> = HashMap::new();
        let mut host_infos_lookup: Vec<Option<usize>> = Vec::new();

        let entries = get_regmap_entries();
        for entry in entries.iter() {
            let infos_len = infos.len();
            let mizl_idx = entry.reg_idx;

            let varnode_idxs = Self::find_matching_sla_reg_varnodes(&off2sla_map, entry);

            let mut tmp_infos: SmallVec<RegisterInfo, 4> = SmallVec::new();
            let mut host_tmp_info: Option<usize> = None;
            if varnode_idxs.len() == 0 {
                // couldn't find a sleigh register for this native register.
                // this is pretty normal when sleigh doesn't have an OS
                // specific register available.
                let name = Self::conv_name_fallback(entry.reg_idx).unwrap_or("???".to_owned());
                let addr = Self::conv_nat2sla_addr(entry.reg_idx).unwrap_or(u32::MAX);
                tmp_infos.push(RegisterInfo {
                    name: name,
                    kind: RegisterKind::GeneralPurpose,
                    role: RegisterRole::None,
                    addr: addr,
                    mizl_idx: entry.reg_idx,
                    dbg_idx: -1,
                    bit_len: entry.size * 8,
                });

                host_tmp_info = Some(infos_len);
            } else {
                for varnode_idx in varnode_idxs {
                    let base_sym = &sla_symbols[varnode_idx as usize];
                    if let SymbolInner::VarnodeSym(varnode_sym) = &base_sym.inner {
                        tmp_infos.push(RegisterInfo {
                            name: base_sym.name.to_owned(),
                            kind: RegisterKind::GeneralPurpose,
                            role: RegisterRole::None,
                            addr: varnode_sym.offset,
                            mizl_idx: entry.reg_idx,
                            dbg_idx: -1,
                            bit_len: varnode_sym.size * 8,
                        });

                        if entry.size == varnode_sym.size {
                            // registers are an exact match, store in host_infos
                            host_tmp_info = Some(infos_len + tmp_infos.len() - 1);
                        }
                    } else {
                        // shouldn't happen, but at least we have a name
                        tmp_infos.push(RegisterInfo {
                            name: base_sym.name.to_owned(),
                            kind: RegisterKind::GeneralPurpose,
                            role: RegisterRole::None,
                            addr: u32::MAX,
                            mizl_idx: entry.reg_idx,
                            dbg_idx: -1,
                            bit_len: entry.size * 8,
                        });
                    }
                }
            }

            for i in 0..tmp_infos.len() {
                let tmp_info = &tmp_infos[i];
                reg_infos_lookup.insert(tmp_info.name.to_owned(), infos_len + i);
            }

            infos.extend(tmp_infos);

            if host_tmp_info.is_some() {
                // we can't preallocate a vec big enough because rust currently
                // doesn't have a way to find the length/last index of an enum.
                let mizl_idx_usize = mizl_idx as usize;
                while mizl_idx_usize >= host_infos_lookup.len() {
                    host_infos_lookup.push(None);
                }

                host_infos_lookup[mizl_idx_usize] = host_tmp_info;
            }
        }

        Amd64NativeRegisterInfo {
            infos,
            reg_infos_lookup,
            host_infos_lookup,
        }
    }

    fn conv_name_fallback(reg_index: i32) -> Option<String> {
        let reg_code = FromPrimitive::from_i32(reg_index)?;
        let reg_name = match reg_code {
            RegCodeAmd64::Rax => "RAX",
            RegCodeAmd64::Rcx => "RCX",
            RegCodeAmd64::Rdx => "RDX",
            RegCodeAmd64::Rbx => "RBX",
            RegCodeAmd64::Rsp => "RSP",
            RegCodeAmd64::Rbp => "RBP",
            RegCodeAmd64::Rsi => "RSI",
            RegCodeAmd64::Rdi => "RDI",
            RegCodeAmd64::R8 => "R8",
            RegCodeAmd64::R9 => "R9",
            RegCodeAmd64::R10 => "R10",
            RegCodeAmd64::R11 => "R11",
            RegCodeAmd64::R12 => "R12",
            RegCodeAmd64::R13 => "R13",
            RegCodeAmd64::R14 => "R14",
            RegCodeAmd64::R15 => "R15",
            RegCodeAmd64::Rip => "RIP",
            RegCodeAmd64::OrigRax => "ORIG_RAX",
            RegCodeAmd64::Eflags => "eflags",
            RegCodeAmd64::Rflags => "rflags",
            RegCodeAmd64::Es => "ES",
            RegCodeAmd64::Cs => "CS",
            RegCodeAmd64::Ss => "SS",
            RegCodeAmd64::Ds => "DS",
            RegCodeAmd64::Fs => "FS",
            RegCodeAmd64::Gs => "GS",
            RegCodeAmd64::FsBase => "FS_OFFSET",
            RegCodeAmd64::GsBase => "GS_OFFSET",
            RegCodeAmd64::St0 => "ST0",
            RegCodeAmd64::St1 => "ST1",
            RegCodeAmd64::St2 => "ST2",
            RegCodeAmd64::St3 => "ST3",
            RegCodeAmd64::St4 => "ST4",
            RegCodeAmd64::St5 => "ST5",
            RegCodeAmd64::St6 => "ST6",
            RegCodeAmd64::St7 => "ST7",
            RegCodeAmd64::Cwd => "FPUControlWord",
            RegCodeAmd64::Swd => "FPUStatusWord",
            RegCodeAmd64::Ftw => "FPUTagWord",
            RegCodeAmd64::Fop => "todo1",
            RegCodeAmd64::Frip => "todo2",
            RegCodeAmd64::Frdp => "todo3",
            RegCodeAmd64::Mxcsr => "todo4",
            RegCodeAmd64::MxcrMask => "todo5",
            RegCodeAmd64::Cr0 => "todo6",
            RegCodeAmd64::Cr1 => "todo7",
            RegCodeAmd64::Cr3 => "todo8",
            RegCodeAmd64::Cr4 => "todo9",
            RegCodeAmd64::Cr5 => "todo10",
            RegCodeAmd64::Cr6 => "todo11",
            RegCodeAmd64::Cr7 => "todo12",
            RegCodeAmd64::Cr8 => "todo13",
            RegCodeAmd64::Cr9 => "todo14",
            RegCodeAmd64::Cr10 => "todo15",
            RegCodeAmd64::Cr11 => "todo16",
            RegCodeAmd64::Cr12 => "todo17",
            RegCodeAmd64::Cr13 => "todo18",
            RegCodeAmd64::Cr14 => "todo19",
            RegCodeAmd64::Cr15 => "todo20",
            RegCodeAmd64::Xmm0 => "todo21",
            RegCodeAmd64::Xmm1 => "todo22",
            RegCodeAmd64::Xmm2 => "todo23",
            RegCodeAmd64::Xmm3 => "todo24",
            RegCodeAmd64::Xmm4 => "todo25",
            RegCodeAmd64::Xmm5 => "todo26",
            RegCodeAmd64::Xmm6 => "todo27",
            RegCodeAmd64::Xmm7 => "todo28",
            RegCodeAmd64::Xmm8 => "todo29",
            RegCodeAmd64::Xmm9 => "todo30",
            RegCodeAmd64::Xmm10 => "todo31",
            RegCodeAmd64::Xmm11 => "todo32",
            RegCodeAmd64::Xmm12 => "todo33",
            RegCodeAmd64::Xmm13 => "todo34",
            RegCodeAmd64::Xmm14 => "todo35",
            RegCodeAmd64::Xmm15 => "todo36",
            RegCodeAmd64::Dr0 => "todo37",
            RegCodeAmd64::Dr1 => "todo38",
            RegCodeAmd64::Dr2 => "todo39",
            RegCodeAmd64::Dr3 => "todo40",
            RegCodeAmd64::Dr4 => "todo41",
            RegCodeAmd64::Dr5 => "todo42",
            RegCodeAmd64::Dr6 => "todo43",
            RegCodeAmd64::Dr7 => "todo44",
        };
        Some(reg_name.to_owned())
    }

    fn conv_nat2sla_addr(reg_index: i32) -> Option<u32> {
        let reg_code = FromPrimitive::from_i32(reg_index)?;
        let reg_off = match reg_code {
            RegCodeAmd64::Rax => 0x0,
            RegCodeAmd64::Rcx => 0x8,
            RegCodeAmd64::Rdx => 0x10,
            RegCodeAmd64::Rbx => 0x18,
            RegCodeAmd64::Rsp => 0x20,
            RegCodeAmd64::Rbp => 0x28,
            RegCodeAmd64::Rsi => 0x30,
            RegCodeAmd64::Rdi => 0x38,
            RegCodeAmd64::R8 => 0x80,
            RegCodeAmd64::R9 => 0x88,
            RegCodeAmd64::R10 => 0x90,
            RegCodeAmd64::R11 => 0x98,
            RegCodeAmd64::R12 => 0xa0,
            RegCodeAmd64::R13 => 0xa8,
            RegCodeAmd64::R14 => 0xb0,
            RegCodeAmd64::R15 => 0xb8,
            RegCodeAmd64::Rip => 0x288,
            RegCodeAmd64::OrigRax => 0x10000,
            RegCodeAmd64::Eflags => 0x280,
            RegCodeAmd64::Rflags => 0x280,
            RegCodeAmd64::Es => 0x100,
            RegCodeAmd64::Cs => 0x102,
            RegCodeAmd64::Ss => 0x104,
            RegCodeAmd64::Ds => 0x106,
            RegCodeAmd64::Fs => 0x108,
            RegCodeAmd64::Gs => 0x10a,
            RegCodeAmd64::FsBase => 0x110,
            RegCodeAmd64::GsBase => 0x118,
            RegCodeAmd64::St0 => 0x1100,
            RegCodeAmd64::St1 => 0x1110,
            RegCodeAmd64::St2 => 0x1120,
            RegCodeAmd64::St3 => 0x1130,
            RegCodeAmd64::St4 => 0x1140,
            RegCodeAmd64::St5 => 0x1150,
            RegCodeAmd64::St6 => 0x1160,
            RegCodeAmd64::St7 => 0x1170,
            RegCodeAmd64::Cwd => 0x10a0,
            RegCodeAmd64::Swd => 0x10a2,
            RegCodeAmd64::Ftw => 0x10a4,
            RegCodeAmd64::Fop => 0x10a6,
            RegCodeAmd64::Frip => 0x10b0,
            RegCodeAmd64::Frdp => 0x10a8,
            RegCodeAmd64::Mxcsr => 0x1094,
            RegCodeAmd64::MxcrMask => 0x10008,
            RegCodeAmd64::Cr0 => 0x380,
            RegCodeAmd64::Cr1 => 0x388,
            RegCodeAmd64::Cr3 => 0x390,
            RegCodeAmd64::Cr4 => 0x398,
            RegCodeAmd64::Cr5 => 0x3a0,
            RegCodeAmd64::Cr6 => 0x3a8,
            RegCodeAmd64::Cr7 => 0x3b0,
            RegCodeAmd64::Cr8 => 0x3b8,
            RegCodeAmd64::Cr9 => 0x3c0,
            RegCodeAmd64::Cr10 => 0x3d0,
            RegCodeAmd64::Cr11 => 0x3d8,
            RegCodeAmd64::Cr12 => 0x3e0,
            RegCodeAmd64::Cr13 => 0x3e8,
            RegCodeAmd64::Cr14 => 0x3f0,
            RegCodeAmd64::Cr15 => 0x3f8,
            RegCodeAmd64::Xmm0 => 0x1200,
            RegCodeAmd64::Xmm1 => 0x1220,
            RegCodeAmd64::Xmm2 => 0x1240,
            RegCodeAmd64::Xmm3 => 0x1260,
            RegCodeAmd64::Xmm4 => 0x1280,
            RegCodeAmd64::Xmm5 => 0x12a0,
            RegCodeAmd64::Xmm6 => 0x12c0,
            RegCodeAmd64::Xmm7 => 0x12e0,
            RegCodeAmd64::Xmm8 => 0x1300,
            RegCodeAmd64::Xmm9 => 0x1320,
            RegCodeAmd64::Xmm10 => 0x1340,
            RegCodeAmd64::Xmm11 => 0x1360,
            RegCodeAmd64::Xmm12 => 0x1380,
            RegCodeAmd64::Xmm13 => 0x13a0,
            RegCodeAmd64::Xmm14 => 0x13c0,
            RegCodeAmd64::Xmm15 => 0x13e0,
            RegCodeAmd64::Dr0 => 0x300,
            RegCodeAmd64::Dr1 => 0x308,
            RegCodeAmd64::Dr2 => 0x310,
            RegCodeAmd64::Dr3 => 0x318,
            RegCodeAmd64::Dr4 => 0x320,
            RegCodeAmd64::Dr5 => 0x328,
            RegCodeAmd64::Dr6 => 0x330,
            RegCodeAmd64::Dr7 => 0x338,
        };
        Some(reg_off)
    }

    fn _conv_sla2nat_addr(sla_addr: u32) -> Option<i32> {
        let reg_code = match sla_addr {
            0x0 => RegCodeAmd64::Rax,
            0x8 => RegCodeAmd64::Rcx,
            0x10 => RegCodeAmd64::Rdx,
            0x18 => RegCodeAmd64::Rbx,
            0x20 => RegCodeAmd64::Rsp,
            0x28 => RegCodeAmd64::Rbp,
            0x30 => RegCodeAmd64::Rsi,
            0x38 => RegCodeAmd64::Rdi,
            0x80 => RegCodeAmd64::R8,
            0x88 => RegCodeAmd64::R9,
            0x90 => RegCodeAmd64::R10,
            0x98 => RegCodeAmd64::R11,
            0xa0 => RegCodeAmd64::R12,
            0xa8 => RegCodeAmd64::R13,
            0xb0 => RegCodeAmd64::R14,
            0xb8 => RegCodeAmd64::R15,
            0x288 => RegCodeAmd64::Rip,
            0x10000 => RegCodeAmd64::OrigRax,
            0x280 => RegCodeAmd64::Eflags,
            0x100 => RegCodeAmd64::Es,
            0x102 => RegCodeAmd64::Cs,
            0x104 => RegCodeAmd64::Ss,
            0x106 => RegCodeAmd64::Ds,
            0x108 => RegCodeAmd64::Fs,
            0x10a => RegCodeAmd64::Gs,
            0x110 => RegCodeAmd64::FsBase,
            0x118 => RegCodeAmd64::GsBase,
            0x1100 => RegCodeAmd64::St0,
            0x1110 => RegCodeAmd64::St1,
            0x1120 => RegCodeAmd64::St2,
            0x1130 => RegCodeAmd64::St3,
            0x1140 => RegCodeAmd64::St4,
            0x1150 => RegCodeAmd64::St5,
            0x1160 => RegCodeAmd64::St6,
            0x1170 => RegCodeAmd64::St7,
            0x10a0 => RegCodeAmd64::Cwd,
            0x10a2 => RegCodeAmd64::Swd,
            0x10a4 => RegCodeAmd64::Ftw,
            0x10a6 => RegCodeAmd64::Fop,
            0x10b0 => RegCodeAmd64::Frip,
            0x10a8 => RegCodeAmd64::Frdp,
            0x1094 => RegCodeAmd64::Mxcsr,
            0x10008 => RegCodeAmd64::MxcrMask,
            0x380 => RegCodeAmd64::Cr0,
            0x388 => RegCodeAmd64::Cr1,
            0x390 => RegCodeAmd64::Cr3,
            0x398 => RegCodeAmd64::Cr4,
            0x3a0 => RegCodeAmd64::Cr5,
            0x3a8 => RegCodeAmd64::Cr6,
            0x3b0 => RegCodeAmd64::Cr7,
            0x3b8 => RegCodeAmd64::Cr8,
            0x3c0 => RegCodeAmd64::Cr9,
            0x3d0 => RegCodeAmd64::Cr10,
            0x3d8 => RegCodeAmd64::Cr11,
            0x3e0 => RegCodeAmd64::Cr12,
            0x3e8 => RegCodeAmd64::Cr13,
            0x3f0 => RegCodeAmd64::Cr14,
            0x3f8 => RegCodeAmd64::Cr15,
            0x1200 => RegCodeAmd64::Xmm0,
            0x1220 => RegCodeAmd64::Xmm1,
            0x1240 => RegCodeAmd64::Xmm2,
            0x1260 => RegCodeAmd64::Xmm3,
            0x1280 => RegCodeAmd64::Xmm4,
            0x12a0 => RegCodeAmd64::Xmm5,
            0x12c0 => RegCodeAmd64::Xmm6,
            0x12e0 => RegCodeAmd64::Xmm7,
            0x1300 => RegCodeAmd64::Xmm8,
            0x1320 => RegCodeAmd64::Xmm9,
            0x1340 => RegCodeAmd64::Xmm10,
            0x1360 => RegCodeAmd64::Xmm11,
            0x1380 => RegCodeAmd64::Xmm12,
            0x13a0 => RegCodeAmd64::Xmm13,
            0x13c0 => RegCodeAmd64::Xmm14,
            0x13e0 => RegCodeAmd64::Xmm15,
            0x300 => RegCodeAmd64::Dr0,
            0x308 => RegCodeAmd64::Dr1,
            0x310 => RegCodeAmd64::Dr2,
            0x318 => RegCodeAmd64::Dr3,
            0x320 => RegCodeAmd64::Dr4,
            0x328 => RegCodeAmd64::Dr5,
            0x330 => RegCodeAmd64::Dr6,
            0x338 => RegCodeAmd64::Dr7,
            _ => return None,
        };
        Some(reg_code as i32)
    }

    fn find_matching_sla_reg_varnodes<'a>(
        off2sla_map: &'a HashMap<u32, Vec<u32>>,
        entry: &RegmapEntry,
    ) -> SmallVec<u32, 4> {
        let entry_sla_addr_opt = Self::conv_nat2sla_addr(entry.reg_idx);
        if entry_sla_addr_opt.is_some() {
            let entry_sla_addr = entry_sla_addr_opt.unwrap();
            let sla_idxs_opt = off2sla_map.get(&entry_sla_addr);
            if sla_idxs_opt.is_none() {
                return SmallVec::new();
            } else {
                let sla_idxs = sla_idxs_opt.unwrap();
                return SmallVec::from_iter(sla_idxs.into_iter().map(|x| *x));
            }
        } else {
            return SmallVec::new();
        }
    }
}

impl NativeRegisterInfo for Amd64NativeRegisterInfo {
    fn get_all_infos(&self) -> Vec<&RegisterInfo> {
        let infos = &self.infos;
        infos.into_iter().collect()
    }

    fn get_reg_info(&self, search: &str, case_sensitive: bool) -> Option<&RegisterInfo> {
        // we can't provide any guarantees that, when case sensitive,
        // registers are unique. so we don't make all keys lowercase
        // or anything like that.

        if !case_sensitive {
            let search_lower = search.to_lowercase();
            for info in &self.reg_infos_lookup {
                if info.0.to_lowercase() == search_lower {
                    let info_idx = *info.1;
                    return self.infos.get(info_idx);
                }
            }

            None
        } else {
            let info_idx = *self.reg_infos_lookup.get(search)?;
            self.infos.get(info_idx)
        }
    }

    fn get_host_info(&self, mizl_idx: i32) -> Option<&RegisterInfo> {
        let host_infos = &self.host_infos_lookup;
        if mizl_idx as usize >= host_infos.len() {
            return None;
        }

        match &host_infos[mizl_idx as usize] {
            Some(idx) => self.infos.get(*idx),
            None => return None,
        }
    }
}
