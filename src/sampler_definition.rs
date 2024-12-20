use std::io::Write;

use byteorder::{LittleEndian, WriteBytesExt};
use scroll::{ctx::TryFromCtx, Pread, LE};

use crate::{
    common::{optional_write, read_bool, read_string, write_string},
    MinecraftVersion, WriteError,
};

#[derive(Debug)]
pub struct SamplerDefinition {
    pub reg: u16,
    pub access: SamplerAccess,
    pub precision: Precision,
    pub allow_unordered_access: u8,
    pub sampler_type: SamplerType,
    pub texture_format: String,
    pub unknown_int: u32,
    pub unknown_byte: u8,
    pub sampler_state: Option<u8>,

    pub default_texture: Option<String>,
    pub unknown_string: Option<String>,
    pub custom_type_info: Option<CustomTypeInfo>,
}
impl<'a> TryFromCtx<'a, MinecraftVersion> for SamplerDefinition {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], ctx: MinecraftVersion) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;
        let reg: u16 = if ctx == MinecraftVersion::V1_18_30 {
            buffer.gread::<u8>(&mut offset)?.into()
        } else {
            buffer.gread_with(&mut offset, LE)?
        };
        let access: SamplerAccess = buffer.gread_with(&mut offset, ())?;
        let precision: Precision = buffer.gread_with(&mut offset, ())?;
        let allow_unordered_access: u8 = buffer.gread_with(&mut offset, LE)?;
        let sampler_type: SamplerType = buffer.gread_with(&mut offset, ctx)?;
        let texture_format = read_string(buffer, &mut offset)?;

        let unknown_int: u32 = buffer.gread_with(&mut offset, LE)?;
        let unknown_byte: u8 = if ctx != MinecraftVersion::V1_18_30 {
            buffer.gread_with(&mut offset, LE)?
        } else {
            reg.try_into()
                .map_err(|e| scroll::Error::Custom(format!("unknown byte parsing error: {e}")))?
        };
        let mut sampler_state = None;
        if ctx == MinecraftVersion::V1_21_20 {
            if read_bool(buffer, &mut offset)? {
                sampler_state = Some(buffer.gread::<u8>(&mut offset)?);
            }
        }
        let mut default_texture = None;
        let has_default_texture = read_bool(buffer, &mut offset)?;
        if has_default_texture {
            default_texture = Some(read_string(buffer, &mut offset)?);
        }
        let mut unknown_string = None;
        if ctx == MinecraftVersion::V1_20_80 || ctx == MinecraftVersion::V1_21_20 {
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
                sampler_state,

                default_texture,
                unknown_string,
                custom_type_info,
            },
            offset,
        ))
    }
}
impl SamplerDefinition {
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
        writer.write_u8(self.precision as u8)?;
        writer.write_u8(self.allow_unordered_access)?;
        writer.write_u8(self.sampler_type.to_u8(version)?)?;
        write_string(&self.texture_format, writer)?;
        writer.write_u32::<LittleEndian>(self.unknown_int)?;
        if version != MinecraftVersion::V1_18_30 {
            writer.write_u8(self.unknown_byte)?;
        }
        if version == MinecraftVersion::V1_21_20 {
            optional_write(writer, self.sampler_state, |o, v| o.write_u8(v))?;
        }
        optional_write(writer, self.default_texture.as_deref(), |o, v| {
            write_string(v, o)
        })?;
        if version == MinecraftVersion::V1_20_80 || version == MinecraftVersion::V1_21_20 {
            optional_write(writer, self.unknown_string.as_deref(), |o, v| {
                write_string(v, o)
            })?;
        }
        optional_write(writer, self.custom_type_info.as_ref(), |o, v| v.write(o))?;

        Ok(())
    }
}
#[derive(Debug)]
pub struct CustomTypeInfo {
    pub name: String,
    pub size: u32,
}
impl<'a> TryFromCtx<'a> for CustomTypeInfo {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;
        let name = read_string(buffer, &mut offset)?;
        let size = buffer.gread_with(&mut offset, LE)?;
        Ok((Self { name, size }, offset))
    }
}
impl CustomTypeInfo {
    pub fn write<W>(&self, writer: &mut W) -> Result<(), WriteError>
    where
        W: Write,
    {
        write_string(&self.name, writer)?;
        writer.write_u32::<LittleEndian>(self.size)?;
        Ok(())
    }
}

#[derive(Debug, Copy, Clone)]
pub enum SamplerType {
    Type2D,
    Type2DArray,
    Type2DExternal,
    Type3D,
    TypeCube,
    TypeSamplerCubeArray,
    TypeStructuredBuffer,
    TypeRawBuffer,
    TypeAccelerationStructure,
    Type2DShadow,
    Type2DArrayShadow,
}
impl<'a> TryFromCtx<'a, MinecraftVersion> for SamplerType {
    type Error = scroll::Error;
    fn try_from_ctx(
        buffer: &'a [u8],
        version: MinecraftVersion,
    ) -> Result<(Self, usize), Self::Error> {
        let mut sampler_type: u8 = buffer.pread_with(0, LE)?;
        // On versions before 1.21.20, 5 is Structured Buffer
        // After 1.21.20, 5 is SamplerCubeArray
        // Adjust the difference.
        if version != MinecraftVersion::V1_21_20 && sampler_type >= 5 {
            sampler_type += 1;
        }
        let enum_sub = match sampler_type {
            0 => Self::Type2D,
            1 => Self::Type2DArray,
            2 => Self::Type2DExternal,
            3 => Self::Type3D,
            4 => Self::TypeCube,
            5 => Self::TypeSamplerCubeArray,
            6 => Self::TypeStructuredBuffer,
            7 => Self::TypeRawBuffer,
            8 => Self::TypeAccelerationStructure,
            9 => Self::Type2DShadow,
            10 => Self::Type2DArrayShadow,
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
    fn to_u8(self, version: MinecraftVersion) -> Result<u8, WriteError> {
        if version != MinecraftVersion::V1_21_20 {
            return match self {
                Self::TypeSamplerCubeArray => Err(WriteError::Compat("Sampler type is (Sampler Cube Array) ,which is incompatible with versions before 1.21.20".to_string())),
                Self::TypeRawBuffer => Ok(5),
                Self::TypeAccelerationStructure => Ok(6),
                Self::Type2DShadow => Ok(7),
                Self::Type2DArrayShadow => Ok(8),
                _ => Ok(self as u8),
            };
        }
        Ok(self as u8)
    }
}
#[derive(Debug)]
pub enum SamplerAccess {
    None,
    Read,
    Write,
    ReadWrite,
}
#[derive(Debug, Clone, Copy)]
pub enum Precision {
    Low,
    Medium,
    High,
}
impl<'a> TryFromCtx<'a> for Precision {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let precision: u8 = buffer.pread_with(0, LE)?;
        let precision = match precision {
            0 => Self::Low,
            1 => Self::Medium,
            2 => Self::High,
            _ => {
                return Err(scroll::Error::BadInput {
                    size: 0,
                    msg: "Precision of samplerdef is invalid",
                })
            }
        };
        Ok((precision, 1))
    }
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
