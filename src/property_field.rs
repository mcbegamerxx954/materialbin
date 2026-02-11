use crate::{common::read_bool, WriteError};
use byteorder::WriteBytesExt;
use scroll::{ctx::TryFromCtx, Pread};
use std::io::Write;
#[derive(Debug)]
pub struct PropertyField {
    pub field_type: PropertyType,
    pub num: u32,
    pub vector_data: Option<Vec<u8>>,
    pub matrix_data: Option<Vec<u8>>,
}
impl<'a> TryFromCtx<'a> for PropertyField {
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
                    vector_data = Some(buffer[offset..offset + 16_usize].to_vec());
                    offset += 16;
                }
                PropertyType::Mat3 => {
                    matrix_data = Some(buffer[offset..offset + 36_usize].to_vec());
                    offset += 36;
                }
                PropertyType::Mat4 => {
                    matrix_data = Some(buffer[offset..offset + 64_usize].to_vec());
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
impl PropertyField {
    pub fn write<W>(&self, writer: &mut W) -> Result<(), WriteError>
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
                if let Some(data) = &self.matrix_data {
                    writer.write_all(data)?;
                }
            }
            PropertyType::Mat4 => {
                writer.write_u8(self.matrix_data.is_some().into())?;
                if let Some(data) = &self.matrix_data {
                    writer.write_all(data)?;
                }
            }
            // We do absolutely nothing
            PropertyType::External => {}
        }
        Ok(())
    }
}
#[derive(Debug, PartialEq)]
pub enum PropertyType {
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
    pub fn to_u16(&self) -> u16 {
        match self {
            Self::Vec4 => 2,
            Self::Mat3 => 3,
            Self::Mat4 => 4,
            Self::External => 5,
        }
    }
}
