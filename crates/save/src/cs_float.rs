//! Floats as the game's saves write them: Mono's `float.ToString(InvariantCulture)`,
//! the general format with 7 significant digits (`0.4444444`, `0.925`, `1E-06`).

/// Save-file float text.
pub trait CsFloat {
    fn cs_string(self) -> String;
}

impl CsFloat for f32 {
    fn cs_string(self) -> String {
        if self == 0.0 {
            return "0".to_owned();
        }
        if self.is_nan() {
            return "NaN".to_owned();
        }
        if self.is_infinite() {
            return if self > 0.0 { "Infinity" } else { "-Infinity" }.to_owned();
        }
        // Seven significant digits, as `d.dddddde±x`.
        let sci = format!("{:.6e}", f64::from(self));
        let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
        let exp: i32 = exp.parse().unwrap_or(0);
        let sign = if mantissa.starts_with('-') { "-" } else { "" };
        let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
        let digits = digits.trim_end_matches('0');
        let digits = if digits.is_empty() { "0" } else { digits };
        if (-5..7).contains(&exp) {
            let body = if exp >= 0 {
                let int_len = usize::try_from(exp + 1).unwrap_or(1);
                if digits.len() <= int_len {
                    format!("{digits}{}", "0".repeat(int_len - digits.len()))
                } else {
                    format!("{}.{}", &digits[..int_len], &digits[int_len..])
                }
            } else {
                let zeros = usize::try_from(-exp - 1).unwrap_or(0);
                format!("0.{}{digits}", "0".repeat(zeros))
            };
            format!("{sign}{body}")
        } else {
            let frac = if digits.len() > 1 {
                format!(".{}", &digits[1..])
            } else {
                String::new()
            };
            let esign = if exp < 0 { '-' } else { '+' };
            format!("{sign}{}{frac}E{esign}{:02}", &digits[..1], exp.abs())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_format_with_seven_digits() {
        assert_eq!(0.0_f32.cs_string(), "0");
        assert_eq!(1.0_f32.cs_string(), "1");
        assert_eq!((4.0_f32 / 9.0).cs_string(), "0.4444444");
        assert_eq!((0.8_f32 + 0.025 * 5.0).cs_string(), "0.925");
        assert_eq!((-3.0_f32 / 100.0).cs_string(), "-0.03");
        assert_eq!(0.000_001_f32.cs_string(), "1E-06");
        assert_eq!(1234.5_f32.cs_string(), "1234.5");
        assert_eq!(12_345_678.0_f32.cs_string(), "1.234568E+07");
    }
}
