use std::collections::HashMap;

use byteorder::{LittleEndian, WriteBytesExt};
use scroll::{
    ctx::{StrCtx, TryFromCtx},
    Pread, LE,
};
use std::io::Write;

use thiserror::Error;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MinecraftVersion {
    V1_20_80,
    V1_19_60,
    V1_18_30,
}

impl Default for MinecraftVersion {
    fn default() -> Self {
        Self::V1_20_80
    }
}
impl std::fmt::Display for MinecraftVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::V1_20_80 => write!(f, "v1.20.80"),
            Self::V1_19_60 => write!(f, "v1.19.60"),
            Self::V1_18_30 => write!(f, "v1.18.30"),
        }
    }
}

#[derive(Debug)]
pub struct CompiledMaterialDefinition<'a> {
    version: u64,
    encryption_variant: EncryptionVariant,
    name: &'a str,
    parent_name: Option<&'a str>,
    sampler_definitions: HashMap<&'a str, SamplerDefinition<'a>>,
    property_fields: HashMap<&'a str, PropertyField<'a>>,
    passes: HashMap<&'a str, Pass<'a>>,
}
impl<'a> TryFromCtx<'a, MinecraftVersion> for CompiledMaterialDefinition<'a> {
    type Error = scroll::Error;

    fn try_from_ctx(buffer: &'a [u8], ctx: MinecraftVersion) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;
        const MAGIC: u64 = 0xA11DA1A;
        if buffer.gread::<u64>(&mut offset)? != MAGIC {
            return Err(scroll::Error::BadInput {
                size: offset,
                msg: "Invalid magic",
            });
        }
        if read_string(buffer, &mut offset)? != "RenderDragon.CompiledMaterialDefinition" {
            return Err(scroll::Error::BadInput {
                size: offset,
                msg: "Invalid definition",
            });
        }
        let version: u64 = buffer.gread_with(&mut offset, LE)?;
        let encryption_variant: EncryptionVariant = buffer.gread(&mut offset)?;
        if encryption_variant != EncryptionVariant::None {
            return Err(scroll::Error::BadInput {
                size: offset,
                msg: "Not decrypting encrypted material",
            });
        }
        let name = read_string(buffer, &mut offset)?;
        let mut parent_name = None;
        let has_parent_name = read_bool(buffer, &mut offset)?;
        if has_parent_name {
            parent_name = Some(read_string(buffer, &mut offset)?);
        }

        let sampler_definition_count: u8 = buffer.gread_with(&mut offset, LE)?;
        let mut sampler_definitions = HashMap::with_capacity(sampler_definition_count.into());
        for _ in 0..sampler_definition_count {
            let name = read_string(buffer, &mut offset)?;
            let sampler_definition: SamplerDefinition = buffer.gread_with(&mut offset, ctx)?;
            sampler_definitions.insert(name, sampler_definition);
        }

        let property_field_count: u16 = buffer.gread_with(&mut offset, LE)?;
        let mut property_fields = HashMap::with_capacity(property_field_count.into());
        for _ in 0..property_field_count {
            let name = read_string(buffer, &mut offset)?;
            let property_field: PropertyField = buffer.gread(&mut offset)?;
            property_fields.insert(name, property_field);
        }

        let pass_count: u16 = buffer.gread_with(&mut offset, LE)?;
        let mut passes = HashMap::with_capacity(pass_count.into());
        for _ in 0..pass_count {
            let name = read_string(buffer, &mut offset)?;
            let pass: Pass = buffer.gread(&mut offset)?;
            passes.insert(name, pass);
        }
        let end: u64 = buffer.gread_with(&mut offset, LE)?;
        Ok((
            Self {
                version,
                encryption_variant,
                name,
                parent_name,
                sampler_definitions,
                property_fields,
                passes,
            },
            offset,
        ))
    }
}
impl<'a> CompiledMaterialDefinition<'a> {
    pub fn write<W>(&self, writer: &mut W, version: MinecraftVersion) -> Result<(), WriteError>
    where
        W: Write,
    {
        const MAGIC: u64 = 0xA11DA1A;
        writer.write_u64::<LittleEndian>(MAGIC)?;
        write_string("RenderDragon.CompiledMaterialDefinition", writer)?;
        writer.write_u64::<LittleEndian>(self.version)?;
        self.encryption_variant.write(writer)?;
        write_string(self.name, writer)?;
        optional_write(writer, self.parent_name, |o, v| write_string(v, o))?;
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
#[derive(Debug)]
struct SamplerDefinition<'a> {
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
    fn write<W>(&self, writer: &mut W, version: MinecraftVersion) -> Result<(), WriteError>
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
        let str_len: u32 = buffer.gread_with(&mut offset, LE)?;
        let str = buffer.gread_with(&mut offset, StrCtx::Length(str_len as usize))?;
        let size: u32 = buffer.gread_with(&mut offset, LE)?;
        Ok((Self { name: str, size }, offset))
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
fn read_bool(buffer: &[u8], offset: &mut usize) -> Result<bool, scroll::Error> {
    let bool_u8: u8 = buffer.gread_with(offset, LE)?;
    Ok(bool_u8 != 0)
}
fn read_string<'a>(buffer: &'a [u8], offset: &mut usize) -> Result<&'a str, scroll::Error> {
    let str_len: u32 = buffer.gread_with(offset, LE)?;
    let str: &str = buffer.gread_with(offset, StrCtx::Length(str_len as usize))?;
    Ok(str)
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
struct Pass<'a> {
    bitset: &'a str,
    fallback: &'a str,
    default_blendmode: Option<BlendMode>,
    default_flag_values: HashMap<&'a str, &'a str>,
    variants: Vec<Variant<'a>>,
}
impl<'a> TryFromCtx<'a, MinecraftVersion> for Pass<'a> {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], ctx: MinecraftVersion) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;
        let mut bitset = "";
        if ctx == MinecraftVersion::V1_18_30 {
            let has_bitset = buffer.gread_with::<u8>(&mut offset, LE)? == 15;
            // rewind
            offset -= 4;
            if has_bitset {
                bitset = read_string(buffer, &mut offset)?;
            } else {
                // skip reading byte we have no use for
                offset += 1;
            }
        } else {
            bitset = read_string(buffer, &mut offset)?;
        }
        let fallback = read_string(buffer, &mut offset)?;
        let mut default_blendmode: Option<BlendMode> = None;
        let has_blendmode = read_bool(buffer, &mut offset)?;
        if has_blendmode {
            default_blendmode = Some(buffer.gread_with(&mut offset, ())?);
        }
        let flag_dvalue_count: u16 = buffer.gread_with(&mut offset, LE)?;
        let mut default_flag_values = HashMap::with_capacity(flag_dvalue_count.into());

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
impl<'a> Pass<'a> {
    fn write<W>(&self, writer: &mut W, version: MinecraftVersion) -> Result<(), WriteError>
    where
        W: Write,
    {
        if self.bitset.is_empty() {
            return Err(WriteError::Compat(
                "Bitset string is empty, Try fixing it in the main struct".to_string(),
            ));
        } else if version == MinecraftVersion::V1_18_30 {
        }
        write_string(self.bitset, writer)?;
        write_string(self.fallback, writer)?;
        optional_write(writer, self.default_blendmode.as_ref(), |o, v| {
            o.write_u16::<LittleEndian>(v.as_u16())
        })?;
        let len = self.default_flag_values.len().try_into()?;
        writer.write_u16::<LittleEndian>(len)?;
        for (key, value) in self.default_flag_values.iter() {
            write_string(key, writer)?;
            write_string(value, writer)?;
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
struct Variant<'a> {
    is_supported: bool,
    flags: HashMap<&'a str, &'a str>,
    shader_codes: HashMap<PlatformShaderStage<'a>, ShaderCode<'a>>,
}
impl<'a> TryFromCtx<'a> for Variant<'a> {
    type Error = scroll::Error;

    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;
        let is_supported = read_bool(buffer, &mut offset)?;
        let flag_count: u16 = buffer.gread_with(&mut offset, LE)?;
        let shader_code_count: u16 = buffer.gread_with(&mut offset, LE)?;
        let mut flags = HashMap::with_capacity(flag_count.into());
        for _ in 0..flag_count {
            let key = read_string(buffer, &mut offset)?;
            let value = read_string(buffer, &mut offset)?;
            flags.insert(key, value);
        }
        let mut shader_codes = HashMap::with_capacity(shader_code_count.into());
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
impl<'a> Variant<'a> {
    fn write<W>(&self, writer: &mut W) -> Result<(), WriteError>
    where
        W: Write,
    {
        writer.write_u8(self.is_supported.into())?;
        let len = self.flags.len().try_into()?;
        writer.write_u16::<LittleEndian>(len)?;
        let len = self.shader_codes.len().try_into()?;
        writer.write_u16::<LittleEndian>(len)?;
        for flag in self.flags.iter() {
            write_string(flag.0, writer)?;
            write_string(flag.1, writer)?;
        }
        for (platform_stage, code) in self.shader_codes.iter() {
            platform_stage.write(writer)?;
            code.write(writer)?;
        }

        Ok(())
    }
}
#[derive(Debug)]
struct ShaderCode<'a> {
    shader_inputs: HashMap<&'a str, ShaderInput>,
    source_hash: u64,
    bgfx_shader_data: &'a [u8],
}
impl<'a> TryFromCtx<'a> for ShaderCode<'a> {
    type Error = scroll::Error;

    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;

