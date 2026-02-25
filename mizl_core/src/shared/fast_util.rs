pub fn i64_to_str_fast(value: i64) -> String {
    if value == 0 {
        return String::from("0x0");
    }

    const HEX_CHARS: &[u8] = b"0123456789abcdef";
    let mut buffer = [0u8; 18];
    let mut i = 18;
    let mut num = i64::unsigned_abs(value);

    while num != 0 {
        i -= 1;
        buffer[i] = HEX_CHARS[(num & 0xF) as usize];
        num >>= 4;
    }

    buffer[i - 1] = b'x';
    buffer[i - 2] = b'0';
    if value >= 0 {
        // safety: we only use \-x0-f, so there won't be any issues with utf-8
        unsafe { std::str::from_utf8_unchecked(&buffer[i - 2..]).to_string() }
    } else {
        buffer[i - 3] = b'-';
        // safety: ditto
        unsafe { std::str::from_utf8_unchecked(&buffer[i - 3..]).to_string() }
    }
}

pub fn nibble_to_u8_fast(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}
