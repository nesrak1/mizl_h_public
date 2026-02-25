use crate::debugger::debugger::DebuggerThreadIndex;
use crate::ffi::core_framework::prelude::*;
use crate::{
    debugger::{
        debugger::{Debugger, DebuggerError},
        host_debuggers::debugger_linux::DebuggerLinux,
    },
    ffi::core_types::{ErrorFfi, OpaqueMFFI},
};
use num::ToPrimitive;
use std::{
    ffi::CStr,
    os::raw::{c_char, c_uchar, c_void},
};

pub fn debugger_error_ffi(error_opt: Option<&DebuggerError>) -> *mut u8 {
    match error_opt {
        Some(error) => {
            let error_code = error.to_i32().unwrap();
            let error_str = error.to_string();
            let error_mffi_ptr = ErrorFfi::make_error(error_code, Some(error_str));

            error_mffi_ptr
        }
        None => {
            let error_code = i32::MAX;
            let error_mffi_ptr = ErrorFfi::make_error(error_code, None);

            error_mffi_ptr
        }
    }
}

/// Assign the err out parameter but return nothing
pub fn debugger_error_ret(err: *mut *const u8, error_opt: Option<&DebuggerError>) {
    unsafe {
        *err = debugger_error_ffi(error_opt);
    }
}

/// Return the function's default primitive value and assign the err out parameter
pub fn debugger_error_dret<T: Default>(err: *mut *const u8, error_opt: Option<&DebuggerError>) -> T {
    unsafe {
        *err = debugger_error_ffi(error_opt);
    }
    T::default()
}

/// Return a null pointer value and assign the err out parameter
pub fn debugger_error_pret(err: *mut *const u8, error_opt: Option<&DebuggerError>) -> *mut u8 {
    unsafe {
        *err = debugger_error_ffi(error_opt);
    }
    std::ptr::null_mut()
}

// ///////

#[repr(C)]
pub struct DebuggerVTable {
    pub is_big_endian: extern "C" fn(*const c_void) -> i32,
    pub run: extern "C" fn(*const c_void, path: *const c_char, args: *const *const c_char, err: *mut *const u8) -> i32,
    pub wait_next_event: extern "C" fn(*const c_void, no_block: bool, err: *mut *const u8) -> *mut u8,
    pub disassemble_one: extern "C" fn(*const c_void, addr: u64, err: *mut *const u8) -> *mut u8,
    pub read_register_by_name_buf: extern "C" fn(
        *const c_void,
        thread_idx: i32,
        name: *const c_char,
        out_data: *mut c_uchar,
        out_data_len: usize,
        err: *mut *const u8,
    ),
    pub add_breakpoint: extern "C" fn(*const c_void, thread_idx: i32, addr: u64, err: *mut *const u8) -> u32,
    pub step: extern "C" fn(*const c_void, thread_idx: i32, err: *mut *const u8),
    pub cont_all: extern "C" fn(*const c_void, err: *mut *const u8),
}

// #-class DebuggerLinux

static DEBUGGER_LINUX_VTABLE: DebuggerVTable = DebuggerVTable {
    is_big_endian: debugger_linux_is_big_endian,
    run: debugger_linux_run,
    wait_next_event: debugger_linux_wait_next_event,
    disassemble_one: debugger_linux_disassemble_one,
    read_register_by_name_buf: debugger_linux_read_register_by_name_buf,
    add_breakpoint: debugger_linux_add_breakpoint,
    step: debugger_linux_step,
    cont_all: debugger_linux_cont_all,
};

#[unsafe(no_mangle)]
pub extern "C" fn debugger_linux_new() -> *mut u8 {
    let debugger_lin = DebuggerLinux::new();
    let debugger_lin_box = Box::new(debugger_lin);
    let debugger_lin_box_ptr = Box::into_raw(debugger_lin_box);

    let debugger_lin_ptr = OpaqueMFFI::serialize(
        debugger_lin_box_ptr as *const c_void,
        Some(&DEBUGGER_LINUX_VTABLE as *const DebuggerVTable as *const c_void),
        debugger_linux_free,
    );

    debugger_lin_ptr
}

extern "C" fn debugger_linux_free(ffi_obj: *mut u8) {
    let obj = OpaqueMFFI::get_data_ptr(ffi_obj);
    unsafe {
        _ = Box::from_raw(obj as *mut DebuggerLinux);
    }
}

