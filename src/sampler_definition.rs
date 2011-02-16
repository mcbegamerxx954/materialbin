use std::io::Write;

use byteorder::{LittleEndian, WriteBytesExt};
use scroll::{ctx::TryFromCtx, Pread, LE};

use crate::{
    common::{optional_write, read_bool, read_string, write_string},
    MinecraftVersion, WriteError,
};

#[derive(Debug)]
pub struct SamplerDefinition<'a> {
    reg: u16,
    access: SamplerAccess,
    precision: u8,
    allow_unordered_access: u8,
    sampler_type: SamplerType,
    texture_format: &'a str,
    unknown_int: u32,
    unknown_byte: u8,
    default_texture: Option<&'a str>,
    unknown_string: Option<&'a str>,
    custom_type_info: Option<CustomTypeInfo<'a>>,
}
impl<'a> TryFromCtx<'a, MinecraftVersion> for SamplerDefinition<'a> {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], ctx: MinecraftVersion) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;
        let reg: u16 = if ctx == MinecraftVersion::V1_18_30 {
            buffer.gread::<u8>(&mut offset)?.into()
        } else {
            buffer.gread_with(&mut offset, LE)?
        };
        let access: SamplerAccess = buffer.gread_with(&mut offset, ())?;
        let precision: u8 = buffer.gread_with(&mut offset, LE)?;
        let allow_unordered_access: u8 = buffer.gread_with(&mut offset, LE)?;
        let sampler_type: SamplerType = buffer.gread_with(&mut offset, ())?;
        let texture_format = read_string(buffer, &mut offset)?;

        let unknown_int: u32 = buffer.gread_with(&mut offset, LE)?;
        let mut unknown_byte: u8 = reg
            .try_into()
            .map_err(|e| scroll::Error::Custom(format!("unknown byte parsing error: {e}")))?;
        if ctx != MinecraftVersion::V1_18_30 {
            unknown_byte = buffer.gread_with(&mut offset, LE)?;
        }
        let mut default_texture: Option<&str> = None;
        let has_default_texture = read_bool(buffer, &mut offset)?;
        if has_default_texture {
            default_texture = Some(read_string(buffer, &mut offset)?);
        }

        let mut unknown_string: Option<&str> = None;
        if ctx == MinecraftVersion::V1_20_80 {
            let has_unknown_string = read_bool(buffer, &mut offset)?;
            if has_unknown_string {
                unknown_string = Some(read_string(buffer, &mut offset)?);
            }
        }
        let mut custom_type_info: Option<CustomTypeInfo> = None;
        let has_custom_type = read_bool(buffer, &mut offset)?;
        if has_custom_type {
            custom_type_info = Some(buffer.gread_with(&mut offset, ())?)
        }
        Ok((
            Self {
                reg,
                access,
                precision,
                allow_unordered_access,
                sampler_type,
                texture_format,
                unknown_int,
                unknown_byte,
                default_texture,
                unknown_string,
                custom_type_info,
            },
            offset,
        ))
    }
}
impl<'a> SamplerDefinition<'a> {
    pub fn write<W>(&self, writer: &mut W, version: MinecraftVersion) -> Result<(), WriteError>
    where
        W: Write,
    {
        if version == MinecraftVersion::V1_18_30 {
            writer.write_u8(self.reg.try_into()?)?;
        } else {
            writer.write_u16::<LittleEndian>(self.reg)?;
        }
        writer.write_u8(self.access.as_u8())?;
        writer.write_u8(self.precision)?;
        writer.write_u8(self.allow_unordered_access)?;
        writer.write_u8(self.sampler_type.to_u8())?;
        write_string(self.texture_format, writer)?;
        writer.write_u32::<LittleEndian>(self.unknown_int)?;
        if version != MinecraftVersion::V1_18_30 {
            writer.write_u8(self.unknown_byte)?;
        }
        optional_write(writer, self.default_texture, |o, v| write_string(v, o))?;
        if version == MinecraftVersion::V1_20_80 {
            optional_write(writer, self.unknown_string, |o, v| write_string(v, o))?;
        }
        optional_write(writer, self.custom_type_info.as_ref(), |o, v| v.write(o))?;
        Ok(())
    }
}
#[derive(Debug)]
struct CustomTypeInfo<'a> {
    name: &'a str,
    size: u32,
}
impl<'a> TryFromCtx<'a> for CustomTypeInfo<'a> {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;
        let name = read_string(buffer, &mut offset)?;
        let size = buffer.gread_with(&mut offset, LE)?;
        Ok((Self { name, size }, offset))
    }
}
impl<'a> CustomTypeInfo<'a> {
    fn write<W>(&self, writer: &mut W) -> Result<(), WriteError>
    where
        W: Write,
    {
        write_string(self.name, writer)?;
        writer.write_u32::<LittleEndian>(self.size)?;
        Ok(())
    }
}

#[derive(Debug)]
enum SamplerType {
    Type2D,
    Type2DArray,
    Type2DExternal,
    Type3D,
    TypeCube,
    TypeStructuredBuffer,
    TypeRawBuffer,
    TypeAccelerationStructure,
    Type2DShadow,
    Type2DArrayShadow,
}
impl<'a> TryFromCtx<'a> for SamplerType {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let sampler_type: u8 = buffer.pread_with(0, LE)?;
        let enum_sub = match sampler_type {
            0 => Self::Type2D,
            1 => Self::Type2DArray,
            2 => Self::Type2DExternal,
            3 => Self::Type3D,
            4 => Self::TypeCube,
            5 => Self::TypeStructuredBuffer,
            6 => Self::TypeRawBuffer,
            7 => Self::TypeAccelerationStructure,
            8 => Self::Type2DShadow,
            9 => Self::Type2DArrayShadow,
            _ => {
                return Err(scroll::Error::Custom(format!(
                    "Invalid sapmler_type: {sampler_type}"
                )))
            }
        };
        Ok((enum_sub, 1))
    }
}
impl SamplerType {
    fn to_u8(&self) -> u8 {
        match self {
            Self::Type2D => 0,
            Self::Type2DArray => 1,
            Self::Type2DExternal => 2,
            Self::Type3D => 3,
            Self::TypeCube => 4,
            Self::TypeStructuredBuffer => 5,
            Self::TypeRawBuffer => 6,
            Self::TypeAccelerationStructure => 7,
            Self::Type2DShadow => 8,
            Self::Type2DArrayShadow => 9,
        }
    }
}
#[derive(Debug)]
enum SamplerAccess {
    None,
    Read,
    Write,
    ReadWrite,
}
impl<'a> TryFromCtx<'a> for SamplerAccess {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let access: u8 = buffer.pread_with(0, LE)?;
        match access {
            0 => Ok((Self::None, 1)),
            1 => Ok((Self::Read, 1)),
            2 => Ok((Self::Write, 1)),
            3 => Ok((Self::ReadWrite, 1)),
            _ => Err(scroll::Error::Custom(
                "Sampler Access is not valid".to_owned(),
            )),
        }
    }
}
impl SamplerAccess {
    fn as_u8(&self) -> u8 {
        match self {
            Self::None => 0,
            Self::Read => 1,
            Self::Write => 2,
            Self::ReadWrite => 3,
        }
    }
}