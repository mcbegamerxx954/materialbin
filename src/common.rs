use crate::WriteError;
use byteorder::WriteBytesExt;
use scroll::{ctx::StrCtx, Pread, LE};
use std::io::Write;
pub fn read_bool(buffer: &[u8], offset: &mut usize) -> Result<bool, scroll::Error> {
    let bool_u8: u8 = buffer.gread_with(offset, LE)?;
    Ok(bool_u8 != 0)
}
pub fn read_string<'a>(buffer: &'a [u8], offset: &mut usize) -> Result<&'a str, scroll::Error> {
    let str_len: u32 = buffer.gread_with(offset, LE)?;
    let str: &str = buffer.gread_with(offset, StrCtx::Length(str_len as usize))?;
    Ok(str)
}
pub fn write_string<W>(string: &str, writer: &mut W) -> Result<(), WriteError>
where
    W: Write,
{
    use byteorder::LE;
    let len: u32 = string.len().try_into()?;
    writer.write_u32::<LE>(len)?;
    writer.write_all(string.as_bytes())?;
    Ok(())
}
pub fn optional_write<T, W, Fn, E>(
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
