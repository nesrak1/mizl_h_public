use crate::{
    consts::arch::Endianness,
    database::{
        gbf::GbfFile,
        gbf_binary_search::BinarySearchMatch,
        gbf_node_kind::GbfNodeKind,
        gbf_record::{GbfFieldValue, GbfRecord},
        gbf_table_schema::GbfTableSchema,
    },
    memory::memview::MemViewError,
};

// for LONGKEY_FIXED_REC
pub struct GbfLongFixedNode<'a> {
    pub gbf: &'a GbfFile,
    pub entry_count: i32,
    pub prev_leaf_nid: i32,
    pub next_leaf_nid: i32,
    pub start_addr: u64,
    pub value_len: i32,
}

impl<'g> GbfLongFixedNode<'g> {
    pub const HDR_KIND_LEN: u64 = 1;
    pub const HDR_ENTRY_COUNT_LEN: u64 = 4;
    pub const HDR_PREV_LEAF_LEN: u64 = 4;
    pub const HDR_NEXT_LEAF_LEN: u64 = 4;
    pub const HDR_LEN: u64 =
        Self::HDR_KIND_LEN + Self::HDR_ENTRY_COUNT_LEN + Self::HDR_PREV_LEAF_LEN + Self::HDR_NEXT_LEAF_LEN;

    pub const KEY_LEN: u64 = 8;

