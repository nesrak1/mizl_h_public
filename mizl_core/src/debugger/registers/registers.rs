pub enum RegisterKind {
    GeneralPurpose,
    FloatingPoint,
    Control,
    Flag,
}

pub enum RegisterRole {
    None,
    Flag,
    ProgramCounter,
    StackPointer,
    BasePointer,
}

pub struct RegisterInfo {
    pub name: String,
    pub kind: RegisterKind,
    pub role: RegisterRole,
    // the address in sleigh register address space
    pub addr: u32,
    // mizl's local register index
    pub mizl_idx: i32,
    // the remote debugger's register index or -1 if not a remote debugger
    pub dbg_idx: i64,
    pub bit_len: i32,
}

impl RegisterInfo {
    pub fn new(
        name: String,
        kind: RegisterKind,
        role: RegisterRole,
        addr: u32,
        mizl_idx: i32,
        dbg_idx: i64,
        bit_len: i32,
    ) -> RegisterInfo {
        RegisterInfo {
            name,
            kind,
            role,
            addr,
            mizl_idx,
            dbg_idx,
            bit_len,
        }
    }
}

pub trait NativeRegisterInfo {
    fn get_all_infos(&self) -> Vec<&RegisterInfo>;
    fn get_reg_info(&self, search: &str, case_sensitive: bool) -> Option<&RegisterInfo>;
    fn get_host_info(&self, mizl_idx: i32) -> Option<&RegisterInfo>;
}
