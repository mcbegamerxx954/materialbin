use std::io::Write;

use byteorder::{LittleEndian, WriteBytesExt};
use indexmap::IndexMap;
use scroll::{ctx::TryFromCtx, Pread, LE};

use crate::{
    common::{optional_write, read_bool, read_string, write_string},
    MinecraftVersion, WriteError,
};

#[derive(Debug)]
pub struct Pass {
    pub bitset: String,
    pub fallback: String,
    pub default_blendmode: Option<BlendMode>,
    pub default_flag_values: IndexMap<String, String>,
    pub variants: Vec<Variant>,
}
impl<'a> TryFromCtx<'a, MinecraftVersion> for Pass {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], ctx: MinecraftVersion) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;
        let bitset = if ctx == MinecraftVersion::V1_18_30 {
            let has_bitset = buffer.gread_with::<u8>(&mut offset, LE)? == 15;
            // rewind
            offset -= 4;
            if has_bitset {
                read_string(buffer, &mut offset)?
            } else {
                // skip reading byte we have no use for
                offset += 1;
                "".to_string()
            }
        } else {
            read_string(buffer, &mut offset)?
        };
        let fallback = read_string(buffer, &mut offset)?;
        let mut default_blendmode: Option<BlendMode> = None;
        let has_blendmode = read_bool(buffer, &mut offset)?;
        if has_blendmode {
            default_blendmode = Some(buffer.gread_with(&mut offset, ())?);
        }

        let flag_dvalue_count: u16 = buffer.gread_with(&mut offset, LE)?;
        let mut default_flag_values = IndexMap::with_capacity(flag_dvalue_count.into());
        for _ in 0..flag_dvalue_count {
            let key = read_string(buffer, &mut offset)?;
            let value = read_string(buffer, &mut offset)?;
            default_flag_values.insert(key, value);
        }

        let variant_count: u16 = buffer.gread_with(&mut offset, LE)?;
        let variants: Vec<Variant> = (0..variant_count)
            .flat_map(|_| buffer.gread(&mut offset))
            .collect();
        Ok((
            Self {
                bitset,
                fallback,
                default_blendmode,
                default_flag_values,
                variants,
            },
            offset,
        ))
    }
}
impl Pass {
    pub fn write<W>(&self, writer: &mut W, version: MinecraftVersion) -> Result<(), WriteError>
    where
        W: Write,
    {
        if self.bitset.is_empty() {
            return Err(WriteError::Compat(
                "Bitset string is empty, Try fixing it in the main struct".to_string(),
            ));
        } else if version == MinecraftVersion::V1_18_30 {
        }
        write_string(&self.bitset, writer)?;
        write_string(&self.fallback, writer)?;
        optional_write(writer, self.default_blendmode.as_ref(), |o, v| {
            o.write_u16::<LittleEndian>(v.as_u16())
        })?;
        let len = self.default_flag_values.len().try_into()?;
        writer.write_u16::<LittleEndian>(len)?;
        for (key, value) in self.default_flag_values.iter() {
            write_string(&key, writer)?;
            write_string(&value, writer)?;
        }
        let len = self.variants.len().try_into()?;
        writer.write_u16::<LittleEndian>(len)?;
        for variant in self.variants.iter() {
            variant.write(writer)?;
        }
        Ok(())
    }
}
#[derive(Debug)]
pub struct Variant {
    pub is_supported: bool,
    pub flags: IndexMap<String, String>,
    pub shader_codes: IndexMap<PlatformShaderStage, ShaderCode>,
}
impl<'a> TryFromCtx<'a> for Variant {
    type Error = scroll::Error;

    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;
        let is_supported = read_bool(buffer, &mut offset)?;
        let flag_count: u16 = buffer.gread_with(&mut offset, LE)?;
        let shader_code_count: u16 = buffer.gread_with(&mut offset, LE)?;
        let mut flags = IndexMap::with_capacity(flag_count.into());
        for _ in 0..flag_count {
            let key = read_string(buffer, &mut offset)?;
            let value = read_string(buffer, &mut offset)?;
            flags.insert(key, value);
        }
        let mut shader_codes = IndexMap::with_capacity(shader_code_count.into());
        for _ in 0..shader_code_count {
            let stage: PlatformShaderStage = buffer.gread(&mut offset)?;
            let shader_code: ShaderCode = buffer.gread(&mut offset)?;
            shader_codes.insert(stage, shader_code);
        }
        Ok((
            Self {
                is_supported,
                flags,
                shader_codes,
            },
            offset,
        ))
    }
}
impl Variant {
    pub fn write<W>(&self, writer: &mut W) -> Result<(), WriteError>
    where
        W: Write,
    {
        writer.write_u8(self.is_supported.into())?;
        let len = self.flags.len().try_into()?;
        writer.write_u16::<LittleEndian>(len)?;
        let len = self.shader_codes.len().try_into()?;
        writer.write_u16::<LittleEndian>(len)?;
        for flag in self.flags.iter() {
            write_string(&flag.0, writer)?;
            write_string(&flag.1, writer)?;
        }
        for (platform_stage, code) in self.shader_codes.iter() {
            platform_stage.write(writer)?;
            code.write(writer)?;
        }

        Ok(())
    }
}
#[derive(PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum BlendMode {
    None,
    Replace,
    AlphaBlend,
    ColorBlendAlphaAdd,
    PreMultiplied,
    InvertColor,
    Additive,
    AdditiveAlpha,
    Multiply,
    MultiplyBoth,
    InverseSrcAlpha,
    SrcAlpha,
}
impl<'a> TryFromCtx<'a> for BlendMode {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let int: u16 = buffer.pread_with(0, LE)?;
        let enum_type = match int {
            0 => Self::None,
            1 => Self::Replace,
            2 => Self::AlphaBlend,
            3 => Self::ColorBlendAlphaAdd,
            4 => Self::PreMultiplied,
            5 => Self::InvertColor,
            6 => Self::Additive,
            7 => Self::AdditiveAlpha,
            8 => Self::Multiply,
            9 => Self::MultiplyBoth,
            10 => Self::InverseSrcAlpha,
            11 => Self::SrcAlpha,
            _ => return Err(scroll::Error::Custom(format!("Invalid blend_mode: {int}"))),
        };
        Ok((enum_type, 2))
    }
}
impl BlendMode {
    fn as_u16(&self) -> u16 {
        match self {
            Self::None => 0,
            Self::Replace => 1,
            Self::AlphaBlend => 2,
            Self::ColorBlendAlphaAdd => 3,
            Self::PreMultiplied => 4,
            Self::InvertColor => 5,
            Self::Additive => 6,
            Self::AdditiveAlpha => 7,
            Self::Multiply => 8,
            Self::MultiplyBoth => 9,
            Self::InverseSrcAlpha => 10,
            Self::SrcAlpha => 11,
        }
    }
}
#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
#[repr(u8)]
pub enum ShaderCodePlatform {
    Direct3DSm40, //Windows
    Direct3DSm50, //Windows
    Direct3DSm60, //Windows
    Direct3DSm65, //Windows
    Direct3DXB1,  //?
    Direct3DXBX,  //?
    Glsl120,      //?
    Glsl430,      //?
    Essl100,      //Android
    Essl300,      //?
    Essl310,      //Android (since 1.20.20.20)
    Metal,        //iOS
    Vulkan,       //Nintendo Switch
    Nvn,          //?
    Pssl,         //?
}

