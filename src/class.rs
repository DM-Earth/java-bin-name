use core::{
    convert::Infallible,
    fmt::{Debug, Display},
};

use alloc::{boxed::Box, vec::Vec};
use smallvec::SmallVec;

use crate::{
    Cursor, Parse, ReprForm, TypeSignature, method::MethodDescriptor, strip_digits_prefix,
};

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
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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

/// Signature of a class or interface declaration.
#[derive(PartialEq, Eq, Clone)]
pub struct ClassSignature<'a> {
    /// Generic parameters.
    pub params: SmallVec<[TypeParameter<'a>; 1]>,
    /// Super class.
    pub extends: TypeSignature<'a>,
    /// Super interfaces.
    pub impls: Box<[TypeSignature<'a>]>,
}

/// Generic parameter of a class signature.
#[derive(PartialEq, Eq, Clone)]
pub struct TypeParameter<'a> {
    /// Name of this generic parameter.
    pub name: &'a str,
    /// Class bounds.
    pub bound_class: Option<TypeSignature<'a>>,
    /// Interface bounds.
    pub bound_interface: Box<[TypeSignature<'a>]>,
}

/// Errors encountered while parsing a class signature.
#[derive(Debug, Clone)]
pub enum InvalidClassSignature {
    /// Unclosed angles.
    UnclosedAngles,
    /// Unknown type signature tag.
    UnknownTypeTag(crate::UnknownTypeTag),
    /// Expected `ReferenceTypeSignature`.
    ExpectedReference,
    /// Expected `ClassTypeSignature`.
    ExpectedClass,
}

impl<'a> Parse<'a> for ClassSignature<'a> {
    type Error = InvalidClassSignature;

    fn parse_from(cursor: &mut Cursor<'a>) -> Result<Self, Self::Error> {
        let mut params = SmallVec::new();
        if cursor.0.starts_with('<') {
            cursor.get_char();
            let contents = cursor.try_advance(|s| {
                crate::angle_safe_split(s, &['>']).ok_or(InvalidClassSignature::UnclosedAngles)
            })?;
            cursor.get_char();
            let mut contents = Cursor(contents);

            struct UnbakedParam<'a> {
                name: &'a str,
                bounds: SmallVec<[TypeSignature<'a>; 1]>,
            }

            impl<'a> From<UnbakedParam<'a>> for TypeParameter<'a> {
                fn from(value: UnbakedParam<'a>) -> Self {
                    let mut it = value.bounds.into_iter();
                    TypeParameter {
                        name: value.name,
                        bound_class: it.next(),
                        bound_interface: it.collect(),
                    }
                }
            }

            let mut unbaked: Option<UnbakedParam<'a>> = None;
            while let Some(ident) = contents
                .try_advance(|s| crate::angle_safe_rsplit(s, &[':', ';']).ok_or(()))
                .ok()
                .or_else(|| {
                    Some(contents.0)
                        .filter(|s| !s.is_empty())
                        .inspect(|_| contents.clear())
                })
            {
                let ident = ident.strip_suffix(':').unwrap_or(ident);
                if let Some(current) = unbaked.as_mut()
                    && ident.ends_with(';')
                {
                    let bound = crate::parse(ident)?;
                    if !matches!(
                        bound,
                        TypeSignature::Class { .. }
                            | TypeSignature::Type(_)
                            | TypeSignature::Array(_)
                    ) {
                        return Err(InvalidClassSignature::ExpectedReference);
                    }
                    current.bounds.push(bound);
                } else {
                    if let Some(prev) = unbaked.replace(UnbakedParam {
                        name: ident,
                        bounds: SmallVec::new(),
                    }) {
                        params.push(prev.into());
                    }
                }
            }
            if let Some(last) = unbaked {
                params.push(last.into());
            }
        }

        Ok(Self {
            params,
            extends: {
                let sig = TypeSignature::parse_from(cursor)?;
                if !matches!(sig, TypeSignature::Class { .. }) {
                    return Err(InvalidClassSignature::ExpectedClass);
                }
                sig
            },
            impls: {
                let mut buf = Vec::new();
                while !cursor.0.is_empty() {
                    let sig = TypeSignature::parse_from(cursor)?;
                    if !matches!(sig, TypeSignature::Class { .. }) {
                        return Err(InvalidClassSignature::ExpectedClass);
                    }
                    buf.push(sig);
                }
                buf.into()
            },
        })
    }
}

