use crate::{
    database::{
        gbf::GbfFile,
        gbf_binary_search::BinarySearchMatch,
        gbf_long_fixed_node::{GbfLongFixedIterator, GbfLongFixedNode},
        gbf_long_interior_node::GbfLongInteriorNode,
        gbf_long_var_node::{GbfLongVarIterator, GbfLongVarNode},
        gbf_node_kind::GbfNodeKind,
        gbf_record::GbfRecord,
        gbf_table_schema::GbfTableSchema,
    },
    memory::memview::MemViewError,
};

// a table view that reads a specific table
pub struct GbfTableView<'g, 's> {
    gbf: &'g GbfFile,
    schema: &'s GbfTableSchema,
    root_nid: i32,
}

impl<'g, 's> GbfTableView<'g, 's> {
    pub fn new(
        gbf: &'g GbfFile,
        schema: &'s GbfTableSchema,
        root_nid: i32,
    ) -> Result<GbfTableView<'g, 's>, MemViewError> {
        // should error if root_nid is invalid
        Ok(GbfTableView { gbf, schema, root_nid })
    }

    pub fn get_record_at_long(&self, key: i64) -> Result<Option<GbfRecord>, MemViewError> {
        let leaf_node_nid = self.get_leaf_node_long(key)?;
        let node_kind = self.gbf.read_block_kind(leaf_node_nid)?;
        match node_kind {
            GbfNodeKind::LONGKEY_VAR_REC => {
                let var_node = GbfLongVarNode::new(self.gbf, leaf_node_nid)?;
                var_node.get_entry(key, &self.schema)
            }
            GbfNodeKind::LONGKEY_FIXED_REC => {
                let fixed_node = GbfLongFixedNode::new(self.gbf, leaf_node_nid, self.schema.get_value_len())?;
                fixed_node.get_entry(key, &self.schema)
            }
            _ => {
                let err_str = format!("unexpected block id {} while finding record", node_kind);
                Err(MemViewError::generic_dynamic(err_str))
            }
        }
    }

    pub fn get_record_before_long(&self, key: i64) -> Result<Option<GbfRecord>, MemViewError> {
        let leaf_node_nid = self.get_leaf_node_long(key)?;
        let node_kind = self.gbf.read_block_kind(leaf_node_nid)?;
        match node_kind {
            GbfNodeKind::LONGKEY_VAR_REC => {
                let var_node = GbfLongVarNode::new(self.gbf, leaf_node_nid)?;
                var_node.get_entry_before(key, self.schema)
            }
            GbfNodeKind::LONGKEY_FIXED_REC => {
                let fixed_node = GbfLongFixedNode::new(self.gbf, leaf_node_nid, self.schema.get_value_len())?;
                fixed_node.get_entry_before(key, self.schema)
            }
            _ => {
                let err_str = format!("unexpected block id {} while finding record", node_kind);
                return Err(MemViewError::generic_dynamic(err_str));
            }
        }
    }

    pub fn get_record_at_before_long(&self, key: i64) -> Result<Option<GbfRecord>, MemViewError> {
        let leaf_node_nid = self.get_leaf_node_long(key)?;
        let node_kind = self.gbf.read_block_kind(leaf_node_nid)?;
        match node_kind {
            GbfNodeKind::LONGKEY_VAR_REC => {
                let var_node = GbfLongVarNode::new(self.gbf, leaf_node_nid)?;
                var_node.get_entry_at_before(key, self.schema)
            }
            GbfNodeKind::LONGKEY_FIXED_REC => {
                let fixed_node = GbfLongFixedNode::new(self.gbf, leaf_node_nid, self.schema.get_value_len())?;
                fixed_node.get_entry_at_before(key, self.schema)
            }
            _ => {
                let err_str = format!("unexpected block id {} while finding record", node_kind);
                return Err(MemViewError::generic_dynamic(err_str));
            }
        }
    }

    pub fn get_record_after_long(&self, key: i64) -> Result<Option<GbfRecord>, MemViewError> {
        let leaf_node_nid = self.get_leaf_node_long(key)?;
        let node_kind = self.gbf.read_block_kind(leaf_node_nid)?;
        match node_kind {
            GbfNodeKind::LONGKEY_VAR_REC => {
                let var_node = GbfLongVarNode::new(self.gbf, leaf_node_nid)?;
                var_node.get_entry_after(key, self.schema)
            }
            GbfNodeKind::LONGKEY_FIXED_REC => {
                let fixed_node = GbfLongFixedNode::new(self.gbf, leaf_node_nid, self.schema.get_value_len())?;
                fixed_node.get_entry_after(key, self.schema)
            }
            _ => {
                let err_str = format!("unexpected block id {} while finding record", node_kind);
                return Err(MemViewError::generic_dynamic(err_str));
            }
        }
    }

