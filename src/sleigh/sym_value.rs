use crate::sleigh::consts::AttributeId;
use crate::sleigh::expression::Expression;
use crate::sleigh::sla_reader::{SlaBinReader, SlaElement};
use super::sla_file::{SymbolInner, Symbol};

pub struct ValueSym {
    pub patexp: Expression,
}

impl ValueSym {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> Symbol {
        let name = elem.as_str_or(AttributeId::Name, "");
        let id = elem.as_uint_or(AttributeId::Id, 0) as u32;
        let scope = elem.as_uint_or(AttributeId::Scope, 0) as u32;
        reader.seek_elem_children_start(elem);

        let mut child_iter = reader.read_elem_children(elem.epos);
        let patexp_elem = child_iter.next().expect("pattern expression missing");
        let patexp = Expression::new(reader, &patexp_elem);

        reader.read_elem_end(elem.id);
        Symbol {
            name,
            id,
            scope,
            inner: SymbolInner::ValueSym(Box::new(ValueSym {
                patexp,
            })),
        }
    }
}