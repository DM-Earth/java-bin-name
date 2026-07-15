use core::fmt::{Debug, Display, Write as _};

use alloc::boxed::Box;
use smallvec::SmallVec;

use crate::{Cursor, Parse};

/// Error type marking the leading character of a type descriptor is invalid.
#[derive(Debug, Clone)]
pub struct UnknownTypeTag(char);

impl core::error::Error for UnknownTypeTag {}

impl Display for UnknownTypeTag {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "unknown field type '{}'", self.0)
    }
}

/// A primitive type of Java language.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
#[allow(missing_docs)]
pub enum PrimitiveType {
    Byte,
    Char,
    Double,
    Float,
    Int,
    Long,
    Short,
    Boolean,
}

impl PrimitiveType {
    /// Key character of this type.
    #[inline]
    pub const fn key(self) -> char {
        match self {
            Self::Byte => 'B',
            Self::Char => 'C',
            Self::Double => 'D',
            Self::Float => 'F',
            Self::Int => 'I',
            Self::Long => 'J',
            Self::Short => 'S',
            Self::Boolean => 'Z',
        }
    }
}

impl Debug for PrimitiveType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Byte => write!(f, "byte"),
            Self::Char => write!(f, "char"),
            Self::Double => write!(f, "double"),
            Self::Float => write!(f, "float"),
            Self::Int => write!(f, "int"),
            Self::Long => write!(f, "long"),
            Self::Short => write!(f, "short"),
            Self::Boolean => write!(f, "boolean"),
        }
    }
}

/// The type of a field, parameter, local variable, or value.
///
/// See [JVMS 4.3.2](https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.3.2).
#[derive(Clone, PartialEq, Eq, Hash)]
#[allow(missing_docs)] // pointless
pub enum FieldType<'a> {
    Primitive(PrimitiveType),
    Class(&'a str),
    Array(Box<Self>),
}

impl Display for FieldType<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Primitive(ty) => f.write_char(ty.key()),
            Self::Class(class_name) => write!(f, "L{class_name};"),
            Self::Array(field_ty) => write!(f, "[{field_ty}"),
        }
    }
}

impl<'a> Parse<'a> for FieldType<'a> {
    type Error = UnknownTypeTag;

    fn parse_from(cursor: &mut Cursor<'a>) -> Result<Self, Self::Error> {
        let leading = cursor.get_char();
        match leading {
            'B' => Ok(Self::Primitive(PrimitiveType::Byte)),
            'C' => Ok(Self::Primitive(PrimitiveType::Char)),
            'D' => Ok(Self::Primitive(PrimitiveType::Double)),
            'F' => Ok(Self::Primitive(PrimitiveType::Float)),
            'I' => Ok(Self::Primitive(PrimitiveType::Int)),
            'J' => Ok(Self::Primitive(PrimitiveType::Long)),
            'S' => Ok(Self::Primitive(PrimitiveType::Short)),
            'Z' => Ok(Self::Primitive(PrimitiveType::Boolean)),
            'L' => cursor
                .try_advance(|s| s.split_once(';').ok_or(UnknownTypeTag('L')))
                .map(Self::Class),
            '[' => Self::parse_from(cursor).map(|t| Self::Array(Box::new(t))),
            _ => Err(UnknownTypeTag(leading)),
        }
    }
}

impl Debug for FieldType<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Primitive(ty) => Debug::fmt(ty, f),
            Self::Class(arg0) => Debug::fmt(arg0, f),
            Self::Array(arg0) => {
                Debug::fmt(arg0, f)?;
                write!(f, "[]")
            }
        }
    }
}

/// Signature of a Java type.
///
/// See [JVMS 4.7.9.1](https://docs.oracle.com/javase/specs/jvms/se26/html/jvms-4.html#jvms-4.7.9.1).
#[derive(Clone, PartialEq, Eq)]
#[allow(missing_docs, variant_size_differences)]
pub enum TypeSignature<'a> {
    Primitive(PrimitiveType),
    Class {
        sig: ClassTypeSignature<'a>,
        suffix: Option<ClassTypeSignature<'a>>,
    },
    Type(&'a str),
    Array(Box<Self>),
}

/// Signature of a class type.
#[derive(Clone, PartialEq, Eq)]
pub struct ClassTypeSignature<'a> {
    /// Simple or full name of the class, depending on its location.
    pub name: &'a str,
    /// Generic arguments.
    ///
    /// `*` is denoted by `None`.
    pub args: SmallVec<[Option<Box<TypeArgument<'a>>>; 1]>,
}

/// Type of an (generic) argument.
#[derive(Clone, PartialEq, Eq)]
pub struct TypeArgument<'a> {
    /// Wildcard variant.
    pub kind: TypeArgumentKind,
    /// Underlying type signature.
    pub signature: TypeSignature<'a>,
}