impl<'a> TryFromCtx<'a> for ShaderCodePlatform {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let int: u8 = buffer.pread_with(0, LE)?;
        let enum_type = match int {
            0 => Self::Direct3DSm40,
            1 => Self::Direct3DSm50,
            2 => Self::Direct3DSm60,
            3 => Self::Direct3DSm65,
            4 => Self::Direct3DXB1,
            5 => Self::Direct3DXBX,
            6 => Self::Glsl120,
            7 => Self::Glsl430,
            8 => Self::Essl100,
            9 => Self::Essl300,
            10 => Self::Essl310,
            11 => Self::Metal,
            12 => Self::Vulkan,
            13 => Self::Nvn,
            14 => Self::Pssl,
            _ => return Err(scroll::Error::Custom(format!("Invalid ShaderCodePlatform"))),
        };
        Ok((enum_type, 1))
    }
}
#[derive(Debug)]
pub struct ShaderCode {
    pub shader_inputs: IndexMap<String, ShaderInput>,
    pub source_hash: u64,
    pub bgfx_shader_data: Vec<u8>,
}
impl<'a> TryFromCtx<'a> for ShaderCode {
    type Error = scroll::Error;

    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;
        let input_count: u16 = buffer.gread_with(&mut offset, LE)?;
        let mut shader_inputs = IndexMap::with_capacity(input_count.into());
        for _ in 0..input_count {
            let name = read_string(buffer, &mut offset)?;
            let input: ShaderInput = buffer.gread(&mut offset)?;
            shader_inputs.insert(name, input);
        }
        let source_hash: u64 = buffer.gread_with(&mut offset, LE)?;
        let bsd_len: u32 = buffer.gread_with(&mut offset, LE)?;
        let bsd_size: usize = bsd_len.try_into().unwrap();
        let bgfx_shader_data = buffer[offset..offset + bsd_size].to_vec();
        offset += bsd_size;
        Ok((
            Self {
                shader_inputs,
                source_hash,
                bgfx_shader_data,
            },
            offset,
        ))
    }
}
impl ShaderCode {
    pub fn write<W>(&self, writer: &mut W) -> Result<(), WriteError>
    where
        W: Write,
    {
        let len = self.shader_inputs.len().try_into()?;
        writer.write_u16::<LittleEndian>(len)?;
        for (name, input) in self.shader_inputs.iter() {
            write_string(&name, writer)?;
            input.write(writer)?;
        }
        writer.write_u64::<LittleEndian>(self.source_hash)?;
        let len: u32 = self.bgfx_shader_data.len().try_into()?;
        writer.write_u32::<byteorder::LittleEndian>(len)?;
        writer.write_all(&self.bgfx_shader_data)?;
        Ok(())
    }
}
#[derive(Debug)]
pub struct ShaderInput {
    pub input_type: ShaderInputType,
    pub attribute: Attribute,
    pub is_per_instance: bool,
    pub precision_constraint: Option<PrecisionConstraint>,
    pub interpolation_constraint: Option<InterpolationConstraint>,
}

