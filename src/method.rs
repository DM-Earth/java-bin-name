use core::fmt::{Debug, Display};

use smallvec::SmallVec;

use crate::{Cursor, Parse, UnknownTypeTag, ty::FieldType};

/// Error thrown when parsing a method descriptor.
#[derive(Debug, Clone)]
pub enum InvalidMethodDescriptor {
    /// Brackets not exist or not enclosed.
    BrokenBrackets,
    /// Error when parsing field type.
    UnknownFieldTy(UnknownTypeTag),
}

impl core::error::Error for InvalidMethodDescriptor {}

impl From<UnknownTypeTag> for InvalidMethodDescriptor {
    fn from(value: UnknownTypeTag) -> Self {
        Self::UnknownFieldTy(value)
    }
}

impl Display for InvalidMethodDescriptor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BrokenBrackets => write!(f, "broken brackets"),
            Self::UnknownFieldTy(unknown_field_ty) => {
                write!(f, "{unknown_field_ty}")
            }
        }
    }
}

/// Descriptor of a method despite of its signature.
///
/// See [JVMS 4.3.3](https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.3.3).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MethodDescriptor<'a> {
    /// Zero or more parameter descriptors, representing the types of parameters that the method takes.
    pub params: SmallVec<[FieldType<'a>; 4]>,
    /// The return descriptor.
    pub ret: MethodReturnDescriptor<'a>,
}

impl Display for MethodDescriptor<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "(")?;
        for param in &self.params {
            write!(f, "{param}")?;
        }
        write!(f, "){}", self.ret)
    }
}

impl<'a> Parse<'a> for MethodDescriptor<'a> {
    type Error = InvalidMethodDescriptor;

    fn parse_from(cursor: &mut Cursor<'a>) -> Result<Self, Self::Error> {
        if cursor.get_char() != '(' {
            return Err(InvalidMethodDescriptor::BrokenBrackets);
        }
        let mut params_raw = Cursor::new(cursor.try_advance(|s| {
            s.split_once(')')
                .ok_or(InvalidMethodDescriptor::BrokenBrackets)
        })?);
        let mut params = SmallVec::new();
        while !params_raw.get().is_empty() {
            params.push(FieldType::parse_from(&mut params_raw)?);
        }
        Ok(Self {
            params,
            ret: MethodReturnDescriptor::parse_from(cursor)?,
        })
    }
}

/// Method return type.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum MethodReturnDescriptor<'a> {
    /// Represents to `VoidDescriptor` in JVMS.
    Void,
    /// Valid return type.
    Type(FieldType<'a>),
}

impl Display for MethodReturnDescriptor<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MethodReturnDescriptor::Void => write!(f, "V"),
            MethodReturnDescriptor::Type(field_ty) => write!(f, "{field_ty}"),
        }
    }
}

impl<'a> Parse<'a> for MethodReturnDescriptor<'a> {
    type Error = UnknownTypeTag;

    fn parse_from(cursor: &mut Cursor<'a>) -> Result<Self, Self::Error> {
        if cursor.get().chars().next().is_some_and(|c| c == 'V') {
            cursor.advance_by('V'.len_utf8());
            Ok(Self::Void)
        } else {
            FieldType::parse_from(cursor).map(Self::Type)
        }
    }
}

impl Debug for MethodReturnDescriptor<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Void => write!(f, "void"),
            Self::Type(ty) => Debug::fmt(ty, f),
        }
    }
}

impl Debug for MethodDescriptor<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "method(")?;
        let mut it = self.params.iter();
        if let Some(first) = it.next() {
            Debug::fmt(first, f)?;
        }
        for param in it {
            write!(f, ", ")?;
            Debug::fmt(param, f)?;
        }
        write!(f, ") -> ")?;
        Debug::fmt(&self.ret, f)
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use smallvec::SmallVec;

    use crate::{
        FieldType, MethodDescriptor, MethodReturnDescriptor, PrimitiveType, parse, validate_rw,
    };

    #[test]
    fn return_desc_void() {
        assert_eq!(
            parse::<'_, MethodReturnDescriptor<'_>>("V").unwrap(),
            MethodReturnDescriptor::Void
        );
        validate_rw::<'_, MethodReturnDescriptor<'_>>("V");
    }

    #[test]
    fn return_desc_primitive() {
        assert_eq!(
            parse::<'_, MethodReturnDescriptor<'_>>("I").unwrap(),
            MethodReturnDescriptor::Type(FieldType::Primitive(PrimitiveType::Int))
        );
        validate_rw::<'_, MethodReturnDescriptor<'_>>("I");
    }

    #[test]
    fn empty_to_void() {
        assert_eq!(
            parse::<'_, MethodDescriptor<'_>>("()V").unwrap(),
            MethodDescriptor {
                params: SmallVec::new(),
                ret: MethodReturnDescriptor::Void
            }
        );
        validate_rw::<'_, MethodDescriptor<'_>>("()V");
    }

    #[test]
    fn mixed() {
        assert_eq!(
            parse::<'_, MethodDescriptor<'_>>(
                "(I[BLjava/lang/String;Ljava/lang/Object;Z)[Ljava/lang/String;"
            )
            .unwrap(),
            MethodDescriptor {
                params: smallvec::smallvec![
                    FieldType::Primitive(PrimitiveType::Int),
                    FieldType::Array(Box::new(FieldType::Primitive(PrimitiveType::Byte))),
                    FieldType::Class("java/lang/String"),
                    FieldType::Class("java/lang/Object"),
                    FieldType::Primitive(PrimitiveType::Boolean),
                ],
                ret: MethodReturnDescriptor::Type(FieldType::Array(Box::new(FieldType::Class(
                    "java/lang/String"
                ))))
            }
        );
        validate_rw::<'_, MethodDescriptor<'_>>(
            "(I[BLjava/lang/String;Ljava/lang/Object;Z)[Ljava/lang/String;",
        );
    }
}