        let input_count: u16 = buffer.gread_with(&mut offset, LE)?;
        let mut shader_inputs = HashMap::with_capacity(input_count.into());
        for _ in 0..input_count {
            let name = read_string(buffer, &mut offset)?;
            let input: ShaderInput = buffer.gread(&mut offset)?;
            shader_inputs.insert(name, input);
        }
        let source_hash: u64 = buffer.gread_with(&mut offset, LE)?;
        let bsd_len: u32 = buffer.gread_with(&mut offset, LE)?;
        let bgfx_shader_data = &buffer[offset..offset + bsd_len as usize];
        offset += bsd_len as usize;
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
impl<'a> ShaderCode<'a> {
    fn write<W>(&self, writer: &mut W) -> Result<(), WriteError>
    where
        W: Write,
    {
        let len = self.shader_inputs.len().try_into()?;
        writer.write_u16::<LittleEndian>(len)?;
        for (name, input) in self.shader_inputs.iter() {
            write_string(name, writer)?;
            input.write(writer)?;
        }
        writer.write_u64::<LittleEndian>(self.source_hash)?;
        let len: u32 = self.bgfx_shader_data.len().try_into()?;
        writer.write_u32::<byteorder::LittleEndian>(len)?;
        writer.write_all(self.bgfx_shader_data)?;
        Ok(())
    }
}
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
#[repr(u8)]
enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
}