impl Display for ClassSignature<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if !self.params.is_empty() {
            write!(f, "<")?;
            for param in &self.params {
                write!(f, "{}:", param.name)?;
                if let Some(bound) = &param.bound_class {
                    write!(f, "{bound}")?;
                    for bound in &param.bound_interface {
                        write!(f, ":{bound}")?;
                    }
                }
            }
            write!(f, ">")?;
        }
        write!(f, "{}", self.extends)?;
        for sig in &self.impls {
            write!(f, "{}", sig)?;
        }
        Ok(())
    }
}

impl Display for InvalidClassSignature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnclosedAngles => write!(f, "unclosed angles"),
            Self::UnknownTypeTag(err) => write!(f, "{err}"),
            Self::ExpectedReference => {
                write!(f, "expected type signature to be `ReferenceTypeSignature`")
            }
            Self::ExpectedClass => write!(f, "expected type signature to be `ClassTypeSignature`"),
        }
    }
}

impl core::error::Error for InvalidClassSignature {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::UnknownTypeTag(err) => Some(err),
            _ => None,
        }
    }
}

impl From<crate::UnknownTypeTag> for InvalidClassSignature {
    fn from(value: crate::UnknownTypeTag) -> Self {
        Self::UnknownTypeTag(value)
    }
}

impl Debug for ClassSignature<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if !self.params.is_empty() {
            let mut iter = self.params.iter().peekable();
            write!(f, "<")?;
            while let Some(param) = iter.next() {
                write!(f, "{param:?}")?;
                if iter.peek().is_some() {
                    write!(f, ", ")?;
                }
            }
            write!(f, ">")?;
        }
        write!(f, " extends {:?}", self.extends)?;
        if !self.impls.is_empty() {
            let mut iter = self.impls.iter().peekable();
            write!(f, " implements ")?;
            while let Some(mom) = iter.next() {
                write!(f, "{mom:?}")?;
                if iter.peek().is_some() {
                    write!(f, ", ")?;
                }
            }
        }
        Ok(())
    }
}

impl Debug for TypeParameter<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.name)?;
        let mut iter = self
            .bound_class
            .iter()
            .chain(&self.bound_interface)
            .peekable();
        if iter.peek().is_some() {
            write!(f, ": ")?;
        }
        while let Some(bound) = iter.next() {
            write!(f, "{bound:?}")?;
            if iter.peek().is_some() {
                write!(f, " + ")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use smallvec::SmallVec;

    use crate::{
        CanonicalClassName, ClassName, ClassSignature, ReducedClassTypeSignature, ReprForm,
        TypeParameter, TypeSignature, parse, validate_rw,
    };

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

    #[test]
    fn class_sig() {
        assert_eq!(
            parse::<'_, ClassSignature<'_>>(
                "Ljava/lang/Object;Ljava/io/Serializable;Ljava/lang/Cloneable;"
            )
            .unwrap(),
            ClassSignature {
                params: SmallVec::new(),
                extends: TypeSignature::Class {
                    sig: ReducedClassTypeSignature {
                        name: "java/lang/Object",
                        args: SmallVec::new()
                    },
                    suffix: None
                },
                impls: Box::new([
                    TypeSignature::Class {
                        sig: ReducedClassTypeSignature {
                            name: "java/io/Serializable",
                            args: SmallVec::new()
                        },
                        suffix: None
                    },
                    TypeSignature::Class {
                        sig: ReducedClassTypeSignature {
                            name: "java/lang/Cloneable",
                            args: SmallVec::new()
                        },
                        suffix: None
                    },
                ])
            }
        );
        validate_rw::<'_, ClassSignature<'_>>(
            "Ljava/lang/Object;Ljava/io/Serializable;Ljava/lang/Cloneable;",
        );

        assert_eq!(
            parse::<'_, ClassSignature<'_>>("<T:Ljava/lang/Object;K:V:>Ljava/lang/Object;")
                .unwrap(),
            ClassSignature {
                params: smallvec::smallvec![
                    TypeParameter {
                        name: "T",
                        bound_class: Some(TypeSignature::Class {
                            sig: ReducedClassTypeSignature {
                                name: "java/lang/Object",
                                args: SmallVec::new()
                            },
                            suffix: None
                        }),
                        bound_interface: Box::new([])
                    },
                    TypeParameter {
                        name: "K",
                        bound_class: None,
                        bound_interface: Box::new([])
                    },
                    TypeParameter {
                        name: "V",
                        bound_class: None,
                        bound_interface: Box::new([])
                    }
                ],
                extends: TypeSignature::Class {
                    sig: ReducedClassTypeSignature {
                        name: "java/lang/Object",
                        args: SmallVec::new()
                    },
                    suffix: None
                },
                impls: Box::new([])
            }
        );
        validate_rw::<'_, ClassSignature<'_>>("<T:Ljava/lang/Object;K:V:>Ljava/lang/Object;");
    }
}
