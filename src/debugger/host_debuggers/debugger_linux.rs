use super::{debugger_linux_memview::DebuggerLinuxMemView, debugger_linux_superpt as superpt};
use crate::{
    debugger::{
        breakpoint::{BreakpointContainer, BreakpointEntry, BreakpointWrapMemView},
        chunked_free_memview::ChunkedFreeMemView,
        debugger::{Debugger, DebuggerError, DebuggerEvent, DebuggerEventKind, DebuggerFlags, DebuggerThreadIndex},
        host_debugger_infos::{
            regmap_arch::ArchNativeRegisterInfo, regmap_arch_amd64::RegSrcAmd64, regmap_os_natreg::get_regmap_entries,
        },
        host_debuggers::debugger_linux_sighandler::sigchld_register,
        registers::registers::{NativeRegisterInfo, RegisterInfo},
    },
    memory::memview::MemView,
    sleigh::{
        disasm::{Disasm, DisasmDispInstruction},
        pspec_file::Pspec,
        sla_file::Sleigh,
    },
};
use crossbeam::channel::{bounded, Receiver, Sender};
use libc;
use std::{
    collections::HashMap,
    ffi::CString,
    fs,
    ops::DerefMut,
    path::Path,
    sync::{Arc, Mutex, MutexGuard, RwLock},
    thread::{self, ThreadId},
};

struct DebuggerLinuxThread {
    pid: i32,
    _paused: bool,
    proc_mem: DebuggerLinuxMemView,
    reg_mem: ChunkedFreeMemView,
}

enum DebuggerLinuxCmdReqOp {
    SingleStep(DebuggerThreadIndex),
    ContinueOne(DebuggerThreadIndex),
    Continue,
    DisasmOne(u64),
    LoadRegCache(i32),
    // ...
}

enum DebuggerLinuxCmdRspOp {
    Error(DebuggerError),
    Success,
    ResultDisasmOne(DisasmDispInstruction),
}

struct DebuggerLinuxState {
    // the "current" thread which is really just a convenience thing.
    // it's normally the last stopped thread unless the user switched.
    cur_thread_pid: Option<i32>,
    // only one thread can step at a time, so this is fine to be on state.
    // todo: we should actually enforce this rule
    stepping_thread_pid: Option<i32>,
    threads: HashMap<i32, DebuggerLinuxThread>,
    bp_cont: BreakpointContainer,
    reg_mem_dirty: bool,
    pending_events: Vec<libc::epoll_event>,
}

struct DebuggerLinuxChannelContainer {
    // cmd thread -> dbg thread
    cmd_req_tx: Sender<DebuggerLinuxCmdReqOp>,
    cmd_req_rx: Receiver<DebuggerLinuxCmdReqOp>,
    // dbg thread -> cmd thread
    cmd_rsp_tx: Sender<DebuggerLinuxCmdRspOp>,
    cmd_rsp_rx: Receiver<DebuggerLinuxCmdRspOp>,
    // epoll/action/sigchld -> dbg thread
    epoll_fd: i32,
    action_fd: i32,
    sigchld_fd: i32,
}

struct DebuggerLinuxSessionState {
    dbg_thread_id: ThreadId,
    chan_cont: DebuggerLinuxChannelContainer,
}

pub struct DebuggerLinux {
    // set on startup
    disasm: Disasm,
    nat_reg_info: ArchNativeRegisterInfo,
    // configured when process is actually loaded
    state: Arc<Mutex<DebuggerLinuxState>>,
    session_state: RwLock<Option<DebuggerLinuxSessionState>>,
}

impl DebuggerLinuxThread {
    pub fn new(pid: i32) -> DebuggerLinuxThread {
        let proc_mem = DebuggerLinuxMemView::new(pid);
        let reg_mem = ChunkedFreeMemView::new(64);
        DebuggerLinuxThread {
            pid,
            _paused: false,
            proc_mem,
            reg_mem,
        }
    }
}

impl DebuggerLinuxChannelContainer {
    pub fn new(epoll_fd: i32, action_fd: i32, sigchld_fd: i32) -> DebuggerLinuxChannelContainer {
        let (cmd_req_tx, cmd_req_rx) = bounded(1);
        let (cmd_rsp_tx, cmd_rsp_rx) = bounded(1);
        DebuggerLinuxChannelContainer {
            cmd_req_tx,
            cmd_req_rx,
            cmd_rsp_tx,
            cmd_rsp_rx,
            epoll_fd,
            action_fd,
            sigchld_fd,
        }
    }
}

impl DebuggerLinuxSessionState {
    pub fn new(dbg_thread_id: ThreadId, chan_cont: DebuggerLinuxChannelContainer) -> DebuggerLinuxSessionState {
        DebuggerLinuxSessionState {
            dbg_thread_id,
            chan_cont,
        }
    }
}

