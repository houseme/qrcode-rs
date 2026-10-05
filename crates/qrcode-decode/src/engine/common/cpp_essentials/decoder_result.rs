// Modified derivative of rxing 0.9.3; see engine/ORIGIN.md and source-provenance.json.
use std::marker::PhantomData;

use crate::engine::{Exceptions, common::ECIStringBuilder};

use super::StructuredAppendInfo;

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct DecoderResult<T>
where
    T: Copy + Clone + Default + Eq + PartialEq,
{
    content: ECIStringBuilder,
    ecLevel: String,
    versionNumber: u32, // = 0;
    structuredAppend: StructuredAppendInfo,
    isMirrored: bool, // = false;
    //Error _error;
    //std::shared_ptr<CustomData> _extra;
    error: Option<Exceptions>,
    marker: PhantomData<T>,
}

impl<T> Default for DecoderResult<T>
where
    T: Copy + Clone + Default + Eq + PartialEq,
{
    fn default() -> Self {
        Self {
            content: Default::default(),
            ecLevel: Default::default(),
            versionNumber: 0,
            structuredAppend: Default::default(),
            isMirrored: false,
            error: None,
            marker: PhantomData,
        }
    }
}

impl<T> DecoderResult<T>
where
    T: Copy + Clone + Default + Eq + PartialEq,
{
    pub fn with_eci_string_builder(src: ECIStringBuilder) -> Self {
        DecoderResult::<T> { content: src, ..Default::default() }
    }

    pub fn isValid(&self) -> bool {
        self.content.symbology.code != 0 && self.error.is_none()
        //return includeErrors || (_content.symbology.code != 0 && !_error);
    }

    #[cfg(test)]
    pub fn content(&self) -> &ECIStringBuilder {
        &self.content
    }

    /// Transfers raw content without reallocating the payload.
    pub fn into_content(self) -> ECIStringBuilder {
        self.content
    }
}

impl<T> DecoderResult<T>
where
    T: Copy + Clone + Default + Eq + PartialEq,
{
    pub fn ecLevel(&self) -> &str {
        &self.ecLevel
    }
    pub fn setEcLevel(&mut self, ecLevel: String) {
        self.ecLevel = ecLevel
    }
    pub fn withEcLevel(mut self, ecLevel: String) -> DecoderResult<T> {
        self.setEcLevel(ecLevel);
        self
    }

    pub fn versionNumber(&self) -> u32 {
        self.versionNumber
    }
    pub fn setVersionNumber(&mut self, vn: u32) {
        self.versionNumber = vn
    }
    pub fn withVersionNumber(mut self, vn: u32) -> DecoderResult<T> {
        self.setVersionNumber(vn);
        self
    }

    pub fn structuredAppend(&self) -> &StructuredAppendInfo {
        &self.structuredAppend
    }
    pub fn setStructuredAppend(&mut self, sai: StructuredAppendInfo) {
        self.structuredAppend = sai
    }
    pub fn withStructuredAppend(mut self, sai: StructuredAppendInfo) -> DecoderResult<T> {
        self.setStructuredAppend(sai);
        self
    }

    pub fn setIsMirrored(&mut self, is_mirrored: bool) {
        self.isMirrored = is_mirrored
    }
    pub fn withIsMirrored(mut self, is_mirrored: bool) -> DecoderResult<T> {
        self.setIsMirrored(is_mirrored);
        self
    }

    pub fn error(&self) -> &Option<Exceptions> {
        &self.error
    }
    pub fn setError(&mut self, error: Option<Exceptions>) {
        self.error = error
    }
    pub fn withError(mut self, error: Option<Exceptions>) -> DecoderResult<T> {
        self.setError(error);
        self
    }

    pub fn withIsModel1(mut self, is_model_1: bool) -> DecoderResult<T> {
        if is_model_1 {
            self.content.symbology.modifier = 48
        }
        self
    }

    // pub fn build(self) -> DecoderResult<T> {

    // }
}
