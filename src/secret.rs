use sha2::{Digest, Sha256};

const HEX: &[u8; 16] = b"0123456789abcdef";

fn key() -> [u8; 32] {
    let mid = std::fs::read_to_string("/etc/machine-id").unwrap_or_default();
    let user = std::env::var("USER").unwrap_or_default();
    let mut h = Sha256::new();
    h.update(b"lzt::v2::");
    h.update(mid.trim().as_bytes());
    h.update(b"::");
    h.update(user.as_bytes());
    h.finalize().into()
}

fn xor(data: &mut [u8]) {
    let key = key();
    for (i, chunk) in data.chunks_mut(32).enumerate() {
        let mut h = Sha256::new();
        h.update(key);
        h.update((i as u64).to_le_bytes());
        let block = h.finalize();
        for (b, k) in chunk.iter_mut().zip(block.iter()) {
            *b ^= k;
        }
    }
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    if b.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(b.len() / 2);
    let mut i = 0;
    while i < b.len() {
        out.push((hex_val(b[i])? << 4) | hex_val(b[i + 1])?);
        i += 2;
    }
    Some(out)
}

pub fn seal(plain: &str) -> String {
    let mut buf = plain.as_bytes().to_vec();
    xor(&mut buf);
    to_hex(&buf)
}

pub fn open(stored: &str) -> Option<String> {
    let mut buf = from_hex(stored)?;
    xor(&mut buf);
    String::from_utf8(buf).ok()
}