impl DebuggerLinux {
    pub fn new() -> DebuggerLinux {
        let disasm: Disasm = Self::setup_disasm();
        let nat_reg_info = ArchNativeRegisterInfo::new(&disasm.sleigh);
        let state = Arc::new(Mutex::new(DebuggerLinuxState {
            cur_thread_pid: None,
            stepping_thread_pid: None,
            threads: HashMap::new(),
            bp_cont: BreakpointContainer::new(),
            reg_mem_dirty: true,
            pending_events: Vec::new(),
        }));
        DebuggerLinux {
            disasm,
            nat_reg_info,
            state,
            session_state: RwLock::new(None),
        }
    }

    fn setup_disasm() -> Disasm {
        let sla_data: Vec<u8>;
        let pspec_data: String;
        if cfg!(target_arch = "x86_64") {
            sla_data = fs::read("x86-64.sla").expect("can't read sla");
            pspec_data = fs::read_to_string("x86-64.pspec").expect("can't read pspec");
        } else {
            unimplemented!()
        }

        let sleigh = Sleigh::new(&sla_data);
        let pspec = Pspec::new(pspec_data).expect("error in pspec");

        let initial_ctx = pspec.get_initial_ctx(&sleigh).expect("error in pspec");
        Disasm::new(sleigh, initial_ctx)
    }

    fn get_thread_pid_or_current(
        state: &DebuggerLinuxState,
        thread_idx: DebuggerThreadIndex,
    ) -> Result<i32, DebuggerError> {
        match thread_idx {
            DebuggerThreadIndex::Current => state.cur_thread_pid.ok_or(DebuggerError::NoThreads),
            DebuggerThreadIndex::Specific(i) => Ok(i as i32),
        }
    }

    fn is_debugger_thread(&self) -> bool {
        let sstate_opt_guard = self.session_state.read().unwrap();
        let sstate_opt = sstate_opt_guard.as_ref();
        match sstate_opt {
            Some(sstate) => thread::current().id() == sstate.dbg_thread_id,
            None => false,
        }
    }

    // runs in: dbg thread
    fn load_reg_cache(&self, state: &mut DebuggerLinuxState, thread_pid: i32) -> Result<(), DebuggerError> {
        let thread_mut = state.threads.get_mut(&thread_pid).ok_or(DebuggerError::InvalidThread)?;

        let reg_data = superpt::getregs(thread_mut.pid);
        let fpreg_data = superpt::getfpregs(thread_mut.pid);

        for item in get_regmap_entries() {
            let src_bytes_start = item.native_off;
            let src_bytes_end = src_bytes_start + item.size as usize;
            let src_bytes: &[u8];
            src_bytes = match item.source {
                x if x == RegSrcAmd64::Standard as i32 => &reg_data[src_bytes_start..src_bytes_end],
                x if x == RegSrcAmd64::FloatingPoint as i32 => &fpreg_data[src_bytes_start..src_bytes_end],
                _ => unimplemented!(),
            };

            let reg_info = match self.nat_reg_info.get_host_info(item.reg_idx) {
                Some(v) => v,
                None => return Err(DebuggerError::InternalError),
            };

            // max means we have no idea where this is in sleigh space.
            // don't try to guess since we would've had a unique addr
            // assigned even if it was not in the sleigh.
            if reg_info.addr == u32::MAX {
                continue;
            }

            let mut dst_addr = reg_info.addr as u64;
            thread_mut
                .reg_mem
                .write_bytes(&mut dst_addr, &src_bytes)
                .or(Err(DebuggerError::InvalidRegister))?;
        }

        state.reg_mem_dirty = false;
        Ok(())
    }

    // runs in: dbg thread
    fn step_impl(
        &self,
        mut state: MutexGuard<'_, DebuggerLinuxState>,
        thread_idx: DebuggerThreadIndex,
    ) -> Result<(), DebuggerError> {
        let thread_pid = Self::get_thread_pid_or_current(&state, thread_idx)?;
        {
            state.reg_mem_dirty = true;

            // when the user thread continues before receiving a trap,
            // call singlestep again rather than continue. once we hit
            // the trap we were expecting, switch back to cont.
            state.stepping_thread_pid = Some(thread_pid);
        }
        std::mem::drop(state); // unlock state

        superpt::singlestep(thread_pid);
        Ok(())
    }

    // runs in: dbg thread
    fn cont_one_impl(
        &self,
        mut state: MutexGuard<'_, DebuggerLinuxState>,
        thread_idx: DebuggerThreadIndex,
    ) -> Result<(), DebuggerError> {
        let thread_pid = Self::get_thread_pid_or_current(&state, thread_idx)?;
        {
            state.reg_mem_dirty = true;
        }
        std::mem::drop(state); // unlock state

        superpt::cont(thread_pid);
        Ok(())
    }

