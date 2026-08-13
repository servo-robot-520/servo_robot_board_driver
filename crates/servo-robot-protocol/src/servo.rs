//! # Authors
//! greenhand520
//! # Since
//! version:
//! # Date
//! 2026/7/8 11:52

//! Commands to operate servos are only packaged with commands, without any other processing

use crate::error::FrameError;
use crate::frame::{FromPayload, ToPayload};
use alloc::vec::Vec;

/// Servo command wrapper - transparently forwards raw bytes
///
/// This struct wraps raw servo command bytes without any processing.
/// The protocol layer adds frame header (TYPE, LEN) and CRC verification.
/// It may contain multiple servo operation commands
#[derive(Debug, Clone)]
pub struct ServoCmdWrapper {
    /// Raw servo command bytes
    data: Vec<u8>,
}

impl ServoCmdWrapper {
    /// Create a new servo command from raw bytes
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Get the raw command bytes
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Consume self and return the raw bytes
    pub fn into_data(self) -> Vec<u8> {
        self.data
    }
}

impl ToPayload for ServoCmdWrapper {
    fn to_payload(&self) -> Vec<u8> {
        self.data.clone()
    }
}

impl FromPayload for ServoCmdWrapper {
    fn from_payload(payload: &[u8]) -> Result<Self, FrameError> {
        Ok(Self {
            data: payload.to_vec(),
        })
    }
}