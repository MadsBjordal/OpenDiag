//! K-line message framing.
//!
//! * ISO 9141-2 (OBD): request `68 6A F1 <data> <cs>`, answer `48 6B <ecu> <data> <cs>`.
//! * ISO 14230-4 (KWP2000, OBD): `<fmt> <target> <source> [len] <data> <cs>` where
//!   `fmt = 0x80 | length` (0xC0 for functional requests); length 0 means a
//!   separate length byte follows the addresses.
//!
//! The checksum is the 8-bit sum of all preceding bytes.

pub fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |a, b| a.wrapping_add(*b))
}

pub fn iso9141_request(payload: &[u8]) -> Result<Vec<u8>, String> {
    if payload.is_empty() || payload.len() > 7 {
        return Err("ISO 9141-2 requests carry 1–7 data bytes".into());
    }
    let mut f = vec![0x68, 0x6A, 0xF1];
    f.extend_from_slice(payload);
    f.push(checksum(&f));
    Ok(f)
}

/// Functional OBD request (target 0x33, tester 0xF1).
pub fn kwp_request(payload: &[u8]) -> Result<Vec<u8>, String> {
    if payload.is_empty() || payload.len() > 63 {
        return Err("KWP2000 requests carry 1–63 data bytes".into());
    }
    let mut f = vec![0xC0 | payload.len() as u8, 0x33, 0xF1];
    f.extend_from_slice(payload);
    f.push(checksum(&f));
    Ok(f)
}

/// The fast-init StartCommunication request (ISO 14230-4).
pub const KWP_START_COMMUNICATION: [u8; 5] = [0xC1, 0x33, 0xF1, 0x81, 0x66];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub target: u8,
    pub source: u8,
    pub data: Vec<u8>,
}

pub fn split_kwp(bytes: &[u8]) -> Result<Vec<Frame>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let fmt = bytes[i];
        if fmt & 0x80 == 0 {
            return Err(format!("unaddressed KWP frame (format byte {fmt:02X})"));
        }
        let mut hdr = 3;
        let mut len = (fmt & 0x3F) as usize;
        if len == 0 {
            len = *bytes.get(i + 3).ok_or("frame cut off")? as usize;
            hdr = 4;
        }
        let end = i + hdr + len + 1;
        if end > bytes.len() {
            return Err("frame cut off".into());
        }
        if checksum(&bytes[i..end - 1]) != bytes[end - 1] {
            return Err("checksum error".into());
        }
        out.push(Frame {
            target: bytes[i + 1],
            source: bytes[i + 2],
            data: bytes[i + hdr..end - 1].to_vec(),
        });
        i = end;
    }
    Ok(out)
}

/// ISO 9141-2 frames carry no length: a frame ends where the checksum fits
/// and the next frame header (48 6B) or the end of the data follows.
pub fn split_iso9141(bytes: &[u8]) -> Result<Vec<Frame>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes.len() - i < 5 || bytes[i] != 0x48 || bytes[i + 1] != 0x6B {
            return Err(format!("unexpected bytes at position {i}"));
        }
        let mut found = None;
        for j in (i + 5)..=(i + 12).min(bytes.len()) {
            let at_boundary = j == bytes.len()
                || (j + 1 < bytes.len() && bytes[j] == 0x48 && bytes[j + 1] == 0x6B);
            if at_boundary && checksum(&bytes[i..j - 1]) == bytes[j - 1] {
                found = Some(j);
                break;
            }
        }
        let j = found.ok_or("checksum error")?;
        out.push(Frame {
            target: bytes[i + 1],
            source: bytes[i + 2],
            data: bytes[i + 3..j - 1].to_vec(),
        });
        i = j;
    }
    Ok(out)
}

/// Honda motorcycle frames: `<type> <total length> <data…> <cs>`, the checksum
/// making the sum of the whole frame 0 (mod 256). Format from open-source
/// projects (see docs/connectors.md); not verified on hardware by OpenDiag.
pub fn honda_checksum(bytes: &[u8]) -> u8 {
    0u8.wrapping_sub(checksum(bytes))
}

pub fn honda_frame(head: &[u8]) -> Vec<u8> {
    let mut f = head.to_vec();
    f.push(honda_checksum(&f));
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso9141_round_trip() {
        assert_eq!(
            iso9141_request(&[0x01, 0x00]).unwrap(),
            vec![0x68, 0x6A, 0xF1, 0x01, 0x00, 0xC4]
        );
        // Two answers back to back: 48 6B 10 41 00 BE 1F B8 10 <cs> and a second ECU.
        let mut a = vec![0x48, 0x6B, 0x10, 0x41, 0x00, 0xBE, 0x1F, 0xB8, 0x10];
        a.push(checksum(&a));
        let mut b = vec![0x48, 0x6B, 0x18, 0x41, 0x00, 0x80, 0x00, 0x00, 0x01];
        b.push(checksum(&b));
        let both = [a.clone(), b].concat();
        let f = split_iso9141(&both).unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].source, 0x10);
        assert_eq!(f[0].data, vec![0x41, 0x00, 0xBE, 0x1F, 0xB8, 0x10]);
        assert_eq!(f[1].source, 0x18);
        let mut bad = a.clone();
        *bad.last_mut().unwrap() ^= 1;
        assert!(split_iso9141(&bad).is_err());
        assert!(iso9141_request(&[0; 8]).is_err());
    }

    #[test]
    fn kwp_round_trip() {
        assert_eq!(
            checksum(&KWP_START_COMMUNICATION[..4]),
            KWP_START_COMMUNICATION[4]
        );
        assert_eq!(
            kwp_request(&[0x01, 0x0C]).unwrap(),
            vec![0xC2, 0x33, 0xF1, 0x01, 0x0C, 0xF3]
        );
        let mut r = vec![0x84, 0xF1, 0x11, 0x41, 0x0C, 0x1A, 0xF8];
        r.push(checksum(&r));
        let mut long = vec![0x80, 0xF1, 0x11, 0x03, 0x49, 0x02, 0x01];
        long.push(checksum(&long));
        let f = split_kwp(&[r, long].concat()).unwrap();
        assert_eq!(f[0].data, vec![0x41, 0x0C, 0x1A, 0xF8]);
        assert_eq!(f[1].data, vec![0x49, 0x02, 0x01]);
        assert!(split_kwp(&[0x84, 0xF1]).is_err());
        assert!(split_kwp(&[0x04, 0x01]).is_err());
    }

    #[test]
    fn honda_checksums_match_published_frames() {
        assert_eq!(
            honda_frame(&[0x72, 0x05, 0x00, 0xF0]),
            vec![0x72, 0x05, 0x00, 0xF0, 0x99]
        );
        assert_eq!(
            honda_frame(&[0xFE, 0x04, 0xFF]),
            vec![0xFE, 0x04, 0xFF, 0xFF]
        );
        assert_eq!(
            honda_frame(&[0xFE, 0x04, 0x72]),
            vec![0xFE, 0x04, 0x72, 0x8C]
        );
        assert_eq!(
            honda_frame(&[0x02, 0x04, 0x00]),
            vec![0x02, 0x04, 0x00, 0xFA]
        );
    }
}
