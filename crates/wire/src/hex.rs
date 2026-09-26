pub fn key_from_hex(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }

    let mut key = [0u8; 32];
    for place in 0..32 {
        let pair = &text[place * 2..place * 2 + 2];
        let byte = u8::from_str_radix(pair, 16);
        if byte.is_err() {
            return None;
        }

        key[place] = byte.unwrap();
    }

    return Some(key);
}

pub fn key_to_hex(key: &[u8; 32]) -> String {
    let mut text = String::new();

    for byte in key.iter() {
        text += &format!("{:02x}", byte);
    }

    return text;
}

pub fn signature_from_hex(text: &str) -> Option<[u8; 64]> {
    if text.len() != 128 {
        return None;
    }

    let mut signature = [0u8; 64];

    for place in 0..64 {
        let byte = u8::from_str_radix(&text[place * 2..place * 2 + 2], 16);
        if byte.is_err() {
            return None;
        }

        signature[place] = byte.unwrap();
    }

    return Some(signature);
}

pub fn signature_to_hex(signature: &[u8; 64]) -> String {
    let mut text = String::new();

    for byte in signature.iter() {
        text += &format!("{:02x}", byte);
    }

    return text;
}