impl<'a> TryFromCtx<'a> for ShaderInput {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;
        let input_type: ShaderInputType = buffer.gread(&mut offset)?;
        let attribute: Attribute = buffer.gread(&mut offset)?;
        let is_per_instance = read_bool(buffer, &mut offset)?;
        let mut precision_constraint: Option<PrecisionConstraint> = None;
        let has_precision_constraint = read_bool(buffer, &mut offset)?;
        if has_precision_constraint {
            precision_constraint = Some(buffer.gread(&mut offset)?);
        }
        let mut interpolation_constraint: Option<InterpolationConstraint> = None;
        let has_interpolation_constraint = read_bool(buffer, &mut offset)?;
        if has_interpolation_constraint {
            interpolation_constraint = Some(buffer.gread(&mut offset)?);
        }
        Ok((
            Self {
                input_type,
                attribute,
                is_per_instance,
                precision_constraint,
                interpolation_constraint,
            },
            offset,
        ))
    }
}

impl ShaderInput {
    pub fn write<W>(&self, writer: &mut W) -> Result<(), WriteError>
    where
        W: Write,
    {
        writer.write_u8(self.input_type as u8)?;
        let (index, subindex) = self.attribute.to_tuple();
        writer.write_u8(index)?;
        writer.write_u8(subindex)?;
        writer.write_u8(self.is_per_instance as u8)?;
        optional_write(writer, self.precision_constraint.as_ref(), |o, v| {
            o.write_u8(*v as u8)
        })?;
        optional_write(writer, self.interpolation_constraint.as_ref(), |o, v| {
            o.write_u8(*v as u8)
        })?;
        Ok(())
    }
}
#[derive(Eq, PartialEq, Debug, Clone, Copy)]
#[repr(u8)]
pub enum ShaderInputType {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Int,
    Int2,
    Int3,
    Int4,
    UInt,
    UInt2,
    UInt3,
    UInt4,
    Mat4,
}
impl<'a> TryFromCtx<'a> for ShaderInputType {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let int: u8 = buffer.pread_with(0, LE)?;
        let enum_type = match int {
            0 => Self::Float,
            1 => Self::Vec2,
            2 => Self::Vec3,
            3 => Self::Vec4,
            4 => Self::Int,
            5 => Self::Int2,
            6 => Self::Int3,
            7 => Self::Int4,
            8 => Self::UInt,
            9 => Self::UInt2,
            10 => Self::UInt3,
            11 => Self::UInt4,
            12 => Self::Mat4,
            _ => return Err(scroll::Error::Custom(format!("Invalid ShaderInputType"))),
        };
        Ok((enum_type, 1))
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
#[repr(u8)]
pub enum PrecisionConstraint {
    Low,
    Medium,
    High,
}

impl<'a> TryFromCtx<'a> for PrecisionConstraint {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let int: u8 = buffer.pread_with(0, LE)?;
        let enum_type = match int {
            0 => Self::Low,
            1 => Self::Medium,
            2 => Self::High,
            _ => {
                return Err(scroll::Error::Custom(format!(
                    "Invalid PrecisionConstraint: {int}"
                )))
            }
        };
        Ok((enum_type, 1))
    }
}
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterpolationConstraint {
    Flat,
    Smooth,
    NoPerspective,
    Centroid,
}
impl<'a> TryFromCtx<'a> for InterpolationConstraint {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let int: u8 = buffer.pread_with(0, LE)?;
        let enum_type = match int {
            0 => Self::Flat,
            1 => Self::Smooth,
            2 => Self::NoPerspective,
            3 => Self::Centroid,
            _ => {
                return Err(scroll::Error::Custom(format!(
                    "Invalid InterpolationConstraint: {int}"
                )))
            }
        };
        Ok((enum_type, 1))
    }
}
#[derive(Debug)]
pub enum Attribute {
    Position,
    Normal,
    Tangent,
    Bitangent,
    Color0,
    Color1,
    Color2,
    Color3,
    Indices,
    Weights,
    TexCoord0,
    TexCoord1,
    TexCoord2,
    TexCoord3,
    TexCoord4,
    TexCoord5,
    TexCoord6,
    TexCoord7,
    TexCoord8,
    //Unknown_8_0(8, 0),
    FrontFacing,
}
impl<'a> TryFromCtx<'a> for Attribute {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let index: u8 = buffer.pread(0)?;
        let sub_index: u8 = buffer.pread(1)?;
        let enum_type = match (index, sub_index) {
            (0, 0) => Self::Position,
            (1, 0) => Self::Normal,
            (2, 0) => Self::Tangent,
            (3, 0) => Self::Bitangent,
            (4, 0) => Self::Color0,
            (4, 1) => Self::Color1,
            (4, 2) => Self::Color2,
            (4, 3) => Self::Color3,
            (5, 0) => Self::Indices,
            (6, 0) => Self::Weights,
            (7, 0) => Self::TexCoord0,
            (7, 1) => Self::TexCoord1,
            (7, 2) => Self::TexCoord2,
            (7, 3) => Self::TexCoord3,
            (7, 4) => Self::TexCoord4,
            (7, 5) => Self::TexCoord5,
            (7, 6) => Self::TexCoord6,
            (7, 7) => Self::TexCoord7,
            (7, 8) => Self::TexCoord8,
            (9, 0) => Self::FrontFacing,
            _ => {
                return Err(scroll::Error::Custom(format!(
                    "Attribute tuple[{:?}] is invalid",
                    (index, sub_index)
                )))
            }
        };
        Ok((enum_type, 2))
    }
}
impl Attribute {
    fn to_tuple(&self) -> (u8, u8) {
        match self {
            Self::Position => (0, 0),
            Self::Normal => (1, 0),
            Self::Tangent => (2, 0),
            Self::Bitangent => (3, 0),
            Self::Color0 => (4, 0),
            Self::Color1 => (4, 1),
            Self::Color2 => (4, 2),
            Self::Color3 => (4, 3),
            Self::Indices => (5, 0),
            Self::Weights => (6, 0),
            Self::TexCoord0 => (7, 0),
            Self::TexCoord1 => (7, 1),
            Self::TexCoord2 => (7, 2),

            Self::TexCoord3 => (7, 3),
            Self::TexCoord4 => (7, 4),
            Self::TexCoord5 => (7, 5),

            Self::TexCoord6 => (7, 6),
            Self::TexCoord7 => (7, 7),
            Self::TexCoord8 => (7, 8),
            Self::FrontFacing => (9, 0),
        }
    }
}
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
#[repr(u8)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
    Unknown,
}