/// Kind of a type argument that is not full wildcard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[allow(missing_docs)]
pub enum TypeArgumentKind {
    #[default]
    Exact,
    /// `? extends T` as `+`.
    Extends,
    /// `? super T` as `-`.
    Super,
}

impl Display for TypeSignature<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TypeSignature::Primitive(ty) => write!(f, "{}", ty.key()),
            TypeSignature::Class { sig, suffix } => {
                write!(f, "L{}", sig)?;
                if let Some(suffix) = suffix {
                    write!(f, ".{}", suffix)?;
                }
                write!(f, ";")?;
                Ok(())
            }
            TypeSignature::Type(ty) => write!(f, "T{};", ty),
            TypeSignature::Array(ty) => write!(f, "[{}", ty),
        }
    }
}

impl Display for ClassTypeSignature<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.name)?;
        if !self.args.is_empty() {
            write!(f, "<")?;
            for arg in &self.args {
                if let Some(arg) = arg {
                    write!(f, "{}", arg)?;
                } else {
                    write!(f, "*")?;
                }
            }
            write!(f, ">")?;
        }
        Ok(())
    }
}

impl Display for TypeArgument<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.kind {
            TypeArgumentKind::Exact => (),
            TypeArgumentKind::Extends => write!(f, "+")?,
            TypeArgumentKind::Super => write!(f, "-")?,
        }
        write!(f, "{}", self.signature)?;
        Ok(())
    }
}

impl<'a> Parse<'a> for TypeSignature<'a> {
    type Error = UnknownTypeTag;

    fn parse_from(cursor: &mut Cursor<'a>) -> Result<Self, Self::Error> {
        let leading = cursor.get_char();
        match leading {
            'B' => Ok(Self::Primitive(PrimitiveType::Byte)),
            'C' => Ok(Self::Primitive(PrimitiveType::Char)),
            'D' => Ok(Self::Primitive(PrimitiveType::Double)),
            'F' => Ok(Self::Primitive(PrimitiveType::Float)),
            'I' => Ok(Self::Primitive(PrimitiveType::Int)),
            'J' => Ok(Self::Primitive(PrimitiveType::Long)),
            'S' => Ok(Self::Primitive(PrimitiveType::Short)),
            'Z' => Ok(Self::Primitive(PrimitiveType::Boolean)),
            'L' => {
                let mut offset0 = if let Some((a, b)) = cursor.0.split_once('<')
                    && !a.contains(['.', ';'])
                {
                    let mut rem = b;
                    let mut layers = 1usize;
                    while layers > 0 {
                        let pos = rem.find(['<', '>']).expect("unclosed angles");
                        match rem.as_bytes()[pos] {
                            b'<' => layers += 1,
                            b'>' => layers -= 1,
                            _ => unreachable!(),
                        }
                        rem = &rem[pos + 1..];
                    }
                    unsafe { rem.as_ptr().byte_offset_from_unsigned(cursor.0.as_ptr()) }
                } else {
                    0
                };
                offset0 += cursor.0[offset0..]
                    .find([';', '.'])
                    .ok_or(UnknownTypeTag('L'))?;
                let major = cursor.advance(|s| s.split_at(offset0));
                let suffix = match cursor.get_char() {
                    '.' => {
                        let mut offset1 = if let Some((a, b)) = cursor.0.split_once('<')
                            && !a.contains(';')
                        {
                            let mut rem = b;
                            let mut layers = 1usize;
                            while layers > 0 {
                                let pos = rem.find(['<', '>']).expect("unclosed angles");
                                match rem.as_bytes()[pos] {
                                    b'<' => layers += 1,
                                    b'>' => layers -= 1,
                                    _ => unreachable!(),
                                }
                                rem = &rem[pos + 1..];
                            }
                            unsafe { rem.as_ptr().byte_offset_from_unsigned(cursor.0.as_ptr()) }
                        } else {
                            0
                        };
                        offset1 += cursor.0[offset1..].find(';').ok_or(UnknownTypeTag('L'))?;
                        let suffix = cursor.advance(|s| s.split_at(offset1));
                        cursor.get_char();
                        Some(suffix)
                    }
                    ';' => None,
                    _ => unreachable!(),
                };

                Ok(Self::Class {
                    sig: ClassTypeSignature::parse_from(&mut Cursor(major))?,
                    suffix: suffix
                        .map(|src| ClassTypeSignature::parse_from(&mut Cursor(src)))
                        .transpose()?,
                })
            }
            'T' => cursor
                .try_advance(|s| s.split_once(';').ok_or(UnknownTypeTag('T')))
                .map(Self::Type),
            '[' => Self::parse_from(cursor).map(|t| Self::Array(Box::new(t))),
            _ => Err(UnknownTypeTag(leading)),
        }
    }
}

