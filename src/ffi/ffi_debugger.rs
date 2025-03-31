use crate::debugger::{
    debugger::{Debugger, DebuggerError, DebuggerThreadIndex},
    host_debuggers::debugger_linux::DebuggerLinux,
};
use num::ToPrimitive;
use std::{
    ffi::CStr,
    mem,
    os::raw::{c_char, c_uchar, c_void},
    u32,
};

// ALL CODE IS SUBJECT TO CHANGE

// we are using packed heaps ("pheeps") to handle passing data
// back through ffi. the packet heap is a single allocation that
// represents the entire return value. since our return types
// are decently simple, we can easily calculate the size of the
// return value ahead of time and then copy and paste fields
// from the rust struct directly into the pheep. while there is
// some runtime cost, it's not nearly as bad as any other format
// like json or protobuf.
// the format is as simple as possible. the first usize of data
// is either the size of the pheep (not including this value) or
// an error code if the value is negative starting at -1. after
// the size (assuming an error wasn't returned), fields are
// either i/u32, i/u64, i/usize, and pointers to the dynamic area.
// the dynamic area is reserved for strings and lists with
// variable size, and will always be prefixed by a u32 four bytes
// before where the string or list pointer points. strings are
// null terminated in case it helps with C programs, but don't
// trust that to be the end of the string necessarily. you should
// rely on the string length instead in case the string has null
// characters within it, though.
// the macros below should help generate fast code. if the
// compiler is smart (it should be) then the static portion of
// the pheep should write to statically compiled offsets
// (besides bounds checks which are inevitable if we want to
// stay in safe land.) in the dynamic portion, there will be a
// counter to keep up with the current position, but otherwise
// not much of a change. this makes for some pretty fast compile
// time serialization.

// helpers

const WRDSZ: usize = mem::size_of::<usize>();
const ANY32SZ: usize = mem::size_of::<u32>();
const ANY64SZ: usize = mem::size_of::<u64>();

macro_rules! rb_write_u32 {
    ($res_buf:expr, $res_ctr:expr, $value:expr) => {
        $res_buf[$res_ctr..($res_ctr + ANY32SZ)].copy_from_slice(&u32::to_ne_bytes($value));
        $res_ctr += ANY32SZ;
    };
}

macro_rules! rb_write_u64 {
    ($res_buf:expr, $res_ctr:expr, $value:expr) => {
        $res_buf[$res_ctr..($res_ctr + ANY64SZ)].copy_from_slice(&u64::to_ne_bytes($value));
        $res_ctr += ANY64SZ;
    };
}

macro_rules! rb_write_isize {
    ($res_buf:expr, $res_ctr:expr, $value:expr) => {
        $res_buf[$res_ctr..($res_ctr + WRDSZ)].copy_from_slice(&isize::to_ne_bytes($value));
        $res_ctr += WRDSZ;
    };
}

macro_rules! rb_write_ptr {
    ($res_buf:expr, $res_ctr:expr, $res_dyn_ctr:expr, $res_dyn_off:expr, $buf_len:expr) => {
        $res_buf[$res_ctr..($res_ctr + WRDSZ)].copy_from_slice(&usize::to_ne_bytes($res_dyn_ctr + $res_dyn_off));
        $res_ctr += WRDSZ;
        $res_dyn_ctr += $res_dyn_off + $buf_len;
    };
}

macro_rules! rb_write_str {
    ($res_buf:expr, $res_ctr:expr, $value:expr, $value_strlen:expr, $value_strlen_pad:expr) => {
        $res_buf[$res_ctr..($res_ctr + ANY32SZ)].copy_from_slice(&u32::to_ne_bytes($value_strlen as u32));
        $res_buf[($res_ctr + ANY32SZ)..(($res_ctr + ANY32SZ) + $value_strlen)].copy_from_slice($value.as_bytes());
        $res_buf[($res_ctr + ANY32SZ) + $value_strlen] = 0;
        $res_ctr += ANY32SZ + $value_strlen_pad;
    };
}

// for unused assignment warnings
macro_rules! rb_write_finish {
    ($res_buf:expr, $res_ctr:expr) => {
        _ = $res_ctr;
    };
}

