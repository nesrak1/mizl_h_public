use std::collections::HashMap;

use crate::{
    database::{
        gbf::GbfFile,
        gbf_record::GbfFieldKind,
        gbf_table_schema::GbfTableSchema,
        gbf_table_view::{GbfTableView, GbfTableViewIterator},
    },
    memory::memview::MemViewError,
};

// list of tables and their schemas
pub struct GbfTableDef {
    pub schema: GbfTableSchema,
    pub root_nid: i32,
}

impl GbfTableDef {
    pub fn new(schema: GbfTableSchema, root_nid: i32) -> GbfTableDef {
        GbfTableDef { schema, root_nid }
    }
}

pub struct GbfTables {
    pub table_defs: HashMap<String, GbfTableDef>,
}

impl GbfTables {
    // since we hardcode the schema, we might as well hardcode indices as well
    const TABLE_NAME_IDX: usize = 0;
    const _SCHEMA_VERSION_IDX: usize = 1;
    const ROOT_BUFFER_ID_IDX: usize = 2;
    const KEY_TYPE_IDX: usize = 3;
    const FIELD_TYPES_IDX: usize = 4;
    const FIELD_NAMES_IDX: usize = 5;
    const _INDEX_COLUMN_IDX: usize = 6;
    const _MAX_KEY_IDX: usize = 7;
    const _RECORD_COUNT_IDX: usize = 8;

    // the root tables list always uses this hardcoded schema
    fn make_schema() -> GbfTableSchema {
        let mut schema = GbfTableSchema::new("Master table".into(), "TableNum".into(), GbfFieldKind::Long);
        schema.add_column(GbfFieldKind::String, "TableName".into());
        schema.add_column(GbfFieldKind::Int, "SchemaVersion".into());
        schema.add_column(GbfFieldKind::Int, "RootBufferId".into());
        schema.add_column(GbfFieldKind::Byte, "KeyType".into());
        schema.add_column(GbfFieldKind::Bytes, "FieldTypes".into());
        schema.add_column(GbfFieldKind::String, "FieldNames".into());
        schema.add_column(GbfFieldKind::Int, "IndexColumn".into());
        schema.add_column(GbfFieldKind::Long, "MaxKey".into());
        schema.add_column(GbfFieldKind::Int, "RecordCount".into());
        schema
    }

    pub fn new_empty() -> GbfTables {
        GbfTables {
            table_defs: HashMap::new(),
        }
    }

    pub fn new(gbf: &GbfFile, root_nid: i32) -> Result<GbfTables, MemViewError> {
        let base_schema = Self::make_schema();
        let tv = GbfTableView::new(gbf, &base_schema, root_nid)?;
        let tv_iter = GbfTableViewIterator::new(&tv, i64::MIN)?;

        let mut table_defs: HashMap<String, GbfTableDef> = HashMap::new();

        for item in tv_iter {
            let item_uw = item?;

            let name = item_uw.get_string(Self::TABLE_NAME_IDX)?;
            let root_buffer_id = item_uw.get_int(Self::ROOT_BUFFER_ID_IDX)?;
            let key_type = item_uw.get_byte(Self::KEY_TYPE_IDX)?;
            let field_types_buf = item_uw.get_bytes(Self::FIELD_TYPES_IDX)?;
            let mut field_names_str = item_uw.get_string(Self::FIELD_NAMES_IDX)?;

            // we need to both remove the trailing ; from field_names_str and extract
            // and remove the key name (which is not part of field_types_buf)
            let key_name: String;
            if let Some(pos) = field_names_str.find(';') {
                key_name = field_names_str[..pos].to_string();
                let remaining = &field_names_str[pos + 1..];
                field_names_str = remaining.strip_suffix(';').unwrap_or(remaining).to_string();
            } else {
                // shouldn't happen?
                key_name = "Key".into();
                field_names_str = "".into();
            }

            let key_kind = match GbfFieldKind::from_u8(key_type as u8) {
                Some(v) => v,
                None => return Err(MemViewError::generic_static("read invalid key kind")),
            };

            let mut field_kinds: Vec<GbfFieldKind> = Vec::new();
            for field_type in field_types_buf {
                if field_type == 0xff {
                    // field extension indicator hit
                    // todo: handle sparse field list
                    break;
                }

                let field_kind = match GbfFieldKind::from_u8(field_type) {
                    Some(v) => v,
                    None => return Err(MemViewError::generic_static("read invalid field kind")),
                };
                field_kinds.push(field_kind);
            }

            let mut field_names: Vec<String> = Vec::new();
            if field_names_str.len() > 0 {
                for field_name in field_names_str.split(';') {
                    field_names.push(field_name.to_string());
                }
            }

            if field_kinds.len() != field_names.len() {
                let err_str = format!(
                    "field kinds and field names length mismatch ({} != {})",
                    field_kinds.len(),
                    field_names.len()
                );
                return Err(MemViewError::generic_dynamic(err_str));
            }

            let table_def_lookup_name = name.clone();

            let mut iter_schema = GbfTableSchema::new(name, key_name, key_kind);
            for (kind, name) in field_kinds.into_iter().zip(field_names.into_iter()) {
                iter_schema.add_column(kind, name);
            }

            let iter_table_def = GbfTableDef::new(iter_schema, root_buffer_id);
            table_defs.insert(table_def_lookup_name, iter_table_def);
        }

        Ok(GbfTables { table_defs })
    }
}
