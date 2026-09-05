use core::fmt::{Debug, Display};

use alloc::{boxed::Box, vec::Vec};
use smallvec::SmallVec;

use crate::{Cursor, Parse, TypeParameter, TypeSignature, UnknownTypeTag, ty::FieldType};

/// Error thrown when parsing a method descriptor.
#[derive(Debug, Clone)]
#[non_exhaustive]
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

/// Signature of a method declaration.
#[derive(PartialEq, Eq, Clone)]
pub struct MethodSignature<'a> {
    /// Generic parameters.
    pub params: SmallVec<[TypeParameter<'a>; 1]>,
    /// Method arguments.
    pub args: SmallVec<[TypeSignature<'a>; 2]>,
    /// Return type of the method.
    pub result: MethodReturnSignature<'a>,
    /// Exceptions to be thrown.
    pub throws: Box<[TypeSignature<'a>]>,
}

/// Result section in a method signature.
#[derive(PartialEq, Eq, Clone)]
pub enum MethodReturnSignature<'a> {
    /// No return value.
    Void,
    /// Returns something.
    Type(TypeSignature<'a>),
}

/// Errors encountered while parsing a method signature.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum InvalidMethodSignature {
    /// Unclosed angles.
    UnclosedAngles,
    /// Unclosed brackets.
    UnclosedBrackets,
    /// Unknown type signature tag.
    UnknownTypeTag(UnknownTypeTag),
    /// Expected `ReferenceTypeSignature`.
    ExpectedReference,
    /// Expected left bracket.
    ExpectedBracket,
    /// Expected `ClassTypeSignature` or `TypeVariableSignature`.
    ExpectedClassOrTypeVar,
}

impl<'a> Parse<'a> for MethodSignature<'a> {
    type Error = InvalidMethodSignature;

    fn parse_from(cursor: &mut Cursor<'a>) -> Result<Self, Self::Error> {
        let mut params = SmallVec::new();
        if cursor.0.starts_with('<') {
            cursor.get_char();
            let contents = cursor.try_advance(|s| {
                crate::angle_safe_split(s, &['>']).ok_or(InvalidMethodSignature::UnclosedAngles)
            })?;
            cursor.get_char();
            let mut contents = Cursor(contents);
            crate::parse_type_params(&mut contents, |param| params.push(param))?;
        }

        let mut args = SmallVec::new();
        if cursor.get_char() != '(' {
            return Err(InvalidMethodSignature::ExpectedBracket);
        }
        let contents = cursor.try_advance(|s| {
            // this is safe since there won't be any other right bracket.
            s.split_once(')')
                .ok_or(InvalidMethodSignature::UnclosedBrackets)
        })?;
        let mut contents = Cursor(contents);
        while !contents.0.is_empty() {
            args.push(TypeSignature::parse_from(&mut contents)?);
        }

        let result = MethodReturnSignature::parse_from(cursor)?;

        let mut throws = Vec::new();
        while !cursor.0.is_empty() && cursor.get_char() == '^' {
            let exception = TypeSignature::parse_from(cursor)?;
            if !matches!(
                exception,
                TypeSignature::Class { .. } | TypeSignature::Type(_)
            ) {
                return Err(InvalidMethodSignature::ExpectedClassOrTypeVar);
            }
            throws.push(exception);
        }

        Ok(Self {
            params,
            args,
            result,
            throws: throws.into(),
        })
    }
}

impl Display for MethodSignature<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if !self.params.is_empty() {
            crate::display_type_params(&self.params, f)?;
        }
        write!(f, "(")?;
        for arg in &self.args {
            write!(f, "{arg}")?;
        }
        write!(f, "){}", self.result)?;
        for thrown in &self.throws {
            write!(f, "^{}", thrown)?;
        }
        Ok(())
    }
}

impl<'a> Parse<'a> for MethodReturnSignature<'a> {
    type Error = UnknownTypeTag;

    fn parse_from(cursor: &mut Cursor<'a>) -> Result<Self, Self::Error> {
        if cursor.0.starts_with('V') {
            cursor.advance_by(1);
            Ok(Self::Void)
        } else {
            TypeSignature::parse_from(cursor).map(Self::Type)
        }
    }
}

impl Display for MethodReturnSignature<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MethodReturnSignature::Void => write!(f, "V"),
            MethodReturnSignature::Type(sig) => write!(f, "{sig}"),
        }
    }
}

