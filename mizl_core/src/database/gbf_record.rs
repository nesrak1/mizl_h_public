use crate::ffi::core_framework::prelude::*;
use crate::ffi::definitions::database::GbfFieldValueFfi;
use crate::memory::memview::MemViewError;
use mizl_pm::FfiSerialize;

#[derive(FfiSerialize)]
pub struct GbfRecord {
    pub key: GbfFieldValue,
    pub values: Vec<GbfFieldValue>,
}

impl GbfRecord {
    pub fn new(key: GbfFieldValue, values: Vec<GbfFieldValue>) -> GbfRecord {
        GbfRecord { key, values }
    }

    fn get_value_or_err(&self, index: usize) -> Result<&GbfFieldValue, MemViewError> {
        match self.values.get(index) {
            Some(v) => Ok(v),
            None => return Err(MemViewError::generic_static("out of bounds record access")),
        }
    }

    pub fn get_boolean(&self, index: usize) -> Result<bool, MemViewError> {
        match self.get_value_or_err(index)? {
            GbfFieldValue::Boolean(v) => Ok(*v),
            GbfFieldValue::Byte(v) => Ok(*v != 0),
            GbfFieldValue::Short(v) => Ok(*v != 0),
            GbfFieldValue::Int(v) => Ok(*v != 0),
            GbfFieldValue::Long(v) => Ok(*v != 0),
            _ => return Err(MemViewError::generic_static("unexpected field type")),
        }
    }

    pub fn get_byte(&self, index: usize) -> Result<i8, MemViewError> {
        match self.get_value_or_err(index)? {
            GbfFieldValue::Byte(v) => Ok(*v),
            GbfFieldValue::Short(v) => Ok(*v as i8),
            GbfFieldValue::Int(v) => Ok(*v as i8),
            GbfFieldValue::Long(v) => Ok(*v as i8),
            _ => return Err(MemViewError::generic_static("unexpected field type")),
        }
    }

    pub fn get_short(&self, index: usize) -> Result<i16, MemViewError> {
        match self.get_value_or_err(index)? {
            GbfFieldValue::Short(v) => Ok(*v),
            GbfFieldValue::Byte(v) => Ok(*v as i16),
            GbfFieldValue::Int(v) => Ok(*v as i16),
            GbfFieldValue::Long(v) => Ok(*v as i16),
            _ => return Err(MemViewError::generic_static("unexpected field type")),
        }
    }

    pub fn get_int(&self, index: usize) -> Result<i32, MemViewError> {
        match self.get_value_or_err(index)? {
            GbfFieldValue::Int(v) => Ok(*v),
            GbfFieldValue::Byte(v) => Ok(*v as i32),
            GbfFieldValue::Short(v) => Ok(*v as i32),
            GbfFieldValue::Long(v) => Ok(*v as i32),
            _ => return Err(MemViewError::generic_static("unexpected field type")),
        }
    }

    pub fn get_long(&self, index: usize) -> Result<i64, MemViewError> {
        match self.get_value_or_err(index)? {
            GbfFieldValue::Long(v) => Ok(*v),
            GbfFieldValue::Byte(v) => Ok(*v as i64),
            GbfFieldValue::Short(v) => Ok(*v as i64),
            GbfFieldValue::Int(v) => Ok(*v as i64),
            _ => return Err(MemViewError::generic_static("unexpected field type")),
        }
    }

    pub fn get_string(&self, index: usize) -> Result<String, MemViewError> {
        match self.get_value_or_err(index)? {
            GbfFieldValue::String(v) => Ok(v.clone()),
            _ => return Err(MemViewError::generic_static("unexpected field type")),
        }
    }

    pub fn get_bytes(&self, index: usize) -> Result<Vec<u8>, MemViewError> {
        match self.get_value_or_err(index)? {
            GbfFieldValue::Bytes(v) => Ok(v.clone()),
            _ => return Err(MemViewError::generic_static("unexpected field type")),
        }
    }
}

// ////////////////////////////////////

pub enum GbfFieldKind {
    Byte = 0,
    Short = 1,
    Int = 2,
    Long = 3,
    String = 4,
    Bytes = 5,
    Boolean = 6,
    //FixedField10,
}

impl GbfFieldKind {
    pub fn from_u8(value: u8) -> Option<GbfFieldKind> {
        // todo: handle indexed fields!!!
        match value & 0xf {
            Self::BOOLEAN => Some(Self::Boolean),
            Self::BYTE => Some(Self::Byte),
            Self::SHORT => Some(Self::Short),
            Self::INT => Some(Self::Int),
            Self::LONG => Some(Self::Long),
            Self::STRING => Some(Self::String),
            Self::BYTES => Some(Self::Bytes),
            _ => None,
        }
    }

    pub fn to_u8(&self, shifted: bool) -> u8 {
        let v = match self {
            Self::Boolean => Self::BOOLEAN,
            Self::Byte => Self::BYTE,
            Self::Short => Self::SHORT,
            Self::Int => Self::INT,
            Self::Long => Self::LONG,
            Self::String => Self::STRING,
            Self::Bytes => Self::BYTES,
        };

        if !shifted { v } else { v << 4 }
    }

    pub fn get_len(&self) -> i32 {
        match self {
            GbfFieldKind::Boolean => 1,
            GbfFieldKind::Byte => 1,
            GbfFieldKind::Short => 2,
            GbfFieldKind::Int => 4,
            GbfFieldKind::Long => 8,
            GbfFieldKind::String => -1,
            GbfFieldKind::Bytes => -1,
        }
    }
}

impl GbfFieldKind {
    pub const BYTE: u8 = 0;
    pub const SHORT: u8 = 1;
    pub const INT: u8 = 2;
    pub const LONG: u8 = 3;
    pub const STRING: u8 = 4;
    pub const BYTES: u8 = 5;
    pub const BOOLEAN: u8 = 6;
    //pub const FIXED10: u8 = 7;
}

pub enum GbfFieldValue {
    Boolean(bool),
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    String(String),
    Bytes(Vec<u8>),
}
