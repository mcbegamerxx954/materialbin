use aes::cipher::{generic_array::GenericArray, typenum};
use aes_gcm::{
    aead::{Aead, Payload},
    KeyInit,
};
use byteorder::{LittleEndian, WriteBytesExt};
use indexmap::IndexMap;
use pass::Pass;
use property_field::PropertyField;
use sampler_definition::SamplerDefinition;
use scroll::{ctx::TryFromCtx, Pread, LE};
use std::{backtrace::Backtrace, borrow::Cow, fmt::Display, io::Write};
pub mod bgfx_shader;
#[cfg(feature = "ffi")]
mod cffi;
mod common;
pub mod pass;
pub mod property_field;
pub mod sampler_definition;

use crate::common::{optional_write, read_bool, read_string, write_string};
pub const ALL_VERSIONS: [MinecraftVersion; 6] = [
    // This version causes parsing issues
    MinecraftVersion::V1_18_30,
    MinecraftVersion::V1_19_60,
    MinecraftVersion::V1_20_80,
    MinecraftVersion::V1_21_20,
    MinecraftVersion::V1_21_110,
    MinecraftVersion::V26_0_24,
];
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord, Default)]
pub enum MinecraftVersion {
    V1_18_30,
    V1_19_60,

    V1_20_80,

    V1_21_20,
    V1_21_110,
    #[default]
    V26_0_24,
}
type MBGenericArray = GenericArray<u8, typenum::U12>;
impl std::fmt::Display for MinecraftVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::V1_20_80 => write!(f, "1.20.80"),
            Self::V1_19_60 => write!(f, "1.19.60"),
            Self::V1_18_30 => write!(f, "1.18.30"),
            Self::V1_21_20 => write!(f, "1.21.20"),
            Self::V1_21_110 => write!(f, "1.21.110"),
            Self::V26_0_24 => write!(f, "26.0.24"),
        }
    }
}

#[derive(Debug)]
pub struct CompiledMaterialDefinition {
    pub version: u64,
    pub encryption_variant: EncryptionVariant,
    pub name: String,
    pub parent_name: Option<String>,
    pub sampler_definitions: IndexMap<String, SamplerDefinition>,
    pub property_fields: IndexMap<String, PropertyField>,
    pub uniform_overrides: Option<IndexMap<String, String>>,
    pub passes: IndexMap<String, Pass>,
}
impl<'a> TryFromCtx<'a, MinecraftVersion> for CompiledMaterialDefinition {
    type Error = MyError;

