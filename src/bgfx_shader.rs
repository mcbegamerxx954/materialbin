use byteorder::{LittleEndian, WriteBytesExt};
use scroll::{
    ctx::{StrCtx, TryFromCtx},
    Pread, LE,
};
use std::io::Write;

use crate::{MyError, WriteError};
pub struct BgfxShader {
    pub magic: u32,
    pub hash: u32,
    pub uniforms: Vec<Uniform>,
    pub code: Vec<u8>,
    pub attributes: Option<Vec<u16>>,
    pub size: Option<u16>,
}
impl<'a> TryFromCtx<'a> for BgfxShader {
    type Error = MyError;
    fn try_from_ctx(input: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let offset = &mut 0;
        let magic = input.gread_with(offset, LE)?;
        let hash = input.gread_with(offset, LE)?;
        let uniform_count: u16 = input.gread_with(offset, LE)?;
        let uniforms: Vec<Uniform> = (0..uniform_count)
            .flat_map(|_| input.gread(offset))
            .collect();
        let code_len: u32 = input.gread_with(offset, LE)?;
        let code_len: usize = code_len.try_into().map_err(|e| {
            scroll::Error::Custom(
                format!("Code len: {code_len} does not fit in usize, error: {e}").into(),
            )
        })?;
        let code = input.gread_with::<&[u8]>(offset, code_len)?.to_vec();
        let _dumbbyte: u8 = input.gread(offset)?;

        let mut attributes = None;
        let mut size = None;
        if let Ok(attr_count) = input.gread::<u8>(offset) {
            if attr_count != 0 {
                // let _: u16 = input.gread(offset)?;
                let parsed: Result<Vec<u16>, scroll::Error> =
                    (0..attr_count).map(|_| input.gread(offset)).collect();
                attributes = Some(parsed?);
                size = Some(input.gread(offset)?);
            }
        }
        Ok((
            Self {
                magic,
                hash,
                uniforms,
                code,
                attributes,
                size,
            },
            *offset,
        ))
    }
}
impl BgfxShader {
    pub fn write<W>(&self, writer: &mut W) -> Result<(), WriteError>
    where
        W: Write,
    {
        writer.write_u32::<LittleEndian>(self.magic)?;
        writer.write_u32::<LittleEndian>(self.hash)?;
        writer.write_u16::<LittleEndian>(self.uniforms.len().try_into()?)?;
        for uniform in self.uniforms.iter() {
            uniform.write(writer)?;
        }
        writer.write_u32::<LittleEndian>(self.code.len().try_into()?)?;
        writer.write_all(&self.code)?;
        writer.write_u8(0)?;
        if let Some(attrs) = &self.attributes {
            writer.write_u8(attrs.len() as u8)?;
            for attr in attrs {
                writer.write_u16::<LittleEndian>(*attr)?;
            }
            if let Some(size) = &self.size {
                writer.write_u16::<LittleEndian>(*size)?;
            }
        }
        Ok(())
    }
}
pub struct Uniform {
    pub name: String,
    pub utype: u8,
    pub num: u8,
    pub reg_index: u16,
    pub reg_count: u16,
}

impl<'a> TryFromCtx<'a> for Uniform {
    type Error = MyError;
    fn try_from_ctx(input: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;
        let str_len: u8 = input.gread(&mut offset)?;
        let name = input
            .gread_with::<&str>(&mut offset, StrCtx::Length(str_len.into()))?
            .to_owned();
        let utype = input.gread(&mut offset)?;
        let num = input.gread(&mut offset)?;
        let reg_index = input.gread_with(&mut offset, LE)?;
        let reg_count = input.gread_with(&mut offset, LE)?;
        Ok((
            Self {
                name,
                utype,
                num,
                reg_index,
                reg_count,
            },
            offset,
        ))
    }
}
impl Uniform {
    pub fn write<W>(&self, writer: &mut W) -> Result<(), WriteError>
    where
        W: Write,
    {
        writer.write_u8(self.name.len().try_into()?)?;
        writer.write_all(self.name.as_bytes())?;
        writer.write_u8(self.utype)?;
        writer.write_u8(self.num)?;
        writer.write_u16::<LittleEndian>(self.reg_index)?;
        writer.write_u16::<LittleEndian>(self.reg_count)?;
        Ok(())
    }
}
