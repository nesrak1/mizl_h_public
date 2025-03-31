#[derive(PartialEq, Clone, Copy)]
pub enum Endianness {
    LittleEndian,
    BigEndian,
}

#[derive(PartialEq, Clone, Copy)]
pub enum Bitness {
    Bit8,
    Bit16,
    Bit32,
    Bit64,
}
