use super::sla_file::{Symbol, SymbolInner};
use crate::sleigh::consts::AttributeId;
use crate::sleigh::expression::Expression;
use crate::sleigh::sla_reader::{SlaBinReader, SlaElement};

pub struct ContextSym {
    pub varnode: u32,
    pub low: i32,
    pub high: i32,
    pub flow: bool,
    pub patexp: Expression,
}

impl ContextSym {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> Symbol {
        let name = elem.as_str_or(AttributeId::Name, "");
        let id = elem.as_uint_or(AttributeId::Id, 0) as u32;
        let scope = elem.as_uint_or(AttributeId::Scope, 0) as u32;
        let varnode = elem.as_uint_or(AttributeId::Varnode, 0) as u32;
        let low = elem.as_int_or(AttributeId::Low, 0) as i32;
        let high = elem.as_int_or(AttributeId::High, 0) as i32;
        let flow = elem.as_bool_or(AttributeId::Varnode, false);
        reader.seek_elem_children_start(elem);

        let mut child_iter = reader.read_elem_children(elem.epos);
        let patexp_elem = child_iter.next().expect("pattern expression missing");
        let patexp = Expression::new(reader, &patexp_elem);

        reader.read_elem_end(elem.id);
        Symbol {
            name,
            id,
            scope,
            inner: SymbolInner::ContextSym(Box::new(ContextSym {
                varnode,
                low,
                high,
                flow,
                patexp,
            })),
        }
    }
}
