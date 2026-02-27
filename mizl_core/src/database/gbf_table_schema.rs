use std::collections::HashSet;

use crate::{
    consts::arch::Endianness,
    database::{
        gbf_helpers::{read_bytestring, read_string},
        gbf_record::{GbfFieldKind, GbfFieldValue, GbfRecord},
    },
    memory::memview::{MemView, MemViewError},
};

pub struct GbfTableSchema {
    pub name: String,
    pub key_name: String,
    pub key_kind: GbfFieldKind,
    pub sparse_columns: Option<HashSet<i32>>,
    pub kinds: Vec<GbfFieldKind>,
    pub names: Vec<String>,
}

impl GbfTableSchema {
    pub fn new(
        name: String,
        key_name: String,
        key_kind: GbfFieldKind,
        sparse_columns: Option<HashSet<i32>>,
    ) -> GbfTableSchema {
        GbfTableSchema {
            name,
            key_name,
            key_kind,
            sparse_columns,
            kinds: Vec::new(),
            names: Vec::new(),
        }
    }

    pub fn add_column(&mut self, kind: GbfFieldKind, name: String) {
        self.kinds.push(kind);
        self.names.push(name);
    }

    pub fn remove_column(&mut self, index: usize) {
        self.kinds.remove(index);
        self.names.remove(index);
    }

    pub fn get_value_len(&self) -> i32 {
        let mut len = 0;
        for kind in &self.kinds {
            let this_len = kind.get_len();
            if this_len < 0 {
                // one of the fields is variable length
                return -1;
            }

            len += this_len;
        }
        return len;
    }

    pub fn get_column_idx(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|e| e == name)
    }

    pub fn read_record(
        &self,
        key: GbfFieldValue,
        mv: &Box<dyn MemView>,
        at: &mut u64,
    ) -> Result<GbfRecord, MemViewError> {
        let mut values: Vec<GbfFieldValue> = Vec::new();

        if let Some(sparse_columns) = &self.sparse_columns {
            // read required fields
            for i in 0..self.kinds.len() {
                let kind = &self.kinds[i];
                if !sparse_columns.contains(&(i as i32)) {
                    values.push(Self::read_value(kind, mv, at)?);
                } else {
                    values.push(Self::default_value(kind));
                }
            }

            // read optional fields
            let sparse_field_count = mv.read_u8(at)? as usize;
            for _ in 0..sparse_field_count {
                let this_sparse_field_idx = mv.read_u8(at)? as usize;
                let kind = &self.kinds[this_sparse_field_idx];
                values[this_sparse_field_idx] = Self::read_value(kind, mv, at)?;
            }
        } else {
            for kind in &self.kinds {
                values.push(Self::read_value(kind, mv, at)?);
            }
        }

        Ok(GbfRecord::new(key, values))
    }

    fn read_value(kind: &GbfFieldKind, mv: &Box<dyn MemView>, at: &mut u64) -> Result<GbfFieldValue, MemViewError> {
        let endian = Endianness::BigEndian; // always big endian
        let value = match kind {
            GbfFieldKind::Boolean => GbfFieldValue::Boolean(mv.read_u8(at)? != 0),
            GbfFieldKind::Byte => GbfFieldValue::Byte(mv.read_i8(at)?),
            GbfFieldKind::Short => GbfFieldValue::Short(mv.read_i16(at, endian)?),
            GbfFieldKind::Int => GbfFieldValue::Int(mv.read_i32(at, endian)?),
            GbfFieldKind::Long => GbfFieldValue::Long(mv.read_i64(at, endian)?),
            GbfFieldKind::String => GbfFieldValue::String(match read_string(&mv, at)? {
                Some(v) => v,          // regular string
                None => String::new(), // null string (should we be handling this?)
            }),
            GbfFieldKind::Bytes => {
                GbfFieldValue::Bytes(match read_bytestring(&mv, at)? {
                    Some(v) => v,                  // regular bytestring
                    None => Vec::with_capacity(0), // empty bytestring (shouldn't happen?)
                })
            }
        };
        Ok(value)
    }

    fn default_value(kind: &GbfFieldKind) -> GbfFieldValue {
        match kind {
            GbfFieldKind::Boolean => GbfFieldValue::Boolean(false),
            GbfFieldKind::Byte => GbfFieldValue::Byte(0),
            GbfFieldKind::Short => GbfFieldValue::Short(0),
            GbfFieldKind::Int => GbfFieldValue::Int(0),
            GbfFieldKind::Long => GbfFieldValue::Long(0),
            GbfFieldKind::String => GbfFieldValue::String(String::new()),
            GbfFieldKind::Bytes => GbfFieldValue::Bytes(Vec::with_capacity(0)),
        }
    }
}
