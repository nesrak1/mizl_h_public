use crate::sleigh::consts::AttributeId;
use crate::sleigh::sla_reader::{SlaBinReader, SlaElement};
use super::sla_file::{SymbolInner, Symbol};

pub struct UseropSym {
    pub index: i32,
}

impl UseropSym {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> Symbol {
        let name = elem.as_str_or(AttributeId::Name, "");
        let id = elem.as_uint_or(AttributeId::Id, 0) as u32;
        let scope = elem.as_uint_or(AttributeId::Scope, 0) as u32;
        let index = elem.as_int_or(AttributeId::Index, 0) as i32;
        reader.seek_elem_children_start(elem);

        reader.read_elem_end(elem.id);
        Symbol {
            name,
            id,
            scope,
            inner: SymbolInner::Userop(Box::new(UseropSym {
                index,
            })),
        }
    }
}