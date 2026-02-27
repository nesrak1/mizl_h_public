#[macro_use]
extern crate num_derive;

pub mod binary_formats;
pub mod consts;
pub mod database;
pub mod debugger;
pub mod ffi;
pub mod memory;
pub mod remote;
pub mod shared;
pub mod sleigh;

use crossbeam::{channel::unbounded, select};
use database::gbf_chained_buf_memview::GbfChainedBufMemView;
use database::{gbf::GbfFile, gbf_table_view::GbfTableView};
use database::{gbf_record::GbfFieldValue, gbf_table_view::GbfTableViewIterator};
use debugger::{
    debugger::{Debugger, DebuggerEvent, DebuggerEventKind, DebuggerHelper, DebuggerThreadIndex},
    host_debuggers::debugger_linux::DebuggerLinux,
    registers::registers::RegisterInfo,
};
use memory::memview::{MemView, StaticMemView};
use sleigh::disasm::{DisasmDispInstructionRun, DisasmDispInstructionRunType};
use std::fs::File;
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
            DisasmDispInstructionRunType::Normal => "\x1b[0;37m",
            DisasmDispInstructionRunType::Mnemonic => "\x1b[0;96m",
            DisasmDispInstructionRunType::Register => "\x1b[0;93m",
            DisasmDispInstructionRunType::Number => "\x1b[0;95m",
        };
        color_text += &text[(text_idx as usize)..((text_idx + run.length) as usize)];
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

fn disasm_at_addr<DBG>(debugger: &DBG, mut dis_addr: u64, len: i32) -> bool
where
    DBG: Debugger,
{
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

pub fn u8_to_str_fast(value: u8) -> String {
    if value == 0 {
        return String::from("00");
    }

    const HEX_CHARS: &[u8] = b"0123456789abcdef";
    let mut buffer = [0u8; 2];

    buffer[0] = HEX_CHARS[((value >> 4) & 0xF) as usize];
    buffer[1] = HEX_CHARS[(value & 0xF) as usize];

    // safety: we only use \-x0-f, so there won't be any issues with utf-8
    unsafe { std::str::from_utf8_unchecked(&buffer).to_string() }
}

fn main() {
    let file_data = std::fs::read("db.2.gbf").unwrap();

    let mv_i = StaticMemView::new(file_data);
    let mv: Box<dyn MemView> = Box::new(mv_i);

    let mut at = 0;
    let gbf = match GbfFile::new(mv, &mut at) {
        Ok(v) => v,
        Err(e) => {
            println!("error reading main file: {}", e);
            return;
        }
    };

    for (table_name, table_def) in &gbf.tables.table_defs {
        println!("found table {}", table_name);
    }

    let symbols = gbf.tables.table_defs.get("Symbols").expect("no metadata");
    let symbols_nid = symbols.root_nid;
    let symbol_schema = &symbols.schema;

    println!("symbol nid: {}", symbols_nid);

    let symbol_tv = match GbfTableView::new(&gbf, symbol_schema, symbols_nid) {
        Ok(v) => v,
        Err(e) => {
            println!("error reading metadata: {}", e);
            return;
        }
    };

    for name in &symbol_schema.names {
        println!("column: {name}");
    }

    let name_idx = symbol_schema.get_column_idx("Name").unwrap();
    let address_idx = symbol_schema.get_column_idx("Address").unwrap();
    // let namespace_idx = symbol_schema.get_column_idx("Namespace").unwrap();
    // let symbol_type_idx = symbol_schema.get_column_idx("Symbol Type").unwrap();
    // let string_data_idx = symbol_schema.get_column_idx("String Data").unwrap();
    // let flags_idx = symbol_schema.get_column_idx("Flags").unwrap();
    // let locator_hash_idx = symbol_schema.get_column_idx("Locator Hash").unwrap();
    // let primary_idx = symbol_schema.get_column_idx("Primary").unwrap();
    // let datatype_idx = symbol_schema.get_column_idx("Datatype").unwrap();
    // let variable_offset_idx = symbol_schema.get_column_idx("Variable Offset").unwrap();

    let symbol_tvi = GbfTableViewIterator::new(&symbol_tv, i64::MIN).expect("error on iterator ctor");
    for field in symbol_tvi {
        let field_uw = field.expect("error during field read");
        let key_value = match field_uw.key {
            GbfFieldValue::Long(v) => v,
            _ => panic!("error during key get"),
        };
        let name_value = field_uw.get_string(name_idx).expect("error during value get");
        let address_value = field_uw.get_long(address_idx).expect("error during value get");
        println!("key: {}, name: {}, address: {}", key_value, name_value, address_value);
    }

    let cbmv = GbfChainedBufMemView::new(&gbf, 10).expect("should be able to read cbmv");
    let max_address = cbmv.max_address().expect("should be able to read max address");
    let mut read_bytes = vec![0; max_address as usize];
    let mut cbmv_at = 0u64;
    cbmv.read_bytes(&mut cbmv_at, &mut read_bytes, max_address as i32)
        .expect("should be able to read");

    {
        let mut file = File::create("test.bin").expect("should be able to open file");
        file.write_all(&read_bytes).expect("should be able to write to file");
    }

    // let metadata_key_idx = metadata_schema.get_column_idx("Key").expect("no key field");
    // let metadata_value_idx = metadata_schema.get_column_idx("Value").expect("no value field");

    // let metadata_tvi = GbfTableViewIterator::new(&metadata_tv, i64::MIN).expect("error on iterator ctor");
    // for mdfield in metadata_tvi {
    //     let mdfield_uw = mdfield.expect("error during field read");
    //     let key_value = mdfield_uw.get_string(metadata_key_idx).expect("error during key read");
    //     let value_value = mdfield_uw
    //         .get_string(metadata_value_idx)
    //         .expect("error during value read");
    //     println!("key: {}, value: {}", key_value, value_value);
    // }
}

fn main_real() {
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
            let event = match debugger_proc_copy.wait_next_event(false) {
                Ok(v) => v,
                Err(_) => {
                    println!("error while reading next debugger event");
                    return;
                }
            };

            let _ = dbg_tx.send(event);
        }
    });

    thread::spawn(move || {
        loop {
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();

            let _ = inp_tx.send(input.trim().to_string());
        }
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

                    if args.len() > 2 {
                        match u64::from_str_radix(args[2], 16) {
                            Ok(v) => disasm_at_addr(&*debugger, v, len),
                            Err(_) => disasm_at_pc(&*debugger, &pc_reg, len),
                        };
                    } else {
                        disasm_at_pc(&*debugger, &pc_reg, len);
                    }

                    last_disasm_len = len;
                } else if cmd == "mem" {
                    if args.len() < 3 {
                        println!("incorrect arguments");
                    } else {
                        let byte_count = match i32::from_str_radix(args[1], 10) {
                            Ok(v) => v,
                            Err(_) => 10,
                        };
                        let addr: Option<u64> = match u64::from_str_radix(args[2], 16) {
                            Ok(v) => Some(v),
                            Err(_) => None,
                        };
                        if addr.is_none() {
                            println!("incorrect arguments");
                        } else {
                            let mut out_data = vec![0u8; byte_count as usize];
                            match debugger.read_bytes(DebuggerThreadIndex::Current, addr.unwrap(), &mut out_data) {
                                Ok(_) => {
                                    for i in 0..byte_count as usize {
                                        print!("{} ", u8_to_str_fast(out_data[i]));
                                    }
                                    println!("");
                                }
                                Err(e) => {
                                    println!("failed to read data: {}", e);
                                }
                            }
                        }
                    }
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