impl<'a> TryFromCtx<'a> for ShaderStage {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let int: u8 = buffer.pread_with(0, LE)?;
        let enum_type = match int {
            0 => Self::Vertex,
            1 => Self::Fragment,
            2 => Self::Compute,
            3 => Self::Unknown,
            _ => return Err(scroll::Error::Custom(format!("Invalid ShaderStage: {int}"))),
        };
        Ok((enum_type, 1))
    }
}
#[derive(Eq, PartialEq, Hash, Debug)]
pub struct PlatformShaderStage {
    pub stage_name: String,
    pub platform_name: String,
    pub stage: ShaderStage,
    pub platform: ShaderCodePlatform,
}
impl<'a> TryFromCtx<'a> for PlatformShaderStage {
    type Error = scroll::Error;

    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;
        let stage_name = read_string(buffer, &mut offset).unwrap();
        let platform_name = read_string(buffer, &mut offset).unwrap();
        let stage: ShaderStage = buffer.gread(&mut offset).unwrap();
        let platform: ShaderCodePlatform = buffer.gread(&mut offset).unwrap();
        Ok((
            Self {
                stage_name,
                platform_name,
                stage,
                platform,
            },
            offset,
        ))
    }
}
impl PlatformShaderStage {
    pub fn write<W>(&self, writer: &mut W) -> Result<(), WriteError>
    where
        W: Write,
    {
        write_string(&self.stage_name, writer)?;
        write_string(&self.platform_name, writer)?;
        writer.write_u8(self.stage as u8)?;
        writer.write_u8(self.platform as u8)?;
        Ok(())
    }
}
