use crate::sleigh::constructor::Constructor;
use crate::sleigh::consts::{AttributeId, ElementId};
use crate::sleigh::decision::Decision;
use crate::sleigh::sla_file::{Symbol, SymbolInner};
use crate::sleigh::sla_reader::{SlaBinReader, SlaElement};

pub struct SubtableSym {
    pub ctors: Vec<Constructor>,
    pub decision: Decision,
}

impl SubtableSym {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> Symbol {
        let name = elem.as_str_or(AttributeId::Name, "");
        let id = elem.as_uint_or(AttributeId::Id, 0) as u32;
        let scope = elem.as_uint_or(AttributeId::Scope, 0) as u32;
        let numct = elem.as_int_or(AttributeId::Numct, 0) as i32;
        reader.seek_elem_children_start(elem);

        let mut ctors_left = numct;
        let mut decisions_left = 1;

        let mut ctors: Vec<Constructor> = Vec::new();
        let mut decision: Option<Decision> = None;
        for child in reader.read_elem_children(elem.epos) {
            if ctors_left > 0 {
                ctors_left -= 1;
                if child.id != ElementId::Constructor {
                    panic!("expected constructor element");
                }
                ctors.push(Constructor::new(reader, &child));
            } else if decisions_left > 0 {
                // skip these
                decisions_left -= 1;
                if child.id != ElementId::Decision {
                    panic!("expected decision element");
                }
                decision = Some(Decision::new(reader, &child));
            } else {
                panic!("all scopes and symbols read but some elements still exist");
            }
        }

        if decision.is_none() {
            panic!("decision not found");
        }

        reader.read_elem_end(elem.id);
        Symbol {
            name,
            id,
            scope,
            inner: SymbolInner::SubtableSym(Box::new(SubtableSym {
                ctors,
                decision: decision.unwrap(),
            })),
        }
    }
}