impl<'a> TryFromCtx<'a> for ShaderStage {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let int: u8 = buffer.pread_with(0, LE)?;
        let enum_type = match int {
            0 => Self::Vertex,
            1 => Self::Fragment,
            2 => Self::Compute,
            _ => return Err(scroll::Error::Custom(format!("Invalid ShaderStage: {int}"))),
        };
        Ok((enum_type, 1))
    }
}

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
#[repr(u8)]
enum ShaderCodePlatform {
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
struct ShaderInput {
    input_type: ShaderInputType,
    attribute: Attribute,
    is_per_instance: bool,
    precision_constraint: Option<PrecisionConstraint>,
    interpolation_constraint: Option<InterpolationConstraint>,
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
fn optional_write<T, W, Fn, E>(
    output: &mut W,
    option: Option<T>,
    write_fn: Fn,
) -> Result<(), WriteError>
where
    W: Write,
    Fn: FnOnce(&mut W, T) -> Result<(), E>,
    WriteError: From<E>,
{
    output.write_u8(option.is_some().into())?;
    if let Some(value) = option {
        write_fn(output, value)?;
    }
    Ok(())
}
impl ShaderInput {
    fn write<W>(&self, writer: &mut W) -> Result<(), WriteError>
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
#[derive(Eq, PartialEq, Hash, Debug)]
struct PlatformShaderStage<'a> {
    stage_name: &'a str,
    platform_name: &'a str,
    stage: ShaderStage,
    platform: ShaderCodePlatform,
}
impl<'a> TryFromCtx<'a> for PlatformShaderStage<'a> {
    type Error = scroll::Error;

    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;
        let stage_name = read_string(buffer, &mut offset)?;
        let platform_name = read_string(buffer, &mut offset)?;
        let stage: ShaderStage = buffer.gread(&mut offset)?;
        let platform: ShaderCodePlatform = buffer.gread(&mut offset)?;
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
impl<'a> PlatformShaderStage<'a> {
    fn write<W>(&self, writer: &mut W) -> Result<(), WriteError>
    where
        W: Write,
    {
        write_string(self.stage_name, writer)?;
        write_string(self.platform_name, writer)?;
        writer.write_u8(self.stage as u8)?;
        writer.write_u8(self.platform as u8)?;
        Ok(())
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
#[repr(u8)]
enum ShaderInputType {
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
enum PrecisionConstraint {
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
enum InterpolationConstraint {
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
enum Attribute {
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
#[derive(Debug)]
struct PropertyField<'a> {
    field_type: PropertyType,
    num: u32,
    vector_data: Option<&'a [u8]>,
    matrix_data: Option<&'a [u8]>,
}
impl<'a> TryFromCtx<'a> for PropertyField<'a> {
    type Error = scroll::Error;

    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;

        let field_type: PropertyType = buffer.gread(&mut offset)?;
        let num: u32 = buffer.gread(&mut offset)?;
        let has_data = read_bool(buffer, &mut offset)?;
        let mut vector_data = None;
        let mut matrix_data = None;

        if has_data {
            match field_type {
                PropertyType::Vec4 => {
                    vector_data = Some(&buffer[offset..offset + 16_usize]);
                    offset += 16;
                }
                PropertyType::Mat3 => {
                    matrix_data = Some(&buffer[offset..offset + 36_usize]);
                    offset += 36;
                }
                PropertyType::Mat4 => {
                    matrix_data = Some(&buffer[offset..offset + 64_usize]);
                    offset += 64;
                }
                // We do nothing
                PropertyType::External => {}
            }
        }
        Ok((
            Self {
                field_type,
                num,
                vector_data,
                matrix_data,
            },
            offset,
        ))
    }
}
impl<'a> PropertyField<'a> {
    fn write<W>(&self, writer: &mut W) -> Result<(), WriteError>
    where
        W: Write,
    {
        writer.write_u16::<byteorder::LittleEndian>(self.field_type.to_u16())?;
        if self.field_type != PropertyType::External {
            writer.write_u32::<byteorder::LittleEndian>(self.num)?;
        }
        match self.field_type {
            PropertyType::Vec4 => {
                writer.write_u8(self.vector_data.is_some().into())?;
                if let Some(data) = &self.vector_data {
                    writer.write_all(data)?;
                }
            }
            PropertyType::Mat3 => {
                writer.write_u8(self.matrix_data.is_some().into())?;
                if let Some(data) = self.matrix_data {
                    writer.write_all(data)?;
                }
            }
            PropertyType::Mat4 => {
                writer.write_u8(self.matrix_data.is_some().into())?;
                if let Some(data) = self.matrix_data {
                    writer.write_all(data)?;
                }
            }
            // We do absolutely nothing
            PropertyType::External => {}
        }
        Ok(())
    }
}
#[derive(Error, Debug)]
pub enum WriteError {
    #[error("String length to u16 failed")]
    IntConvert(#[from] std::num::TryFromIntError),
    #[error("Io error")]
    IoError(#[from] std::io::Error),
    #[error("Compat error")]
    Compat(String),
}
fn write_string<W>(string: &str, writer: &mut W) -> Result<(), WriteError>
where
    W: Write,
{
    use byteorder::LE;
    let len: u32 = string.len().try_into()?;
    writer.write_u32::<LE>(len)?;
    writer.write_all(string.as_bytes())?;
    Ok(())
}

#[derive(Debug, PartialEq)]
enum PropertyType {
    Vec4,
    Mat3,
    Mat4,
    External,
}
impl<'a> TryFromCtx<'a> for PropertyType {
    type Error = scroll::Error;

    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let property_type: u16 = buffer.pread(0)?;
        let enum_type = match property_type {
            2 => Self::Vec4,
            3 => Self::Mat3,
            4 => Self::Mat4,
            5 => Self::External,
            _ => {
                return Err(scroll::Error::Custom(format!(
                    "property type is invalid: {property_type}"
                )))
            }
        };
        Ok((enum_type, 2))
    }
}
impl PropertyType {
    fn to_u16(&self) -> u16 {
        match self {
            Self::Vec4 => 2,
            Self::Mat3 => 3,
            Self::Mat4 => 4,
            Self::External => 5,
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
#[repr(u16)]
enum BlendMode {
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

#[derive(PartialEq, Eq, Debug)]
enum EncryptionVariant {
    None,
    SimplePassphrase,
    KeyPair,
}
impl<'a> TryFromCtx<'a> for EncryptionVariant {
    type Error = scroll::Error;
    fn try_from_ctx(buffer: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;
        let encryption: u32 = buffer.gread_with(&mut offset, LE)?;
        let enum_type = match encryption {
            0x4E4F4E45_u32 => Self::None,
            0x534D504C_u32 => Self::SimplePassphrase,
            0x4B595052_u32 => Self::KeyPair,
            _ => {
                return Err(scroll::Error::Custom(format!(
                    "Invalid EnctyptionVariant: {encryption}"
                )))
            }
        };
        Ok((enum_type, offset))
    }
}
impl EncryptionVariant {
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
}
