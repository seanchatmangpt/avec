//! Indirect jump-table codec and framing dispatch.
//!
//! Replaces branch-predictor-stalling match/switch statements over variable-type
//! byte tokens with array-indexed function pointer tables compiling to single
//! indirect jumps (`br x8` on arm64 / `jmpq` on x86_64).

pub type KernelDecodeFn = unsafe fn(src: *const u64, src_len: usize, out: *mut u64, out_len: usize);

unsafe fn decode_rle_width1(src: *const u64, src_len: usize, out: *mut u64, out_len: usize) {
    if src_len > 0 && out_len > 0 && !src.is_null() && !out.is_null() {
        let slice = unsafe { core::slice::from_raw_parts(src, src_len) };
        let out_slice = unsafe { core::slice::from_raw_parts_mut(out, out_len) };
        crate::unpack::unpack_1bit(slice, out_slice);
    }
}

unsafe fn decode_rle_width2(src: *const u64, src_len: usize, out: *mut u64, out_len: usize) {
    if src_len > 0 && out_len > 0 && !src.is_null() && !out.is_null() {
        let slice = unsafe { core::slice::from_raw_parts(src, src_len) };
        let out_slice = unsafe { core::slice::from_raw_parts_mut(out, out_len) };
        crate::unpack::unpack_2bit(slice, out_slice);
    }
}

unsafe fn decode_rle_width4(src: *const u64, src_len: usize, out: *mut u64, out_len: usize) {
    if src_len > 0 && out_len > 0 && !src.is_null() && !out.is_null() {
        let slice = unsafe { core::slice::from_raw_parts(src, src_len) };
        let out_slice = unsafe { core::slice::from_raw_parts_mut(out, out_len) };
        crate::unpack::unpack_4bit(slice, out_slice);
    }
}

unsafe fn decode_rle_width8(src: *const u64, src_len: usize, out: *mut u64, out_len: usize) {
    if src_len > 0 && out_len > 0 && !src.is_null() && !out.is_null() {
        let slice = unsafe { core::slice::from_raw_parts(src, src_len) };
        let out_slice = unsafe { core::slice::from_raw_parts_mut(out, out_len) };
        crate::unpack::unpack_8bit(slice, out_slice);
    }
}

unsafe fn decode_invalid(_src: *const u64, _src_len: usize, _out: *mut u64, _out_len: usize) {
    // Fail-closed no-op trap
}

/// Function jump-table aligned to hardware cache line
#[repr(C, align(64))]
pub struct DispatchTable {
    pub targets: [KernelDecodeFn; 16],
}

pub static KERNEL_DISPATCH: DispatchTable = DispatchTable {
    targets: [
        decode_invalid,    // 0x0
        decode_rle_width1, // 0x1: width 1 bit
        decode_rle_width2, // 0x2: width 2 bits
        decode_invalid,
        decode_rle_width4, // 0x4: width 4 bits
        decode_invalid,
        decode_invalid,
        decode_invalid,
        decode_rle_width8, // 0x8: width 8 bits
        decode_invalid,
        decode_invalid,
        decode_invalid,
        decode_invalid,
        decode_invalid,
        decode_invalid,
        decode_invalid,
    ],
};

/// Branchless dispatch: replaces match blocks with indirect indexed call
#[inline(always)]
pub fn dispatch_decode(type_token: u8, src: &[u64], out: &mut [u64]) {
    let index = (type_token & 0x0F) as usize;
    unsafe {
        let func = KERNEL_DISPATCH.targets[index];
        func(src.as_ptr(), src.len(), out.as_mut_ptr(), out.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jump_table_dispatch_parity() {
        let src = [0b10101010u64; 2];
        let mut out = [0u64; 64];

        // Token 0x01 -> width 1 bit
        dispatch_decode(1, &src, &mut out);
        assert_eq!(out[0], 0);
        assert_eq!(out[1], 1);

        // Token 0x08 -> width 8 bit
        let mut out8 = [0u64; 16];
        dispatch_decode(8, &src, &mut out8);
        assert_eq!(out8[0], 0xAA);

        // Invalid token does not panic
        dispatch_decode(0x05, &src, &mut out);
    }
}
