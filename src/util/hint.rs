use core::hint::cold_path;

/// Returns the given value, and hints to the compiler that
/// the path leading to this function call is unlikely to
/// be taken
///
/// See also: `likely`, `unlikely`, `core::hint::cold_path`
#[cold]
pub const fn cold<T>(value: T) -> T {
    value
}

/// Hints to the compiler that the given value is likely to
/// be `true`, and the condition branch is likely to be taken
///
/// Same as `core::hint::likely`, however that function is
/// not currently available in stable Rust
#[must_use]
#[inline(always)]
pub const fn likely(value: bool) -> bool {
    if value {
        true
    } else {
        cold_path();
        false
    }
}

/// Hints to the compiler that the given value is likely to
/// be `false`, and the condition branch is unlikely to be taken
///
/// Same as `core::hint::unlikely`, however that function is
/// not currently available in stable Rust
#[must_use]
#[inline(always)]
pub const fn unlikely(value: bool) -> bool {
    if value {
        cold_path();
        true
    } else {
        false
    }
}
