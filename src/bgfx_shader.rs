use byteorder::{LittleEndian, WriteBytesExt};
use scroll::{
    ctx::{StrCtx, TryFromCtx},
    Pread, LE,
};
use std::io::Write;

use crate::WriteError;
pub struct BgfxShader {
    pub magic: u32,
    pub hash: u32,
    pub uniforms: Vec<Uniform>,
    pub code: Vec<u8>,
}
impl<'a> TryFromCtx<'a> for BgfxShader {
    type Error = scroll::Error;
    fn try_from_ctx(input: &'a [u8], _: ()) -> Result<(Self, usize), Self::Error> {
        let mut offset = 0;
        let magic = input.gread_with(&mut offset, LE)?;
        let hash = input.gread_with(&mut offset, LE)?;
        let uniform_count: u16 = input.gread_with(&mut offset, LE)?;
        let uniforms: Vec<Uniform> = (0..uniform_count)
            .flat_map(|_| input.gread(&mut offset))
            .collect();
        let code_len: u32 = input.gread_with(&mut offset, LE)?;
        let code_len: usize = code_len.try_into().map_err(|e| {
            scroll::Error::Custom(format!(
                "Code len: {code_len} does not fit in usize, error: {e}"
            ))
        })?;
        let code = input.gread_with::<&[u8]>(&mut offset, code_len)?.to_vec();
        let _dumbbyte: u8 = input.gread(&mut offset)?;
        Ok((
            Self {
                magic,
                hash,
                uniforms,
                code,
            },
            offset,
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
    type Error = scroll::Error;
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