    // runs in: dbg thread
    fn cont_impl(&self, mut state: MutexGuard<'_, DebuggerLinuxState>) -> Result<(), DebuggerError> {
        let mut thread_pids: Vec<i32>;
        {
            state.reg_mem_dirty = true;
            thread_pids = Vec::with_capacity(state.threads.len());
            for (pid, _) in &state.threads {
                thread_pids.push(*pid);
            }
        }
        std::mem::drop(state); // unlock state

        for thread_pid in thread_pids {
            superpt::cont(thread_pid);
        }
        Ok(())
    }

    // runs in: dbg thread (or cmd thread assuming we checked /proc/mem)
    fn disassemble_one_impl(
        &self,
        mut state_guard: MutexGuard<'_, DebuggerLinuxState>,
        addr: u64,
    ) -> Result<DisasmDispInstruction, DebuggerError> {
        let disasm = &self.disasm;
        let state = state_guard.deref_mut();
        let cur_thread_pid = state.cur_thread_pid.ok_or(DebuggerError::NoThreads)?;
        let thread = state
            .threads
            .get_mut(&cur_thread_pid)
            .ok_or(DebuggerError::InvalidThread)?;

        let display_ins: DisasmDispInstruction;
        {
            // temporary wrapper to patch breakpoint bytes
            let mem_bp_wrapped = BreakpointWrapMemView {
                mem_view: &mut thread.proc_mem,
                bp_cont: &state.bp_cont,
            };
            display_ins = disasm
                .disasm_display(&mem_bp_wrapped, addr)
                .or(Err(DebuggerError::DisassemblyFailed))?;
        }

        Ok(display_ins)
    }

    fn read_register_final(
        state: &mut DebuggerLinuxState,
        thread_pid: i32,
        reg_start: u64,
        out_data: &mut [u8],
        read_size: i32,
    ) -> Result<(), DebuggerError> {
        let thread = state.threads.get(&thread_pid).ok_or(DebuggerError::InvalidThread)?;
        let mut reg_start_mut = reg_start;
        thread
            .reg_mem
            .read_bytes(&mut reg_start_mut, out_data, read_size)
            .or(Err(DebuggerError::InvalidRegister))?;

        Ok(())
    }

    // runs in: cmd thread
    fn send_cmd_req(&self, req_op: DebuggerLinuxCmdReqOp) -> DebuggerLinuxCmdRspOp {
        // rwlock, no need to drop
        let sstate_opt_guard = self.session_state.read().unwrap();
        let sstate_opt = sstate_opt_guard.as_ref();
        let sstate = match sstate_opt {
            Some(sstate) => sstate,
            None => return DebuggerLinuxCmdRspOp::Error(DebuggerError::NoThreads),
        };

        let chan_cont = &sstate.chan_cont;
        chan_cont.cmd_req_tx.send(req_op).unwrap();

        let data = [0x2F2F2F2F2F2F2F2Fu64; 1];
        unsafe {
            libc::write(chan_cont.action_fd, &data as *const u64 as *const libc::c_void, 8);
        }

        chan_cont.cmd_rsp_rx.recv().unwrap()
    }
}

impl Debugger for DebuggerLinux {
    fn is_big_endian(&self) -> bool {
        false
    }

    fn get_flags(&self) -> DebuggerFlags {
        todo!();
    }

    fn set_flags(&self, _flags: DebuggerFlags) -> Result<(), DebuggerError> {
        todo!();
    }