    fn try_from_ctx(buffer: &'a [u8], ctx: MinecraftVersion) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;
        const MAGIC: u64 = 0xA11DA1A;
        if buffer.gread::<u64>(&mut offset)? != MAGIC {
            return Err(scroll::Error::BadInput {
                size: offset,
                msg: "Invalid starting magic",
            }
            .into());
        }
        if read_string(buffer, &mut offset)? != "RenderDragon.CompiledMaterialDefinition" {
            return Err(scroll::Error::BadInput {
                size: offset,
                msg: "Invalid definition",
            }
            .into());
        }
        let version: u64 = buffer.gread_with(&mut offset, LE)?;
        if version == 23 && ctx != MinecraftVersion::V26_0_24 {
            return Err(scroll::Error::BadInput {
                size: offset,
                msg: "Wrong material bin version",
            }
            .into());
        }
        let encryption_variant: EncryptionVariant = buffer.gread(&mut offset)?;
        let cow_content = encryption_variant.handle_encryption(buffer, &mut offset)?;
        let content = &*cow_content;
        let name = read_string(content, &mut offset)?;
        let mut parent_name = None;
        let has_parent_name = read_bool(content, &mut offset)?;
        if has_parent_name {
            parent_name = Some(read_string(content, &mut offset)?);
        }
        let sampler_definition_count: u8 = content.gread_with(&mut offset, LE)?;
        let mut sampler_definitions = IndexMap::with_capacity(sampler_definition_count.into());
        for _ in 0..sampler_definition_count {
            let name = read_string(content, &mut offset)?;
            let sampler_definition: SamplerDefinition = content.gread_with(&mut offset, ctx)?;
            sampler_definitions.insert(name, sampler_definition);
        }
        let property_field_count: u16 = content.gread_with(&mut offset, LE)?;
        let mut property_fields = IndexMap::with_capacity(property_field_count.into());
        for _ in 0..property_field_count {
            let name = read_string(content, &mut offset)?;
            let property_field: PropertyField = content.gread(&mut offset)?;
            property_fields.insert(name, property_field);
        }
        let mut uniform_overrides = None;
        if ctx >= MinecraftVersion::V1_21_110 {
            if name != "Core/Builtins" {
                let mut indexmap = IndexMap::new();
                let builtin_count: u16 = content.gread_with(&mut offset, LE)?;
                for _ in 0..builtin_count {
                    let key = read_string(content, &mut offset)?;
                    let value = read_string(content, &mut offset)?;
                    indexmap.insert(key, value);
                }
                uniform_overrides = Some(indexmap);
            }
        }
        let pass_count: u16 = content.gread_with(&mut offset, LE)?;
        let mut passes = IndexMap::with_capacity(pass_count.into());
        for _ in 0..pass_count {
            let name = read_string(content, &mut offset)?;
            let pass: Pass = content.gread_with(&mut offset, ctx)?;
            passes.insert(name, pass);
        }
        // Just so we parse the whole thing
        if content.gread_with::<u64>(&mut offset, LE)? != MAGIC {
            return Err(scroll::Error::BadInput {
                size: offset,
                msg: "Invalid ending magic",
            }
            .into());
        }
        // if offset != buffer.len() - 1 {
        //     return Err(scroll::Error::BadInput {
        //         size: offset,
        //         msg: "Tragic news",
        //     }.into());
        // }
        Ok((
            Self {
                version,
                encryption_variant,
                name,
                parent_name,
                sampler_definitions,
                property_fields,
                uniform_overrides,
                passes,
            },
            offset,
        ))
    }
}
impl CompiledMaterialDefinition {
    pub fn write<W>(&self, writer: &mut W, version: MinecraftVersion) -> Result<(), WriteError>
    where
        W: Write,
    {
        const MAGIC: u64 = 0xA11DA1A;
        writer.write_u64::<LittleEndian>(MAGIC)?;
        write_string("RenderDragon.CompiledMaterialDefinition", writer)?;
        let ver: u64 = match version {
            v if v <= MinecraftVersion::V1_21_110 => 22,
            MinecraftVersion::V26_0_24 => 23,
            // MinecraftVersion::V26_10_20 => 25,
            _ => self.version,
        };
        writer.write_u64::<LittleEndian>(ver)?;
        self.encryption_variant.write(writer)?;
        write_string(&self.name, writer)?;
        optional_write(writer, self.parent_name.as_deref(), |o, v| {
            write_string(v, o)
        })?;
        let len = self.sampler_definitions.len().try_into()?;
        writer.write_u8(len)?;
        for (name, sampler_definition) in self.sampler_definitions.iter() {
            write_string(name, writer)?;
            sampler_definition.write(writer, version)?;
        }
        let len = self.property_fields.len().try_into()?;
        writer.write_u16::<LittleEndian>(len)?;
        for (name, property_field) in self.property_fields.iter() {
            write_string(name, writer)?;
            property_field.write(writer)?;
        }
        if version >= MinecraftVersion::V1_21_110 && self.name != "Core/Builtins" {
            match &self.uniform_overrides {
                Some(overrides) => {
                    writer.write_u16::<LittleEndian>(overrides.len().try_into()?)?;
                    for (key, value) in overrides {
                        write_string(key, writer)?;
                        write_string(value, writer)?;
                    }
                }
                None => writer.write_u16::<LittleEndian>(0)?,
            }
        }
        let len = self.passes.len().try_into()?;
        writer.write_u16::<LittleEndian>(len)?;
        for (name, pass) in self.passes.iter() {
            write_string(name, writer)?;
            pass.write(writer, version)?;
        }
        writer.write_u64::<LittleEndian>(MAGIC)?;
        Ok(())
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum EncryptionVariant {
    None,
    SimplePassphrase,
    KeyPair,
}
impl<'a> TryFromCtx<'a> for EncryptionVariant {
    type Error = MyError;
    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;
        let encryption: u32 = buffer.gread_with(&mut offset, LE)?;
        let enum_type = match encryption {
            0x4E4F4E45_u32 => Self::None,
            0x534D504C_u32 => Self::SimplePassphrase,
            0x4B595052_u32 => Self::KeyPair,
            _ => {
                return Err(scroll::Error::BadInput {
                    size: 0,
                    msg: "Invalid EncryptionVariant: {encryption}",
                }
                .into());
            }
        };
        Ok((enum_type, offset))
    }
}
impl EncryptionVariant {
    fn handle_encryption<'b>(
        &self,
        data: &'b [u8],
        offset: &mut usize,
    ) -> Result<Cow<'b, [u8]>, MyError> {
        match self {
            Self::None => Ok(Cow::Borrowed(data)),
            Self::SimplePassphrase => Err(scroll::Error::BadInput {
                size: 0,
                msg: "SimplePassphrase encryption isnt supported yet",
            }
            .into()),
            Self::KeyPair => {
                //   let mut offset = 0;
                let encryption_key = read_array(data, offset)?;
                let encryption_nonce = read_array(data, offset)?;
                let encryption_nonce = trunc_slice(encryption_nonce, 12);
                let content = read_array(data, offset)?;
                let decrypt = aes_gcm::Aes256Gcm::new(
                    GenericArray::<u8, typenum::U32>::from_slice(encryption_key),
                );
                let decrypted =
                    decrypt.decrypt(MBGenericArray::from_slice(encryption_nonce), content)?;
                // The buffer got practically trimmed soo
                *offset = 0;
                Ok(Cow::Owned(decrypted))
            }
        }
    }
    fn write<W>(&self, output: &mut W) -> Result<(), std::io::Error>
    where
        W: Write,
    {
        let int = match self {
            Self::None => 0x4E4F4E45_u32,
            Self::SimplePassphrase => 0x534D504C_u32,
            Self::KeyPair => 0x4B595052_u32,
        };
        output.write_u32::<byteorder::LE>(int)?;
        Ok(())
    }
    fn is_encrypted(&self) -> bool {
        *self != Self::None
    }
}
fn trunc_slice<T>(slice: &[T], len: usize) -> &[T] {
    slice.get(..len).unwrap_or(slice)
}
#[derive(Debug)]
pub enum WriteError {
    IntConvert(std::num::TryFromIntError),
    IoError(std::io::Error),
    Compat(String),
}
impl From<std::io::Error> for WriteError {
    fn from(value: std::io::Error) -> Self {
        Self::IoError(value)
    }
}
impl From<std::num::TryFromIntError> for WriteError {
    fn from(io_error: std::num::TryFromIntError) -> Self {
        Self::IntConvert(io_error)
    }
}
impl std::error::Error for WriteError {}
impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IntConvert(err) => write!(f, "Int conversion failed: {err}"),
            Self::IoError(err) => write!(f, "Io error: {err}"),
            Self::Compat(info) => write!(f, "Compat error: {info}"),
        }
    }
}
#[macro_export]
macro_rules! option_read {
    ($buf:expr, $offset:expr, $func:expr) => {
        // let should_read = crate::common::read_bool($offset, $buf);
        if crate::common::read_bool($offset, $buf)? {
            Some($func)
        } else {
            None
        }
    };
}
#[derive(Debug)]
pub struct MyError {
    #[cfg(feature = "backtracing")]
    backtrace: Box<Backtrace>,
    thingy: MyErrorThingy,
}

