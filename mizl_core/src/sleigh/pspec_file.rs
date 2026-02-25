use super::{
    memory::write_ctx_u32_bits_range,
    sla_file::{Sleigh, SymbolInner},
};
use roxmltree::Document;
use std::collections::HashMap;

pub struct PspecConstSetEntry {
    pub name: String,
    pub val: u64, // originally a bigint but not sure why?
}

pub struct PspecConstSet {
    pub tracked: bool,
    pub space: String,
    pub entries: Vec<PspecConstSetEntry>,
}

pub struct PspecRegister {
    pub name: String,
    pub rename: String,
    pub alias: String,
    pub group: String,
    pub hidden: bool,
    pub vector_lane_sizes: Vec<i32>,
}

pub struct Pspec {
    pub properties: HashMap<String, String>,
    pub program_counter: String,
    pub context_settings: Vec<PspecConstSet>,
    pub registers: Vec<PspecRegister>,
}

#[derive(Debug)]
pub enum PspecError {
    InvalidXml,
    InvalidFormat,
    BadState(&'static str),
}

// skip non-elements such as comments
macro_rules! cont_non_elm {
    ($condition:expr) => {
        if !($condition.is_element()) {
            continue;
        }
    };
}

impl Pspec {
    pub fn new(pspec_contents: String) -> Result<Pspec, PspecError> {
        let doc = match Document::parse(pspec_contents.as_str()) {
            Ok(v) => v,
            Err(_) => return Err(PspecError::InvalidXml),
        };
        Self::decode(doc)
    }

    fn decode(doc: Document) -> Result<Pspec, PspecError> {
        let processor_spec_elm = doc.root_element();
        if processor_spec_elm.tag_name().name() != "processor_spec" {
            return Err(PspecError::InvalidFormat);
        }

        let mut properties: HashMap<String, String> = HashMap::new();
        let mut program_counter = String::new();
        let mut context_settings: Vec<PspecConstSet> = Vec::new();
        let mut registers: Vec<PspecRegister> = Vec::new();

        for main_elm in processor_spec_elm.children() {
            cont_non_elm!(main_elm);

            let name = main_elm.tag_name().name();
            match name {
                "properties" => {
                    for prop_elm in main_elm.children() {
                        cont_non_elm!(prop_elm);

                        if prop_elm.tag_name().name() != "property" {
                            return Err(PspecError::InvalidFormat);
                        }

                        let prop_key_atr = prop_elm.attribute("key").ok_or(PspecError::InvalidFormat)?;
                        let prop_value_atr = prop_elm.attribute("value").ok_or(PspecError::InvalidFormat)?;

                        properties.insert(prop_key_atr.to_owned(), prop_value_atr.to_owned());
                    }
                }
                "programcounter" => {
                    let pc_reg_atr = main_elm.attribute("register").ok_or(PspecError::InvalidFormat)?;
                    program_counter = pc_reg_atr.to_owned();
                }
                "context_data" => {
                    for ctx_elm in main_elm.children() {
                        cont_non_elm!(ctx_elm);

                        let ctx_name = ctx_elm.tag_name().name();
                        let ctx_tracked = match ctx_name {
                            "context_set" => false,
                            "tracked_set" => true,
                            _ => return Err(PspecError::InvalidFormat),
                        };

                        let ctx_space_atr = ctx_elm.attribute("space").ok_or(PspecError::InvalidFormat)?;
                        // todo: first, last

                        let mut ctx_entries: Vec<PspecConstSetEntry> = Vec::new();
                        for set_elm in ctx_elm.children() {
                            cont_non_elm!(set_elm);

                            let set_name_atr = set_elm.attribute("name").ok_or(PspecError::InvalidFormat)?;
                            let set_val_atr = set_elm.attribute("val").ok_or(PspecError::InvalidFormat)?;
                            let set_val_int = match str::parse::<u64>(set_val_atr) {
                                Ok(v) => v,
                                Err(_) => return Err(PspecError::InvalidFormat),
                            };

                            ctx_entries.push(PspecConstSetEntry {
                                name: set_name_atr.to_owned(),
                                val: set_val_int,
                            });
                        }

                        context_settings.push(PspecConstSet {
                            tracked: ctx_tracked,
                            space: ctx_space_atr.to_owned(),
                            entries: ctx_entries,
                        });
                    }
                }
                "register_data" => {
                    for reg_elm in main_elm.children() {
                        cont_non_elm!(reg_elm);

                        if reg_elm.tag_name().name() != "register" {
                            return Err(PspecError::InvalidFormat);
                        }

                        let reg_name_atr = reg_elm.attribute("name").ok_or(PspecError::InvalidFormat)?;
                        let reg_rename_atr = reg_elm.attribute("rename").unwrap_or("");
                        let reg_alias_atr = reg_elm.attribute("alias").unwrap_or("");
                        let reg_group_atr = reg_elm.attribute("group").unwrap_or("");
                        let reg_hidden_atr = reg_elm.attribute("hidden").unwrap_or("false") == "true";
                        let reg_vls_atr = reg_elm.attribute("vector_lane_sizes").unwrap_or("0");

                        let reg_vls_list = if reg_vls_atr.len() != 0 {
                            reg_vls_atr
                                .split(",")
                                .map(|n| n.parse::<i32>().map_err(|_| PspecError::InvalidFormat))
                                .collect::<Result<Vec<i32>, _>>()?
                        } else {
                            vec![]
                        };

                        registers.push(PspecRegister {
                            name: reg_name_atr.to_owned(),
                            rename: reg_rename_atr.to_owned(),
                            alias: reg_alias_atr.to_owned(),
                            group: reg_group_atr.to_owned(),
                            hidden: reg_hidden_atr,
                            vector_lane_sizes: reg_vls_list,
                        });
                    }
                }
                _ => {}
            };
        }

        Ok(Pspec {
            properties,
            program_counter,
            context_settings,
            registers,
        })
    }

    pub fn get_initial_ctx(&self, sleigh: &Sleigh) -> Result<Vec<u32>, PspecError> {
        let root_scope = sleigh
            .symbol_table
            .scopes
            .get(0)
            .ok_or(PspecError::BadState("root scope must exist"))?;
        let symbols: &_ = &sleigh.symbol_table.symbols;

        let mut context_ctx = vec![0; sleigh.get_context_size() as usize];
        for pspec_ctx in &self.context_settings {
            if pspec_ctx.space != "ram" || pspec_ctx.tracked {
                continue;
            }

            for pspec_set in &pspec_ctx.entries {
                let name = &pspec_set.name;
                let val = pspec_set.val;

                let ctx_sym_idx = *(root_scope
                    .lookup
                    .get(name)
                    .ok_or(PspecError::BadState("pspec ctx sym didn't exist"))?);
                let symbol = &symbols[ctx_sym_idx];
                let ctx_sym = if let SymbolInner::ContextSym(v) = &symbol.inner {
                    v
                } else {
                    panic!("not a subtable symbol")
                };

                write_ctx_u32_bits_range(&mut context_ctx, ctx_sym.low, ctx_sym.high, val as u32);
            }
        }

        Ok(context_ctx)
    }
}