macro_rules! rb_write_finish_dyn {
    ($res_buf:expr, $res_ctr:expr, $res_dyn_ctr:expr) => {
        _ = $res_ctr;
        _ = $res_dyn_ctr;
    };
}
// //////////////////////////////

fn debugger_error_to_i32(error: DebuggerError) -> i32 {
    return -(error.to_i32().unwrap_or(0) + 1);
}

fn debugger_error_to_isize(error: DebuggerError) -> isize {
    return -(error.to_isize().unwrap_or(0) + 1);
}

// ///////

#[repr(C)]
pub struct DebuggerVTable {
    pub is_big_endian: extern "C" fn(*const c_void) -> i32,
    pub run: extern "C" fn(*const c_void, path: *const c_char, args: *const *const c_char) -> i32,
    pub wait_next_event: extern "C" fn(*const c_void) -> *mut u8,
    pub disassemble_one: extern "C" fn(*const c_void, addr: u64) -> *mut u8,
    pub read_register_by_name_buf: extern "C" fn(
        *const c_void,
        thread_idx: i32,
        name: *const c_char,
        out_data: *mut c_uchar,
        out_data_len: usize,
    ) -> i32,
    pub add_breakpoint: extern "C" fn(*const c_void, thread_idx: i32, addr: u64) -> i32,
    pub step: extern "C" fn(*const c_void, thread_idx: i32) -> i32,
    pub cont_all: extern "C" fn(*const c_void) -> i32,
    //
    pub drop_pheep: extern "C" fn(*mut u8),
    pub drop: extern "C" fn(*mut c_void),
}

#[repr(C)]
pub struct DebuggerFFI {
    pub obj: *mut c_void,
    pub vtable: *const DebuggerVTable,
}

extern "C" fn debugger_linux_is_big_endian(obj: *const c_void) -> i32 {
    let dbg = unsafe { &*(obj as *const DebuggerLinux) };
    if dbg.is_big_endian() {
        1
    } else {
        0
    }
}

extern "C" fn debugger_linux_run(obj: *const c_void, path: *const c_char, args: *const *const c_char) -> i32 {
    let dbg = unsafe { &*(obj as *const DebuggerLinux) };
    let mut args_strs: Vec<&str> = Vec::new();
    let path_str = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(v) => v,
        Err(_) => return -1,
    };
    let args_ptr = args;
    loop {
        let this_arg = unsafe { *args_ptr };
        if this_arg.is_null() {
            break;
        }

        let this_arg_str = match unsafe { CStr::from_ptr(path) }.to_str() {
            Ok(v) => v,
            Err(_) => return -1,
        };
        args_strs.push(this_arg_str);
    }
    match dbg.run(path_str, &args_strs) {
        Ok(pid) => pid,
        Err(e) => -(e.to_i32().unwrap_or(0) + 1),
    }
}

extern "C" fn debugger_linux_wait_next_event(obj: *const c_void) -> *mut u8 {
    let dbg = unsafe { &*(obj as *const DebuggerLinux) };
    let result = dbg.wait_next_event();
    match result {
        Ok(evt) => {
            let pheep_size = WRDSZ + ANY32SZ + ANY32SZ + ANY32SZ;
            let mut res_buf: Vec<u8> = vec![0; pheep_size];
            let res_ptr = res_buf.as_mut_ptr();

            let mut res_ctr = 0;
            rb_write_isize!(res_buf, res_ctr, pheep_size as isize);
            rb_write_u32!(res_buf, res_ctr, evt.kind.to_u32().unwrap_or(0));
            rb_write_u32!(res_buf, res_ctr, evt.code);
            rb_write_u32!(res_buf, res_ctr, evt.pid);
            rb_write_finish!(res_buf, res_ctr);

            mem::forget(res_buf);
            res_ptr
        }
        Err(e) => {
            // todo: we should probably point to well known static addresses instead
            let mut res_buf: Vec<u8> = vec![0; 8];
            let res_ptr = res_buf.as_mut_ptr();

            let mut res_ctr = 0;
            rb_write_isize!(res_buf, res_ctr, debugger_error_to_isize(e));
            rb_write_finish!(res_buf, res_ctr);

            mem::forget(res_buf);
            res_ptr
        }
    }
}

