use core::slice;
use scroll::Pread;

use crate::{CompiledMaterialDefinition, ALL_VERSIONS};
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Buffer {
    data: *mut u8,
    len: usize,
}

#[no_mangle]
/// Update a material file
/// # Safety
/// - Input pointer and length are valid
/// - You free the output later
extern "C" fn update_file(
    in_length: usize,
    in_buffer: *const u8,
    out_buffer: *mut Buffer,
) -> libc::c_int {
    let slice = unsafe { slice::from_raw_parts(in_buffer, in_length) };
    let mut output = Vec::with_capacity(slice.len());
    for version in ALL_VERSIONS.into_iter().rev() {
        if let Ok(parsed) = slice.pread_with::<CompiledMaterialDefinition>(0, version) {
            if parsed
                .write(&mut output, crate::MinecraftVersion::V1_21_20)
                .is_err()
            {
                continue;
            }
            let mut boxed = output.into_boxed_slice();
            let bufdata = Buffer {
                data: boxed.as_mut_ptr(),
                len: boxed.len(),
            };
            unsafe { *out_buffer = bufdata };
            std::mem::forget(boxed);
            return 0;
        }
    }
    -1
}
#[no_mangle]
extern "C" fn free_buf(buf: Buffer) {
    let s = unsafe { std::slice::from_raw_parts_mut(buf.data, buf.len) };
    let s = s.as_mut_ptr();
    unsafe {
        drop(Box::from_raw(s));
    }
}
