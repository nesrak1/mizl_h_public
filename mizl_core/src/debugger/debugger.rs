use super::{fast_util::read_swap_bytes, registers::registers::RegisterInfo};
use crate::ffi::core_framework::prelude::*;
use crate::sleigh::disasm::DisasmDispInstruction;
use bitflags::bitflags;
use std::fmt;

#[derive(Debug, ToPrimitive, Clone, Copy)]
pub enum DebuggerError {
    InvalidArguments = 0,
    ForkFailed = 1,
    AlreadyRunning = 2,
    NotStopped = 3,
    DisassemblyFailed = 4,
    MemoryAccessFailed = 5,
    InternalError = 6,
    InvalidRegister = 7,
    InvalidThread = 8,
    InvalidBreakpoint = 9,
    NoThreads = 10,
}

#[derive(Debug, ToPrimitive, Clone, Copy, PartialEq)]
pub enum DebuggerEventKind {
    Failed = 0,
    NoEvent = 1,
    UnknownEvent = 2,
    BreakpointHit = 3,
    StepComplete = 4,
    StepCompleteSyscall = 5,
    MiscSignalReceived = 6,
    ThreadSpawned = 7,
    ThreadKilled = 8,
    UserEvent = 9,
}

bitflags! {
    #[derive(Default)]
    pub struct DebuggerFlags: u32 {
        const NonStop = 1 << 0;
    }
}

#[derive(FfiSerialize)]
pub struct DebuggerEvent {
    #[ffi_serialize_enum]
    pub kind: DebuggerEventKind,
    pub code: u32, // native event code
    pub pid: u32,  // native pid
}

#[derive(Clone, Copy)]
pub enum DebuggerThreadIndex {
    Current,
    Specific(u32),
}

impl fmt::Display for DebuggerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DebuggerError::InvalidArguments => write!(f, "the action was requested with invalid aguments"),
            DebuggerError::ForkFailed => write!(f, "failed to fork while trying to run a process"),
            DebuggerError::AlreadyRunning => write!(f, "can't run the debugger while already debugging"),
            DebuggerError::NotStopped => write!(f, "can't perform this action while the process is running"),
            DebuggerError::DisassemblyFailed => write!(f, "could not disassemble the instruction"),
            DebuggerError::MemoryAccessFailed => write!(f, "could not read/write the requested memory"),
            DebuggerError::InternalError => write!(f, "an internal operation failed"),
            DebuggerError::InvalidRegister => write!(f, "the requested register doesn't exist"),
            DebuggerError::InvalidThread => write!(f, "the requested thread doesn't exist"),
            DebuggerError::InvalidBreakpoint => write!(f, "the requested breakpoint doesn't exist"),
            DebuggerError::NoThreads => write!(f, "there are no running threads to process"),
        }
    }
}

impl fmt::Display for DebuggerEventKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DebuggerEventKind::Failed => write!(f, "request to read events failed (or is empty)"),
            DebuggerEventKind::NoEvent => write!(f, "no event"),
            DebuggerEventKind::UnknownEvent => write!(f, "unknown event"),
            DebuggerEventKind::BreakpointHit => write!(f, "breakpoint hit"),
            DebuggerEventKind::StepComplete => write!(f, "step complete"),
            DebuggerEventKind::StepCompleteSyscall => write!(f, "step complete syscall"),
            DebuggerEventKind::MiscSignalReceived => write!(f, "misc signal received"),
            DebuggerEventKind::ThreadSpawned => write!(f, "thread spawned"),
            DebuggerEventKind::ThreadKilled => write!(f, "thread killed"),
            DebuggerEventKind::UserEvent => write!(f, "custom user event"),
        }
    }
}

pub trait Debugger {
    fn is_big_endian(&self) -> bool;
    fn get_flags(&self) -> DebuggerFlags;
    fn set_flags(&self, flags: DebuggerFlags) -> Result<(), DebuggerError>;

    // first args element should be the binary itself
    fn run(&self, path: &str, args: &[&str]) -> Result<i32, DebuggerError>;

    fn wait_next_event(&self, no_block: bool) -> Result<DebuggerEvent, DebuggerError>;
    fn add_event_id(&self, id: u32) -> Result<(), DebuggerError>;
    fn remove_event_id(&self, id: u32) -> Result<(), DebuggerError>;

    fn disassemble_one(&self, addr: u64) -> Result<DisasmDispInstruction, DebuggerError>;

    fn get_register_infos(&self, thread_idx: DebuggerThreadIndex) -> Vec<&RegisterInfo>;
    fn read_register_by_idx_buf(
        &self,
        thread_idx: DebuggerThreadIndex,
        reg_idx: i32,
        out_data: &mut [u8],
    ) -> Result<(), DebuggerError>;
    fn read_register_by_name_buf(
        &self,
        thread_idx: DebuggerThreadIndex,
        name: &str,
        out_data: &mut [u8],
    ) -> Result<(), DebuggerError>;

    // todo: count is probably unnecessary
    fn read_bytes(&self, thread_idx: DebuggerThreadIndex, addr: u64, out_data: &mut [u8])
        -> Result<u64, DebuggerError>;
    fn write_bytes(&self, thread_idx: DebuggerThreadIndex, addr: u64, data: &[u8]) -> Result<u64, DebuggerError>;

    fn add_breakpoint(&self, thread_idx: DebuggerThreadIndex, addr: u64) -> Result<u32, DebuggerError>;
    //fn add_breakpoint_of_type(&self, addr: u64, bp_type_idx: u32) -> u32;
    fn remove_breakpoint(&self, thread_idx: DebuggerThreadIndex, bp_idx: u32) -> Result<(), DebuggerError>;

    fn step(&self, thread_idx: DebuggerThreadIndex) -> Result<(), DebuggerError>;
    fn cont_all(&self) -> Result<(), DebuggerError>;
    fn cont_one(&self, thread_idx: DebuggerThreadIndex) -> Result<(), DebuggerError>;
}

pub trait DebuggerHelper {
    fn read_register_by_idx<T>(&self, thread_idx: DebuggerThreadIndex, reg_idx: i32) -> Result<T, DebuggerError>
    where
        T: Default + Copy;

    fn read_register_by_name<T>(&self, thread_idx: DebuggerThreadIndex, name: &str) -> Result<T, DebuggerError>
    where
        T: Default + Copy;
}

impl<BT: Debugger> DebuggerHelper for BT {
    fn read_register_by_idx<T>(&self, thread_idx: DebuggerThreadIndex, reg_idx: i32) -> Result<T, DebuggerError>
    where
        T: Default + Copy,
    {
        let mut buffer = vec![0u8; std::mem::size_of::<T>()];
        self.read_register_by_idx_buf(thread_idx, reg_idx, &mut buffer)?;
        Ok(read_swap_bytes(&buffer, self.is_big_endian()))
    }

    fn read_register_by_name<T>(&self, thread_idx: DebuggerThreadIndex, name: &str) -> Result<T, DebuggerError>
    where
        T: Default + Copy,
    {
        let mut buffer = vec![0u8; std::mem::size_of::<T>()];
        self.read_register_by_name_buf(thread_idx, name, &mut buffer)?;
        Ok(read_swap_bytes(&buffer, self.is_big_endian()))
    }
}

impl DebuggerEvent {
    pub fn new(kind: DebuggerEventKind, code: u32) -> DebuggerEvent {
        DebuggerEvent { kind, code, pid: 0 }
    }

    pub fn new_with_pid(kind: DebuggerEventKind, code: u32, pid: u32) -> DebuggerEvent {
        DebuggerEvent { kind, code, pid }
    }
}