impl<'a> Parse<'a> for ClassTypeSignature<'a> {
    type Error = UnknownTypeTag;

    fn parse_from(input: &mut Cursor<'a>) -> Result<Self, UnknownTypeTag> {
        let src = input.get();
        input.0 = "";
        let (name, rem) = src.split_once('<').unzip();
        let name = name.unwrap_or(src);
        let mut args = SmallVec::new();
        if let Some(rem) = rem {
            let mut cursor = Cursor(rem.strip_suffix('>').unwrap_or(rem));
            while !cursor.0.is_empty() {
                let mut c1 = Cursor(cursor.0);
                if c1.get_char() == '*' {
                    args.push(None);
                    cursor = c1;
                } else {
                    args.push(Some(Box::new(TypeArgument::parse_from(&mut cursor)?)));
                }
            }
        }
        Ok(Self { name, args })
    }
}

impl<'a> Parse<'a> for TypeArgument<'a> {
    type Error = UnknownTypeTag;

    fn parse_from(src: &mut Cursor<'a>) -> Result<Self, UnknownTypeTag> {
        let kind = match src.0.as_bytes().first() {
            Some(b'+') => {
                src.get_char();
                TypeArgumentKind::Extends
            }
            Some(b'_') => {
                src.get_char();
                TypeArgumentKind::Super
            }
            _ => TypeArgumentKind::Exact,
        };
        Ok(Self {
            kind,
            signature: TypeSignature::parse_from(src)?,
        })
    }
}

impl Debug for TypeSignature<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TypeSignature::Primitive(ty) => write!(f, "{ty:?}"),
            TypeSignature::Class { sig, suffix } => {
                write!(f, "{:?}", sig)?;
                if let Some(suffix) = suffix {
                    write!(f, ".{:?}", suffix)?;
                }
                Ok(())
            }
            TypeSignature::Type(ty) => write!(f, "{}", ty),
            TypeSignature::Array(ty) => write!(f, "{:?}[]", ty),
        }
    }
}

impl Debug for ClassTypeSignature<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.name)?;
        if !self.args.is_empty() {
            write!(f, "<")?;
            let mut iter = self.args.iter().peekable();
            while let Some(val) = iter.next() {
                write!(f, "{:?}", val)?;
                if iter.peek().is_some() {
                    write!(f, ", ")?;
                }
            }
            write!(f, ">")?;
        }
        Ok(())
    }
}

impl Debug for TypeArgument<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.kind {
            TypeArgumentKind::Exact => write!(f, "{:?}", self.signature),
            TypeArgumentKind::Extends => write!(f, "? extends {:?}", self.signature),
            TypeArgumentKind::Super => write!(f, "? super {:?}", self.signature),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use smallvec::{SmallVec, smallvec};

    use crate::{
        ClassTypeSignature, FieldType, PrimitiveType, TypeArgument, TypeArgumentKind,
        TypeSignature, parse, validate_rw,
    };

    #[test]
    fn primitives() {
        macro_rules! primitives {
            ($($v:ident,$l:expr),*$(,)?) => {
                $(
                assert_eq!(parse::<'_, FieldType<'_>>($l).unwrap(), FieldType::Primitive(PrimitiveType::$v));
                validate_rw::<'_, FieldType<'_>>($l);

                assert_eq!(parse::<'_, TypeSignature<'_>>($l).unwrap(), TypeSignature::Primitive(PrimitiveType::$v));
                validate_rw::<'_, TypeSignature<'_>>($l);
                )*
            };
        }

        primitives! {
            Byte, "B",
            Char, "C",
            Double, "D",
            Float, "F",
            Int, "I",
            Long, "J",
            Short, "S",
            Boolean, "Z",
        }
    }

    #[test]
    fn class() {
        assert_eq!(
            parse::<'_, FieldType<'_>>("Ljava/lang/Object;").unwrap(),
            FieldType::Class("java/lang/Object")
        );
        validate_rw::<'_, FieldType<'_>>("Ljava/lang/Object;");

        assert_eq!(
            parse::<'_, TypeSignature<'_>>("Ljava/lang/Object;").unwrap(),
            TypeSignature::Class {
                sig: ClassTypeSignature {
                    name: "java/lang/Object",
                    args: SmallVec::new()
                },
                suffix: None
            }
        );
        validate_rw::<'_, TypeSignature<'_>>("Ljava/lang/Object;");
    }

