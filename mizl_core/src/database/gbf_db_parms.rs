use crate::ffi::core_framework::prelude::*;
use crate::{
    consts::arch::Endianness,
    memory::memview::{MemView, MemViewError},
};
use mizl_pm::FfiSerialize;

// some initial table information found in the first block (0x4000 usually)
#[derive(FfiSerialize)]
pub struct GbfDbParms {
    pub node_code: u8,
    pub data_len: i32,
    pub version: u8,
    pub values: Vec<i32>,
}

impl GbfDbParms {
    pub const MASTER_TABLE_ROOT_BUFFER_ID_PARM: usize = 0;
    pub const DATABASE_ID_HIGH_PARM: usize = 1;
    pub const DATABASE_ID_LOW_PARM: usize = 2;

    pub fn read(mv: &Box<dyn MemView>, at: &mut u64) -> Result<GbfDbParms, MemViewError> {
        let endian = Endianness::BigEndian; // always big endian

        let node_code = mv.read_u8(at)?;
        let data_len = mv.read_i32(at, endian)?;
        let version = mv.read_u8(at)?;

        let values_count = (data_len - 1) / 4; // data_len - version field size, always 3?

        // we need at least 3 values, if there are more we can ignore them
        if values_count < 3 {
            return Err(MemViewError::generic_static("expected at least 3 db parms"));
        }

        let mut values: Vec<i32> = Vec::with_capacity(values_count as usize);
        for _ in 0..values_count {
            values.push(mv.read_i32(at, endian)?);
        }

        Ok(GbfDbParms {
            node_code,
            data_len,
            version,
            values,
        })
    }
}