    // runs in: dbg thread
    fn run(&self, path: &str, args: &[&str]) -> Result<i32, DebuggerError> {
        // strip null bytes (this should probably be an error later)
        let cstr_prog = CString::new(path.replace("\0", "")).unwrap();
        let mut cstr_argv: Vec<_> = args
            .iter()
            .map(|arg| CString::new((*arg).replace("\0", "")).unwrap())
            .collect();

        // consumer really was supposed to provide executable as first argument, so let's fix that
        if cstr_argv.len() == 0 {
            // the OsStr conversion and unwrap is a bit icky to me but not sure what to do
            let path_nonb = path.replace("\0", "");
            let name_nonb = Path::new(path)
                .file_name()
                .and_then(|os_str| os_str.to_str())
                .unwrap_or(path_nonb.as_str());

            let cstr_arg0 = CString::new(name_nonb).unwrap();
            cstr_argv.push(cstr_arg0);
        }

        // need to make a new list of just ptrs to the previous list, otherwise they go out of
        // scope which isn't what we want
        let mut ptr_argv: Vec<_> = cstr_argv.iter().map(|arg| arg.as_ptr()).collect();

        // null terminating argument
        ptr_argv.push(std::ptr::null());

        // do the fork now
        let fork_id = unsafe { libc::fork() };
        if fork_id == -1 {
            return Err(DebuggerError::ForkFailed);
        }

        if fork_id == 0 {
            // child
            superpt::traceme();

            unsafe {
                // handle errors: https://stackoverflow.com/a/1586277
                // some debuggers may use error codes like 127 or but we
                // wouldn't know whether our code that returned the error...
                let _ = libc::execv(cstr_prog.as_ptr(), ptr_argv.as_ptr());
                libc::_exit(0);
            }
        } else {
            // parent

            // the setup for creating a new thread requires us to wait here.
            // todo: we should check the status of this
            _ = superpt::waitpid(fork_id);

            // set up events to notify wait_next_event
            // todo: this is kinda nasty. we should have something to
            // automatically close/unset whatever we drop the object.
            // todo: check statuses
            let epoll_fd: i32;
            let action_fd: i32;
            let sigchld_fd: i32;
            unsafe {
                // setup epoll
                epoll_fd = libc::epoll_create1(0);
                if epoll_fd < 0 {
                    return Err(DebuggerError::InternalError);
                }

                // setup action eventfd
                action_fd = libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK);
                if action_fd < 0 {
                    libc::close(epoll_fd);
                    return Err(DebuggerError::InternalError);
                }

                // at first glance, signalfd should be perfect for epolling for either
                // a user action or a SIGCHLD signal. we just have to epoll for it,
                // plus we get a free signalfd_siginfo object that tells us about why
                // we stopped, right? unfortunately, linux (or rather, unix systems).
                // in order for signalfd to work correctly, ALL threads in the process
                // must have SIGCHLD blocked. if even one thread has it unblocked,
                // that thread will be assigned to take care of the signal (which
                // probably means doing nothing but discarding signal.) even if we get
                // the epoll event, signalfd most likely has been consumed and we'll
                // either block on read or skip it if EFD_NONBLOCK is set. since we
                // can't guarantee the consumer can even block the signal in every
                // thread (i.e., any language with a runtime), this won't work.
                // the alternative? global signal handlers, yay! now we will have to
                // keep track of every signal handler that's been registered. and we
                // better hope nobody else wants to handle SIGCHLD.
                sigchld_fd = libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK);
                if sigchld_fd < 0 {
                    libc::close(action_fd);
                    libc::close(epoll_fd);
                    return Err(DebuggerError::InternalError);
                }

                // register handler now
                sigchld_register(sigchld_fd);

                // // this doesn't really work, see above comment block
                // let mut mask: libc::sigset_t = std::mem::zeroed();
                // libc::sigemptyset(&mut mask);
                // libc::sigaddset(&mut mask, libc::SIGCHLD);
                // //libc::pthread_sigmask(libc::SIG_BLOCK, &mut mask, std::ptr::null_mut());
                // sigchld_fd = libc::signalfd(-1, &mask, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK);
                // if sigchld_fd < 0 {
                //     libc::close(epoll_fd);
                //     libc::pthread_sigmask(libc::SIG_UNBLOCK, &mut mask, std::ptr::null_mut());
                //     return Err(DebuggerError::InternalError);
                // }

                // add both fds to epoll
                let mut action_evt = libc::epoll_event {
                    events: libc::EPOLLIN as u32,
                    u64: action_fd as u64,
                };
                let mut sigchld_evt = libc::epoll_event {
                    events: libc::EPOLLIN as u32,
                    u64: sigchld_fd as u64,
                };
                libc::epoll_ctl(epoll_fd, libc::EPOLL_CTL_ADD, action_fd, &mut action_evt);
                libc::epoll_ctl(epoll_fd, libc::EPOLL_CTL_ADD, sigchld_fd, &mut sigchld_evt);
            }

            let mut state = self.state.lock().unwrap();
            state.threads.insert(fork_id, DebuggerLinuxThread::new(fork_id));
            state.cur_thread_pid = Some(fork_id);
            {
                let mut sstate_opt = self.session_state.write().unwrap();
                let chan_cont = DebuggerLinuxChannelContainer::new(epoll_fd, action_fd, sigchld_fd);
                let sstate = DebuggerLinuxSessionState::new(thread::current().id(), chan_cont);
                *sstate_opt = Some(sstate);
            }