    pub fn get_record_at_after_long(&self, key: i64) -> Result<Option<GbfRecord>, MemViewError> {
        let leaf_node_nid = self.get_leaf_node_long(key)?;
        let node_kind = self.gbf.read_block_kind(leaf_node_nid)?;
        match node_kind {
            GbfNodeKind::LONGKEY_VAR_REC => {
                let var_node = GbfLongVarNode::new(self.gbf, leaf_node_nid)?;
                var_node.get_entry_at_after(key, self.schema)
            }
            GbfNodeKind::LONGKEY_FIXED_REC => {
                let fixed_node = GbfLongFixedNode::new(self.gbf, leaf_node_nid, self.schema.get_value_len())?;
                fixed_node.get_entry_at_after(key, self.schema)
            }
            _ => {
                let err_str = format!("unexpected block id {} while finding record", node_kind);
                return Err(MemViewError::generic_dynamic(err_str));
            }
        }
    }

    fn get_leaf_node_long(&self, key: i64) -> Result<i32, MemViewError> {
        // does not detect getting stuck in infinite loops
        let mut cur_nid = self.root_nid;
        loop {
            let node_kind = self.gbf.read_block_kind(cur_nid)?;
            match node_kind {
                GbfNodeKind::LONGKEY_INTERIOR => {
                    // all values point to another block
                    let interior = GbfLongInteriorNode::new(self.gbf, cur_nid)?;
                    cur_nid = interior.get_entry(key)?;
                }
                GbfNodeKind::LONGKEY_FIXED_REC | GbfNodeKind::LONGKEY_VAR_REC => {
                    // this is a leaf node, return this index
                    return Ok(cur_nid);
                }
                _ => {
                    let err_str = format!("unexpected block id {} while finding long leaf node", node_kind);
                    return Err(MemViewError::generic_dynamic(err_str));
                }
            }
        }
    }
}

// ////////////////////////////////////

enum GbfTableViewIteratorKind<'g, 's> {
    EmptyIterator,
    LongVarIterator(GbfLongVarIterator<'g, 's>),
    LongFixedIterator(GbfLongFixedIterator<'g, 's>),
}

pub struct GbfTableViewIterator<'g, 's> {
    iterator: GbfTableViewIteratorKind<'g, 's>,
}

impl<'g, 's> GbfTableViewIterator<'g, 's> {
    pub fn new(tv: &'s GbfTableView<'g, 's>, key: i64) -> Result<GbfTableViewIterator<'g, 's>, MemViewError> {
        let leaf_node_nid = tv.get_leaf_node_long(key)?;
        let node_kind = tv.gbf.read_block_kind(leaf_node_nid)?;
        let iterator: GbfTableViewIteratorKind;
        match node_kind {
            GbfNodeKind::LONGKEY_VAR_REC => {
                let var_node = GbfLongVarNode::new(tv.gbf, leaf_node_nid)?;
                if var_node.entry_count > 0 {
                    let mut entry_idx = match var_node.find_entry_index_by_key(key)? {
                        BinarySearchMatch::Found(v) => v,
                        BinarySearchMatch::Missing(v) => v, // start to right of missing key
                    };
                    if entry_idx < 0 {
                        entry_idx = 0;
                    }

                    let var_iterator = GbfLongVarIterator::new(var_node, entry_idx, tv.schema);
                    iterator = GbfTableViewIteratorKind::LongVarIterator(var_iterator);
                } else {
                    iterator = GbfTableViewIteratorKind::EmptyIterator;
                }
            }
            GbfNodeKind::LONGKEY_FIXED_REC => {
                let fixed_node = GbfLongFixedNode::new(tv.gbf, leaf_node_nid, tv.schema.get_value_len())?;
                if fixed_node.entry_count > 0 {
                    let mut entry_idx = match fixed_node.find_entry_index_by_key(key)? {
                        BinarySearchMatch::Found(v) => v,
                        BinarySearchMatch::Missing(v) => v, // start to right of missing key
                    };
                    if entry_idx < 0 {
                        entry_idx = 0;
                    }

                    let fixed_iterator = GbfLongFixedIterator::new(fixed_node, entry_idx, tv.schema);
                    iterator = GbfTableViewIteratorKind::LongFixedIterator(fixed_iterator);
                } else {
                    iterator = GbfTableViewIteratorKind::EmptyIterator;
                }
            }
            _ => {
                let err_str = format!("unsupported block id {} while iterating records", node_kind);
                return Err(MemViewError::generic_dynamic(err_str));
            }
        }

        Ok(GbfTableViewIterator { iterator })
    }
}

impl<'g, 's> Iterator for GbfTableViewIterator<'g, 's> {
    type Item = Result<GbfRecord, MemViewError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iterator {
            GbfTableViewIteratorKind::EmptyIterator => None,
            GbfTableViewIteratorKind::LongVarIterator(ref mut i) => i.next(),
            GbfTableViewIteratorKind::LongFixedIterator(ref mut i) => i.next(),
        }
    }
}
