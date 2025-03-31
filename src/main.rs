#[macro_use]
extern crate num_derive;

pub mod binary_formats;
pub mod consts;
pub mod debugger;
pub mod memory;
pub mod sleigh;

use crossbeam::{channel::unbounded, select};
use debugger::{
    debugger::{Debugger, DebuggerEvent, DebuggerEventKind, DebuggerHelper, DebuggerThreadIndex},
    host_debuggers::debugger_linux::DebuggerLinux,
    registers::registers::RegisterInfo,
};
use sleigh::disasm::DisasmDispInstructionRun;
use std::{
    io::{self, Write},
    sync::Arc,
    thread,
};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn colorize_text(text: &str, runs: &Vec<DisasmDispInstructionRun>) -> String {
    let mut color_text = String::new();
    let mut text_idx = 0;
    for run in runs {
        color_text += match run.run_type {
            sleigh::disasm::DisasmDispInstructionRunType::Normal => "\x1b[0;37m",
            sleigh::disasm::DisasmDispInstructionRunType::Mnemonic => "\x1b[0;96m",
            sleigh::disasm::DisasmDispInstructionRunType::Register => "\x1b[0;93m",
            sleigh::disasm::DisasmDispInstructionRunType::Number => "\x1b[0;95m",
        };
        color_text += &text[(text_idx as usize)..((text_idx+run.length) as usize)];
        text_idx += run.length;
    }

    return color_text + "\x1b[0;37m";
}

fn disasm_at_pc<DBG>(debugger: &DBG, pc_reg: &RegisterInfo, len: i32) -> bool
where
    DBG: Debugger,
{
    let pc_reg_val: u64 = match debugger.read_register_by_idx(DebuggerThreadIndex::Current, pc_reg.mizl_idx) {
        Ok(v) => v,
        Err(e) => {
            println!("couldn't read pc: {}", e);
            return false;
        }
    };

    let mut dis_addr = pc_reg_val;
    for _ in 0..len {
        let disp_ins = debugger.disassemble_one(dis_addr);
        match disp_ins {
            Ok(v) => {
                let text_color = colorize_text(&v.text, &v.runs);
                println!("\x1b[0;92m{:#10x}\x1b[0;37m: {}", dis_addr, text_color);
                dis_addr += v.len;
            }
            Err(e) => {
                println!("<disassembly failed> {}", e);
                dis_addr += 1;
            }
        }
    }

    return true;
}

enum MainEvent {
    Command(String),
    Debugger(DebuggerEvent),
    Error,
}