    #[test]
    fn array_primitive() {
        assert_eq!(
            parse::<'_, FieldType<'_>>("[I").unwrap(),
            FieldType::Array(Box::new(FieldType::Primitive(PrimitiveType::Int)))
        );
        validate_rw::<'_, FieldType<'_>>("[I");

        assert_eq!(
            parse::<'_, TypeSignature<'_>>("[I").unwrap(),
            TypeSignature::Array(Box::new(TypeSignature::Primitive(PrimitiveType::Int)))
        );
        validate_rw::<'_, TypeSignature<'_>>("[I");
    }

    #[test]
    fn array_primitive_2d() {
        assert_eq!(
            parse::<'_, FieldType<'_>>("[[I").unwrap(),
            FieldType::Array(Box::new(FieldType::Array(Box::new(FieldType::Primitive(
                PrimitiveType::Int
            )))))
        );
        validate_rw::<'_, FieldType<'_>>("[[I");
        assert_eq!(
            parse::<'_, TypeSignature<'_>>("[[I").unwrap(),
            TypeSignature::Array(Box::new(TypeSignature::Array(Box::new(
                TypeSignature::Primitive(PrimitiveType::Int)
            ))))
        );
        validate_rw::<'_, TypeSignature<'_>>("[[I");
    }

    #[test]
    fn array_class() {
        assert_eq!(
            parse::<'_, FieldType<'_>>("[Ljava/lang/Object;").unwrap(),
            FieldType::Array(Box::new(FieldType::Class("java/lang/Object")))
        );
        validate_rw::<'_, FieldType<'_>>("[Ljava/lang/Object;");
        validate_rw::<'_, TypeSignature<'_>>("[Ljava/lang/Object;");
    }

    #[test]
    fn array_class_2d() {
        assert_eq!(
            parse::<'_, FieldType<'_>>("[[Ljava/lang/Object;").unwrap(),
            FieldType::Array(Box::new(FieldType::Array(Box::new(FieldType::Class(
                "java/lang/Object"
            )))))
        );
        validate_rw::<'_, FieldType<'_>>("[[Ljava/lang/Object;");
        validate_rw::<'_, TypeSignature<'_>>("[[Ljava/lang/Object;");
    }

    #[test]
    fn class_generics() {
        assert_eq!(
            parse::<'_, TypeSignature<'_>>(
                "Ljava/util/Map<Ljava/lang/String;+Ljava/lang/Integer;>;"
            )
            .unwrap(),
            TypeSignature::Class {
                sig: ClassTypeSignature {
                    name: "java/util/Map",
                    args: smallvec![
                        Some(Box::new(TypeArgument {
                            kind: TypeArgumentKind::Exact,
                            signature: TypeSignature::Class {
                                sig: ClassTypeSignature {
                                    name: "java/lang/String",
                                    args: SmallVec::new(),
                                },
                                suffix: None
                            }
                        })),
                        Some(Box::new(TypeArgument {
                            kind: TypeArgumentKind::Extends,
                            signature: TypeSignature::Class {
                                sig: ClassTypeSignature {
                                    name: "java/lang/Integer",
                                    args: SmallVec::new(),
                                },
                                suffix: None
                            }
                        })),
                    ]
                },
                suffix: None
            }
        );
        validate_rw::<'_, TypeSignature<'_>>(
            "Ljava/util/Map<Ljava/lang/String;Ljava/lang/Integer;>;",
        );
    }

    #[test]
    fn class_generics_any() {
        assert_eq!(
            parse::<'_, TypeSignature<'_>>("Ljava/util/concurrent/Future<*>;").unwrap(),
            TypeSignature::Class {
                sig: ClassTypeSignature {
                    name: "java/util/concurrent/Future",
                    args: smallvec![None]
                },
                suffix: None
            }
        );
        validate_rw::<'_, TypeSignature<'_>>("Ljava/util/concurrent/Future<*>;");
    }

    #[test]
    fn class_generics_suffix() {
        assert_eq!(
            parse::<'_, TypeSignature<'_>>("LOuter<TT;>.Inner<TU;>;").unwrap(),
            TypeSignature::Class {
                sig: ClassTypeSignature {
                    name: "Outer",
                    args: smallvec![Some(Box::new(TypeArgument {
                        kind: TypeArgumentKind::Exact,
                        signature: TypeSignature::Type("T"),
                    }))]
                },
                suffix: Some(ClassTypeSignature {
                    name: "Inner",
                    args: smallvec![Some(Box::new(TypeArgument {
                        kind: TypeArgumentKind::Exact,
                        signature: TypeSignature::Type("U"),
                    }))]
                },)
            }
        );
        validate_rw::<'_, TypeSignature<'_>>("LOuter<TT;>.Inner<TU;>;");
    }

    #[test]
    fn type_sig() {
        assert_eq!(
            parse::<'_, TypeSignature<'_>>("TT;").unwrap(),
            TypeSignature::Type("T")
        );
        validate_rw::<'_, TypeSignature<'_>>("TT;");
    }
}
