use std::fmt::{Debug, Display};

use crate::{Cursor, Parse, class::ClassName};

/// Error type marking the leading character of a field descriptor is invalid.
#[derive(Debug, Clone)]
pub struct UnknownFieldType(char);

impl std::error::Error for UnknownFieldType {}

impl Display for UnknownFieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown field type '{}'", self.0)
    }
}

/// The type of a field, parameter, local variable, or value.
///
/// See [JVMS 4.3.2](https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html#jvms-4.3.2).
#[derive(Clone, PartialEq, Eq, Hash)]
#[allow(missing_docs)] // pointless
pub enum FieldType<'a> {
    Byte,
    Char,
    Double,
    Float,
    Int,
    Long,
    Short,
    Boolean,
    Class(Box<ClassName<'a>>),
    Array(Box<Self>),
}

impl Display for FieldType<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Byte => write!(f, "B"),
            Self::Char => write!(f, "C"),
            Self::Double => write!(f, "D"),
            Self::Float => write!(f, "F"),
            Self::Int => write!(f, "I"),
            Self::Long => write!(f, "J"),
            Self::Short => write!(f, "S"),
            Self::Boolean => write!(f, "Z"),
            Self::Class(class_name) => write!(f, "L{class_name};"),
            Self::Array(field_ty) => write!(f, "[{field_ty}"),
        }
    }
}

impl<'a> Parse<'a> for FieldType<'a> {
    type Error = UnknownFieldType;

    fn parse_from(cursor: &mut Cursor<'a>) -> Result<Self, Self::Error> {
        let leading = cursor.get_char();
        match leading {
            'B' => Ok(Self::Byte),
            'C' => Ok(Self::Char),
            'D' => Ok(Self::Double),
            'F' => Ok(Self::Float),
            'I' => Ok(Self::Int),
            'J' => Ok(Self::Long),
            'S' => Ok(Self::Short),
            'Z' => Ok(Self::Boolean),
            'L' => cursor
                .try_advance(|s| s.split_once(';').ok_or(UnknownFieldType('L')))
                .map(|s| {
                    Self::Class(Box::new(
                        ClassName::parse_from(&mut Cursor::new(s)).unwrap(),
                    ))
                }),
            '[' => Self::parse_from(cursor).map(|t| Self::Array(Box::new(t))),
            _ => Err(UnknownFieldType(leading)),
        }
    }
}

impl Debug for FieldType<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Byte => write!(f, "byte"),
            Self::Char => write!(f, "char"),
            Self::Double => write!(f, "double"),
            Self::Float => write!(f, "float"),
            Self::Int => write!(f, "int"),
            Self::Long => write!(f, "long"),
            Self::Short => write!(f, "short"),
            Self::Boolean => write!(f, "boolean"),
            Self::Class(arg0) => Debug::fmt(arg0, f),
            Self::Array(arg0) => {
                Debug::fmt(arg0, f)?;
                write!(f, "[]")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{CanonicalClassName, ClassName, FieldType, ReprForm, parse, validate_rw};

    #[test]
    fn primitives() {
        macro_rules! primitives {
            ($($v:ident,$l:expr),*$(,)?) => {
                $(
                assert_eq!(parse::<'_, FieldType<'_>>($l).unwrap(), FieldType::$v);
                validate_rw::<'_, FieldType<'_>>($l);
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
            FieldType::Class(Box::new(ClassName::TopLevel(CanonicalClassName {
                package: Some("java/lang"),
                simple: "Object",
                form: ReprForm::Internal,
            })))
        );
        validate_rw::<'_, FieldType<'_>>("Ljava/lang/Object;");
    }

    #[test]
    fn array_primitive() {
        assert_eq!(
            parse::<'_, FieldType<'_>>("[I").unwrap(),
            FieldType::Array(Box::new(FieldType::Int))
        );
        validate_rw::<'_, FieldType<'_>>("[I");
    }

    #[test]
    fn array_primitive_2d() {
        assert_eq!(
            parse::<'_, FieldType<'_>>("[[I").unwrap(),
            FieldType::Array(Box::new(FieldType::Array(Box::new(FieldType::Int))))
        );
        validate_rw::<'_, FieldType<'_>>("[[I");
    }

    #[test]
    fn array_class() {
        assert_eq!(
            parse::<'_, FieldType<'_>>("[Ljava/lang/Object;").unwrap(),
            FieldType::Array(Box::new(FieldType::Class(Box::new(ClassName::TopLevel(
                CanonicalClassName {
                    package: Some("java/lang"),
                    simple: "Object",
                    form: ReprForm::Internal,
                }
            )))))
        );
        validate_rw::<'_, FieldType<'_>>("[Ljava/lang/Object;");
    }

    #[test]
    fn array_class_2d() {
        assert_eq!(
            parse::<'_, FieldType<'_>>("[[Ljava/lang/Object;").unwrap(),
            FieldType::Array(Box::new(FieldType::Array(Box::new(FieldType::Class(
                Box::new(ClassName::TopLevel(CanonicalClassName {
                    package: Some("java/lang"),
                    simple: "Object",
                    form: ReprForm::Internal,
                }))
            )))))
        );
        validate_rw::<'_, FieldType<'_>>("[[Ljava/lang/Object;");
    }
}
