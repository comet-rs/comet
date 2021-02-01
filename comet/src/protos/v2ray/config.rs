// Automatically generated rust module for 'config.proto' file

#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(unused_imports)]
#![allow(unknown_lints)]
#![allow(clippy::all)]
#![cfg_attr(rustfmt, rustfmt_skip)]


use std::borrow::Cow;
use quick_protobuf::{MessageRead, MessageWrite, BytesReader, Writer, WriterBackend, Result};
use quick_protobuf::sizeofs::*;
use super::*;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Domain<'a> {
    pub type_pb: mod_Domain::Type,
    pub value: Cow<'a, str>,
    pub attribute: Vec<mod_Domain::Attribute<'a>>,
}

impl<'a> MessageRead<'a> for Domain<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(8) => msg.type_pb = r.read_enum(bytes)?,
                Ok(18) => msg.value = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(26) => msg.attribute.push(r.read_message::<mod_Domain::Attribute>(bytes)?),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for Domain<'a> {
    fn get_size(&self) -> usize {
        0
        + if self.type_pb == config::mod_Domain::Type::Plain { 0 } else { 1 + sizeof_varint(*(&self.type_pb) as u64) }
        + if self.value == "" { 0 } else { 1 + sizeof_len((&self.value).len()) }
        + self.attribute.iter().map(|s| 1 + sizeof_len((s).get_size())).sum::<usize>()
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if self.type_pb != config::mod_Domain::Type::Plain { w.write_with_tag(8, |w| w.write_enum(*&self.type_pb as i32))?; }
        if self.value != "" { w.write_with_tag(18, |w| w.write_string(&**&self.value))?; }
        for s in &self.attribute { w.write_with_tag(26, |w| w.write_message(s))?; }
        Ok(())
    }
}

pub mod mod_Domain {

use std::borrow::Cow;
use super::*;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Attribute<'a> {
    pub key: Cow<'a, str>,
    pub typed_value: mod_Domain::mod_Attribute::OneOftyped_value,
}

impl<'a> MessageRead<'a> for Attribute<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.key = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(16) => msg.typed_value = mod_Domain::mod_Attribute::OneOftyped_value::bool_value(r.read_bool(bytes)?),
                Ok(24) => msg.typed_value = mod_Domain::mod_Attribute::OneOftyped_value::int_value(r.read_int64(bytes)?),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for Attribute<'a> {
    fn get_size(&self) -> usize {
        0
        + if self.key == "" { 0 } else { 1 + sizeof_len((&self.key).len()) }
        + match self.typed_value {
            mod_Domain::mod_Attribute::OneOftyped_value::bool_value(ref m) => 1 + sizeof_varint(*(m) as u64),
            mod_Domain::mod_Attribute::OneOftyped_value::int_value(ref m) => 1 + sizeof_varint(*(m) as u64),
            mod_Domain::mod_Attribute::OneOftyped_value::None => 0,
    }    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if self.key != "" { w.write_with_tag(10, |w| w.write_string(&**&self.key))?; }
        match self.typed_value {            mod_Domain::mod_Attribute::OneOftyped_value::bool_value(ref m) => { w.write_with_tag(16, |w| w.write_bool(*m))? },
            mod_Domain::mod_Attribute::OneOftyped_value::int_value(ref m) => { w.write_with_tag(24, |w| w.write_int64(*m))? },
            mod_Domain::mod_Attribute::OneOftyped_value::None => {},
    }        Ok(())
    }
}

pub mod mod_Attribute {

use super::*;

#[derive(Debug, PartialEq, Clone)]
pub enum OneOftyped_value {
    bool_value(bool),
    int_value(i64),
    None,
}

impl Default for OneOftyped_value {
    fn default() -> Self {
        OneOftyped_value::None
    }
}

}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Type {
    Plain = 0,
    Regex = 1,
    Domain = 2,
    Full = 3,
}

impl Default for Type {
    fn default() -> Self {
        Type::Plain
    }
}

