use super::debugger_linux::DebuggerLinuxPauseState;
use crate::debugger::debugger::DebuggerEventKind;

pub fn convert_si_code(si_code: i32) -> (DebuggerLinuxPauseState, DebuggerEventKind) {
    match si_code {
        libc::SI_KERNEL => (DebuggerLinuxPauseState::StepCompleted, DebuggerEventKind::StepComplete),
        libc::TRAP_BRKPT => (
            DebuggerLinuxPauseState::SwBreakpointHit,
            DebuggerEventKind::BreakpointHit,
        ),
        libc::TRAP_TRACE => (
            DebuggerLinuxPauseState::StoppedUnknownReason,
            DebuggerEventKind::MiscSignalReceived,
        ),
        _ => (
            DebuggerLinuxPauseState::StoppedUnknownReason,
            DebuggerEventKind::MiscSignalReceived,
        ),
    }
}
