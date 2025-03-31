use crate::sleigh::consts::{AttributeId, AttributeKind, ElementId};
use num::FromPrimitive;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::str;

pub struct SlaBinReader {
    buffer: Vec<u8>,
    pos: Cell<usize>,
}

#[repr(u16)]
#[derive(FromPrimitive, Clone, Copy, PartialEq, Eq)]
pub enum SpaceType {
    None,
    AddressSpace,
    StackSpace,
    JoinSpace,
    FSpecSpace,
    IopSpace,
    SpaceBase,
}

pub struct SpaceInfo {
    pub space_type: SpaceType,
    pub index: i32,
}

pub enum SlaAttributeValue {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    String(String),
    Space(SpaceInfo),
}

pub struct SlaAttribute {
    pub id: AttributeId,
    pub kind: AttributeKind,
    // remove this later
    pub spos: usize,
    // start of attribute bytes
    pub value: SlaAttributeValue,
}

pub struct SlaElement {
    pub start: bool,
    pub id: ElementId,
    // todo: fix naming of these
    // start of element bytes
    pub spos: usize,
    // start of attribute bytes
    pub apos: usize,
    // end of element bytes (after attributes)
    pub epos: usize,
    pub attrs: BTreeMap<AttributeId, SlaAttribute>,
}

// safe reader in case new attributes are added or ordering is changed
impl SlaBinReader {
    pub fn new(buffer: Vec<u8>) -> Self {
        SlaBinReader {
            buffer,
            pos: Cell::new(0),
        }
    }

    pub fn read_elem(&self) -> SlaElement {
        let start_pos = self.get_pos();

        let byte1 = self.read_u8();
        let elem_type = Self::get_element_type(byte1);
        if elem_type != 1 && elem_type != 2 {
            panic!("not an element");
        }

        let elem_id: ElementId;
        if Self::is_extended_elem(byte1) {
            let byte1e = self.read_u8();
            elem_id = FromPrimitive::from_u16(Self::get_element_id_ext(byte1, byte1e)).unwrap_or(ElementId::None);
        } else {
            elem_id = FromPrimitive::from_u16(Self::get_element_id(byte1)).unwrap_or(ElementId::None);
        }

        return if elem_type == 1 {
            let attr_pos = self.get_pos();
            let child_iter = self.read_elem_attrs(attr_pos);
            let mut attrs: BTreeMap<AttributeId, SlaAttribute> = BTreeMap::new();
            for attr in child_iter {
                attrs.insert(attr.id, attr);
            }
            let end_pos = self.get_pos(); // get attribute end pos

            SlaElement {
                start: true,
                id: elem_id,
                spos: start_pos,
                apos: attr_pos,
                epos: end_pos,
                attrs,
            }
        } else {
            let end_pos = self.get_pos();
            SlaElement {
                start: false,
                id: elem_id,
                spos: start_pos,
                apos: end_pos,
                epos: end_pos,
                attrs: BTreeMap::new(),
            }
        };
    }

    pub fn read_elem_start(&self, check_id: ElementId) -> SlaElement {
        let elem = self.read_elem();
        assert_eq!(elem.id, check_id);
        assert!(elem.start);
        return elem;
    }

    pub fn read_elem_end(&self, check_id: ElementId) -> SlaElement {
        let elem = self.read_elem();
        assert_eq!(elem.id, check_id);
        assert!(!elem.start);
        return elem;
    }