impl From<i32> for Type {
    fn from(i: i32) -> Self {
        match i {
            0 => Type::Plain,
            1 => Type::Regex,
            2 => Type::Domain,
            3 => Type::Full,
            _ => Self::default(),
        }
    }
}

impl<'a> From<&'a str> for Type {
    fn from(s: &'a str) -> Self {
        match s {
            "Plain" => Type::Plain,
            "Regex" => Type::Regex,
            "Domain" => Type::Domain,
            "Full" => Type::Full,
            _ => Self::default(),
        }
    }
}

}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct CIDR<'a> {
    pub ip: Cow<'a, [u8]>,
    pub prefix: u32,
}

impl<'a> MessageRead<'a> for CIDR<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.ip = r.read_bytes(bytes).map(Cow::Borrowed)?,
                Ok(16) => msg.prefix = r.read_uint32(bytes)?,
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for CIDR<'a> {
    fn get_size(&self) -> usize {
        0
        + if self.ip == Cow::Borrowed(b"") { 0 } else { 1 + sizeof_len((&self.ip).len()) }
        + if self.prefix == 0u32 { 0 } else { 1 + sizeof_varint(*(&self.prefix) as u64) }
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if self.ip != Cow::Borrowed(b"") { w.write_with_tag(10, |w| w.write_bytes(&**&self.ip))?; }
        if self.prefix != 0u32 { w.write_with_tag(16, |w| w.write_uint32(*&self.prefix))?; }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct GeoIP<'a> {
    pub country_code: Cow<'a, str>,
    pub cidr: Vec<CIDR<'a>>,
}

impl<'a> MessageRead<'a> for GeoIP<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.country_code = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(18) => msg.cidr.push(r.read_message::<CIDR>(bytes)?),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for GeoIP<'a> {
    fn get_size(&self) -> usize {
        0
        + if self.country_code == "" { 0 } else { 1 + sizeof_len((&self.country_code).len()) }
        + self.cidr.iter().map(|s| 1 + sizeof_len((s).get_size())).sum::<usize>()
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if self.country_code != "" { w.write_with_tag(10, |w| w.write_string(&**&self.country_code))?; }
        for s in &self.cidr { w.write_with_tag(18, |w| w.write_message(s))?; }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct GeoIPList<'a> {
    pub entry: Vec<GeoIP<'a>>,
}

impl<'a> MessageRead<'a> for GeoIPList<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.entry.push(r.read_message::<GeoIP>(bytes)?),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for GeoIPList<'a> {
    fn get_size(&self) -> usize {
        0
        + self.entry.iter().map(|s| 1 + sizeof_len((s).get_size())).sum::<usize>()
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        for s in &self.entry { w.write_with_tag(10, |w| w.write_message(s))?; }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct GeoSite<'a> {
    pub country_code: Cow<'a, str>,
    pub domain: Vec<Domain<'a>>,
}

impl<'a> MessageRead<'a> for GeoSite<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.country_code = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(18) => msg.domain.push(r.read_message::<Domain>(bytes)?),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for GeoSite<'a> {
    fn get_size(&self) -> usize {
        0
        + if self.country_code == "" { 0 } else { 1 + sizeof_len((&self.country_code).len()) }
        + self.domain.iter().map(|s| 1 + sizeof_len((s).get_size())).sum::<usize>()
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        if self.country_code != "" { w.write_with_tag(10, |w| w.write_string(&**&self.country_code))?; }
        for s in &self.domain { w.write_with_tag(18, |w| w.write_message(s))?; }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct GeoSiteList<'a> {
    pub entry: Vec<GeoSite<'a>>,
}

impl<'a> MessageRead<'a> for GeoSiteList<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.entry.push(r.read_message::<GeoSite>(bytes)?),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for GeoSiteList<'a> {
    fn get_size(&self) -> usize {
        0
        + self.entry.iter().map(|s| 1 + sizeof_len((s).get_size())).sum::<usize>()
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        for s in &self.entry { w.write_with_tag(10, |w| w.write_message(s))?; }
        Ok(())
    }
}

