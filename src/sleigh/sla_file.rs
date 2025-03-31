use super::{
    consts::ElementId,
    sla_reader::{SlaBinReader, SlaElement},
};
use crate::sleigh::consts::AttributeId;
use crate::sleigh::sym_context::ContextSym;
use crate::sleigh::sym_operand::OperandSym;
use crate::sleigh::sym_startendnext::{EndSym, Next2Sym, StartSym};
use crate::sleigh::sym_subtable::SubtableSym;
use crate::sleigh::sym_userop::UseropSym;
use crate::sleigh::sym_value::ValueSym;
use crate::sleigh::sym_valuemap::ValuemapSym;
use crate::sleigh::sym_varlist::VarlistSym;
use crate::sleigh::sym_varnode::VarnodeSym;
use flate2::read::ZlibDecoder;
use std::collections::{HashMap, VecDeque};
use std::{
    fmt::{self, Debug, Display},
    io::Read,
};

pub enum SymbolInner {
    OperandSym(Box<OperandSym>),
    VarnodeSym(Box<VarnodeSym>),
    Userop(Box<UseropSym>),
    ValueSym(Box<ValueSym>),
    ContextSym(Box<ContextSym>),
    EndSym,
    EpsilonSym,
    NameSym,
    Next2Sym,
    StartSym,
    SubtableSym(Box<SubtableSym>),
    ValuemapSym(Box<ValuemapSym>),
    VarlistSym(Box<VarlistSym>),
}

pub struct Symbol {
    pub name: String,
    pub id: u32,
    pub scope: u32,
    pub inner: SymbolInner,
}

pub struct SourceFile {
    pub name: String,
    pub index: i32,
}

#[derive(Debug)]
pub enum AddrSpaceType {
    Normal,
    Unique,
    Other,
}

impl Display for AddrSpaceType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

pub struct Space {
    pub space_type: AddrSpaceType,
    pub name: String,
    pub index: i32,
    pub big_endian: bool,
    pub delay: i32,
    pub size: i32,
    pub physical: bool,
}

pub struct Scope {
    pub id: u32,
    pub parent: u32,
    pub lookup: HashMap<String, usize>,
}

pub struct SymbolTable {
    pub scopes: Vec<Scope>,
    pub symbols: Vec<Symbol>,
}

pub struct Sleigh {
    pub version: i32,
    pub big_endian: bool,
    pub align: i32,
    pub uniq_base: u64,
    pub max_delay: u32,
    pub uniq_mask: u32,
    pub num_sections: u32,
    pub source_files: Vec<SourceFile>,
    pub default_space: String,
    pub spaces: Vec<Space>,
    pub symbol_table: SymbolTable,
}

impl Sleigh {
    pub fn new(data: &[u8]) -> Sleigh {
        assert!(data.len() > 4);
        assert!(data[0] == 0x73 && data[1] == 0x6c && data[2] == 0x61 && data[3] >= 4);
        if data[3] != 4 {
            panic!("unsupported sleigh type");
        }

        let mut decoder = ZlibDecoder::new(&data[4..]);
        let mut buf: Vec<u8> = Vec::new();
        if decoder.read_to_end(&mut buf).is_err() {
            panic!("zlib decode failed");
        }

        let reader = SlaBinReader::new(buf);
        Self::decode(&reader)
    }

    fn decode(reader: &SlaBinReader) -> Sleigh {
        let sleigh_elem = reader.read_elem_start(ElementId::Sleigh);

        // attribs
        let version = sleigh_elem.as_int_or(AttributeId::Version, 0) as i32;
        let big_endian = sleigh_elem.as_bool_or(AttributeId::Bigendian, false);
        let align = sleigh_elem.as_int_or(AttributeId::Align, 1) as i32;
        let uniq_base = sleigh_elem.as_int_or(AttributeId::Uniqbase, 0) as u64;
        let max_delay = sleigh_elem.as_int_or(AttributeId::Maxdelay, 0) as u32;
        let uniq_mask = sleigh_elem.as_int_or(AttributeId::Uniqmask, 0) as u32;
        let num_sections = sleigh_elem.as_int_or(AttributeId::Numsections, 0) as u32;

        reader.seek_elem_children_start(&sleigh_elem);

        // elems
        //// source files
        let source_files_elem = reader.read_elem_start(ElementId::Sourcefiles);

        let mut source_files: Vec<SourceFile> = Vec::new();
        for item in reader.read_elem_children(source_files_elem.epos) {
            source_files.push(SourceFile::new(reader, &item));
        }

        reader.read_elem_end(source_files_elem.id);

        //// spaces
        let spaces_elem = reader.read_elem_start(ElementId::Spaces);
        let default_space = spaces_elem.as_str_or(AttributeId::Defaultspace, "");

        let mut spaces: Vec<Space> = Vec::new();
        for item in reader.read_elem_children(spaces_elem.epos) {
            spaces.push(Space::new(reader, &item));
        }

        reader.read_elem_end(spaces_elem.id);

        //// symbol table
        let symbol_table_elem = reader.read_elem_start(ElementId::SymbolTable);
        let symbol_table = SymbolTable::new(reader, &symbol_table_elem);

        Sleigh {
            version,
            big_endian,
            align,
            uniq_base,
            max_delay,
            uniq_mask,
            num_sections,
            source_files,
            default_space,
            spaces,
            symbol_table,
        }
    }