    pub fn read_elem_children(&self, epos: usize) -> impl Iterator<Item = SlaElement> + '_ {
        self.seek(epos);
        return std::iter::from_fn(move || {
            let byte1 = self.peek_u8();
            let elem_type = Self::get_element_type(byte1);
            if elem_type == 1 {
                Some(self.read_elem())
            } else if elem_type == 2 {
                None
            } else {
                panic!("not an element");
            }
        });
    }

    pub fn read_elem_attrs(&self, apos: usize) -> impl Iterator<Item = SlaAttribute> + '_ {
        self.seek(apos);
        return std::iter::from_fn(move || {
            let byte1 = self.peek_u8();
            if Self::get_element_type(byte1) != 3 {
                None
            } else {
                let attr = self.read_attr();
                Some(attr)
            }
        });
    }

    pub fn read_attr(&self) -> SlaAttribute {
        let start_pos = self.get_pos();

        let byte1 = self.read_u8();
        if Self::get_element_type(byte1) != 3 {
            panic!("not an attribute");
        }

        let attr_id: AttributeId;
        if Self::is_extended_elem(byte1) {
            let byte1e = self.read_u8();
            attr_id = FromPrimitive::from_u16(Self::get_element_id_ext(byte1, byte1e)).unwrap_or(AttributeId::None);
        } else {
            attr_id = FromPrimitive::from_u16(Self::get_element_id(byte1)).unwrap_or(AttributeId::None);
        }

        let byte2 = self.read_u8();
        let attr_kind = FromPrimitive::from_u8(Self::get_attribute_type(byte2)).unwrap_or(AttributeKind::None);

        let value: SlaAttributeValue;
        if attr_kind == AttributeKind::Boolean {
            let bool_value = self.read_attr_bool(byte2);
            value = SlaAttributeValue::Bool(bool_value);
        } else if attr_kind == AttributeKind::PositiveSignedInteger {
            let int_value = self.read_attr_int(byte2);
            value = SlaAttributeValue::Int(int_value);
        } else if attr_kind == AttributeKind::NegativeSignedInteger {
            let int_value = self.read_attr_int(byte2);
            value = SlaAttributeValue::Int(int_value);
        } else if attr_kind == AttributeKind::UnsignedInteger {
            let uint_value = self.read_attr_uint(byte2);
            value = SlaAttributeValue::UInt(uint_value);
        } else if attr_kind == AttributeKind::String {
            let str_value = self.read_attr_str(byte2);
            value = SlaAttributeValue::String(str_value.to_string());
        } else if attr_kind == AttributeKind::BasicAddressSpace {
            let space_value = self.read_attr_space(byte2);
            value = SlaAttributeValue::Space(space_value);
        } else {
            value = SlaAttributeValue::Null;
        }

        return SlaAttribute {
            id: attr_id,
            kind: attr_kind,
            spos: start_pos,
            value,
        };
    }

    fn read_attr_bool(&self, byte2: u8) -> bool {
        let attr_type = Self::get_attribute_type(byte2);
        let size = Self::get_attribute_size(byte2);
        if attr_type != AttributeKind::Boolean as u8 {
            panic!("not a uint attribute");
        }

        return size != 0;
    }

    fn read_attr_int(&self, byte2: u8) -> i64 {
        let attr_type = Self::get_attribute_type(byte2);
        let size = Self::get_attribute_size(byte2);
        let val: i64;
        if attr_type == AttributeKind::PositiveSignedInteger as u8 {
            val = self.read_sized_int(size);
        } else if attr_type == AttributeKind::NegativeSignedInteger as u8 {
            val = -self.read_sized_int(size);
        } else {
            panic!("not an int attribute");
        }

        return val;
    }

    fn read_attr_uint(&self, byte2: u8) -> u64 {
        let attr_type = Self::get_attribute_type(byte2);
        let size = Self::get_attribute_size(byte2);
        if attr_type != AttributeKind::UnsignedInteger as u8 {
            panic!("not a uint attribute");
        }

        return self.read_sized_int(size) as u64;
    }

    fn read_attr_str(&self, byte2: u8) -> &str {
        let attr_type = Self::get_attribute_type(byte2);
        let size_size = Self::get_attribute_size(byte2);
        if attr_type != AttributeKind::String as u8 {
            panic!("not a string attribute");
        }

        let size = self.read_sized_int(size_size);
        let strbuf = &self.buffer[self.get_pos()..self.get_pos() + (size as usize)];
        self.seek(self.pos.get() + size as usize);
        return match str::from_utf8(strbuf) {
            Ok(v) => v,
            Err(_) => panic!("failed to decode string"),
        };
    }

    fn read_attr_space(&self, byte2: u8) -> SpaceInfo {
        let attr_type = Self::get_attribute_type(byte2);
        let size = Self::get_attribute_size(byte2);
        let val: SpaceInfo;
        if attr_type == AttributeKind::BasicAddressSpace as u8 {
            val = SpaceInfo {
                space_type: SpaceType::AddressSpace,
                index: self.read_sized_int(size) as i32,
            }
        } else if attr_type == AttributeKind::SpecialAddressSpace as u8 {
            let code = self.read_sized_int(size) as i32;
            let space_type = match code {
                0 => SpaceType::StackSpace,
                1 => SpaceType::JoinSpace,
                2 => SpaceType::FSpecSpace,
                3 => SpaceType::IopSpace,
                4 => SpaceType::SpaceBase,
                _ => SpaceType::None,
            };
            val = SpaceInfo { space_type, index: 0 }
        } else {
            panic!("not an int attribute");
        }

        return val;
    }

    fn read_sized_int(&self, size: u8) -> i64 {
        let mut res: i64 = 0;
        for _ in 0..size {
            res <<= 7;
            let next_byte = self.read_u8();
            res |= (next_byte & 127) as i64;
        }
        return res;
    }

    fn get_element_type(byte1: u8) -> u8 {
        return (byte1 >> 6) & 3;
    }

    fn is_extended_elem(byte1: u8) -> bool {
        return ((byte1 >> 5) & 1) != 0;
    }

    fn get_element_id(byte1: u8) -> u16 {
        return (byte1 & 31) as u16;
    }

    fn get_element_id_ext(byte1: u8, byte1e: u8) -> u16 {
        return ((byte1 & 31) as u16) | ((byte1e & 127) as u16);
    }

    fn get_attribute_type(byte2: u8) -> u8 {
        return (byte2 >> 4) & 15;
    }

    fn get_attribute_size(byte2: u8) -> u8 {
        return byte2 & 15;
    }

    pub fn seek_elem_children_start(&self, elem: &SlaElement) {
        self.seek(elem.epos);
    }

    pub fn seek_elem_children_end(&self, elem: &SlaElement) {
        let mut id_stack: Vec<ElementId> = Vec::new();
        id_stack.push(elem.id);
        loop {
            let byte1 = self.peek_u8();
            let elem_type = Self::get_element_type(byte1);
            if elem_type == 2 {
                if id_stack.len() > 1 {
                    self.read_elem_end(id_stack[id_stack.len() - 1]);
                    id_stack.pop();
                } else {
                    break;
                }
            } else if elem_type == 1 {
                let new_elem = self.read_elem();
                id_stack.push(new_elem.id);
            }
        }
    }

    fn seek(&self, pos: usize) -> usize {
        if self.get_pos() < self.buffer.len() {
            self.pos.set(pos);
            return pos;
        }
        panic!("outside bounds");
    }

    fn read_u8(&self) -> u8 {
        if self.get_pos() < self.buffer.len() {
            let val = self.buffer[self.get_pos()];
            self.inc_pos();
            return val;
        }
        panic!("outside bounds");
    }

    fn peek_u8(&self) -> u8 {
        if self.get_pos() < self.buffer.len() {
            let val = self.buffer[self.get_pos()];
            return val;
        }
        panic!("outside bounds");
    }

    pub fn get_pos(&self) -> usize {
        return self.pos.get();
    }

    fn inc_pos(&self) {
        self.pos.set(self.pos.get() + 1);
    }
}

