// ---------- Ecriture ----------

pub fn put_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub fn put_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub fn put_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub fn put_text(bytes: &mut Vec<u8>, text: &str) {
    put_u16(bytes, text.len() as u16);
    bytes.extend_from_slice(text.as_bytes());
}

pub fn put_tags(bytes: &mut Vec<u8>, tags: &Vec<u64>) {
    put_u16(bytes, tags.len() as u16);

    for tag in tags.iter() {
        put_u64(bytes, *tag);
    }
}

pub fn put_optional_text(bytes: &mut Vec<u8>, text: &Option<String>) {
    if text.is_none() {
        bytes.push(0);
        return;
    }

    bytes.push(1);
    put_text(bytes, text.as_ref().unwrap());
}

pub fn put_optional_key(bytes: &mut Vec<u8>, key: &Option<[u8; 32]>) {
    if key.is_none() {
        bytes.push(0);
        return;
    }

    bytes.push(1);
    bytes.extend_from_slice(&key.unwrap());
}

// ---------- Lecture ----------

pub fn take_u8(bytes: &[u8], at: &mut usize) -> Option<u8> {
    if *at + 1 > bytes.len() {
        return None;
    }

    let value = bytes[*at];
    *at += 1;

    return Some(value);
}

pub fn take_u16(bytes: &[u8], at: &mut usize) -> Option<u16> {
    if *at + 2 > bytes.len() {
        return None;
    }

    let mut value = [0u8; 2];
    value.copy_from_slice(&bytes[*at..*at + 2]);
    *at += 2;

    return Some(u16::from_le_bytes(value));
}

pub fn take_u32(bytes: &[u8], at: &mut usize) -> Option<u32> {
    if *at + 4 > bytes.len() {
        return None;
    }

    let mut value = [0u8; 4];
    value.copy_from_slice(&bytes[*at..*at + 4]);
    *at += 4;

    return Some(u32::from_le_bytes(value));
}

pub fn take_u64(bytes: &[u8], at: &mut usize) -> Option<u64> {
    if *at + 8 > bytes.len() {
        return None;
    }

    let mut value = [0u8; 8];
    value.copy_from_slice(&bytes[*at..*at + 8]);
    *at += 8;

    return Some(u64::from_le_bytes(value));
}

pub fn take_key(bytes: &[u8], at: &mut usize) -> Option<[u8; 32]> {
    if *at + 32 > bytes.len() {
        return None;
    }

    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes[*at..*at + 32]);
    *at += 32;

    return Some(key);
}

pub fn take_signature(bytes: &[u8], at: &mut usize) -> Option<[u8; 64]> {
    if *at + 64 > bytes.len() {
        return None;
    }

    let mut signature = [0u8; 64];
    signature.copy_from_slice(&bytes[*at..*at + 64]);
    *at += 64;

    return Some(signature);
}

pub fn take_text(bytes: &[u8], at: &mut usize) -> Option<String> {
    let length = take_u16(bytes, at);
    if length.is_none() {
        return None;
    }

    let length = length.unwrap() as usize;
    if *at + length > bytes.len() {
        return None;
    }

    let text = String::from_utf8(bytes[*at..*at + length].to_vec());
    if text.is_err() {
        return None;
    }

    *at += length;

    return Some(text.unwrap());
}

pub fn take_version(bytes: &[u8], at: &mut usize) -> Option<[u32; 3]> {
    let major = take_u32(bytes, at);
    let minor = take_u32(bytes, at);
    let patch = take_u32(bytes, at);

    if major.is_none() || minor.is_none() || patch.is_none() {
        return None;
    }

    return Some([major.unwrap(), minor.unwrap(), patch.unwrap()]);
}

pub fn take_tags(bytes: &[u8], at: &mut usize) -> Option<Vec<u64>> {
    let count = take_u16(bytes, at);
    if count.is_none() {
        return None;
    }

    let mut tags = Vec::new();

    for _ in 0..count.unwrap() {
        let tag = take_u64(bytes, at);
        if tag.is_none() {
            return None;
        }

        tags.push(tag.unwrap());
    }

    return Some(tags);
}

pub fn take_optional_text(bytes: &[u8], at: &mut usize) -> Option<Option<String>> {
    let present = take_u8(bytes, at);
    if present.is_none() {
        return None;
    }

    if present.unwrap() == 0 {
        return Some(None);
    }

    let text = take_text(bytes, at);
    if text.is_none() {
        return None;
    }

    return Some(Some(text.unwrap()));
}

pub fn take_optional_key(bytes: &[u8], at: &mut usize) -> Option<Option<[u8; 32]>> {
    let present = take_u8(bytes, at);
    if present.is_none() {
        return None;
    }

    if present.unwrap() == 0 {
        return Some(None);
    }

    let key = take_key(bytes, at);
    if key.is_none() {
        return None;
    }

    return Some(Some(key.unwrap()));
}

pub fn take_optional_version(bytes: &[u8], at: &mut usize) -> Option<Option<[u32; 3]>> {
    let present = take_u8(bytes, at);
    if present.is_none() {
        return None;
    }

    if present.unwrap() == 0 {
        return Some(None);
    }

    let version = take_version(bytes, at);
    if version.is_none() {
        return None;
    }

    return Some(Some(version.unwrap()));
}
