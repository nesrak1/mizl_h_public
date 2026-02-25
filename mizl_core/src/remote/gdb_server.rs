use crate::{
    debugger::debugger::{Debugger, DebuggerEvent},
    shared::fast_util::nibble_to_u8_fast,
};
use crossbeam::{channel::unbounded, select};
use std::{
    io::{BufReader, Error, Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
};

// we don't support extended remote yet, so this will spawn
// up the server and debugger immediately on construction

pub enum GdbServerError {
    IoError(Error),
    GenericError(String),
}

impl From<Error> for GdbServerError {
    fn from(err: Error) -> Self {
        GdbServerError::IoError(err)
    }
}

enum GdbCmdState {
    WaitStart,          // waiting for $
    Normal,             // normal parsing
    Escape,             // escape } read
    ChecksumStart,      // checksum # read
    ChecksumNextNibble, // first checksum nibble read
    RunLengthEncoding,  // rle * read
}

pub struct GdbServer<DBG: Debugger> {
    bin_path: String,
    bin_args: Vec<String>,
    listener: TcpListener,
    debugger: Arc<DBG>,
}

impl<DBG: Debugger + Send + Sync + 'static> GdbServer<DBG> {
    pub fn new(
        path: String,
        args: &[&str],
        debugger: DBG,
        ip: String,
        port: u16,
    ) -> Result<GdbServer<DBG>, GdbServerError> {
        let listener = TcpListener::bind(format!("{}:{}", ip, port))?;
        let debugger = Arc::new(debugger);
        Ok(GdbServer {
            bin_path: path,
            bin_args: args.iter().map(|x| x.to_string()).collect(),
            listener,
            debugger,
        })
    }

    pub fn run_one(&self) -> Result<(), GdbServerError> {
        let (stream, _addr) = self.listener.accept()?;

        let (dbg_tx, dbg_rx) = unbounded::<DebuggerEvent>();
        let (cli_tx, cli_rx) = unbounded::<String>();
        let (errd_tx, errd_rx) = unbounded::<String>(); // fatal errors only
        let (errc_tx, errc_rx) = unbounded::<String>(); // fatal errors only

        let stream_read_copy = stream.try_clone()?;
        let mut stream_write_copy = stream.try_clone()?;
        let debugger_proc_copy = Arc::clone(&self.debugger);

        let bin_path_copy = self.bin_path.clone();
        let bin_args_copy: Vec<String> = self.bin_args.iter().map(|s| s.clone()).collect();

        let _dbg_thread = thread::spawn(move || {
            let bin_args_str: Vec<&str> = bin_args_copy.iter().map(|s| s.as_str()).collect();
            match debugger_proc_copy.run(&bin_path_copy, &bin_args_str) {
                Ok(_) => {}
                Err(e) => {
                    errd_tx.send(format!("binary could not be run: {}", e)).unwrap();
                    return;
                }
            };

            loop {
                let event = match debugger_proc_copy.wait_next_event(false) {
                    Ok(v) => v,
                    Err(e) => {
                        errd_tx
                            .send(format!("could not read next event (debugger may have died): {}", e))
                            .unwrap();
                        return;
                    }
                };

                let _ = dbg_tx.send(event);
            }
        });

        let _cmd_thread = thread::spawn(move || {
            let mut reader = BufReader::new(stream_read_copy);

            let mut cmd = String::new();

            let mut last_char: char = '\0';
            let mut our_checksum: u8 = 0;
            let mut cli_checksum: u8 = 0;
            let mut state = GdbCmdState::WaitStart;

            loop {
                let mut buf = [0u8; 1024];
                let read_len = match reader.read(&mut buf) {
                    Ok(v) => v,
                    Err(e) => {
                        errc_tx.send(format!("failed to read from client: {}", e)).unwrap();
                        return;
                    }
                };

                for i in 0..read_len {
                    let b = buf[i];
                    match state {
                        GdbCmdState::WaitStart => {
                            // wait until first character is $
                            if b == '$' as u8 {
                                state = GdbCmdState::Normal;
                                our_checksum = 0;
                            }
                        }
                        GdbCmdState::Normal => {
                            if b == '}' as u8 {
                                state = GdbCmdState::Escape;
                                our_checksum = our_checksum.wrapping_add(b);
                            } else if b == '#' as u8 {
                                state = GdbCmdState::ChecksumStart;
                                // not included in checksum
                            } else if b == '*' as u8 {
                                state = GdbCmdState::RunLengthEncoding;
                                our_checksum = our_checksum.wrapping_add(b);
                            } else if b == '$' as u8 {
                                // this is not allowed, but we'll treat it as
                                // the start of a new message and discard
                                // the old one.
                                state = GdbCmdState::Normal;
                                our_checksum = 0;

                                // reset cmd memory if it was too long
                                cmd.clear();
                                if cmd.len() > 4096 {
                                    cmd.shrink_to(1024);
                                }
                            } else {
                                last_char = b as char;
                                cmd.push(last_char);
                                our_checksum = our_checksum.wrapping_add(b);
                            }
                        }
                        GdbCmdState::Escape => {
                            last_char = (b ^ 0x20) as char;
                            cmd.push(last_char);
                            state = GdbCmdState::Normal;
                            our_checksum = our_checksum.wrapping_add(b);
                        }
                        GdbCmdState::ChecksumStart => {
                            cli_checksum = match nibble_to_u8_fast(b) {
                                Some(v) => v << 4,
                                None => {
                                    // not valid hex, drop this packet
                                    state = GdbCmdState::WaitStart;
                                    continue;
                                }
                            };

                            state = GdbCmdState::ChecksumNextNibble;
                        }
                        GdbCmdState::ChecksumNextNibble => {
                            cli_checksum |= match nibble_to_u8_fast(b) {
                                Some(v) => v,
                                None => {
                                    // not valid hex, drop this packet
                                    state = GdbCmdState::WaitStart;
                                    continue;
                                }
                            };

                            if cli_checksum != our_checksum {
                                // checksum mismatch, drop this packet
                                state = GdbCmdState::WaitStart;
                                continue;
                            }

                            cli_tx.send(cmd.clone()).unwrap();

                            // reset cmd memory if it was too long
                            if cmd.len() <= 4096 {
                                cmd.clear();
                            } else {
                                cmd = String::new();
                            }
                        }
                        GdbCmdState::RunLengthEncoding => {
                            if cmd.len() == 0 {
                                // no characters to copy from, drop this packet
                                state = GdbCmdState::WaitStart;
                                continue;
                            } else if b == '#' as u8 || b == '$' as u8 {
                                // invalid characters, drop this packet
                                state = GdbCmdState::WaitStart;
                                continue;
                            } else if b < ' ' as u8 || b > 126 {
                                // invalid range, drop this packet
                                state = GdbCmdState::WaitStart;
                                continue;
                            }

                            let copy_count = (b - 29) as usize;
                            cmd.extend(std::iter::repeat(last_char).take(copy_count));
                        }
                    };
                }
            }
        });

        loop {
            select! {
                recv(dbg_rx) -> msg => {
                    // todo: everything
                    let msg = msg.unwrap();
                    println!("{}", msg.kind);
                    //self.send_or_print_err(&mut stream_write_copy, format!("{}", msg.kind));
                }
                recv(cli_rx) -> msg => {
                    let ack_bytes = ['+' as u8];
                    stream_write_copy.write(&ack_bytes).expect("TODO: failed to write ack");

                    let msg = msg.unwrap();
                    self.process_cli_msg(msg);
                }
                recv(errd_rx) -> msg => {
                    // unrecoverable error causes return
                    return Err(GdbServerError::GenericError(msg.unwrap()));
                }
                recv(errc_rx) -> msg => {
                    // unrecoverable error causes return
                    return Err(GdbServerError::GenericError(msg.unwrap()));
                }
            }
        }
    }

    fn send_or_print_err(&self, tcp_stream: &mut TcpStream, err: String) {
        // match tcp_stream.write_fmt(format_args!("E.{}", err)) {
        //     Ok(_) => {}
        //     Err(_) => {
        //         // if we can't send an error to remote, print to screen.
        //         // todo: should be configurable, and probably allow for
        //         // printing to screen regardless if remote can see it.
        //         println!("{}", err);
        //     }
        // }
    }

    fn process_cli_msg(&self, cmd: String) {
        if cmd.len() == 0 {
            // invalid packet, guess we'll do nothing
            return;
        }

        let kind = cmd.chars().next().unwrap();
        match kind {
            'q' => {
                // query
            }
            _ => {}
        }
    }
}
