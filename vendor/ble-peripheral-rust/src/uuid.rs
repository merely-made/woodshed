use uuid::Uuid;

pub trait ShortUuid {
    fn from_short(uuid: u16) -> Uuid;

    fn from_string(uuid_str: &str) -> Uuid;

    fn to_short_string(&self) -> String;
}

impl ShortUuid for Uuid {
    fn from_short(uuid: u16) -> Uuid {
        return Uuid::from_fields(uuid.into(), 0, 0x1000, b"\x80\x00\x00\x80\x5F\x9B\x34\xFB");
    }

    fn from_string(uuid_str: &str) -> Uuid {
        let uuid = uuid_str.to_string();
        match Uuid::parse_str(&uuid.clone()) {
            Ok(uuid) => uuid,
            Err(_) => {
                let long_uuid_str = match uuid.len() {
                    4 => format!("0000{}-0000-1000-8000-00805f9b34fb", uuid),
                    8 => format!("{}-0000-1000-8000-00805f9b34fb", uuid),
                    _ => uuid.clone(),
                };
                Uuid::parse_str(&long_uuid_str)
                    .unwrap_or_else(|_| panic!("Invalid UUID string: {}", uuid))
            }
        }
    }

    fn to_short_string(&self) -> String {
        let b = self.as_bytes();
    
        if &b[4..16] == b"\x00\x00\x10\x00\x80\x00\x00\x80\x5F\x9B\x34\xFB" {
            if b[0] == 0 && b[1] == 0 {
                return format!("{:04x}", u16::from_be_bytes([b[2], b[3]]));
            } else {
                return format!("{:08x}", u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
            }
        }

        self.to_string()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_short_string_with_16_bit_uuid() {
        let uuid = Uuid::from_fields(0x1234, 0, 0x1000, b"\x80\x00\x00\x80\x5F\x9B\x34\xFB");
        assert_eq!(uuid.to_short_string(), "1234");
    }

    #[test]
    fn test_to_short_string_with_32_bit_uuid() {
        let uuid = Uuid::from_fields(0x12345678, 0, 0x1000, b"\x80\x00\x00\x80\x5F\x9B\x34\xFB");
        assert_eq!(uuid.to_short_string(), "12345678");
    }

    #[test]
    fn test_from_string_with_16_bit_uuid() {
        let uuid = Uuid::from_string("1234");
        assert_eq!(uuid.to_short_string(), "1234");
    }

    #[test]
    fn test_from_string_with_32_bit_uuid() {
        let uuid = Uuid::from_string("12345678");
        assert_eq!(uuid.to_short_string(), "12345678");
    }

    #[test]
    fn test_lowercase_hex_output() {
        let uuid = Uuid::from_fields(0x180d, 0, 0x1000, b"\x80\x00\x00\x80\x5F\x9B\x34\xFB");
        assert_eq!(uuid.to_short_string(), "180d");
    }

    #[test]
    fn test_full_uuid_string_lowercase() {
        let uuid = Uuid::from_short(0x180d);
        let full = uuid.to_string();
        assert_eq!(full, "0000180d-0000-1000-8000-00805f9b34fb");
    }

    #[test]
    fn test_from_string_with_full_uuid() {
        let uuid = Uuid::from_string("12345678-0000-1000-8000-00805f9b34fb");
        assert_eq!(uuid.to_short_string(), "12345678");
    }
}

