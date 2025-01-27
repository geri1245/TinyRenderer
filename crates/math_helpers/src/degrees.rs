use std::ops::{Add, RangeInclusive, Rem, Sub};

/// Normalizes a value to a given interval
/// # Examples
/// ```
/// normalize_to_interval(-PI, 0..2PI) == PI
/// ```
pub fn normalize_to_interval<T>(value: T, range: RangeInclusive<T>) -> T
where
    T: Rem<Output = T> + Sub<Output = T> + Add<Output = T> + PartialOrd + Copy,
{
    let modulod_value = (value - *range.start()) % *range.end() + *range.start();
    if modulod_value < *range.start() {
        modulod_value + (*range.end() - *range.start())
    } else if modulod_value > *range.end() {
        modulod_value - (*range.end() - *range.start())
    } else {
        modulod_value
    }
}
