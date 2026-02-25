use crate::{
    consts::arch::Endianness,
    database::{gbf::GbfFile, gbf_binary_search::BinarySearchMatch, gbf_node_kind::GbfNodeKind},
    memory::memview::MemViewError,
};

// for LONGKEY_INTERIOR
pub struct GbfLongInteriorNode<'g> {
    pub gbf: &'g GbfFile,
    pub entry_count: i32,
    pub start_addr: u64,
}

impl<'g> GbfLongInteriorNode<'g> {
    pub const HDR_KIND_LEN: u64 = 1;
    pub const HDR_ENTRY_COUNT_LEN: u64 = 4;
    pub const HDR_LEN: u64 = Self::HDR_KIND_LEN + Self::HDR_ENTRY_COUNT_LEN;

    pub const KEY_LEN: u64 = 8;
    pub const VALUE_LEN: u64 = 4;
    pub const ENTRY_LEN: u64 = Self::KEY_LEN + Self::VALUE_LEN;

    pub fn new(gbf: &'g GbfFile, nid: i32) -> Result<GbfLongInteriorNode<'g>, MemViewError> {
        let endian = Endianness::BigEndian; // always big endian
        let at = &mut gbf.get_buffer_address(nid);
        let start_addr = *at;

        let node_kind = gbf.mv.read_u8(at)?;
        if node_kind != GbfNodeKind::LONGKEY_INTERIOR {
            let err_str = format!("unexpected block id {} while reading long key interior node", node_kind);
            return Err(MemViewError::generic_dynamic(err_str));
        }

        let key_count = gbf.mv.read_i32(at, endian)?;

        Ok(GbfLongInteriorNode {
            gbf,
            entry_count: key_count,
            start_addr,
        })
    }

    fn get_entry_offset(&self, index: i32) -> u64 {
        self.start_addr + Self::HDR_LEN + (index as u64) * Self::ENTRY_LEN
    }

    pub fn get_key_at(&self, index: i32) -> Result<i64, MemViewError> {
        let endian = Endianness::BigEndian; // always big endian
        let at = &mut self.get_entry_offset(index);
        self.gbf.mv.read_i64(at, endian)
    }

    pub fn get_value_at(&self, index: i32) -> Result<i32, MemViewError> {
        let endian = Endianness::BigEndian; // always big endian
        let at = &mut (self.get_entry_offset(index) + Self::KEY_LEN);
        self.gbf.mv.read_i32(at, endian)
    }

    pub fn find_entry_index_by_key(&self, key: i64) -> Result<BinarySearchMatch, MemViewError> {
        if self.entry_count == 0 {
            return Ok(BinarySearchMatch::Missing(0));
        } else if self.entry_count == 1 {
            return Ok(BinarySearchMatch::Found(0));
        }

        let mut min = 1;
        let mut max = self.entry_count - 1;
        while min <= max {
            let i = (min + max) / 2;
            let k = self.get_key_at(i)?;
            if k == key {
                // exact match, return now
                return Ok(BinarySearchMatch::Found(i));
            } else if k < key {
                // search right half
                min = i + 1;
            } else {
                // search left half
                max = i - 1;
            }
        }

        Ok(BinarySearchMatch::Missing(min))
    }

    pub fn get_entry(&self, key: i64) -> Result<i32, MemViewError> {
        let entry_idx = match self.find_entry_index_by_key(key)? {
            BinarySearchMatch::Found(v) => v,
            BinarySearchMatch::Missing(_) => {
                let err_str = format!("found invalid interior node that is either empty or corrupt");
                return Err(MemViewError::generic_dynamic(err_str));
            }
        };
        self.get_value_at(entry_idx)
    }
}
