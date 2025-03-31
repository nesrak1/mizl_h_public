use crate::{
    consts::arch::{Bitness, Endianness},
    memory::memview::{MemView, MemViewError},
};

pub struct ElfHeaderIdent {
    pub magic: [u8; 4],
    pub class: u8,
    pub data: u8,
    pub version: u8,
    pub osabi: u8,
    pub abiversion: u8,
    pub padding: [u8; 7],
}

pub struct ElfHeader {
    pub ident: ElfHeaderIdent,
    pub file_type: u16,
    pub machine: u16,
    pub version: u32,
    pub entry: u64, // u32 on 32-bit
    pub phoff: u64, // u32 on 32-bit
    pub shoff: u64, // u32 on 32-bit
    pub flags: u32,
    pub ehsize: u16,
    pub phentsize: u16,
    pub phnum: u16,
    pub shentsize: u16,
    pub shnum: u16,
    pub shstrndx: u16,
}

pub enum ElfReadError {
    IOError(MemViewError),
}

impl ElfHeaderIdent {
    pub fn new(mv: &Box<dyn MemView>, addr: &mut u64) -> Result<ElfHeaderIdent, MemViewError> {
        let mut magic = [0u8; 4];
        mv.read_bytes(addr, &mut magic, 4)?;
        let class = mv.read_u8(addr)?;
        let data = mv.read_u8(addr)?;
        let version = mv.read_u8(addr)?;
        let osabi = mv.read_u8(addr)?;
        let abiversion = mv.read_u8(addr)?;
        let mut padding = [0u8; 7];
        mv.read_bytes(addr, &mut padding, 7)?;
        Ok(ElfHeaderIdent {
            magic,
            class,
            data,
            version,
            osabi,
            abiversion,
            padding,
        })
    }
}

impl ElfHeader {
    pub fn new(mv: &Box<dyn MemView>, addr: &mut u64) -> Result<ElfHeader, MemViewError> {
        let ident = ElfHeaderIdent::new(mv, addr)?;

        let mut file_type: u16;
        let machine: u16;
        let version: u32;

        // make preliminary endianness before we get to the machine field.
        // thankfully, file_type should never be 0 so we should always
        // be able to tell which byte is 0 and which isn't.
        file_type = mv.read_u16(addr, Endianness::LittleEndian)?;
        if file_type < 0xff {
            // probably little endian
            machine = mv.read_u16(addr, Endianness::LittleEndian)?;
            version = mv.read_u32(addr, Endianness::LittleEndian)?;
        } else {
            // probably big endian
            file_type = u16::swap_bytes(file_type);
            machine = mv.read_u16(addr, Endianness::BigEndian)?;
            version = mv.read_u32(addr, Endianness::BigEndian)?;
        }

        let (bitness, endianness) = Self::get_endianness_and_bitness(ident.class, ident.data, machine);

        let entry: u64;
        let phoff: u64;
        let shoff: u64;
        let flags: u32;
        let ehsize: u16;
        let phentsize: u16;
        let phnum: u16;
        let shentsize: u16;
        let shnum: u16;
        let shstrndx: u16;

        if bitness == Bitness::Bit64 {
            entry = mv.read_u64(addr, endianness)?;
            phoff = mv.read_u64(addr, endianness)?;
            shoff = mv.read_u64(addr, endianness)?;
        } else {
            entry = mv.read_u32(addr, endianness)? as u64;
            phoff = mv.read_u32(addr, endianness)? as u64;
            shoff = mv.read_u32(addr, endianness)? as u64;
        }

        flags = mv.read_u32(addr, endianness)?;
        ehsize = mv.read_u16(addr, endianness)?;
        phentsize = mv.read_u16(addr, endianness)?;
        phnum = mv.read_u16(addr, endianness)?;
        shentsize = mv.read_u16(addr, endianness)?;
        shnum = mv.read_u16(addr, endianness)?;
        shstrndx = mv.read_u16(addr, endianness)?;

        Ok(ElfHeader {
            ident,
            file_type,
            machine,
            version,
            entry,
            phoff,
            shoff,
            flags,
            ehsize,
            phentsize,
            phnum,
            shentsize,
            shnum,
            shstrndx,
        })
    }

    // todo: need full format
    pub fn get_endianness_and_bitness(class: u8, data: u8, machine: u16) -> (Bitness, Endianness) {
        match machine {
            0x0003 => (Bitness::Bit32, Endianness::LittleEndian),
            0x003f => (Bitness::Bit64, Endianness::LittleEndian),
            _ => {
                // fallback
                let bitness = if class == 2 { Bitness::Bit64 } else { Bitness::Bit32 };
                let endianness = if data == 2 {
                    Endianness::BigEndian
                } else {
                    Endianness::LittleEndian
                };
                (bitness, endianness)
            }
        }
    }
}
