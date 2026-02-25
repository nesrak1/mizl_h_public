use arc_swap::ArcSwap;
use std::sync::{Arc, LazyLock, Mutex};

// ah yes, the joy of multithreaded signal processing code.
// in this file, we need to be able to - at any time - add
// a new fd for a child but also be able to read the list
// of fds at any time, including while we're writing to the
// list which would prevent a RwLock from working. here, we
// clone the entire list (we assume this list won't be very
// large) and do an atomic pointer replace with ArcSwap.

static SIGCHLD_FDS: LazyLock<ArcSwap<Vec<i32>>> = LazyLock::new(|| ArcSwap::from_pointee(Vec::new()));
static SIGCHLD_SETUP: LazyLock<Arc<Mutex<bool>>> = LazyLock::new(|| Arc::new(Mutex::new(false)));

pub fn sigchld_register(fd: i32) -> bool {
    let mut result = false;
    // use rcu in case we register in two threads
    // at the same time (but please don't do this)
    SIGCHLD_FDS.rcu(|current| {
        if current.contains(&fd) {
            // why are we adding a pid we already added?
            result = false;
            Arc::clone(current)
        } else {
            let mut sigchld_fds_copy = current.to_vec();
            sigchld_fds_copy.push(fd);
            result = true;
            Arc::new(sigchld_fds_copy)
        }
    });
    // we are guaranteed to have at least one item at this point.
    // let's setup the signal handler, but let's make sure only
    // one thread is doing that ;)
    {
        let mut sigchld_setup = match SIGCHLD_SETUP.lock() {
            Ok(guard) => guard,
            Err(_) => return result, // give up, hopefully it was already registered
        };
        let is_setup = *sigchld_setup;
        if !is_setup {
            // setup sig handler
            unsafe {
                let mut sigaction: libc::sigaction = std::mem::zeroed();
                // we are not restart compatible but I'll leave this here because why not
                // we should make something like gdb's EINTR wrapper so we don't have to
                // worry about it.
                sigaction.sa_flags = libc::SA_SIGINFO | libc::SA_RESTART;
                sigaction.sa_sigaction = sigchld_handler as libc::sighandler_t;
                libc::sigemptyset(&mut sigaction.sa_mask);
                libc::sigaction(libc::SIGCHLD, &sigaction, std::ptr::null_mut());
            }

            *sigchld_setup = true;
        }
    }
    return result;
}

pub fn sigchld_unregister(fd: i32) -> bool {
    let mut result = false;
    SIGCHLD_FDS.rcu(|current| {
        if current.contains(&fd) {
            result = true;
            let new_list = current.iter().filter(|&&x| x != fd).copied().collect();
            Arc::new(new_list)
        } else {
            // why are we removing a pid we haven't added?
            result = false;
            Arc::clone(current)
        }
    });
    // todo: we don't check if we need to unregister the handler here
    // this may be a little tricky since it's probably a bad idea to
    // lock the mutex inside the rcu, but the rcu is the only place we
    // can 100% know for sure whether or not the size is 0. so it's
    // probably best for the user to clean up the SIGCHLD handler...
    return result;
}

extern "C" fn sigchld_handler(_sig: libc::c_int, _info: *mut libc::siginfo_t, _data: *mut libc::c_void) {
    // I have no idea if this is thread safe (does it allocate?)
    let sigchld_fds = SIGCHLD_FDS.load();
    let custom_data = [0x48646C6863676953u64; 1];
    for &fd in sigchld_fds.iter() {
        unsafe {
            libc::write(fd, &custom_data as *const u64 as *const libc::c_void, 8);
        }
    }
}
