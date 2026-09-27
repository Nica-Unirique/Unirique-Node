use std::fs;

pub const SHARE_FILE: &str = "data/share.csv";
pub const SHARES_FILE: &str = "data/shares.csv";
pub const SHARE_DEFAULT: Option<u32> = Some(4);

pub fn read_default_share() -> Option<u32> {
    let text = fs::read_to_string(SHARE_FILE);
    if text.is_err() {
        let _ = fs::write(SHARE_FILE, format!("share\n{}\n", share_to_text(SHARE_DEFAULT)));
        return SHARE_DEFAULT;
    }

    let text = text.unwrap();
    let line = text.lines().nth(1);
    if line.is_none() {
        return SHARE_DEFAULT;
    }

    return parse_share(line.unwrap().trim());
}

pub fn parse_share(text: &str) -> Option<u32> {
    if text == "infinite" {
        return None;
    }

    let number = text.parse::<u32>();
    if number.is_err() || number.clone().unwrap() < 1 {
        return SHARE_DEFAULT;
    }

    return Some(number.unwrap());
}

pub fn share_to_text(share: Option<u32>) -> String {
    if share.is_none() {
        return String::from("infinite");
    }

    return share.unwrap().to_string();
}

pub fn version_to_text(version: [u32; 3]) -> String {
    return format!("{}.{}.{}", version[0], version[1], version[2]);
}

/// Un texte sans `,` ni retour a la ligne, pour tenir dans une case du CSV.
/// Chacun de ces caracteres, et `/` lui-meme, devient `/` suivi de son code
/// ASCII : `,` devient `/44`, `/` devient `/47`.
pub fn escape_csv(text: &str) -> String {
    let mut escaped = String::new();

    for letter in text.chars() {
        if letter == ',' || letter == '/' || letter == '\n' || letter == '\r' {
            escaped += &format!("/{}", letter as u32);
        } else {
            escaped.push(letter);
        }
    }

    return escaped;
}

pub fn write_default_share(share: Option<u32>) -> bool {
    return fs::write(SHARE_FILE, format!("share\n{}\n", share_to_text(share))).is_ok();
}
