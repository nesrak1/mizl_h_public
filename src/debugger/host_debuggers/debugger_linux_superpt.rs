use libc;

const NULLPTR: usize = 0usize;

// this file is used to handle ptrace across many architectures easily.
// while the nix crate exists, it's missing support for many archs,
// and adding new ones is hard when it's an external library. the libc
// crate should support most of all architectures rust runs on, which
// means we just need to implement wrappers for the small number of
// ptrace functions we need.

pub const GETREGS_BYTESIZE: usize = if cfg!(target_arch = "x86_64") {
    0xd8
} else {
    // please add the getregs size for your new architecture here.
    // this should cause a compile time error if it's not in this list.
    panic!()
};

pub const GETFPREGS_BYTESIZE: usize = if cfg!(target_arch = "x86_64") {
    0x200
} else {
    // please add the getregs size for your new architecture here.
    // this should cause a compile time error if it's not in this list.
    panic!()
};

// ////////////

pub fn traceme() {
    // ???: can this fail?
    unsafe {
        libc::ptrace(libc::PTRACE_TRACEME, 0, NULLPTR, NULLPTR);
    }
}

pub fn singlestep(pid: i32) {
    unsafe {
        libc::ptrace(libc::PTRACE_SINGLESTEP, libc::pid_t::from(pid), NULLPTR, NULLPTR);
    }
}

pub fn cont(pid: i32) {
    unsafe {
        libc::ptrace(libc::PTRACE_CONT, libc::pid_t::from(pid), NULLPTR, NULLPTR);
    }
}

pub fn getregs(pid: i32) -> [u8; GETREGS_BYTESIZE] {
    let mut buffer = [0u8; GETREGS_BYTESIZE];
    // safety: please assure GETREGS_BYTESIZE is correct for the system.
    // there's no other check we can do here because the output of this
    // call differs depending on the architecture.
    unsafe {
        libc::ptrace(
            libc::PTRACE_GETREGS,
            libc::pid_t::from(pid),
            NULLPTR,
            buffer.as_mut_ptr(),
        );
    }
    return buffer;
}

pub fn getfpregs(pid: i32) -> [u8; GETFPREGS_BYTESIZE] {
    let mut buffer = [0u8; GETFPREGS_BYTESIZE];
    // safety: please assure GETREGS_BYTESIZE is correct for the system.
    // there's no other check we can do here because the output of this
    // call differs depending on the architecture.
    unsafe {
        libc::ptrace(
            libc::PTRACE_GETFPREGS,
            libc::pid_t::from(pid),
            NULLPTR,
            buffer.as_mut_ptr(),
        );
    }
    return buffer;
}

pub fn waitpid(pid: i32) -> (i32, i32) {
    let mut status = 0;
    let ret_pid: i32;
    unsafe {
        ret_pid = libc::waitpid(pid, &mut status, 0);
    }
    return (status, ret_pid);
}

pub fn waitpid_nohang(pid: i32) -> (i32, i32) {
    let mut status = 0;
    let ret_pid: i32;
    unsafe {
        ret_pid = libc::waitpid(pid, &mut status, libc::WNOHANG);
    }
    return (status, ret_pid);
}

pub fn getsiginfo(pid: i32) -> libc::siginfo_t {
    let mut siginfo: libc::siginfo_t = unsafe { std::mem::zeroed() };
    unsafe {
        let errno_loc = libc::__errno_location();
        *errno_loc = 0;
        _ = libc::ptrace(libc::PTRACE_GETSIGINFO, libc::pid_t::from(pid), NULLPTR, &mut siginfo);
    }

    return siginfo;
}

pub fn peekdata(pid: i32, addr: u64) -> Result<i64, ()> {
    let ret_word;
    unsafe {
        let errno_loc = libc::__errno_location();
        *errno_loc = 0;
        ret_word = libc::ptrace(libc::PTRACE_PEEKDATA, libc::pid_t::from(pid), addr, NULLPTR);
        if *errno_loc != 0 {
            return Err(());
        }
    }

    return Ok(ret_word);
}

pub fn pokedata(pid: i32, addr: u64, value: i64) -> Result<i64, ()> {
    let ret_word;
    unsafe {
        let errno_loc = libc::__errno_location();
        *errno_loc = 0;
        ret_word = libc::ptrace(libc::PTRACE_POKEDATA, libc::pid_t::from(pid), addr, value);
        if *errno_loc != 0 {
            return Err(());
        }
    }

    return Ok(ret_word);
}
