// A BIOS that answers real-mode calls from a script, for the VBE layer's tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::cell::RefCell;

use desk_hal::{Bios, BiosRegs, HalError};

/// One call as the mock saw it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BiosCall {
    pub vector: u8,
    pub regs: BiosRegs,
    /// The buffer as the caller passed it.
    pub buffer_in: Option<Vec<u8>>,
}

/// What the BIOS does for one call: the registers it returns, and the buffer contents it leaves.
#[derive(Clone, Debug, Default)]
pub struct BiosReply {
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32,
    pub buffer: Option<Vec<u8>>,
}

impl BiosReply {
    /// VBE success: AX = 004Fh.
    pub fn ok() -> BiosReply {
        BiosReply {
            eax: 0x004F,
            ..BiosReply::default()
        }
    }
    pub fn with_buffer(mut self, bytes: Vec<u8>) -> BiosReply {
        self.buffer = Some(bytes);
        self
    }
    pub fn with_ax(mut self, ax: u16) -> BiosReply {
        self.eax = ax.into();
        self
    }
}

#[derive(Default)]
struct State {
    replies: VecDeque<BiosReply>,
    calls: Vec<BiosCall>,
    /// The bytes of the first megabyte that `read_low` can see, as (linear address, bytes) pieces.
    low: Vec<(u32, Vec<u8>)>,
}

/// Replies are consumed in order, one per `int` call; a call with no reply left fails with `Unsupported`.
#[derive(Default)]
pub struct MockBios {
    state: RefCell<State>,
}

impl MockBios {
    pub fn new() -> MockBios {
        MockBios::default()
    }

    pub fn push_reply(&self, reply: BiosReply) {
        self.state.borrow_mut().replies.push_back(reply);
    }

    /// Makes `bytes` visible to `read_low` at the real-mode address `segment:offset`.
    pub fn put_low(&self, segment: u16, offset: u16, bytes: &[u8]) {
        let linear = (u32::from(segment) << 4) + u32::from(offset);
        self.state.borrow_mut().low.push((linear, bytes.to_vec()));
    }

    pub fn calls(&self) -> Vec<BiosCall> {
        self.state.borrow().calls.clone()
    }
}

impl Bios for MockBios {
    fn int(&self, vector: u8, regs: &mut BiosRegs, buffer: Option<&mut [u8]>) -> Result<(), HalError> {
        let mut st = self.state.borrow_mut();
        let buffer_in = buffer.as_deref().map(<[u8]>::to_vec);
        st.calls.push(BiosCall {
            vector,
            regs: *regs,
            buffer_in,
        });
        let reply = st.replies.pop_front().ok_or(HalError::Unsupported)?;
        regs.eax = reply.eax;
        regs.ebx = reply.ebx;
        regs.ecx = reply.ecx;
        regs.edx = reply.edx;
        if let (Some(buf), Some(bytes)) = (buffer, reply.buffer) {
            let n = buf.len().min(bytes.len());
            buf[..n].copy_from_slice(&bytes[..n]);
        }
        Ok(())
    }

    fn read_low(&self, segment: u16, offset: u16, out: &mut [u8]) -> Result<(), HalError> {
        let start = (u32::from(segment) << 4) + u32::from(offset);
        if u64::from(start) + out.len() as u64 > 0x10_0000 {
            return Err(HalError::InvalidArgument);
        }
        let st = self.state.borrow();
        for (i, o) in out.iter_mut().enumerate() {
            let addr = start + i as u32;
            *o = st
                .low
                .iter()
                .rev()
                .find_map(|(base, bytes)| bytes.get(addr.wrapping_sub(*base) as usize).copied())
                .unwrap_or(0);
        }
        Ok(())
    }
}