impl SlaElement {
    pub fn is_null(&self, attr: AttributeId) -> bool {
        let attr_maybe = self.attrs.get(&attr);
        match attr_maybe {
            Some(attr) => match attr.value {
                SlaAttributeValue::Null => true,
                _ => false,
            },
            None => true,
        }
    }

    // should we be silently failing if an attribute doesn't exist?
    // this makes it easier than not checking if certain optional
    // fields don't exist, but maybe it's more prone to breakage.

    pub fn as_bool_or(&self, attr: AttributeId, default: bool) -> bool {
        let attr_maybe = self.attrs.get(&attr);
        match attr_maybe {
            Some(attr) => match attr.value {
                SlaAttributeValue::Bool(v) => v,
                _ => default,
            },
            None => default,
        }
    }

    pub fn as_int_or(&self, attr: AttributeId, default: i64) -> i64 {
        let attr_maybe = self.attrs.get(&attr);
        match attr_maybe {
            Some(attr) => match attr.value {
                SlaAttributeValue::Int(v) => v,
                _ => default,
            },
            None => default,
        }
    }

    pub fn as_uint_or(&self, attr: AttributeId, default: u64) -> u64 {
        let attr_maybe = self.attrs.get(&attr);
        match attr_maybe {
            Some(attr) => match attr.value {
                SlaAttributeValue::UInt(v) => v,
                _ => default,
            },
            None => default,
        }
    }

    pub fn as_str_or(&self, attr: AttributeId, default: &str) -> String {
        let attr_maybe = self.attrs.get(&attr);
        match attr_maybe {
            Some(attr) => match &attr.value {
                SlaAttributeValue::String(v) => v.to_owned(),
                _ => default.to_string(),
            },
            None => default.to_string(),
        }
    }

    pub fn as_space(&self, attr: AttributeId) -> SpaceInfo {
        let attr_maybe = self.attrs.get(&attr);
        let default = SpaceInfo {
            space_type: SpaceType::None,
            index: -1,
        };
        match attr_maybe {
            Some(attr) => match &attr.value {
                SlaAttributeValue::Space(v) => SpaceInfo {
                    space_type: v.space_type,
                    index: v.index,
                },
                _ => default,
            },
            None => default,
        }
    }
}
