#ifndef MIZL_DEBUGGER_H
#define MIZL_DEBUGGER_H

#include "common.h"

typedef enum
{
    DEBUGGER_ERROR_INVALID_ARGUMENTS = 0,
    DEBUGGER_ERROR_FORK_FAILED = 1,
    DEBUGGER_ERROR_ALREADY_RUNNING = 2,
    DEBUGGER_ERROR_NOT_STOPPED = 3,
    DEBUGGER_ERROR_DISASSEMBLY_FAILED = 4,
    DEBUGGER_ERROR_MEMORY_ACCESS_FAILED = 5,
    DEBUGGER_ERROR_INTERNAL_ERROR = 6,
    DEBUGGER_ERROR_INVALID_REGISTER = 7,
    DEBUGGER_ERROR_INVALID_THREAD = 8,
    DEBUGGER_ERROR_INVALID_BREAKPOINT = 9,
    DEBUGGER_ERROR_NO_THREADS = 10,
} DebuggerError;

typedef enum
{
    DEBUGGER_EVENT_KIND_FAILED = 0,
    DEBUGGER_EVENT_KIND_NO_EVENT = 1,
    DEBUGGER_EVENT_KIND_UNKNOWN_EVENT = 2,
    DEBUGGER_EVENT_KIND_BREAKPOINT_HIT = 3,
    DEBUGGER_EVENT_KIND_STEP_COMPLETE = 4,
    DEBUGGER_EVENT_KIND_STEP_COMPLETE_SYSCALL = 5,
    DEBUGGER_EVENT_KIND_MISC_SIGNAL_RECEIVED = 6,
    DEBUGGER_EVENT_KIND_THREAD_SPAWNED = 7,
    DEBUGGER_EVENT_KIND_THREAD_KILLED = 8,
    DEBUGGER_EVENT_KIND_USER_EVENT = 9,
} DebuggerEventKind;

typedef struct
{
    DebuggerEventKind kind;
    uint32_t code;
    uint32_t pid;
} DebuggerEvent;

// /////

typedef enum
{
    DISASM_DISP_INSTRUCTION_RUN_TYPE_NORMAL = 0,
    DISASM_DISP_INSTRUCTION_RUN_TYPE_MNEMONIC = 1,
    DISASM_DISP_INSTRUCTION_RUN_TYPE_REGISTER = 2,
    DISASM_DISP_INSTRUCTION_RUN_TYPE_NUMBER = 3,
} DisasmDispInstructionRunType;

typedef struct
{
    uint32_t length;
    DisasmDispInstructionRunType run_type;
} DisasmDispInstructionRun;

typedef struct
{
    uint64_t addr;
    uint64_t len;
    char *text;
    PhVec(DisasmDispInstructionRun *) runs;
} DisasmDispInstruction;

// /////

typedef struct PhOpaque(Debugger) Debugger;

Debugger *debugger_linux_new();

int debugger_get_big_endian(Debugger *self);
int debugger_run(Debugger *self, char *path, char **args, PhErr(DebuggerError) * err);
DebuggerEvent *debugger_wait_next_event(Debugger *self, bool no_block, PhErr(DebuggerError) * err);
DisasmDispInstruction *debugger_disassemble_one(Debugger *self, uint64_t addr, PhErr(DebuggerError) * err);
void debugger_read_register_by_name_buf(Debugger *self, int32_t thread_idx, char *name, char *out_data, size_t out_data_len, PhErr(DebuggerError) * err);
uint32_t debugger_add_breakpoint(Debugger *self, int32_t thread_idx, uint64_t addr, PhErr(DebuggerError) * err);
void debugger_step(Debugger *self, int32_t thread_idx, PhErr(DebuggerError) * err);
void debugger_cont_all(Debugger *self, PhErr(DebuggerError) * err);

#endif // MIZL_DEBUGGER_H