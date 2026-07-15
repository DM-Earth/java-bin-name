//! Java binary name parser and writer.
//!
//! # Parsing
//!
//! Types who implemented `Parse` trait could be parsed from a string slice using [`parse`] function.
//!
//! Differences between JLS and class file representation will be handled automatically.
//!
//! ## Error Handling
//!
//! This crate won't handle naming errors unless necessary.
//! Validate the string slices after parse if desired.
//!
//! # Writing
//!
//! Use `Display` or `ToString` for writing.
//!
//! # Supported Items
//!
//! - Classes or interfaces: [`ClassName`]
//! - Methods: [`MethodDescriptor`]
//! - Field types: [`FieldType`]
//! - Type signatures: [`TypeSignature`]

#![no_std]

extern crate alloc;
extern crate std;

mod class;
mod method;
mod ty;

use core::{convert::Infallible, fmt::Debug};

pub use class::*;
pub use method::*;
pub use ty::*;

/// Types that could be parsed from a borrowed string cursor.
pub trait Parse<'s>: Sized {
    /// The error type.
    type Error;

    /// Parse from the given string cursor.
    #[allow(clippy::missing_errors_doc)]
    fn parse_from(cursor: &mut Cursor<'s>) -> Result<Self, Self::Error>;
}

/// Representation form of a component across different contexts.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReprForm {
    /// JLS-specified standard binary name format.
    JLS,
    /// Internal representation used by class files.
    ///
    /// See [JVMS 4.2.1](https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.2.1)
    /// for more information about internal form of a class, interface or package.
    Internal,
}

impl ReprForm {
    #[inline]
    const fn package_separator(&self) -> char {
        match self {
            Self::JLS => '.',
            Self::Internal => '/',
        }
    }
}

/// Cursor of a string slice.
#[derive(Debug, Clone)]
pub struct Cursor<'a>(&'a str);

impl<'a> Cursor<'a> {
    /// Creates a new string cursor from slice.
    #[inline]
    pub const fn new(s: &'a str) -> Self {
        Self(s)
    }

    /// Reads a single character.
    ///
    /// # Panics
    ///
    /// Panics if there's no valid character left in this slice.
    pub fn get_char(&mut self) -> char {
        let c = self.0.chars().next().expect("end of file");
        self.advance_by(c.len_utf8());
        c
    }

    /// Advances the inner cursor with given function.
    #[inline]
    #[allow(clippy::missing_panics_doc)] // infalliable
    pub fn advance<F, U>(&mut self, f: F) -> U
    where
        F: FnOnce(&'a str) -> (U, &'a str),
    {
        self.try_advance(|s| Result::<_, Infallible>::Ok(f(s)))
            .unwrap()
    }

    /// Advances the inner cursor if successed.
    ///
    /// # Errors
    ///
    /// Returns an error if the given function returns one.
    #[inline]
    pub fn try_advance<F, U, Err>(&mut self, f: F) -> Result<U, Err>
    where
        F: FnOnce(&'a str) -> Result<(U, &'a str), Err>,
    {
        let (ret, leftover) = f(self.0)?;
        self.0 = leftover;
        Ok(ret)
    }

    /// Returns the underlying slice.
    #[inline]
    pub const fn get(&self) -> &'a str {
        self.0
    }

    /// Overrides the underlying slice.
    #[inline]
    pub const fn set(&mut self, s: &'a str) {
        self.0 = s;
    }

    /// Advances the inner cursor by `n` bytes.
    ///
    /// # Panics
    ///
    /// Panics if the inner slice doesn't has `n` remaining bytes.
    #[inline]
    pub fn advance_by(&mut self, n: usize) {
        self.advance(|s| ((), &s[n..]))
    }

    /// Discards this cursor.
    #[inline]
    pub fn clear(&mut self) {
        self.0 = "";
    }
}

fn strip_digits_prefix(s: &str) -> (Option<u32>, &str) {
    let mut chars = s.char_indices();
    let (digits, last_index) = chars
        .by_ref()
        .map_while(|(index, c)| c.to_digit(10).map(|d| (index, d)))
        .fold((0, None), |(n, _), (ci, d)| (n * 10 + d, Some(ci)));
    let leftover = last_index.map_or(s, |i| &s[i + 1..]);
    (last_index.map(|_| digits), leftover)
}

/// Parses a name.
#[allow(clippy::missing_errors_doc)]
#[inline]
pub fn parse<'a, T>(value: &'a str) -> Result<T, T::Error>
where
    T: Parse<'a>,
{
    T::parse_from(&mut Cursor(value))
}

#[cfg(test)]
fn validate_rw<'a, T>(value: &'a str)
where
    T: Parse<'a> + core::fmt::Display,
    T::Error: Debug,
{
    use core::marker::PhantomData;

    use alloc::string::String;

    struct ClearValidator<T>(String, PhantomData<T>);

    impl<'a, T> Parse<'a> for ClearValidator<T>
    where
        T: Parse<'a> + core::fmt::Display,
    {
        type Error = T::Error;

        fn parse_from(cursor: &mut Cursor<'a>) -> Result<Self, Self::Error> {
            use alloc::string::ToString as _;

            let val = T::parse_from(cursor)?;
            assert_eq!(cursor.get(), "", "non-empty string buf left");
            Ok(Self(val.to_string(), PhantomData))
        }
    }

    assert_eq!(
        parse::<'a, ClearValidator<T>>(value).map(|o| o.0).unwrap(),
        value
    );
}
