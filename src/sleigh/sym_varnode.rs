use crate::sleigh::consts::AttributeId;
use crate::sleigh::sla_reader::{SlaBinReader, SlaElement, SpaceInfo};
use super::sla_file::{SymbolInner, Symbol};

pub struct VarnodeSym {
    pub space: SpaceInfo,
    pub offset: u32,
    pub size: i32,
}

impl VarnodeSym {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> Symbol {
        let name = elem.as_str_or(AttributeId::Name, "");
        let id = elem.as_uint_or(AttributeId::Id, 0) as u32;
        let scope = elem.as_uint_or(AttributeId::Scope, 0) as u32;
        let space = elem.as_space(AttributeId::Space);
        let offset = elem.as_uint_or(AttributeId::Off, 0) as u32;
        let size = elem.as_int_or(AttributeId::Size, 0) as i32;
        reader.seek_elem_children_start(elem);

        reader.read_elem_end(elem.id);
        Symbol {
            name,
            id,
            scope,
            inner: SymbolInner::VarnodeSym(Box::new(VarnodeSym {
                space,
                offset,
                size,
            })),
        }
    }
}