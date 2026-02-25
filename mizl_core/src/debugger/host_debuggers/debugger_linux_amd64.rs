use super::debugger_linux::DebuggerLinuxPauseState;
use crate::debugger::debugger::DebuggerEventKind;

pub fn convert_si_code(si_code: i32) -> (DebuggerLinuxPauseState, DebuggerEventKind) {
    match si_code {
        libc::SI_KERNEL => (
            DebuggerLinuxPauseState::SwBreakpointHit,
            DebuggerEventKind::BreakpointHit,
        ),
        libc::TRAP_BRKPT => (
            DebuggerLinuxPauseState::SyscallHitEnd,
            DebuggerEventKind::StepCompleteSyscall,
        ),
        libc::TRAP_TRACE => (DebuggerLinuxPauseState::StepCompleted, DebuggerEventKind::StepComplete),
        _ => (
            DebuggerLinuxPauseState::StoppedUnknownReason,
            DebuggerEventKind::MiscSignalReceived,
        ),
    }
}