    pub fn get_context_size(&self) -> i32 {
        // I guess the sleigh file has no direct way to access
        // which varnode is the context register? so we just
        // look for a contextsym and find the referenced context
        // register (assuming there are any)
        for sym in &self.symbol_table.symbols {
            if let SymbolInner::ContextSym(ctx_sym) = &sym.inner {
                let base_ctx_id = ctx_sym.varnode;
                let context_reg_box = &self.symbol_table.symbols[base_ctx_id as usize];
                let context_reg_sym = if let SymbolInner::VarnodeSym(v) = &context_reg_box.inner {
                    v
                } else {
                    panic!("not a varnode symbol")
                };

                return context_reg_sym.size;
            }
        }
        return 0;
    }

    // expects little endian order, but I haven't seen
    // big endian archs with overlapping registers yet
    pub fn get_varnodes_by_offset(&self) -> HashMap<u32, Vec<u32>> {
        let mut map = HashMap::new();
        for sym in &self.symbol_table.symbols {
            if let SymbolInner::VarnodeSym(varnode_sym) = &sym.inner {
                map.entry(varnode_sym.offset).or_insert(vec![]).push(sym.id);
            }
        }
        map
    }
}

impl SourceFile {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> SourceFile {
        let name = elem.as_str_or(AttributeId::Name, "");
        let index = elem.as_int_or(AttributeId::Index, 0) as i32;

        reader.read_elem_end(elem.id);
        SourceFile { name, index }
    }
}

impl Space {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> Space {
        let space_type = match elem.id {
            ElementId::Space => AddrSpaceType::Normal,
            ElementId::SpaceUnique => AddrSpaceType::Unique,
            ElementId::SpaceOther => AddrSpaceType::Other,
            _ => panic!("not a valid space type"),
        };

        let name = elem.as_str_or(AttributeId::Name, "");
        let index = elem.as_int_or(AttributeId::Index, 0) as i32;
        let big_endian = elem.as_bool_or(AttributeId::Bigendian, false);
        let delay = elem.as_int_or(AttributeId::Delay, 0) as i32;
        let size = elem.as_int_or(AttributeId::Size, 0) as i32;
        let physical = elem.as_bool_or(AttributeId::Physical, false);
        reader.read_elem_end(elem.id);

        Space {
            space_type,
            name,
            index,
            big_endian,
            delay,
            size,
            physical,
        }
    }
}

impl SymbolTable {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> SymbolTable {
        let scope_size = elem.as_int_or(AttributeId::Scopesize, 0) as i32;
        let symbol_size = elem.as_int_or(AttributeId::Symbolsize, 0) as i32;
        reader.seek_elem_children_start(elem);

        let mut scopes_left = scope_size;
        let mut symbol_heads_left = symbol_size;
        let mut symbols_left = symbol_size;

        let mut scopes: Vec<Scope> = Vec::with_capacity(scope_size as usize);
        let mut symbols: Vec<Symbol> = Vec::with_capacity(symbol_size as usize);
        let mut symbol_head_infos: VecDeque<(String, u32)> = VecDeque::new();
        for child in reader.read_elem_children(elem.epos) {
            if scopes_left > 0 {
                scopes_left -= 1;
                if child.id != ElementId::Scope {
                    panic!("expected scope element");
                }
                scopes.push(Scope::new(reader, &child));
            } else if symbol_heads_left > 0 {
                symbol_heads_left -= 1;
                let name = child.as_str_or(AttributeId::Name, "");
                let scope = child.as_uint_or(AttributeId::Scope, 0) as u32;
                symbol_head_infos.push_back((name, scope));
                reader.seek_elem_children_start(&child);
                reader.read_elem_end(child.id);
            } else if symbols_left > 0 {
                symbols_left -= 1;
                let mut sym: Symbol = match child.id {
                    ElementId::OperandSym => OperandSym::new(reader, &child),
                    ElementId::VarnodeSym => VarnodeSym::new(reader, &child),
                    ElementId::Userop => UseropSym::new(reader, &child),
                    ElementId::ValueSym => ValueSym::new(reader, &child),
                    ElementId::ContextSym => ContextSym::new(reader, &child),
                    ElementId::EndSym => EndSym::new(reader, &child),
                    // SlaElementId::EpsilonSym => ,
                    // SlaElementId::NameSym => ,
                    ElementId::Next2Sym => Next2Sym::new(reader, &child),
                    ElementId::StartSym => StartSym::new(reader, &child),
                    ElementId::SubtableSym => SubtableSym::new(reader, &child),
                    ElementId::ValuemapSym => ValuemapSym::new(reader, &child),
                    ElementId::VarlistSym => VarlistSym::new(reader, &child),
                    _ => panic!("{} symbol not supported", child.id),
                };

                // restore info from head
                (sym.name, sym.scope) = symbol_head_infos.pop_front().expect("symbol heads was empty");

                scopes[sym.scope as usize].add_symbol(sym.name.as_str(), symbols.len());
                symbols.push(sym);
            } else {
                panic!("all scopes and symbols read but some elements still exist");
            }
        }

        reader.read_elem_end(elem.id);
        SymbolTable { scopes, symbols }
    }
}

impl Scope {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> Scope {
        let id = elem.as_uint_or(AttributeId::Id, 0) as u32;
        let parent = elem.as_uint_or(AttributeId::Name, 0) as u32;

        reader.read_elem_end(elem.id);
        Scope {
            id,
            parent,
            lookup: HashMap::new(),
        }
    }

    pub fn add_symbol(&mut self, name: &str, id: usize) {
        self.lookup.insert(name.to_owned(), id);
    }
}
