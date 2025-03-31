use super::debugger_linux_superpt as superpt;
use crate::memory::memview::{MemView, MemViewError};
use libc::c_long;
use smallvec::{smallvec, SmallVec};
use std::{
    fs::File,
    io::{ErrorKind, Read, Seek, SeekFrom, Write},
    mem,
    sync::{Arc, Mutex},
};

const WRDSZ: usize = mem::size_of::<usize>();

pub struct DebuggerLinuxMemView {
    pid: i32,
    proc_mem: Option<Arc<Mutex<File>>>,
}

// what about process_vm_readv?
impl DebuggerLinuxMemView {
    pub fn new(pid: i32) -> Self {
        let proc_mem = match File::options()
            .read(true)
            .write(true)
            .open(format!("/proc/{}/mem", pid))
        {
            Ok(v) => Some(Arc::new(Mutex::new(v))),
            Err(_) => None, // fallback to PEEKDATA
        };

        DebuggerLinuxMemView { pid, proc_mem }
    }

    pub fn is_using_proc_mem(&self) -> bool {
        self.proc_mem.is_some()
    }

    // c_long should be the same size as usize (I think?)
    fn from_bytes(bytes: &[u8; WRDSZ]) -> c_long {
        c_long::from_ne_bytes(*bytes)
    }

    fn to_bytes(v: c_long) -> Vec<u8> {
        v.to_ne_bytes().to_vec()
    }

    fn to_bytes_n(v: c_long, len: usize) -> Vec<u8> {
        v.to_ne_bytes()[..len].to_vec()
    }
}

impl MemView for DebuggerLinuxMemView {
    fn read_bytes(&self, addr: &mut u64, out_data: &mut [u8], count: i32) -> Result<(), MemViewError> {
        if let Some(proc_mem_mtx) = &self.proc_mem {
            let mut file = proc_mem_mtx.lock().unwrap();
            match file.seek(SeekFrom::Start(*addr)) {
                Ok(_) => (),
                Err(_) => return Err(MemViewError::ReadAccessDenied),
            }
            match file.read_exact(out_data) {
                Ok(_) => (),
                Err(_) => return Err(MemViewError::ReadAccessDenied),
            }
            *addr += count as u64;
            Ok(())
        } else {
            let pid = self.pid;

            let mut bytes_left = count as usize;
            let mut bytes: SmallVec<u8, 8> = smallvec![0; count as usize];
            while bytes_left > 0 {
                let v = match superpt::peekdata(pid, *addr) {
                    Ok(v) => v,
                    Err(_) => return Err(MemViewError::ReadAccessDenied),
                };
                if bytes_left >= WRDSZ {
                    bytes.extend_from_slice(&Self::to_bytes(v));
                    bytes_left -= WRDSZ;
                } else if bytes_left < WRDSZ {
                    bytes.extend_from_slice(&Self::to_bytes_n(v, bytes_left));
                    break;
                }
            }
            out_data.clone_from_slice(&bytes);
            *addr += count as u64;
            Ok(())
        }
    }

    fn write_bytes(&mut self, addr: &mut u64, value: &[u8]) -> Result<(), MemViewError> {
        let count = value.len();
        if let Some(proc_mem_mtx) = &self.proc_mem {
            let mut file = proc_mem_mtx.lock().unwrap();
            match file.seek(SeekFrom::Start(*addr)) {
                Ok(_) => (),
                Err(err) => match err.kind() {
                    ErrorKind::BrokenPipe => return Err(MemViewError::NotLoaded),
                    _ => return Err(MemViewError::WriteAccessDenied),
                },
            }
            let bytes = &value;
            match file.write_all(bytes) {
                Ok(_) => (),
                Err(_) => return Err(MemViewError::WriteAccessDenied),
            }
            *addr += count as u64;
            Ok(())
        } else {
            let pid = self.pid;

            let mut bytes_left = count;
            let mut pos = 0usize;
            while bytes_left > 0 {
                let v: c_long;
                if bytes_left >= WRDSZ {
                    let slice: &[u8; WRDSZ] = &value[pos..pos + WRDSZ].try_into().unwrap();
                    v = Self::from_bytes(slice);
                    bytes_left -= 8;
                    pos += 8;
                } else {
                    let orig_v: c_long = superpt::peekdata(pid, *addr).or(Err(MemViewError::ReadAccessDenied))?;
                    let mask = c_long::wrapping_sub(c_long::wrapping_shl(1, (8 * bytes_left) as u32), 1);

                    let slice: &[u8; WRDSZ] = &value[pos..pos + WRDSZ].try_into().unwrap();
                    v = (orig_v & !mask) | (Self::from_bytes(slice) & mask);
                    bytes_left = 0;
                }
                superpt::pokedata(pid, *addr, v).or(Err(MemViewError::WriteAccessDenied))?;
            }
            *addr += count as u64;
            Ok(())
        }
    }

    fn can_read_while_running(&self) -> bool {
        self.proc_mem.is_some()
    }

    // unsure yet if this is a good idea
    fn can_write_while_running(&self) -> bool {
        false
    }
}
