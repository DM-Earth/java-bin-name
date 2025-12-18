use std::{convert::Infallible, fmt::Display};

use crate::{Cursor, Parse, ReprForm, method::MethodDescriptor, strip_digits_prefix};

/// Binary name of a class or interface.
///
/// See [JLS 13.1](https://docs.oracle.com/javase/specs/jls/se25/html/jls-13.html#jls-13.1).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClassName<'a> {
    /// Binary name of a top level class or interface.
    TopLevel(CanonicalClassName<'a>),
    /// Binary name of a member class or interface, or
    /// a type variable declared by a generic class or interface.
    #[doc(alias = "Generic")]
    Member {
        /// The binary name of its immediately enclosing class or interface.
        parent: Box<Self>,
        /// The simple name of the member or the type variable.
        simple: &'a str,
    },
    /// Binary name of a local class or interface.
    Local {
        /// The binary name of its immediately enclosing class or interface.
        parent: Box<Self>,
        /// The simple name of the local class.
        simple: &'a str,
        /// A non-empty sequence of digits.
        index: u32,
    },
    /// Binary name of an anonymous class.
    Anonymous {
        /// The binary name of its immediately enclosing class or interface.
        parent: Box<Self>,
        /// A non-empty sequence of digits.
        index: u32,
    },
    /// Binary name of a type variable declared by a generic method, or
    /// a constructor.
    #[doc(alias = "ConstructorGeneric")]
    MethodGeneric {
        /// The binary name of the class or interface declaring the method or constructor.
        class: Box<Self>,
        /// The descriptor of the method or constructor.
        method: MethodDescriptor<'a>,
        /// The simple name of the type variable.
        simple: &'a str,
    },
}

impl Display for ClassName<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClassName::TopLevel(canonical_class_name) => write!(f, "{canonical_class_name}"),
            ClassName::Member { parent, simple } => write!(f, "{parent}${simple}"),
            ClassName::Local {
                parent,
                simple,
                index,
            } => write!(f, "{parent}${index}{simple}"),
            ClassName::Anonymous { parent, index } => write!(f, "{parent}${index}"),
            ClassName::MethodGeneric {
                class,
                method,
                simple,
            } => write!(f, "{class}${method}${simple}"),
        }
    }
}

impl<'a> Parse<'a> for ClassName<'a> {
    type Error = Infallible;

    fn parse_from(cursor: &mut Cursor<'a>) -> Result<Self, Self::Error> {
        let s = cursor.get();
        if let Some((parent, simple)) = s.rsplit_once('$') {
            if let Some((parent, method)) = parent.rsplit_once('$')
                && method.chars().next().is_some_and(|c| c == '(')
                && let Ok(method) = MethodDescriptor::parse_from(&mut Cursor::new(method))
            {
                return Ok(Self::MethodGeneric {
                    class: Box::new(Self::parse_from(&mut Cursor::new(parent)).unwrap()),
                    method,
                    simple,
                });
            }

            let parent = Box::new(Self::parse_from(&mut Cursor::new(parent))?);
            let (digits, simple) = strip_digits_prefix(simple);
            cursor.clear();
            Ok(match (digits, simple.is_empty()) {
                // expected non-empty, but have to handle errors that way
                (None, _) => Self::Member { parent, simple },
                (Some(index), true) => Self::Anonymous { parent, index },
                (Some(index), false) => Self::Local {
                    parent,
                    simple,
                    index,
                },
            })
        } else {
            CanonicalClassName::parse_from(cursor).map(Self::TopLevel)
        }
    }
}

/// Canonical, or fully qualified name of a class or interface.
///
/// See [JLS 6.7](https://docs.oracle.com/javase/specs/jls/se25/html/jls-6.html#jls-6.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CanonicalClassName<'a> {
    /// The fully qualified name of the package.
    pub package: Option<&'a str>,
    /// The simple name of the class or interface.
    pub simple: &'a str,
    /// The representation form of this class name.
    pub form: ReprForm,
}

impl Display for CanonicalClassName<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(pkg) = self.package {
            write!(f, "{pkg}{}{}", self.form.package_separator(), self.simple)
        } else {
            write!(f, "{}", self.simple)
        }
    }
}

impl<'a> Parse<'a> for CanonicalClassName<'a> {
    type Error = Infallible;

    fn parse_from(cursor: &mut Cursor<'a>) -> Result<Self, Self::Error> {
        Ok(cursor.advance(|s| {
            let form = if s.contains('/') {
                ReprForm::Internal
            } else {
                ReprForm::JLS
            };
            let (package, c) = s.rsplit_once(form.package_separator()).unzip();
            (
                Self {
                    package,
                    simple: c.unwrap_or(s),
                    form,
                },
                "",
            )
        }))
    }
}

#[cfg(test)]
mod tests {
    use crate::{CanonicalClassName, ClassName, ReprForm, parse, validate_rw};

    #[test]
    fn top_level() {
        assert_eq!(
            parse::<'_, ClassName<'_>>("java.lang.String").unwrap(),
            ClassName::TopLevel(CanonicalClassName {
                package: Some("java.lang"),
                simple: "String",
                form: ReprForm::JLS,
            })
        );

        assert_eq!(
            parse::<'_, ClassName<'_>>("Foo").unwrap(),
            ClassName::TopLevel(CanonicalClassName {
                package: None,
                simple: "Foo",
                form: ReprForm::JLS,
            })
        );

        validate_rw::<'_, ClassName<'_>>("java.lang.String");
    }

    #[test]
    fn top_level_jvm() {
        assert_eq!(
            parse::<'_, ClassName<'_>>("java/lang/String").unwrap(),
            ClassName::TopLevel(CanonicalClassName {
                package: Some("java/lang"),
                simple: "String",
                form: ReprForm::Internal
            })
        );

        validate_rw::<'_, ClassName<'_>>("java/lang/String");
    }

    #[test]
    fn member() {
        assert_eq!(
            parse::<'_, ClassName<'_>>("java.util.Map$Entry").unwrap(),
            ClassName::Member {
                parent: Box::new(ClassName::TopLevel(CanonicalClassName {
                    package: Some("java.util"),
                    simple: "Map",
                    form: ReprForm::JLS
                })),
                simple: "Entry"
            }
        );

        validate_rw::<'_, ClassName<'_>>("java.util.Map$Entry");
    }

    #[test]
    fn local() {
        assert_eq!(
            parse::<'_, ClassName<'_>>("com.example.OuterClass$1LocalClass").unwrap(),
            ClassName::Local {
                parent: Box::new(ClassName::TopLevel(CanonicalClassName {
                    package: Some("com.example"),
                    simple: "OuterClass",
                    form: ReprForm::JLS
                })),
                index: 1,
                simple: "LocalClass"
            }
        );

        validate_rw::<'_, ClassName<'_>>("com.example.OuterClass$1LocalClass");
    }

    #[test]
    fn anonymous() {
        assert_eq!(
            parse::<'_, ClassName<'_>>("com.example.OuterClass$1").unwrap(),
            ClassName::Anonymous {
                parent: Box::new(ClassName::TopLevel(CanonicalClassName {
                    package: Some("com.example"),
                    simple: "OuterClass",
                    form: ReprForm::JLS
                })),
                index: 1,
            }
        );

        validate_rw::<'_, ClassName<'_>>("com.example.OuterClass$1");
    }
}
