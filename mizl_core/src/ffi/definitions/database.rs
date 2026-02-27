use crate::{
    database::{
        gbf::GbfFile, gbf_record::GbfFieldValue, gbf_table_schema::GbfTableSchema, gbf_table_view::GbfTableView,
        gbf_tables::GbfTableDef,
    },
    ffi::{
        core_framework::{
            FfiSerializeTrait, FfiSerializer, I32_SA, I32_SZ, WORD_SA, WORD_SZ, align_ptr_fast, align_usize_fast_const,
            pheap_alloc,
        },
        core_types::{OpaqueMFFI, StringFFI, VecFFI},
        definitions::memview::{MemViewVTable, mem_view_error_cpret, mem_view_error_dret, mem_view_error_pret},
    },
    memory::memview::MemViewError,
};
use std::ffi::{CStr, c_char, c_void};

// #-class GbfDatabase

#[unsafe(no_mangle)]
pub extern "C" fn database_new(mv: *mut u8, at: *mut u64, err: *mut *const u8) -> *mut u8 {
    if mv.is_null() {
        return mem_view_error_pret(err, Some(&MemViewError::InvalidParameter));
    }

    let mv_vtable = OpaqueMFFI::get_vtable_ptr(mv) as *const MemViewVTable;
    let mv_box = match unsafe { ((*mv_vtable).steal)(mv as *const c_void) } {
        Ok(v) => v,
        Err(_) => {
            return mem_view_error_pret(err, Some(&MemViewError::generic_static("`mv` was stolen")));
        }
    };

    let mut atv = 0;
    let database = match GbfFile::new(mv_box, &mut atv) {
        Ok(v) => v,
        Err(e) => return mem_view_error_pret(err, Some(&e)),
    };
    if !at.is_null() {
        unsafe {
            *at = atv;
        }
    }

    let database_box = Box::new(database);
    let database_box_ptr = Box::into_raw(database_box);

    let database_ptr = OpaqueMFFI::serialize(database_box_ptr as *const c_void, None, OpaqueMFFI::free_fn::<GbfFile>);

    database_ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn database_get_db_parms(obj: *const c_void, err: *mut *const u8) -> *mut u8 {
    if obj.is_null() {
        return mem_view_error_pret(err, Some(&MemViewError::InvalidParameter));
    }

    let gbf = unsafe { &*(OpaqueMFFI::get_data_ptr(obj as *mut u8) as *const GbfFile) };

    pheap_alloc(&gbf.db_parms, None)
}

#[unsafe(no_mangle)]
pub extern "C" fn database_get_table_def_by_name(
    obj: *const c_void,
    table_name: *const c_char,
    err: *mut *const u8,
) -> *const GbfTableDef {
    if obj.is_null() || table_name.is_null() {
        return mem_view_error_cpret(err, Some(&MemViewError::InvalidParameter));
    }

    let table_name_str = match unsafe { CStr::from_ptr(table_name) }.to_str() {
        Ok(v) => v,
        Err(_) => return mem_view_error_cpret(err, Some(&MemViewError::InvalidParameter)),
    };

    let gbf = unsafe { &*(OpaqueMFFI::get_data_ptr(obj as *mut u8) as *const GbfFile) };

    let table_def = match gbf.tables.table_defs.get(table_name_str) {
        Some(v) => v,
        None => return std::ptr::null_mut(),
    };

    table_def
}

#[unsafe(no_mangle)]
pub extern "C" fn database_get_table_defs(obj: *const c_void, err: *mut *const u8) -> *mut u8 {
    if obj.is_null() {
        return mem_view_error_pret(err, Some(&MemViewError::InvalidParameter));
    }

    let gbf = unsafe { &*(OpaqueMFFI::get_data_ptr(obj as *mut u8) as *const GbfFile) };

    let mut table_def_ptrs: Vec<usize> = Vec::new();
    for table_def in &gbf.tables.table_defs {
        table_def_ptrs.push(table_def.1 as *const GbfTableDef as usize);
    }

    pheap_alloc(&table_def_ptrs, None)
}

#[unsafe(no_mangle)]
pub extern "C" fn database_get_table_view_by_name(
    _obj: *const c_void,
    _schema: *const c_void,
    _table_name: *const c_char,
    _err: *mut *const u8,
) -> *mut u8 {
    todo!()
}

// #-class GbfTableDef

#[unsafe(no_mangle)]
pub extern "C" fn database_table_def_get_schema(obj: *const c_void, err: *mut *const u8) -> *const GbfTableSchema {
    if obj.is_null() {
        return mem_view_error_cpret(err, Some(&MemViewError::InvalidParameter));
    }

    let gbf_td = unsafe { &*(obj as *const GbfTableDef) };

    &gbf_td.schema
}

#[unsafe(no_mangle)]
pub extern "C" fn database_table_def_get_root_nid(obj: *const c_void, err: *mut *const u8) -> i32 {
    if obj.is_null() {
        return mem_view_error_dret(err, Some(&MemViewError::InvalidParameter));
    }

    let gbf_td = unsafe { &*(obj as *const GbfTableDef) };

    gbf_td.root_nid
}

// #-class GbfTableSchema

#[unsafe(no_mangle)]
pub extern "C" fn database_table_schema_get_name(obj: *const c_void, err: *mut *const u8) -> *mut u8 {
    if obj.is_null() {
        return mem_view_error_pret(err, Some(&MemViewError::InvalidParameter));
    }

    let gbf_ts = unsafe { &*(obj as *const GbfTableSchema) };

    pheap_alloc(&gbf_ts.name, None)
}

#[unsafe(no_mangle)]
pub extern "C" fn database_table_schema_get_key_name(obj: *const c_void, err: *mut *const u8) -> *mut u8 {
    if obj.is_null() {
        return mem_view_error_pret(err, Some(&MemViewError::InvalidParameter));
    }

    let gbf_ts = unsafe { &*(obj as *const GbfTableSchema) };

    pheap_alloc(&gbf_ts.key_name, None)
}

#[unsafe(no_mangle)]
pub extern "C" fn database_table_schema_get_key_kind(obj: *const c_void, err: *mut *const u8) -> i32 {
    if obj.is_null() {
        return mem_view_error_dret(err, Some(&MemViewError::InvalidParameter));
    }

    let gbf_ts = unsafe { &*(obj as *const GbfTableSchema) };

    gbf_ts.key_kind.to_u8(false) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn database_table_schema_get_kinds(obj: *const c_void, err: *mut *const u8) -> *mut u8 {
    if obj.is_null() {
        return mem_view_error_pret(err, Some(&MemViewError::InvalidParameter));
    }

    let gbf_ts = unsafe { &*(obj as *const GbfTableSchema) };
    let mut kinds_vec: Vec<i32> = Vec::new();
    for kind in &gbf_ts.kinds {
        kinds_vec.push(kind.to_u8(false) as i32);
    }

    pheap_alloc(&kinds_vec, None)
}

#[unsafe(no_mangle)]
pub extern "C" fn database_table_schema_get_names(obj: *const c_void, err: *mut *const u8) -> *mut u8 {
    if obj.is_null() {
        return mem_view_error_pret(err, Some(&MemViewError::InvalidParameter));
    }

    let gbf_ts = unsafe { &*(obj as *const GbfTableSchema) };

    pheap_alloc(&gbf_ts.names, None)
}

// #-class GbfFieldValue
pub struct GbfFieldValueFfi;
impl GbfFieldValueFfi {
    pub const fn calculate_alignment() -> usize {
        WORD_SA
    }

    pub const fn calculate_base_size() -> usize {
        let mut size = 0usize;

        size = align_usize_fast_const::<I32_SA>(size);
        size += I32_SZ;

        size = align_usize_fast_const::<WORD_SA>(size);
        size += WORD_SZ;

        size
    }

    pub fn calculate_full_size(obj: &GbfFieldValue) -> usize {
        let mut size = 0usize;

        size += Self::calculate_base_size();

        size = align_usize_fast_const::<WORD_SA>(size);
        match obj {
            GbfFieldValue::Boolean(_) => {}
            GbfFieldValue::Byte(_) => {}
            GbfFieldValue::Short(_) => {}
            GbfFieldValue::Int(_) => {}
            GbfFieldValue::Long(_) => {}
            GbfFieldValue::String(v) => {
                size = align_usize_fast_const::<WORD_SA>(size + I32_SZ) - I32_SZ;
                size += StringFFI::calculate_full_size(v)
            }
            GbfFieldValue::Bytes(v) => {
                size = align_usize_fast_const::<WORD_SA>(size + I32_SZ) - I32_SZ;
                size += VecFFI::calculate_full_size(v)
            }
        };

        size
    }

    pub const fn has_dynamic_size() -> bool {
        true
    }

    pub const fn has_var_length_field() -> bool {
        false
    }

    pub unsafe fn serialize(mut ptrd: *mut u8, obj: &GbfFieldValue) -> *mut u8 {
        unsafe {
            ptrd = align_ptr_fast::<WORD_SA>(ptrd);
            let tag = match obj {
                GbfFieldValue::Byte(_) => 0,
                GbfFieldValue::Short(_) => 1,
                GbfFieldValue::Int(_) => 2,
                GbfFieldValue::Long(_) => 3,
                GbfFieldValue::String(_) => 4,
                GbfFieldValue::Bytes(_) => 5,
                GbfFieldValue::Boolean(_) => 6,
            };
            *(ptrd as *mut i32) = tag;

            ptrd = align_ptr_fast::<WORD_SA>(ptrd.add(I32_SZ));
            match obj {
                GbfFieldValue::Boolean(v) => {
                    *(ptrd as *mut bool) = *v;
                    ptrd = ptrd.add(WORD_SZ);
                }
                GbfFieldValue::Byte(v) => {
                    *(ptrd as *mut i8) = *v;
                    ptrd = ptrd.add(WORD_SZ);
                }
                GbfFieldValue::Short(v) => {
                    *(ptrd as *mut i16) = *v;
                    ptrd = ptrd.add(WORD_SZ);
                }
                GbfFieldValue::Int(v) => {
                    *(ptrd as *mut i32) = *v;
                    ptrd = ptrd.add(WORD_SZ);
                }
                GbfFieldValue::Long(v) => {
                    *(ptrd as *mut i64) = *v;
                    ptrd = ptrd.add(WORD_SZ);
                }
                GbfFieldValue::String(v) => {
                    let str_start = align_ptr_fast::<WORD_SA>(ptrd.add(WORD_SZ).add(I32_SZ));
                    *(ptrd as *mut *mut u8) = str_start;
                    ptrd = StringFFI::serialize(str_start.sub(I32_SZ), v);
                }
                GbfFieldValue::Bytes(v) => {
                    let vec_start = align_ptr_fast::<WORD_SA>(ptrd.add(WORD_SZ).add(I32_SZ));
                    *(ptrd as *mut *mut u8) = vec_start;
                    ptrd = VecFFI::serialize(vec_start.sub(I32_SZ), v);
                }
            };

            ptrd
        }
    }
}
impl FfiSerializer for GbfFieldValueFfi {
    type Target = GbfFieldValue;
    fn calculate_alignment() -> usize {
        GbfFieldValueFfi::calculate_alignment()
    }
    fn calculate_base_size() -> usize {
        GbfFieldValueFfi::calculate_base_size()
    }
    fn calculate_full_size(obj: &GbfFieldValue) -> usize {
        GbfFieldValueFfi::calculate_full_size(obj)
    }
    fn has_dynamic_size() -> bool {
        GbfFieldValueFfi::has_dynamic_size()
    }
    fn has_var_length_field() -> bool {
        GbfFieldValueFfi::has_var_length_field()
    }
    unsafe fn serialize(ptrd: *mut u8, obj: &GbfFieldValue) -> *mut u8 {
        unsafe { GbfFieldValueFfi::serialize(ptrd, obj) }
    }
}
impl FfiSerializeTrait for GbfFieldValue {
    type Ffi = GbfFieldValueFfi;
}

// #-class GbfTableView

#[unsafe(no_mangle)]
pub extern "C" fn database_view_new(
    gbf_obj: *const c_void,
    schema_ptr: *const GbfTableSchema,
    root_nid: i32,
    err: *mut *const u8,
) -> *mut u8 {
    if gbf_obj.is_null() || schema_ptr.is_null() || root_nid == -1 {
        return mem_view_error_pret(err, Some(&MemViewError::InvalidParameter));
    }

    let gbf = unsafe { &*(OpaqueMFFI::get_data_ptr(gbf_obj as *mut u8) as *const GbfFile) };
    let schema = unsafe { &*(schema_ptr) };

    let database_view = match GbfTableView::new(gbf, schema, root_nid) {
        Ok(v) => v,
        Err(e) => return mem_view_error_pret(err, Some(&e)),
    };
    let database_view_box = Box::new(database_view);
    let database_view_box_ptr = Box::into_raw(database_view_box);

    let database_view_ptr = OpaqueMFFI::serialize(
        database_view_box_ptr as *const c_void,
        None,
        OpaqueMFFI::free_fn::<GbfTableView>,
    );

    database_view_ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn database_view_get_record_at_long(obj: *const c_void, key: i64, err: *mut *const u8) -> *mut u8 {
    if obj.is_null() {
        return mem_view_error_pret(err, Some(&MemViewError::InvalidParameter));
    }

    let gbf_tv = unsafe { &*(OpaqueMFFI::get_data_ptr(obj as *mut u8) as *const GbfTableView) };

    let record = match gbf_tv.get_record_at_long(key) {
        Ok(v) => match v {
            Some(v2) => v2,
            None => return std::ptr::null_mut(),
        },
        Err(e) => return mem_view_error_pret(err, Some(&e)),
    };

    pheap_alloc(&record, None)
}

#[unsafe(no_mangle)]
pub extern "C" fn database_view_get_record_after_long(obj: *const c_void, key: i64, err: *mut *const u8) -> *mut u8 {
    if obj.is_null() {
        return mem_view_error_pret(err, Some(&MemViewError::InvalidParameter));
    }

    let gbf_tv = unsafe { &*(OpaqueMFFI::get_data_ptr(obj as *mut u8) as *const GbfTableView) };

    let record = match gbf_tv.get_record_after_long(key) {
        Ok(v) => match v {
            Some(v2) => v2,
            None => return std::ptr::null_mut(),
        },
        Err(e) => return mem_view_error_pret(err, Some(&e)),
    };

    pheap_alloc(&record, None)
}

#[unsafe(no_mangle)]
pub extern "C" fn database_view_get_record_at_after_long(obj: *const c_void, key: i64, err: *mut *const u8) -> *mut u8 {
    if obj.is_null() {
        return mem_view_error_pret(err, Some(&MemViewError::InvalidParameter));
    }

    let gbf_tv = unsafe { &*(OpaqueMFFI::get_data_ptr(obj as *mut u8) as *const GbfTableView) };

    let record = match gbf_tv.get_record_at_after_long(key) {
        Ok(v) => match v {
            Some(v2) => v2,
            None => return std::ptr::null_mut(),
        },
        Err(e) => return mem_view_error_pret(err, Some(&e)),
    };

    pheap_alloc(&record, None)
}

// /////
