#[derive(Clone, Copy)]
pub struct RegmapEntry {
    // the mizl_h register index which maps an OS specific
    // register to a non-OS specific register code
    pub reg_idx: i32,
    // size of register in bytes
    pub size: i32,
    // offset in native register struct
    pub native_off: usize,
    // register source (could be gp, fp, etc.)
    // this index is specific to the OS and is generally the
    // mapping into the struct needed to access the field
    // similar, but not the same as RegisterKind
    pub source: i32,
}

impl RegmapEntry {
    pub const fn new(reg_idx: i32, size: i32, native_off: usize, source: i32) -> RegmapEntry {
        // name is intentionally unused
        RegmapEntry {
            reg_idx,
            size,
            native_off,
            source,
        }
    }
}