fn main() {
    let path = "/bin/ls";
    let args = vec!["ls", "-la"];

    let debugger = Arc::new(DebuggerLinux::new());

    let reg_infos = debugger.get_register_infos(DebuggerThreadIndex::Current);
    let pc_reg = reg_infos.iter().find(|r| r.name == "RIP").unwrap();
    let mut cmd = "".to_owned();
    let mut last_cmd;
    let mut last_disasm_len = 10;

    let (dbg_tx, dbg_rx) = unbounded::<DebuggerEvent>();
    let (inp_tx, inp_rx) = unbounded::<String>();

    let debugger_proc_copy = Arc::clone(&debugger);
    thread::spawn(move || {
        match debugger_proc_copy.run(path, &args) {
            Ok(v) => println!("started with pid {}", v),
            Err(_) => panic!("nope, that didn't work"),
        };

        loop {
            let event = match debugger_proc_copy.wait_next_event() {
                Ok(v) => v,
                Err(_) => {
                    println!("error while reading next debugger event");
                    return;
                }
            };

            let _ = dbg_tx.send(event);
        }
    });

    thread::spawn(move || loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        let _ = inp_tx.send(input.trim().to_string());
    });

    loop {
        last_cmd = cmd.clone();

        // this should be on a timeout but whatever
        print!("cmd> ");
        io::stdout().flush().unwrap();

        let main_event = select! {
            recv(dbg_rx) -> msg => {
                match msg {
                    Ok(v) => {
                        MainEvent::Debugger(v)
                    },
                    Err(_) => {
                        MainEvent::Error
                    },
                }
            },
            recv(inp_rx) -> msg => {
                match msg {
                    Ok(v) => {
                        MainEvent::Command(v.to_owned())
                    },
                    Err(_) => {
                        MainEvent::Error
                    },
                }
            }
        };

        match main_event {
            MainEvent::Command(input) => {
                let trimmed_input = input.trim();
                let args: Vec<&str> = trimmed_input.split(" ").collect();
                cmd = args[0].to_string();
                if cmd == "" && last_cmd != "" {
                    cmd = last_cmd;
                }

                if cmd == "q" {
                    break;
                } else if cmd == "si" {
                    match debugger.step(DebuggerThreadIndex::Current) {
                        Ok(_) => {}
                        Err(e) => println!("error: {}", e),
                    };
                } else if cmd == "c" {
                    match debugger.cont_all() {
                        Ok(_) => {}
                        Err(e) => println!("error: {}", e),
                    };
                } else if cmd == "b" {
                    if args.len() < 2 {
                        println!("incorrect arguments");
                    } else {
                        let bp_addr_str = args[1];
                        match u64::from_str_radix(bp_addr_str, 16) {
                            Ok(bp_addr) => match debugger.add_breakpoint(DebuggerThreadIndex::Current, bp_addr) {
                                Ok(v) => {
                                    println!("created breakpoint {}", v);
                                }
                                Err(e) => println!("error: {}", e),
                            },
                            Err(_) => println!("incorrect arguments"),
                        };
                    }
                } else if cmd == "reg" {
                    if args.len() < 2 {
                        println!("incorrect arguments");
                    } else {
                        let reg_name = args[1];
                        match debugger.read_register_by_name::<u64>(DebuggerThreadIndex::Current, &reg_name) {
                            Ok(v) => {
                                println!("{} = 0x{:016x}", reg_name, v);
                            }
                            Err(e) => println!("error: {}", e),
                        }
                    }
                } else if cmd == "dis" {
                    let len = if args.len() > 1 {
                        match i32::from_str_radix(args[1], 10) {
                            Ok(v) => v,
                            Err(_) => last_disasm_len,
                        }
                    } else {
                        last_disasm_len
                    };

                    disasm_at_pc(&*debugger, &pc_reg, len);
                    last_disasm_len = len;
                }
            }
            MainEvent::Debugger(e) => {
                let event_kind = e.kind;
                match event_kind {
                    DebuggerEventKind::StepComplete | DebuggerEventKind::StepCompleteSyscall => {
                        println!("[step event]");
                        disasm_at_pc(&*debugger, &pc_reg, last_disasm_len);
                    }
                    DebuggerEventKind::BreakpointHit => {
                        println!("[breakpoint hit event]");
                        disasm_at_pc(&*debugger, &pc_reg, last_disasm_len);
                    }
                    DebuggerEventKind::MiscSignalReceived => {
                        let signal = (e.code >> 8) & 0xff;
                        let signal_name = match signal {
                            1 => "SIGHUP",
                            2 => "SIGINT",
                            3 => "SIGQUIT",
                            4 => "SIGILL",
                            5 => "SIGTRAP",
                            6 => "SIGABRT",
                            7 => "SIGBUS",
                            8 => "SIGFPE",
                            9 => "SIGKILL",
                            10 => "SIGUSR1",
                            11 => "SIGSEGV",
                            12 => "SIGUSR2",
                            13 => "SIGPIPE",
                            14 => "SIGALRM",
                            15 => "SIGTERM",
                            16 => "SIGSTKFLT",
                            17 => "SIGCHLD",
                            18 => "SIGCONT",
                            19 => "SIGSTOP",
                            20 => "SIGTSTP",
                            21 => "SIGTTIN",
                            22 => "SIGTTOU",
                            23 => "SIGURG",
                            24 => "SIGXCPU",
                            25 => "SIGXFSZ",
                            26 => "SIGVTALRM",
                            27 => "SIGPROF",
                            28 => "SIGWINCH",
                            29 => "SIGIO",
                            30 => "SIGPWR",
                            31 => "SIGSYS",
                            _ => "UNKNOWN",
                        };
                        println!("[received signal: {}]", signal_name);
                        if signal != 5 && signal != 19 {
                            match debugger.cont_all() {
                                Ok(_) => {}
                                Err(e) => println!("error: {}", e),
                            };
                        }
                    }
                    _ => {
                        println!("[received debugger event: {}]", e.code);
                    }
                }
            }
            MainEvent::Error => {
                println!("[got error while waiting for input or debugger events]");
            }
        };
    }

    println!("mizl_h done");
}
