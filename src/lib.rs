//! QRCode encoder
//!
//! This crate provides a QR code and Micro QR code encoder for binary data.
//!
#![cfg_attr(feature = "image", doc = "```rust")]


mod bits;
mod render;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
