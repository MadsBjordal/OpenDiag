//! ISO 15765-2 transport ("ISO-TP") for classic CAN (8-byte frames, normal addressing).
//!
//! * [`segment`] splits a diagnostic message into CAN frames (used by the simulator,
//!   and later by SocketCAN/J2534 drivers that send frames themselves).
//! * [`Reassembler`] joins received frames back into one message.
//!
//! Flow control frames are parsed; when an ELM327 is used the adapter sends flow
//! control itself, so the reassembler only has to check order and length.

pub const MAX_LEN: usize = 4095;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IsoTpError {
    #[error("empty CAN frame")]
    Empty,
    #[error("frame too short for its type")]
    TooShort,
    #[error("unknown frame type {0:#X}")]
    UnknownType(u8),
    #[error("invalid single frame length {0}")]
    BadSingleLength(usize),
    #[error("message of {0} bytes is too long for classic ISO-TP")]
    TooLong(usize),
    #[error("consecutive frame without a first frame")]
    UnexpectedConsecutive,
    #[error("consecutive frame out of order: expected {expected}, got {got}")]
    OutOfOrder { expected: u8, got: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Single { data: Vec<u8> },
    First { total_len: usize, data: Vec<u8> },
    Consecutive { seq: u8, data: Vec<u8> },
    FlowControl { status: u8, block_size: u8, st_min: u8 },
}

/// Parses the data bytes of one CAN frame (PCI byte first).
pub fn parse(frame: &[u8]) -> Result<Frame, IsoTpError> {
    let pci = *frame.first().ok_or(IsoTpError::Empty)?;
    match pci >> 4 {
        0x0 => {
            let len = (pci & 0x0F) as usize;
            if len == 0 || len > 7 {
                return Err(IsoTpError::BadSingleLength(len));
            }
            if frame.len() < 1 + len {
                return Err(IsoTpError::TooShort);
            }
            Ok(Frame::Single { data: frame[1..1 + len].to_vec() })
        }
        0x1 => {
            if frame.len() < 2 {
                return Err(IsoTpError::TooShort);
            }
            let total_len = (((pci & 0x0F) as usize) << 8) | frame[1] as usize;
            Ok(Frame::First { total_len, data: frame[2..].to_vec() })
        }
        0x2 => Ok(Frame::Consecutive { seq: pci & 0x0F, data: frame[1..].to_vec() }),
        0x3 => {
            if frame.len() < 3 {
                return Err(IsoTpError::TooShort);
            }
            Ok(Frame::FlowControl { status: pci & 0x0F, block_size: frame[1], st_min: frame[2] })
        }
        other => Err(IsoTpError::UnknownType(other)),
    }
}

/// Splits `payload` into 8-byte CAN frames. `pad` fills unused bytes (None = no padding).
pub fn segment(payload: &[u8], pad: Option<u8>) -> Result<Vec<Vec<u8>>, IsoTpError> {
    let len = payload.len();
    if len > MAX_LEN {
        return Err(IsoTpError::TooLong(len));
    }
    let finish = |mut f: Vec<u8>| {
        if let Some(p) = pad {
            f.resize(8, p);
        }
        f
    };
    if len <= 7 {
        let mut f = vec![len as u8];
        f.extend_from_slice(payload);
        return Ok(vec![finish(f)]);
    }
    let mut frames = Vec::new();
    let mut first = vec![0x10 | ((len >> 8) as u8 & 0x0F), (len & 0xFF) as u8];
    first.extend_from_slice(&payload[..6]);
    frames.push(first);
    let mut seq = 1u8;
    for chunk in payload[6..].chunks(7) {
        let mut f = vec![0x20 | seq];
        f.extend_from_slice(chunk);
        frames.push(finish(f));
        seq = (seq + 1) & 0x0F;
    }
    Ok(frames)
}

/// Joins the frames of one sender into a complete message.
#[derive(Debug, Default)]
pub struct Reassembler {
    expected: Option<usize>,
    buf: Vec<u8>,
    next_seq: u8,
}

impl Reassembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// True while a multi-frame message is half received.
    pub fn in_progress(&self) -> bool {
        self.expected.is_some()
    }

    /// Feeds one frame. Returns the full message once it is complete.
    pub fn push(&mut self, frame: &[u8]) -> Result<Option<Vec<u8>>, IsoTpError> {
        match parse(frame)? {
            Frame::Single { data } => {
                self.reset();
                Ok(Some(data))
            }
            Frame::First { total_len, data } => {
                self.reset();
                if total_len <= 7 {
                    return Err(IsoTpError::BadSingleLength(total_len));
                }
                self.expected = Some(total_len);
                self.buf = data;
                self.next_seq = 1;
                Ok(self.take_if_done())
            }
            Frame::Consecutive { seq, data } => {
                if self.expected.is_none() {
                    return Err(IsoTpError::UnexpectedConsecutive);
                }
                if seq != self.next_seq {
                    let expected = self.next_seq;
                    self.reset();
                    return Err(IsoTpError::OutOfOrder { expected, got: seq });
                }
                self.next_seq = (self.next_seq + 1) & 0x0F;
                self.buf.extend_from_slice(&data);
                Ok(self.take_if_done())
            }
            Frame::FlowControl { .. } => Ok(None),
        }
    }

    fn take_if_done(&mut self) -> Option<Vec<u8>> {
        let total = self.expected?;
        if self.buf.len() >= total {
            let mut out = std::mem::take(&mut self.buf);
            out.truncate(total);
            self.reset();
            Some(out)
        } else {
            None
        }
    }

    fn reset(&mut self) {
        self.expected = None;
        self.buf.clear();
        self.next_seq = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_frame_round_trip() {
        let frames = segment(&[0x41, 0x0C, 0x1A, 0xF8], Some(0)).unwrap();
        assert_eq!(frames, vec![vec![0x04, 0x41, 0x0C, 0x1A, 0xF8, 0, 0, 0]]);
        let mut r = Reassembler::new();
        assert_eq!(r.push(&frames[0]).unwrap(), Some(vec![0x41, 0x0C, 0x1A, 0xF8]));
    }

    #[test]
    fn multi_frame_vin_round_trip() {
        // Mode 09 PID 02 answer: 49 02 01 + 17 VIN characters = 20 bytes.
        let mut payload = vec![0x49, 0x02, 0x01];
        payload.extend_from_slice(b"1HGCM82633A004352");
        let frames = segment(&payload, Some(0x00)).unwrap();
        assert_eq!(frames.len(), 3);
        assert_eq!(&frames[0][..2], &[0x10, 0x14]);
        assert_eq!(frames[1][0], 0x21);
        assert_eq!(frames[2][0], 0x22);
        let mut r = Reassembler::new();
        assert_eq!(r.push(&frames[0]).unwrap(), None);
        assert!(r.in_progress());
        assert_eq!(r.push(&frames[1]).unwrap(), None);
        assert_eq!(r.push(&frames[2]).unwrap(), Some(payload));
    }

    #[test]
    fn sequence_wraps_after_15() {
        let payload: Vec<u8> = (0..200u8).collect();
        let frames = segment(&payload, None).unwrap();
        assert_eq!(frames[16][0], 0x20); // seq 16 wraps to 0
        let mut r = Reassembler::new();
        let mut out = None;
        for f in &frames {
            out = r.push(f).unwrap();
        }
        assert_eq!(out, Some(payload));
    }

    #[test]
    fn out_of_order_is_an_error() {
        let payload: Vec<u8> = (0..20u8).collect();
        let frames = segment(&payload, None).unwrap();
        let mut r = Reassembler::new();
        r.push(&frames[0]).unwrap();
        assert_eq!(
            r.push(&frames[2]),
            Err(IsoTpError::OutOfOrder { expected: 1, got: 2 })
        );
        assert!(!r.in_progress());
    }

    #[test]
    fn malformed_frames() {
        assert_eq!(parse(&[]), Err(IsoTpError::Empty));
        assert_eq!(parse(&[0x00]), Err(IsoTpError::BadSingleLength(0)));
        assert_eq!(parse(&[0x05, 1, 2]), Err(IsoTpError::TooShort));
        assert_eq!(parse(&[0x40]), Err(IsoTpError::UnknownType(4)));
        let mut r = Reassembler::new();
        assert_eq!(r.push(&[0x21, 1]), Err(IsoTpError::UnexpectedConsecutive));
        assert!(segment(&vec![0; 5000], None).is_err());
    }

    #[test]
    fn flow_control_parses() {
        assert_eq!(
            parse(&[0x30, 0x00, 0x0A]).unwrap(),
            Frame::FlowControl { status: 0, block_size: 0, st_min: 10 }
        );
    }
}