    pub fn new(gbf: &'g GbfFile, nid: i32, value_len: i32) -> Result<GbfLongFixedNode<'g>, MemViewError> {
        let endian = Endianness::BigEndian; // always big endian
        let at = &mut gbf.get_buffer_address(nid);
        let start_addr = *at;

        let node_kind = gbf.mv.read_u8(at)?;
        if node_kind != GbfNodeKind::LONGKEY_FIXED_REC {
            let err_str = format!("unexpected block id {} while reading long key fixed node", node_kind);
            return Err(MemViewError::generic_dynamic(err_str));
        }

        let key_count = gbf.mv.read_i32(at, endian)?;
        let prev_leaf_nid = gbf.mv.read_i32(at, endian)?;
        let next_leaf_nid = gbf.mv.read_i32(at, endian)?;

        Ok(GbfLongFixedNode {
            gbf,
            entry_count: key_count,
            prev_leaf_nid,
            next_leaf_nid,
            start_addr,
            value_len,
        })
    }

    fn get_entry_offset(&self, index: i32) -> u64 {
        self.start_addr + Self::HDR_LEN + (index as u64) * (Self::KEY_LEN + self.value_len as u64)
    }

    pub fn get_key_at(&self, index: i32) -> Result<i64, MemViewError> {
        let endian = Endianness::BigEndian; // always big endian
        let at = &mut self.get_entry_offset(index);
        self.gbf.mv.read_i64(at, endian)
    }

    // keeping as a result in case we make this a trait
    pub fn get_value_addr_at(&self, index: i32) -> Result<u64, MemViewError> {
        let value_addr = self.get_entry_offset(index) + Self::KEY_LEN;
        Ok(value_addr)
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

    pub fn get_entry_by_index(
        &self,
        key: i64,
        index: i32,
        schema: &GbfTableSchema,
    ) -> Result<Option<GbfRecord>, MemViewError> {
        let at = &mut self.get_value_addr_at(index)?;
        let record = schema.read(GbfFieldValue::Long(key), &self.gbf.mv, at)?;
        Ok(Some(record))
    }

    pub fn get_entry(&self, key: i64, schema: &GbfTableSchema) -> Result<Option<GbfRecord>, MemViewError> {
        let entry_idx = match self.find_entry_index_by_key(key)? {
            BinarySearchMatch::Found(v) => v,
            BinarySearchMatch::Missing(_) => return Ok(None), // no exact match in key list
        };

        self.get_entry_by_index(key, entry_idx, schema)
    }

    fn get_prev_node_last_entry(&self, key: i64, schema: &GbfTableSchema) -> Result<Option<GbfRecord>, MemViewError> {
        // largest entry from previous node
        if self.prev_leaf_nid == -1 {
            return Ok(None); // nothing left of us, so stop the search
        }

        let prev_node = GbfLongFixedNode::new(&self.gbf, self.prev_leaf_nid, self.value_len)?;
        if prev_node.entry_count < 1 {
            return Ok(None); // just in case, there are no entries in this node
        }

        prev_node.get_entry_by_index(key, prev_node.entry_count - 1, schema)
    }

    fn get_next_node_first_entry(&self, key: i64, schema: &GbfTableSchema) -> Result<Option<GbfRecord>, MemViewError> {
        // smallest entry from next node
        if self.next_leaf_nid == -1 {
            return Ok(None); // nothing right of us, so stop the search
        }

        let next_node = GbfLongFixedNode::new(&self.gbf, self.next_leaf_nid, self.value_len)?;
        if next_node.entry_count < 1 {
            return Ok(None); // just in case, there are no entries in this node
        }

        next_node.get_entry_by_index(key, 0, schema)
    }

    pub fn get_entry_at_before(&self, key: i64, schema: &GbfTableSchema) -> Result<Option<GbfRecord>, MemViewError> {
        match self.find_entry_index_by_key(key)? {
            BinarySearchMatch::Found(node_entry_idx) => {
                // found exact match, take it
                self.get_entry_by_index(key, node_entry_idx, schema)
            }
            BinarySearchMatch::Missing(node_entry_idx) => {
                let result_index = node_entry_idx - 1;
                let real_key = self.get_key_at(result_index)?;
                if result_index < 0 {
                    self.get_prev_node_last_entry(real_key, schema)
                } else {
                    self.get_entry_by_index(real_key, result_index, schema)
                }
            }
        }
    }

    pub fn get_entry_before(&self, key: i64, schema: &GbfTableSchema) -> Result<Option<GbfRecord>, MemViewError> {
        match self.find_entry_index_by_key(key)? {
            BinarySearchMatch::Found(node_entry_idx) => {
                // found exact match, take previous one
                let result_index = node_entry_idx - 1;
                if result_index < 0 {
                    self.get_prev_node_last_entry(key, schema)
                } else {
                    self.get_entry_by_index(key, node_entry_idx, schema)
                }
            }
            BinarySearchMatch::Missing(node_entry_idx) => {
                let result_index = node_entry_idx - 1;
                let real_key = self.get_key_at(result_index)?;
                if result_index < 0 {
                    self.get_prev_node_last_entry(real_key, schema)
                } else {
                    self.get_entry_by_index(real_key, result_index, schema)
                }
            }
        }
    }

    pub fn get_entry_at_after(&self, key: i64, schema: &GbfTableSchema) -> Result<Option<GbfRecord>, MemViewError> {
        match self.find_entry_index_by_key(key)? {
            BinarySearchMatch::Found(node_entry_idx) => {
                // found exact match, take it
                self.get_entry_by_index(key, node_entry_idx, schema)
            }
            BinarySearchMatch::Missing(node_entry_idx) => {
                let result_index = node_entry_idx;
                let real_key = self.get_key_at(result_index)?;
                if result_index >= self.entry_count {
                    self.get_next_node_first_entry(real_key, schema)
                } else {
                    self.get_entry_by_index(real_key, result_index, schema)
                }
            }
        }
    }

    pub fn get_entry_after(&self, key: i64, schema: &GbfTableSchema) -> Result<Option<GbfRecord>, MemViewError> {
        match self.find_entry_index_by_key(key)? {
            BinarySearchMatch::Found(node_entry_idx) => {
                // found exact match, take next one
                let result_index = node_entry_idx + 1;
                if result_index >= self.entry_count {
                    self.get_next_node_first_entry(key, schema)
                } else {
                    self.get_entry_by_index(key, result_index, schema)
                }
            }
            BinarySearchMatch::Missing(node_entry_idx) => {
                let result_index = node_entry_idx;
                let real_key = self.get_key_at(result_index)?;
                if result_index >= self.entry_count {
                    self.get_next_node_first_entry(real_key, schema)
                } else {
                    self.get_entry_by_index(real_key, result_index, schema)
                }
            }
        }
    }
}

// ////////////////////////////////////

pub struct GbfLongFixedIterator<'g, 's> {
    cur_node: GbfLongFixedNode<'g>,
    cur_node_idx: i32,
    schema: &'s GbfTableSchema,
}

impl<'g, 's> GbfLongFixedIterator<'g, 's> {
    pub fn new(
        cur_node: GbfLongFixedNode<'g>,
        cur_node_idx: i32,
        schema: &'s GbfTableSchema,
    ) -> GbfLongFixedIterator<'g, 's> {
        GbfLongFixedIterator {
            cur_node,
            cur_node_idx,
            schema,
        }
    }
}

impl<'g, 's> Iterator for GbfLongFixedIterator<'g, 's> {
    type Item = Result<GbfRecord, MemViewError>;

    fn next(&mut self) -> Option<Self::Item> {
        // get value at cur index
        let key = match self.cur_node.get_key_at(self.cur_node_idx) {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        };
        let entry_maybe = match self.cur_node.get_entry_by_index(key, self.cur_node_idx, self.schema) {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        };
        let entry = match entry_maybe {
            Some(v) => v,
            None => return None, // shouldn't happen
        };

        // move cur_node/cur_node_idx ahead one
        if (self.cur_node_idx + 1) < self.cur_node.entry_count {
            // next index is still within this node
            self.cur_node_idx += 1;
        } else {
            if self.cur_node.next_leaf_nid == -1 {
                return None; // we've hit the end
            }

            self.cur_node =
                match GbfLongFixedNode::new(&self.cur_node.gbf, self.cur_node.next_leaf_nid, self.cur_node.value_len) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };

            if self.cur_node.entry_count < 1 {
                return None; // shouldn't happen
            }

            self.cur_node_idx = 0;
        }

        Some(Ok(entry))
    }
}
