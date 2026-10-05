// Modified derivative of rxing 0.9.3; see engine/ORIGIN.md and source-provenance.json.
use crate::engine::Exceptions;
use crate::engine::common::Result;

#[derive(Default, Clone, PartialEq, Eq)]
pub struct Matrix<T: Default + Clone + Copy> {
    width: usize,
    height: usize,
    data: Vec<Option<T>>,
}

impl<T: Default + Clone + Copy> Matrix<T> {
    pub fn new(width: usize, height: usize) -> Result<Matrix<T>> {
        if width != 0 && (width * height) / width != height {
            return Err(Exceptions::illegal_argument_with("invalid size: width * height is too big"));
        }
        Ok(Self { width, height, data: vec![None; width * height] })
    }

    // value_t& operator()(int x, int y)
    // {
    // 	assert(x >= 0 && x < _width && y >= 0 && y < _height);
    // 	return _data[y * _width + x];
    // }

    // const T& operator()(int x, int y) const
    // {
    // 	assert(x >= 0 && x < _width && y >= 0 && y < _height);
    // 	return _data[y * _width + x];
    // }

    fn get_offset(x: usize, y: usize, width: usize) -> usize {
        y * width + x
    }

    pub fn get(&self, x: usize, y: usize) -> Option<T> {
        if x >= self.width || y >= self.height {
            None
        } else if let Some(Some(d)) = self.data.get(Self::get_offset(x, y, self.width)) {
            Some(*d)
        } else {
            None
        }
    }

    pub fn set(&mut self, x: usize, y: usize, value: T) -> T {
        self.data[Self::get_offset(x, y, self.width)] = Some(value);
        self.get(x, y).unwrap()
    }

    // const value_t* begin() const {
    // 	return _data.data();
    // }

    // const value_t* end() const {
    // 	return _data.data() + _width * _height;
    // }
}