impl Display for InvalidMethodSignature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnclosedAngles => write!(f, "unclosed angles"),
            Self::UnclosedBrackets => write!(f, "unclosed brackets"),
            Self::UnknownTypeTag(err) => write!(f, "{err}"),
            Self::ExpectedReference => {
                write!(f, "expected type signature to be `ReferenceTypeSignature`")
            }
            Self::ExpectedBracket => write!(f, "expected left bracket"),
            Self::ExpectedClassOrTypeVar => write!(
                f,
                "expected exception signature to be either `ClassTypeSignature` or `TypeVariableSignature`"
            ),
        }
    }
}

impl core::error::Error for InvalidMethodSignature {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::UnknownTypeTag(err) => Some(err),
            _ => None,
        }
    }
}

impl From<UnknownTypeTag> for InvalidMethodSignature {
    fn from(value: UnknownTypeTag) -> Self {
        Self::UnknownTypeTag(value)
    }
}

impl From<crate::ParseTypeParamsError> for InvalidMethodSignature {
    fn from(value: crate::ParseTypeParamsError) -> Self {
        match value {
            crate::ParseTypeParamsError::UnknownTypeTag(err) => Self::UnknownTypeTag(err),
            crate::ParseTypeParamsError::ExpectedReference => Self::ExpectedReference,
        }
    }
}

impl Debug for MethodSignature<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if !self.params.is_empty() {
            write!(f, "<")?;
            let mut it = self.params.iter().peekable();
            while let Some(param) = it.next() {
                write!(f, "{param:?}")?;
                if it.peek().is_some() {
                    write!(f, ", ")?;
                }
            }
            write!(f, ">::")?;
        }
        write!(f, "(")?;
        let mut it = self.args.iter().peekable();
        while let Some(arg) = it.next() {
            write!(f, "{arg:?}")?;
            if it.peek().is_some() {
                write!(f, ", ")?;
            }
        }
        write!(f, ") -> {:?}", self.result)?;
        if !self.throws.is_empty() {
            write!(f, " throws ")?;
            let mut it = self.throws.iter().peekable();
            while let Some(exception) = it.next() {
                write!(f, "{exception:?}")?;
                if it.peek().is_some() {
                    write!(f, ", ")?;
                }
            }
        }
        Ok(())
    }
}

impl Debug for MethodReturnSignature<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MethodReturnSignature::Void => write!(f, "void"),
            MethodReturnSignature::Type(ty) => write!(f, "{ty:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use smallvec::SmallVec;

    use crate::{
        FieldType, MethodDescriptor, MethodReturnDescriptor, MethodReturnSignature,
        MethodSignature, PrimitiveType, TypeParameter, TypeSignature, parse, validate_rw,
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

    #[test]
    fn method_sig() {
        assert_eq!(
            parse::<'_, MethodSignature<'_>>(
                "(Ljava/lang/String;I)V^Ljava/io/IOException;^Ljava/lang/SecurityException;"
            )
            .unwrap(),
            MethodSignature {
                params: SmallVec::new(),
                args: smallvec::smallvec![
                    TypeSignature::Class {
                        sig: crate::ReducedClassTypeSignature {
                            name: "java/lang/String",
                            args: SmallVec::new()
                        },
                        suffix: None
                    },
                    TypeSignature::Primitive(PrimitiveType::Int)
                ],
                result: MethodReturnSignature::Void,
                throws: Box::new([
                    TypeSignature::Class {
                        sig: crate::ReducedClassTypeSignature {
                            name: "java/io/IOException",
                            args: SmallVec::new()
                        },
                        suffix: None
                    },
                    TypeSignature::Class {
                        sig: crate::ReducedClassTypeSignature {
                            name: "java/lang/SecurityException",
                            args: SmallVec::new()
                        },
                        suffix: None
                    },
                ])
            }
        );
        validate_rw::<'_, MethodSignature<'_>>(
            "(Ljava/lang/String;I)V^Ljava/io/IOException;^Ljava/lang/SecurityException;",
        );

        assert_eq!(
            parse::<'_, MethodSignature<'_>>("<T:Ljava/lang/Exception;>(TT;)TT;^TT;").unwrap(),
            MethodSignature {
                params: smallvec::smallvec![TypeParameter {
                    name: "T",
                    bound_class: Some(TypeSignature::Class {
                        sig: crate::ReducedClassTypeSignature {
                            name: "java/lang/Exception",
                            args: SmallVec::new()
                        },
                        suffix: None
                    }),
                    bound_interface: Box::new([])
                }],
                args: smallvec::smallvec![TypeSignature::Type("T")],
                result: MethodReturnSignature::Type(TypeSignature::Type("T")),
                throws: Box::new([TypeSignature::Type("T")])
            }
        );
        validate_rw::<'_, MethodSignature<'_>>("<T:Ljava/lang/Exception;>(TT;)TT;^TT;");
    }
}