extern "C" fn debugger_linux_is_big_endian(ptr: *const c_void) -> i32 {
    let dbg = unsafe { &*(ptr as *const DebuggerLinux) };
    if dbg.is_big_endian() { 1 } else { 0 }
}

extern "C" fn debugger_linux_run(
    obj: *const c_void,
    path: *const c_char,
    args: *const *const c_char,
    err: *mut *const u8,
) -> i32 {
    let dbg = unsafe { &*(obj as *const DebuggerLinux) };

    let mut args_strs: Vec<&str> = Vec::new();
    let path_str = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(v) => v,
        Err(_) => return debugger_error_dret(err, Some(&DebuggerError::InvalidArguments)),
    };

    let mut args_ptr = args;
    loop {
        let this_arg = unsafe { *args_ptr };
        if this_arg.is_null() {
            break;
        }

        let this_arg_str = match unsafe { CStr::from_ptr(path) }.to_str() {
            Ok(v) => v,
            Err(_) => return debugger_error_dret(err, Some(&DebuggerError::InvalidArguments)),
        };
        args_strs.push(this_arg_str);
        unsafe {
            args_ptr = args_ptr.add(1);
        }
    }

    match dbg.run(path_str, &args_strs) {
        Ok(pid) => pid,
        Err(e) => debugger_error_dret(err, Some(&e)),
    }
}

extern "C" fn debugger_linux_wait_next_event(obj: *const c_void, no_block: bool, err: *mut *const u8) -> *mut u8 {
    let dbg = unsafe { &*(obj as *const DebuggerLinux) };

    let result = dbg.wait_next_event(no_block);
    match result {
        Ok(evt) => pheap_alloc(&evt, None),
        Err(e) => debugger_error_pret(err, Some(&e)),
    }
}

extern "C" fn debugger_linux_disassemble_one(obj: *const c_void, addr: u64, err: *mut *const u8) -> *mut u8 {
    let dbg = unsafe { &*(obj as *const DebuggerLinux) };

    let result = dbg.disassemble_one(addr);
    match result {
        Ok(dis_ins) => pheap_alloc(&dis_ins, None),
        Err(e) => debugger_error_pret(err, Some(&e)),
    }
}

extern "C" fn debugger_linux_read_register_by_name_buf(
    obj: *const c_void,
    thread_idx: i32,
    name: *const c_char,
    out_data: *mut c_uchar,
    out_data_len: usize,
    err: *mut *const u8,
) {
    let dbg = unsafe { &*(obj as *const DebuggerLinux) };

    let thread_idx_enum = if thread_idx < 0 {
        DebuggerThreadIndex::Current
    } else {
        DebuggerThreadIndex::Specific(thread_idx as u32)
    };

    let name = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(v) => v,
        Err(_) => return debugger_error_ret(err, Some(&DebuggerError::InvalidRegister)),
    };

    let out_data_slice = unsafe { std::slice::from_raw_parts_mut(out_data, out_data_len) };

    let result = dbg.read_register_by_name_buf(thread_idx_enum, name, out_data_slice);
    match result {
        Ok(_) => {}
        Err(e) => debugger_error_ret(err, Some(&e)),
    }
}

extern "C" fn debugger_linux_add_breakpoint(
    obj: *const c_void,
    thread_idx: i32,
    addr: u64,
    err: *mut *const u8,
) -> u32 {
    let dbg = unsafe { &*(obj as *const DebuggerLinux) };

    let thread_idx_enum = if thread_idx < 0 {
        DebuggerThreadIndex::Current
    } else {
        DebuggerThreadIndex::Specific(thread_idx as u32)
    };

    let result = dbg.add_breakpoint(thread_idx_enum, addr);
    match result {
        Ok(v) => v,
        Err(e) => debugger_error_dret(err, Some(&e)),
    }
}

extern "C" fn debugger_linux_step(obj: *const c_void, thread_idx: i32, err: *mut *const u8) {
    let dbg = unsafe { &*(obj as *const DebuggerLinux) };

    let thread_idx_enum = if thread_idx < 0 {
        DebuggerThreadIndex::Current
    } else {
        DebuggerThreadIndex::Specific(thread_idx as u32)
    };

    let result = dbg.step(thread_idx_enum);
    match result {
        Ok(_) => {}
        Err(e) => debugger_error_ret(err, Some(&e)),
    }
}