            Ok(fork_id)
        }
    }

    // runs in: dbg thread
    fn wait_next_event(&self) -> Result<DebuggerEvent, DebuggerError> {
        enum SelectResult {
            ActionEvent(DebuggerLinuxCmdReqOp),
            UserIdEvent(i32),
            ChildEvent,
        }

        // these should not change while the program is running. if the program stops
        // and the fds change, an event should fire to pull us out of this loop.

        let sstate_opt_guard = self.session_state.read().unwrap();
        let sstate_opt = sstate_opt_guard.as_ref();
        let sstate = match sstate_opt {
            Some(sstate) => sstate,
            None => return Err(DebuggerError::NoThreads),
        };

        let chan_cont = &sstate.chan_cont;
        let epoll_fd = chan_cont.epoll_fd;
        let action_fd = chan_cont.action_fd;
        let sigchld_fd = chan_cont.sigchld_fd;

        let mut events: [libc::epoll_event; 32] = unsafe { std::mem::zeroed() };
        let mut event_count: usize;
        // if we enter the wait function with pending events, put them in the queue now
        {
            let mut state = self.state.lock().unwrap();
            event_count = 0;
            for pending_event in &state.pending_events {
                events[event_count] = pending_event.clone();
                event_count += 1;
            }
            state.pending_events.clear();
        }
        loop {
            // if we had no pending events, wait until we get more
            if event_count == 0 {
                unsafe {
                    for i in 0..32 {
                        events[i] = std::mem::zeroed();
                    }
                    loop {
                        let res: i32 = libc::epoll_wait(epoll_fd, events.as_mut_ptr(), 32, -1);
                        if res < 0 {
                            if *libc::__errno_location() == libc::EINTR {
                                // expected if our thread does the signal handling
                                continue;
                            }
                        } else {
                            event_count = res as usize;
                        }
                        break;
                    }
                }
            }

            let mut cur_event_idx = 0;
            while cur_event_idx < event_count {
                let evt = &events[cur_event_idx];
                let res: SelectResult;
                cur_event_idx += 1;

                let pid = evt.u64 as i32;
                if pid == action_fd {
                    let mut data = [0u64; 1];
                    unsafe {
                        libc::read(action_fd, &mut data as *mut u64 as *mut libc::c_void, 8);
                    }

                    let req = chan_cont.cmd_req_rx.recv().or(Err(DebuggerError::InternalError))?;
                    res = SelectResult::ActionEvent(req);
                } else if pid == sigchld_fd {
                    let mut data = [0u64; 1];
                    unsafe {
                        libc::read(sigchld_fd, &mut data as *mut u64 as *mut libc::c_void, 8);
                    }

                    res = SelectResult::ChildEvent;
                } else {
                    res = SelectResult::UserIdEvent(pid);
                }

                match res {
                    SelectResult::ActionEvent(req) => {
                        // non-dbg thread asking us to perform action
                        match req {
                            DebuggerLinuxCmdReqOp::SingleStep(thread_idx) => {
                                let state = self.state.lock().unwrap();
                                let rsp = match self.step_impl(state, thread_idx) {
                                    Ok(_) => DebuggerLinuxCmdRspOp::Success,
                                    Err(e) => DebuggerLinuxCmdRspOp::Error(e),
                                };
                                chan_cont.cmd_rsp_tx.send(rsp).unwrap();
                            }
                            DebuggerLinuxCmdReqOp::ContinueOne(thread_idx) => {
                                let state = self.state.lock().unwrap();
                                let rsp = match self.cont_one_impl(state, thread_idx) {
                                    Ok(_) => DebuggerLinuxCmdRspOp::Success,
                                    Err(e) => DebuggerLinuxCmdRspOp::Error(e),
                                };
                                chan_cont.cmd_rsp_tx.send(rsp).unwrap();
                            }
                            DebuggerLinuxCmdReqOp::Continue => {
                                let state = self.state.lock().unwrap();
                                let rsp = match self.cont_impl(state) {
                                    Ok(_) => DebuggerLinuxCmdRspOp::Success,
                                    Err(e) => DebuggerLinuxCmdRspOp::Error(e),
                                };
                                chan_cont.cmd_rsp_tx.send(rsp).unwrap();
                            }
                            DebuggerLinuxCmdReqOp::DisasmOne(addr) => {
                                let state = self.state.lock().unwrap();
                                let rsp = match self.disassemble_one_impl(state, addr) {
                                    Ok(inst) => DebuggerLinuxCmdRspOp::ResultDisasmOne(inst),
                                    Err(e) => DebuggerLinuxCmdRspOp::Error(e),
                                };
                                chan_cont.cmd_rsp_tx.send(rsp).unwrap();
                            }
                            DebuggerLinuxCmdReqOp::LoadRegCache(thread_pid) => {
                                let mut state = self.state.lock().unwrap();
                                let rsp = match self.load_reg_cache(&mut state, thread_pid) {
                                    Ok(_) => DebuggerLinuxCmdRspOp::Success,
                                    Err(e) => DebuggerLinuxCmdRspOp::Error(e),
                                };
                                chan_cont.cmd_rsp_tx.send(rsp).unwrap();
                            }
                        }
                    }
                    SelectResult::ChildEvent => {
                        // sigchild event, handle waitpid
                        loop {
                            // this is in a loop because we may not want to report
                            // every event we receive back. obviously, that's not
                            // the case right now but it's very likely to happen
                            // at some point.
                            let (status, pid) = superpt::waitpid_nohang(-1);
                            if pid <= 0 {
                                // escape if waitpid failed
                                break;
                            } else if libc::WIFSTOPPED(status) {
                                {
                                    let mut state = self.state.lock().unwrap();
                                    if let Some(stepping_thread_pid) = state.stepping_thread_pid {
                                        if stepping_thread_pid == pid {
                                            state.stepping_thread_pid = None;
                                        }
                                    }
                                }
                                let siginfo = superpt::getsiginfo(pid);
                                let result: DebuggerEvent;
                                result = if cfg!(target_arch = "x86_64") {
                                    // these are scrambled because reasons
                                    match siginfo.si_code {
                                        libc::SI_KERNEL => {
                                            DebuggerEvent::new(DebuggerEventKind::BreakpointHit, status as u32)
                                        }
                                        libc::TRAP_BRKPT => {
                                            DebuggerEvent::new(DebuggerEventKind::StepCompleteSyscall, status as u32)
                                        }
                                        libc::TRAP_TRACE => {
                                            DebuggerEvent::new(DebuggerEventKind::StepComplete, status as u32)
                                        }
                                        _ => {
                                            DebuggerEvent::new(DebuggerEventKind::MiscSignalReceived, status as u32)
                                        }
                                    }
                                } else {
                                    match siginfo.si_code {
                                        libc::SI_KERNEL => {
                                            DebuggerEvent::new(DebuggerEventKind::StepComplete, status as u32)
                                        }
                                        libc::TRAP_BRKPT => {
                                            DebuggerEvent::new(DebuggerEventKind::BreakpointHit, status as u32)
                                        }
                                        libc::TRAP_TRACE => {
                                            DebuggerEvent::new(DebuggerEventKind::MiscSignalReceived, status as u32)
                                        }
                                        _ => {
                                            DebuggerEvent::new(DebuggerEventKind::MiscSignalReceived, status as u32)
                                        }
                                    }
                                };
                                
                                // temporary. really silly we lock/unlock in a loop but remember, it's temporary.
                                while cur_event_idx < event_count {
                                    let mut state = self.state.lock().unwrap();
                                    state.pending_events.push(events[cur_event_idx].clone());
                                    cur_event_idx += 1;
                                }
                                return Ok(result);
                            } else {
                                while cur_event_idx < event_count {
                                    let mut state = self.state.lock().unwrap();
                                    state.pending_events.push(events[cur_event_idx].clone());
                                    cur_event_idx += 1;
                                }
                                return Ok(DebuggerEvent::new(DebuggerEventKind::UnknownEvent, status as u32));
                            }
                        }
                    }
                    SelectResult::UserIdEvent(user_id) => {
                        return Ok(DebuggerEvent::new(DebuggerEventKind::UserEvent, user_id as u32));
                    }
                };
            }

            event_count = 0;
        }
    }

    fn add_event_id(&self, id: u32) -> Result<(), DebuggerError> {
        let sstate_opt_guard = self.session_state.read().unwrap();
        let sstate_opt = sstate_opt_guard.as_ref();
        let sstate = match sstate_opt {
            Some(sstate) => sstate,
            None => return Err(DebuggerError::NoThreads),
        };

        let mut custom_evt = libc::epoll_event {
            events: libc::EPOLLIN as u32,
            u64: id as u64,
        };

        let epoll_fd = sstate.chan_cont.epoll_fd;
        unsafe {
            libc::epoll_ctl(epoll_fd, libc::EPOLL_CTL_ADD, id as i32, &mut custom_evt);
        }

        Ok(())
    }

    fn remove_event_id(&self, id: u32) -> Result<(), DebuggerError> {
        let sstate_opt_guard = self.session_state.read().unwrap();
        let sstate_opt = sstate_opt_guard.as_ref();
        let sstate = match sstate_opt {
            Some(sstate) => sstate,
            None => return Err(DebuggerError::NoThreads),
        };

        let epoll_fd = sstate.chan_cont.epoll_fd;
        unsafe {
            libc::epoll_ctl(epoll_fd, libc::EPOLL_CTL_DEL, id as i32, std::ptr::null_mut());
        }

        Ok(())
    }

    // runs in: cmd thread
    // todo: should take thread idx
    fn disassemble_one(&self, addr: u64) -> Result<DisasmDispInstruction, DebuggerError> {
        let mut state_guard = self.state.lock().unwrap();
        let state = state_guard.deref_mut();
        let cur_thread_pid = state.cur_thread_pid.ok_or(DebuggerError::NoThreads)?;
        let thread = state
            .threads
            .get_mut(&cur_thread_pid)
            .ok_or(DebuggerError::InvalidThread)?;

        if thread.proc_mem.is_using_proc_mem() || self.is_debugger_thread() {
            // don't need to send to other debugger thread if we're using
            // /proc/[pid]/mem instead of ptrace which doesn't have to be on
            // dbg thread. if we're on dbg thread, that works too.
            return self.disassemble_one_impl(state_guard, addr);
        } else {
            match self.send_cmd_req(DebuggerLinuxCmdReqOp::DisasmOne(addr)) {
                DebuggerLinuxCmdRspOp::ResultDisasmOne(inst) => return Ok(inst),
                DebuggerLinuxCmdRspOp::Error(e) => return Err(e),
                _ => return Err(DebuggerError::InternalError),
            }
        }
    }

    fn get_register_infos(&self, _: DebuggerThreadIndex) -> Vec<&RegisterInfo> {
        self.nat_reg_info.get_all_infos()
    }

    // runs in: cmd thread
    fn read_register_by_idx_buf(
        &self,
        thread_idx: DebuggerThreadIndex,
        reg_idx: i32,
        out_data: &mut [u8],
    ) -> Result<(), DebuggerError> {
        let mut state = self.state.lock().unwrap();
        let reg_mem_dirty = state.reg_mem_dirty;

        let reg_info = self
            .nat_reg_info
            .get_host_info(reg_idx)
            .ok_or(DebuggerError::InvalidRegister)?;

        let reg_start = reg_info.addr as u64;
        let reg_size = (reg_info.bit_len + 7) / 8 as i32;

        let size = out_data.len();
        // prevent reading more bytes than possible
        if size > i32::MAX as usize || size < (reg_size as usize) {
            return Err(DebuggerError::InvalidRegister);
        }

        let read_size = (size as i32).min(reg_size);
        let use_thread_pid = Self::get_thread_pid_or_current(&state, thread_idx)?;
        if reg_mem_dirty {
            if self.is_debugger_thread() {
                self.load_reg_cache(&mut state, use_thread_pid)?;
                Self::read_register_final(&mut state, use_thread_pid, reg_start, out_data, read_size)?;
            } else {
                std::mem::drop(state);
                match self.send_cmd_req(DebuggerLinuxCmdReqOp::LoadRegCache(use_thread_pid)) {
                    DebuggerLinuxCmdRspOp::Success => (),
                    DebuggerLinuxCmdRspOp::Error(e) => return Err(e),
                    _ => return Err(DebuggerError::InternalError),
                }
                let mut state = self.state.lock().unwrap();
                Self::read_register_final(&mut state, use_thread_pid, reg_start, out_data, read_size)?;
            }
        } else {
            Self::read_register_final(&mut state, use_thread_pid, reg_start, out_data, read_size)?;
        }

        Ok(())
    }

    // runs in: cmd thread
    fn read_register_by_name_buf(
        &self,
        thread_idx: DebuggerThreadIndex,
        name: &str,
        out_data: &mut [u8],
    ) -> Result<(), DebuggerError> {
        let mut state = self.state.lock().unwrap();
        let reg_mem_dirty = state.reg_mem_dirty;

        let reg_info = self
            .nat_reg_info
            .get_reg_info(name, true)
            .ok_or(DebuggerError::InvalidRegister)?;

        let reg_start = reg_info.addr as u64;
        let reg_size = (reg_info.bit_len + 7) / 8 as i32;

        let size = out_data.len();
        // prevent reading more bytes than possible
        if size > i32::MAX as usize || size < (reg_size as usize) {
            return Err(DebuggerError::InvalidRegister);
        }

        let read_size = (size as i32).min(reg_size);
        let use_thread_pid = Self::get_thread_pid_or_current(&state, thread_idx)?;
        if reg_mem_dirty {
            if self.is_debugger_thread() {
                self.load_reg_cache(&mut state, use_thread_pid)?;
                Self::read_register_final(&mut state, use_thread_pid, reg_start, out_data, read_size)?;
            } else {
                std::mem::drop(state);
                match self.send_cmd_req(DebuggerLinuxCmdReqOp::LoadRegCache(use_thread_pid)) {
                    DebuggerLinuxCmdRspOp::Success => (),
                    DebuggerLinuxCmdRspOp::Error(e) => return Err(e),
                    _ => return Err(DebuggerError::InternalError),
                }
                let mut state = self.state.lock().unwrap();
                Self::read_register_final(&mut state, use_thread_pid, reg_start, out_data, read_size)?;
            }
        } else {
            Self::read_register_final(&mut state, use_thread_pid, reg_start, out_data, read_size)?;
        }

        Ok(())
    }

    fn read_bytes(
        &self,
        thread_idx: DebuggerThreadIndex,
        addr: u64,
        out_data: &mut [u8],
        count: u32,
    ) -> Result<u64, DebuggerError> {
        let state = self.state.lock().unwrap();
        let use_thread_pid = Self::get_thread_pid_or_current(&state, thread_idx)?;
        let thread = state.threads.get(&use_thread_pid).ok_or(DebuggerError::InvalidThread)?;

        if thread.proc_mem.is_using_proc_mem() || self.is_debugger_thread() {}

        let mut mut_addr = addr;
        thread
            .proc_mem
            .read_bytes(&mut mut_addr, out_data, count as i32)
            .or(Err(DebuggerError::MemoryAccessFailed))?;

        Ok(mut_addr)
    }

    fn write_bytes(&self, thread_idx: DebuggerThreadIndex, addr: u64, data: &[u8]) -> Result<u64, DebuggerError> {
        let mut state = self.state.lock().unwrap();
        let use_thread_pid = Self::get_thread_pid_or_current(&state, thread_idx)?;
        let thread = state
            .threads
            .get_mut(&use_thread_pid)
            .ok_or(DebuggerError::InvalidThread)?;

        let mut mut_addr = addr;
        thread
            .proc_mem
            .write_bytes(&mut mut_addr, data)
            .or(Err(DebuggerError::MemoryAccessFailed))?;

        Ok(mut_addr)
    }

    fn add_breakpoint(&self, thread_idx: DebuggerThreadIndex, addr: u64) -> Result<u32, DebuggerError> {
        let mut state = self.state.lock().unwrap();
        let use_thread_pid = Self::get_thread_pid_or_current(&state, thread_idx)?;
        let thread = state
            .threads
            .get_mut(&use_thread_pid)
            .ok_or(DebuggerError::InvalidThread)?;

        let bp_bytes: Vec<u8> = vec![0xcc];
        let mut orig_bytes: Vec<u8> = vec![0; bp_bytes.len()];

        let mut mut_addr = addr;
        thread
            .proc_mem
            .read_bytes(&mut mut_addr, &mut orig_bytes, bp_bytes.len() as i32)
            .or(Err(DebuggerError::MemoryAccessFailed))?;

        mut_addr = addr;
        thread
            .proc_mem
            .write_bytes(&mut mut_addr, &bp_bytes)
            .or(Err(DebuggerError::MemoryAccessFailed))?;

        let bp = BreakpointEntry::new(addr, bp_bytes, orig_bytes);
        let bp_idx = state.bp_cont.add_breakpoint(bp);
        Ok(bp_idx)
    }

    fn remove_breakpoint(&self, _thread_idx: DebuggerThreadIndex, _bp_idx: u32) -> Result<(), DebuggerError> {
        todo!()
    }

    // runs in: cmd thread
    fn step(&self, thread_idx: DebuggerThreadIndex) -> Result<(), DebuggerError> {
        if self.is_debugger_thread() {
            let state = self.state.lock().unwrap();
            return self.step_impl(state, thread_idx);
        } else {
            match self.send_cmd_req(DebuggerLinuxCmdReqOp::SingleStep(thread_idx)) {
                DebuggerLinuxCmdRspOp::Success => return Ok(()),
                DebuggerLinuxCmdRspOp::Error(e) => return Err(e),
                _ => return Err(DebuggerError::InternalError),
            }
        }
    }

    // runs in: cmd thread
    fn cont_all(&self) -> Result<(), DebuggerError> {
        let state = self.state.lock().unwrap();
        if let Some(stepping_thread_pid) = state.stepping_thread_pid {
            std::mem::drop(state); // unlock state
            return self.step(DebuggerThreadIndex::Specific(stepping_thread_pid as u32));
        }

        if self.is_debugger_thread() {
            return self.cont_impl(state);
        } else {
            std::mem::drop(state); // unlock state
            match self.send_cmd_req(DebuggerLinuxCmdReqOp::Continue) {
                DebuggerLinuxCmdRspOp::Success => return Ok(()),
                DebuggerLinuxCmdRspOp::Error(e) => return Err(e),
                _ => return Err(DebuggerError::InternalError),
            }
        }
    }

    // runs in: cmd thread
    fn cont_one(&self, thread_idx: DebuggerThreadIndex) -> Result<(), DebuggerError> {
        let state = self.state.lock().unwrap();
        if let Some(stepping_thread_pid) = state.stepping_thread_pid {
            std::mem::drop(state); // unlock state
            return self.step(DebuggerThreadIndex::Specific(stepping_thread_pid as u32));
        }

        if self.is_debugger_thread() {
            return self.cont_one_impl(state, thread_idx);
        } else {
            std::mem::drop(state); // unlock state
            match self.send_cmd_req(DebuggerLinuxCmdReqOp::ContinueOne(thread_idx)) {
                DebuggerLinuxCmdRspOp::Success => return Ok(()),
                DebuggerLinuxCmdRspOp::Error(e) => return Err(e),
                _ => return Err(DebuggerError::InternalError),
            }
        }
    }
}