extern "C" fn debugger_linux_disassemble_one(obj: *const c_void, addr: u64) -> *mut u8 {
    let dbg = unsafe { &*(obj as *const DebuggerLinux) };
    let result = dbg.disassemble_one(addr);
    match result {
        Ok(dis_ins) => {
            // todo: could break if lengths are larger than u32
            let text_strlen = dis_ins.text.len();
            let text_strlen_pad = (text_strlen + 1 + 7) & (!7);
            let runs_buflen = dis_ins.runs.len() * 8;

            let pheep_static_size = WRDSZ + ANY64SZ + ANY64SZ + WRDSZ + WRDSZ;
            let pheep_size = pheep_static_size + ANY32SZ + text_strlen_pad + ANY32SZ + runs_buflen;
            let mut res_buf: Vec<u8> = vec![0; pheep_size];
            let res_ptr = res_buf.as_mut_ptr();

            let mut res_ctr = 0;
            let mut res_dyn_ctr = (res_ptr as usize) + pheep_static_size;
            rb_write_isize!(res_buf, res_ctr, pheep_size as isize);
            rb_write_u64!(res_buf, res_ctr, dis_ins.addr);
            rb_write_u64!(res_buf, res_ctr, dis_ins.len);
            rb_write_ptr!(res_buf, res_ctr, res_dyn_ctr, 4, text_strlen_pad);
            rb_write_ptr!(res_buf, res_ctr, res_dyn_ctr, 4, runs_buflen);
            // dyn
            rb_write_str!(res_buf, res_ctr, dis_ins.text, text_strlen, text_strlen_pad);
            rb_write_u32!(res_buf, res_ctr, dis_ins.runs.len() as u32);
            for run in &dis_ins.runs {
                rb_write_u32!(res_buf, res_ctr, run.length);
                rb_write_u32!(res_buf, res_ctr, run.run_type.to_u32().unwrap_or(0));
            }
            rb_write_finish_dyn!(res_buf, res_ctr, res_dyn_ctr);

            mem::forget(res_buf);
            res_ptr
        }
        Err(e) => {
            // todo: we should probably point to well known static addresses instead
            let mut res_buf: Vec<u8> = vec![0; 8];
            let res_ptr = res_buf.as_mut_ptr();

            let mut res_ctr = 0;
            rb_write_isize!(res_buf, res_ctr, debugger_error_to_isize(e));
            rb_write_finish!(res_buf, res_ctr);

            mem::forget(res_buf);
            res_ptr
        }
    }
}

extern "C" fn debugger_linux_read_register_by_name_buf(
    obj: *const c_void,
    thread_idx: i32,
    name: *const c_char,
    out_data: *mut c_uchar,
    out_data_len: usize,
) -> i32 {
    let dbg = unsafe { &*(obj as *const DebuggerLinux) };

    let thread_idx_rust = if thread_idx == -1 {
        DebuggerThreadIndex::Current
    } else {
        DebuggerThreadIndex::Specific(thread_idx as u32)
    };
    let name = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(v) => v,
        Err(_) => return -1,
    };

    let out_data_slice = unsafe { std::slice::from_raw_parts_mut(out_data, out_data_len) };
    match dbg.read_register_by_name_buf(thread_idx_rust, name, out_data_slice) {
        Ok(_) => 0,
        Err(e) => debugger_error_to_i32(e),
    }
}

extern "C" fn debugger_linux_add_breakpoint(obj: *const c_void, thread_idx: i32, addr: u64) -> i32 {
    let dbg = unsafe { &*(obj as *const DebuggerLinux) };

    let thread_idx_rust = if thread_idx == -1 {
        DebuggerThreadIndex::Current
    } else {
        DebuggerThreadIndex::Specific(thread_idx as u32)
    };

    match dbg.add_breakpoint(thread_idx_rust, addr) {
        Ok(_) => 0,
        Err(e) => debugger_error_to_i32(e),
    }
}

extern "C" fn debugger_linux_step(obj: *const c_void, thread_idx: i32) -> i32 {
    let dbg = unsafe { &*(obj as *const DebuggerLinux) };

    let thread_idx_rust = if thread_idx == -1 {
        DebuggerThreadIndex::Current
    } else {
        DebuggerThreadIndex::Specific(thread_idx as u32)
    };

    match dbg.step(thread_idx_rust) {
        Ok(_) => 0,
        Err(e) => debugger_error_to_i32(e),
    }
}

