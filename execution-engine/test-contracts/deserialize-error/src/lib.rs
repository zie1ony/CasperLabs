#![no_std]
#![feature(alloc)]

extern crate alloc;
extern crate cl_std;

use alloc::vec::Vec;

use cl_std::bytesrepr::FromBytes;
use cl_std::key::Key;

#[no_mangle]
pub extern "C" fn call() {
    let bytes: [u8; 14] = [255, 255, 255, 255, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    let (_vec, _): (Vec<Key>, _) = FromBytes::from_bytes(&bytes).expect("Should work");
}