extern "C" fn debugger_linux_cont_all(obj: *const c_void, err: *mut *const u8) {
    let dbg = unsafe { &*(obj as *const DebuggerLinux) };

    let result = dbg.cont_all();
    match result {
        Ok(_) => {}
        Err(e) => debugger_error_ret(err, Some(&e)),
    }
}

// /////

#[unsafe(no_mangle)]
pub extern "C" fn debugger_get_big_endian(ffi_obj: *mut u8) -> i32 {
    let obj = OpaqueMFFI::get_data_ptr(ffi_obj);
    let vtable = OpaqueMFFI::get_vtable_ptr(ffi_obj) as *const DebuggerVTable;
    unsafe { ((*vtable).is_big_endian)(obj) }
}

#[unsafe(no_mangle)]
pub extern "C" fn debugger_run(
    ffi_obj: *mut u8,
    path: *const c_char,
    args: *const *const c_char,
    err: *mut *const u8,
) -> i32 {
    let obj = OpaqueMFFI::get_data_ptr(ffi_obj);
    let vtable = OpaqueMFFI::get_vtable_ptr(ffi_obj) as *const DebuggerVTable;
    unsafe { ((*vtable).run)(obj, path, args, err) }
}

#[unsafe(no_mangle)]
pub extern "C" fn debugger_wait_next_event(ffi_obj: *mut u8, no_block: i32, err: *mut *const u8) -> *mut u8 {
    let obj = OpaqueMFFI::get_data_ptr(ffi_obj);
    let vtable = OpaqueMFFI::get_vtable_ptr(ffi_obj) as *const DebuggerVTable;
    unsafe { ((*vtable).wait_next_event)(obj, no_block != 0, err) }
}

#[unsafe(no_mangle)]
pub extern "C" fn debugger_disassemble_one(ffi_obj: *mut u8, addr: u64, err: *mut *const u8) -> *mut u8 {
    let obj = OpaqueMFFI::get_data_ptr(ffi_obj);
    let vtable = OpaqueMFFI::get_vtable_ptr(ffi_obj) as *const DebuggerVTable;
    unsafe { ((*vtable).disassemble_one)(obj, addr, err) }
}

#[unsafe(no_mangle)]
pub extern "C" fn debugger_read_register_by_name_buf(
    ffi_obj: *mut u8,
    thread_idx: i32,
    name: *const c_char,
    out_data: *mut c_uchar,
    out_data_len: usize,
    err: *mut *const u8,
) {
    let obj = OpaqueMFFI::get_data_ptr(ffi_obj);
    let vtable = OpaqueMFFI::get_vtable_ptr(ffi_obj) as *const DebuggerVTable;
    unsafe { ((*vtable).read_register_by_name_buf)(obj, thread_idx, name, out_data, out_data_len, err) }
}

#[unsafe(no_mangle)]
pub extern "C" fn debugger_add_breakpoint(ffi_obj: *mut u8, thread_idx: i32, addr: u64, err: *mut *const u8) -> u32 {
    let obj = OpaqueMFFI::get_data_ptr(ffi_obj);
    let vtable = OpaqueMFFI::get_vtable_ptr(ffi_obj) as *const DebuggerVTable;
    unsafe { ((*vtable).add_breakpoint)(obj, thread_idx, addr, err) }
}

#[unsafe(no_mangle)]
pub extern "C" fn debugger_step(ffi_obj: *mut u8, thread_idx: i32, err: *mut *const u8) {
    let obj = OpaqueMFFI::get_data_ptr(ffi_obj);
    let vtable = OpaqueMFFI::get_vtable_ptr(ffi_obj) as *const DebuggerVTable;
    unsafe { ((*vtable).step)(obj, thread_idx, err) }
}

#[unsafe(no_mangle)]
pub extern "C" fn debugger_cont_all(ffi_obj: *mut u8, err: *mut *const u8) {
    let obj = OpaqueMFFI::get_data_ptr(ffi_obj);
    let vtable = OpaqueMFFI::get_vtable_ptr(ffi_obj) as *const DebuggerVTable;
    unsafe { ((*vtable).cont_all)(obj, err) }
}