extern "C" fn debugger_linux_cont_all(obj: *const c_void) -> i32 {
    let dbg = unsafe { &*(obj as *const DebuggerLinux) };
    match dbg.cont_all() {
        Ok(_) => 0,
        Err(e) => debugger_error_to_i32(e),
    }
}

extern "C" fn debugger_linux_drop(obj: *mut c_void) {
    unsafe {
        _ = Box::from_raw(obj as *mut DebuggerLinux);
    }
}

static DEBUGGER_LINUX_VTABLE: DebuggerVTable = DebuggerVTable {
    is_big_endian: debugger_linux_is_big_endian,
    run: debugger_linux_run,
    wait_next_event: debugger_linux_wait_next_event,
    disassemble_one: debugger_linux_disassemble_one,
    read_register_by_name_buf: debugger_linux_read_register_by_name_buf,
    add_breakpoint: debugger_linux_add_breakpoint,
    step: debugger_linux_step,
    cont_all: debugger_linux_cont_all,
    drop_pheep: debugger_drop_pheep,
    drop: debugger_linux_drop,
};

#[no_mangle]
pub extern "C" fn debugger_linux_new() -> *mut DebuggerFFI {
    let obj = Box::new(DebuggerLinux::new());
    let obj_ptr = Box::into_raw(obj) as *mut c_void;
    let ffi = Box::new(DebuggerFFI {
        obj: obj_ptr,
        vtable: &DEBUGGER_LINUX_VTABLE,
    });
    Box::into_raw(ffi)
}

#[no_mangle]
pub extern "C" fn debugger_get_big_endian(ffi: *const DebuggerFFI) -> i32 {
    unsafe { ((*(*ffi).vtable).is_big_endian)((*ffi).obj) }
}

#[no_mangle]
pub extern "C" fn debugger_run(ffi: *const DebuggerFFI, path: *const c_char, args: *const *const c_char) -> i32 {
    unsafe { ((*(*ffi).vtable).run)((*ffi).obj, path, args) }
}

#[no_mangle]
pub extern "C" fn debugger_wait_next_event(ffi: *const DebuggerFFI) -> *mut u8 {
    unsafe { ((*(*ffi).vtable).wait_next_event)((*ffi).obj) }
}

#[no_mangle]
pub extern "C" fn debugger_disassemble_one(ffi: *const DebuggerFFI, addr: u64) -> *mut u8 {
    unsafe { ((*(*ffi).vtable).disassemble_one)((*ffi).obj, addr) }
}

#[no_mangle]
pub extern "C" fn debugger_read_register_by_name_buf(
    ffi: *const DebuggerFFI,
    thread_idx: i32,
    name: *const c_char,
    out_data: *mut c_uchar,
    out_data_len: usize,
) -> i32 {
    unsafe { ((*(*ffi).vtable).read_register_by_name_buf)((*ffi).obj, thread_idx, name, out_data, out_data_len) }
}

#[no_mangle]
pub extern "C" fn debugger_add_breakpoint(ffi: *const DebuggerFFI, thread_idx: i32, addr: u64) -> i32 {
    unsafe { ((*(*ffi).vtable).add_breakpoint)((*ffi).obj, thread_idx, addr) }
}

#[no_mangle]
pub extern "C" fn debugger_step(ffi: *const DebuggerFFI, thread_idx: i32) -> i32 {
    unsafe { ((*(*ffi).vtable).step)((*ffi).obj, thread_idx) }
}

#[no_mangle]
pub extern "C" fn debugger_cont_all(ffi: *const DebuggerFFI) -> i32 {
    unsafe { ((*(*ffi).vtable).cont_all)((*ffi).obj) }
}

#[no_mangle]
pub extern "C" fn debugger_drop(ffi: *mut DebuggerFFI) {
    unsafe {
        let ffi_box = Box::from_raw(ffi);
        ((*ffi_box.vtable).drop)(ffi_box.obj);
    }
}

#[no_mangle]
extern "C" fn debugger_drop_pheep(pheep: *mut u8) {
    unsafe {
        let code = *(pheep as *mut isize);
        if code < 0 {
            // drop error code
            drop(Vec::from_raw_parts(pheep, 8, 8));
        } else {
            let struct_len = code as usize;
            drop(Vec::from_raw_parts(pheep, struct_len, struct_len));
        }
    }
}
