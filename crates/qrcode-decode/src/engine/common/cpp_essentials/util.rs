// Modified derivative of rxing 0.9.3; see engine/ORIGIN.md and source-provenance.json.
use crate::engine::Exceptions;
use crate::engine::common::Result;

use super::Direction;

#[inline(always)]
pub fn opposite(dir: Direction) -> Direction {
    if dir == Direction::Left { Direction::Right } else { Direction::Left }
}

#[inline(always)]
pub fn UpdateMinMax<T: Ord + Copy>(min: &mut T, max: &mut T, val: T) {
    *min = std::cmp::min(*min, val);
    *max = std::cmp::max(*max, val);
}

#[inline(always)]
pub fn UpdateMinMaxFloat(min: &mut f64, max: &mut f64, val: f64) {
    *min = f64::min(*min, val);
    *max = f64::max(*max, val);
}

// template<typename T, typename = std::enable_if_t<std::is_integral_v<T>>>
pub fn ToString<T: Into<usize>>(val: T, len: usize) -> Result<String> {
    let mut len = len as isize;
    let val = val.into();
    let mut val = val as isize;

    let mut result = vec!['0'; len as usize];
    len -= 1;
    // std::string result(len--, '0');
    if val < 0 {
        return Err(Exceptions::format_with("Invalid value"));
    }
    while len >= 0 && val != 0 {
        result[len as usize] = char::from(b'0' + (val % 10) as u8);
        // result.replace_range((len as usize)..(len as usize), &char::from(b'0' + (val % 10) as u8).to_string());

        len -= 1;
        val /= 10;
    }
    // for (; len >= 0 && val != 0; --len, val /= 10) {
    // 	result[len] = '0' + val % 10;}
    if val != 0 {
        return Err(Exceptions::format_with("Invalid value"));
    }

    Ok(result.iter().collect())
}

pub fn AppendBit(val: &mut i32, bit: bool) {
    *val <<= 1;

    *val |= i32::from(bit)
}
