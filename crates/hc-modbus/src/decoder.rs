use hc_core::model::{ByteOrder, DataType, Value};

pub fn decode_registers(
    registers: &[u16],
    data_type: DataType,
    byte_order: ByteOrder,
    scale: f64,
    offset: f64,
) -> Option<Value> {
    use DataType::*;

    let scaled = |raw: f64| -> f64 { raw * scale + offset };

    match data_type {
        Bool => registers.first().map(|&r| Value::Bool(r != 0)),
        U16 => registers.first().map(|&r| Value::U16(r)),
        I16 => registers.first().map(|&r| Value::I16(r as i16)),
        U32 | I32 | F32 => {
            if registers.len() < 2 {
                return None;
            }
            let bytes = assemble_32(registers[0], registers[1], byte_order);
            let raw = u32::from_be_bytes(bytes);
            match data_type {
                U32 => Some(Value::U32(raw)),
                I32 => Some(Value::I32(raw as i32)),
                F32 => Some(Value::F32(scaled(f32::from_bits(raw) as f64) as f32)),
                _ => unreachable!(),
            }
        }
    }
}

fn assemble_32(hi: u16, lo: u16, order: ByteOrder) -> [u8; 4] {
    let h = hi.to_be_bytes();
    let l = lo.to_be_bytes();
    match order {
        ByteOrder::Abcd => [h[0], h[1], l[0], l[1]],
        ByteOrder::Badc => [h[1], h[0], l[1], l[0]],
        ByteOrder::Cdab => [l[0], l[1], h[0], h[1]],
        ByteOrder::Dcba => [l[1], l[0], h[1], h[0]],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ByteOrder::*;
    use DataType::*;

    fn d(v: u16, t: DataType, o: ByteOrder, s: f64, f: f64) -> Option<Value> {
        decode_registers(&[v], t, o, s, f)
    }
    fn d2(a: u16, b: u16, t: DataType, o: ByteOrder, s: f64, f: f64) -> Option<Value> {
        decode_registers(&[a, b], t, o, s, f)
    }

    #[test]
    fn test_bool() {
        assert_eq!(d(1, Bool, Abcd, 1.0, 0.0), Some(Value::Bool(true)));
        assert_eq!(d(0, Bool, Abcd, 1.0, 0.0), Some(Value::Bool(false)));
    }

    #[test]
    fn test_u16() {
        assert_eq!(d(42, U16, Abcd, 1.0, 0.0), Some(Value::U16(42)));
    }

    #[test]
    fn test_i16() {
        assert_eq!(d(0xFFF6, I16, Abcd, 1.0, 0.0), Some(Value::I16(-10)));
    }

    #[test]
    fn test_u32_byte_orders() {
        assert_eq!(
            d2(0xABCD, 0x1234, U32, Abcd, 1.0, 0.0),
            Some(Value::U32(0xABCD1234))
        );
        assert_eq!(
            d2(0xABCD, 0x1234, U32, Badc, 1.0, 0.0),
            Some(Value::U32(0xCDAB3412))
        );
        assert_eq!(
            d2(0xABCD, 0x1234, U32, Cdab, 1.0, 0.0),
            Some(Value::U32(0x1234ABCD))
        );
        assert_eq!(
            d2(0xABCD, 0x1234, U32, Dcba, 1.0, 0.0),
            Some(Value::U32(0x3412CDAB))
        );
    }

    #[test]
    fn test_f32_scaling() {
        assert_eq!(
            d2(0x41C8, 0x0000, F32, Abcd, 0.1, 0.0),
            Some(Value::F32(2.5))
        );
        assert_eq!(
            d2(0x41C8, 0x0000, F32, Abcd, 0.0, 5.0),
            Some(Value::F32(5.0))
        );
    }

    #[test]
    fn test_insufficient() {
        assert_eq!(decode_registers(&[], U32, Abcd, 1.0, 0.0), None);
        assert_eq!(decode_registers(&[1], F32, Abcd, 1.0, 0.0), None);
    }

    #[test]
    fn test_all_byte_orders_f32() {
        let (h, l) = (0x41C8, 0x0000);
        for order in [Abcd, Badc, Cdab, Dcba] {
            let v = d2(h, l, F32, order, 1.0, 0.0).unwrap();
            assert!(matches!(v, Value::F32(_)));
        }
    }

    #[test]
    fn test_i32_negative() {
        assert_eq!(
            d2(0xFFFF, 0xFFF6, I32, Abcd, 1.0, 0.0),
            Some(Value::I32(-10))
        );
    }

    #[test]
    fn test_u32_scale_offset() {
        assert_eq!(
            d2(0x0000, 0x0064, U32, Abcd, 0.5, 10.0),
            Some(Value::U32(0x64))
        );
    }

    #[test]
    fn test_bool_edge() {
        assert_eq!(d(2, Bool, Abcd, 1.0, 0.0), Some(Value::Bool(true)));
        assert_eq!(d(0, Bool, Abcd, 1.0, 0.0), Some(Value::Bool(false)));
    }
}
