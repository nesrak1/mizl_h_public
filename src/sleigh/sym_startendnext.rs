use crate::sleigh::consts::AttributeId;
use crate::sleigh::sla_reader::{SlaBinReader, SlaElement};
use super::sla_file::{SymbolInner, Symbol};

pub struct StartSym {}

pub struct EndSym {}

pub struct Next2Sym {}

impl StartSym {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> Symbol {
        let name = elem.as_str_or(AttributeId::Name, "");
        let id = elem.as_uint_or(AttributeId::Id, 0) as u32;
        let scope = elem.as_uint_or(AttributeId::Scope, 0) as u32;
        reader.seek_elem_children_start(elem);

        reader.read_elem_end(elem.id);
        Symbol {
            name,
            id,
            scope,
            inner: SymbolInner::StartSym,
        }
    }
}

impl EndSym {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> Symbol {
        let name = elem.as_str_or(AttributeId::Name, "");
        let id = elem.as_uint_or(AttributeId::Id, 0) as u32;
        let scope = elem.as_uint_or(AttributeId::Scope, 0) as u32;
        reader.seek_elem_children_start(elem);

        reader.read_elem_end(elem.id);
        Symbol {
            name,
            id,
            scope,
            inner: SymbolInner::EndSym,
        }
    }
}

impl Next2Sym {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> Symbol {
        let name = elem.as_str_or(AttributeId::Name, "");
        let id = elem.as_uint_or(AttributeId::Id, 0) as u32;
        let scope = elem.as_uint_or(AttributeId::Scope, 0) as u32;
        reader.seek_elem_children_start(elem);

        reader.read_elem_end(elem.id);
        Symbol {
            name,
            id,
            scope,
            inner: SymbolInner::Next2Sym,
        }
    }
}