impl From<scroll::Error> for MyError {
    fn from(value: scroll::Error) -> Self {
        Self::new(MyErrorThingy::Scroll(value))
    }
}
impl From<aes_gcm::Error> for MyError {
    fn from(value: aes_gcm::Error) -> Self {
        Self::new(MyErrorThingy::AesError(value))
    }
}
impl MyError {
    #[track_caller]
    fn new(thingy: MyErrorThingy) -> Self {
        Self {
            #[cfg(feature = "backtracing")]
            backtrace: Box::new(Backtrace::capture()),
            thingy,
        }
    }
    #[cfg(feature = "backtracing")]
    pub fn get_backtracey(&self) -> &Box<Backtrace> {
        &self.backtrace
    }
}
impl Display for MyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.thingy)
    }
}
#[derive(Debug)]
pub enum MyErrorThingy {
    Scroll(scroll::Error),
    AesError(aes_gcm::Error),
}
impl Display for MyErrorThingy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scroll(e) => write!(f, "{e}"),
            Self::AesError(e) => write!(f, "aes error:{e}"),
        }
    }
}
fn read_array<'a>(buf: &'a [u8], offset: &mut usize) -> Result<&'a [u8], scroll::Error> {
    let len: u32 = buf.gread_with(offset, LE)?;
    buf.gread_with(offset, len as usize)
}
