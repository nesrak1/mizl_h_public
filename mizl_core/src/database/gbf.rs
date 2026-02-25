use crate::{
    consts::arch::Endianness,
    database::{gbf_db_parms::GbfDbParms, gbf_node_kind::GbfNodeKind, gbf_tables::GbfTables},
    memory::memview::{MemView, MemViewError},
};

// buffers = plain data (block size - buffer prefix size)
// block = prefix + plain data (block size)

// the root object for a GBF database
pub struct GbfFile {
    pub magic: u64,
    pub file_id: i64,
    pub format_version: i32,
    pub block_size: i32,
    pub block_count: i32,
    pub first_free_buffer_idx: i32,
    pub db_parms: GbfDbParms,
    pub tables: GbfTables,
    //
    pub mv: Box<dyn MemView>,
}

impl GbfFile {
    pub const BLOCK_PREFIX_SIZE: u64 = 1 + 4;

    pub fn new(mv: Box<dyn MemView>, at: &mut u64) -> Result<GbfFile, MemViewError> {
        let endian = Endianness::BigEndian; // always big endian

        let magic = mv.read_u64(at, endian)?;
        let file_id = mv.read_i64(at, endian)?;
        let format_version = mv.read_i32(at, endian)?;
        let block_size = mv.read_i32(at, endian)?;
        let first_free_buffer_idx = mv.read_i32(at, endian)?;

        let db_parms_block_idx = 0; // always 0
        let db_parms_kind = Self::read_block_kind_static(&mv, db_parms_block_idx, block_size)?;
        if db_parms_kind != GbfNodeKind::CHAINED_BUFFER_DATA {
            let err_str = format!(
                "expected block id {}, found {}",
                GbfNodeKind::CHAINED_BUFFER_DATA,
                db_parms_kind
            );
            return Err(MemViewError::generic_dynamic(err_str));
        }

        let at_db_parms = &mut Self::get_buffer_address_static(0, block_size);
        let db_parms = GbfDbParms::read(&mv, at_db_parms)?;

        let mv_size = mv.max_address()?;
        if mv_size != u64::MAX {
            if (mv_size % (block_size as u64)) != 0 {
                let err_str = format!(
                    "invalid padding for size {} (expected {} bytes of alignment)",
                    mv_size, block_size
                );
                return Err(MemViewError::generic_dynamic(err_str));
            }
        }

        let block_count = ((mv_size / (block_size as u64)) - 1) as i32;

        let mut gbf_file = GbfFile {
            magic,
            file_id,
            format_version,
            block_size,
            block_count,
            first_free_buffer_idx,
            db_parms,
            tables: GbfTables::new_empty(),
            mv,
        };

        // now read schema definition table
        let root_nid = gbf_file.db_parms.values[GbfDbParms::MASTER_TABLE_ROOT_BUFFER_ID_PARM];
        gbf_file.tables = GbfTables::new(&gbf_file, root_nid)?;

        Ok(gbf_file)
    }

    pub fn read_block_kind_and_addr(&self, block_id: i32) -> Result<(u8, u64), MemViewError> {
        let at = &mut self.get_buffer_address(block_id);
        let kind = self.mv.read_u8(at)?;
        Ok((kind, *at))
    }

    pub fn read_block_kind(&self, block_id: i32) -> Result<u8, MemViewError> {
        Self::read_block_kind_static(&self.mv, block_id, self.block_size)
    }

    fn read_block_kind_static(mv: &Box<dyn MemView>, block_id: i32, block_size: i32) -> Result<u8, MemViewError> {
        let at = &mut Self::get_buffer_address_static(block_id, block_size);
        mv.read_u8(at)
    }

    pub fn get_block_address(&self, block_id: i32) -> u64 {
        Self::get_block_address_static(block_id, self.block_size)
    }

    fn get_block_address_static(block_id: i32, block_size: i32) -> u64 {
        ((block_id + 1) as u64) * (block_size as u64)
    }

    pub fn get_buffer_address(&self, block_id: i32) -> u64 {
        Self::get_buffer_address_static(block_id, self.block_size)
    }

    fn get_buffer_address_static(block_id: i32, block_size: i32) -> u64 {
        Self::get_block_address_static(block_id, block_size) + Self::BLOCK_PREFIX_SIZE
    }

    pub fn get_buffer_size(&self) -> u64 {
        (self.block_size as u64) - Self::BLOCK_PREFIX_SIZE
    }
}
