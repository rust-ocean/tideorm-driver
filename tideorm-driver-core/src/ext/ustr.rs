use std::borrow::Borrow;
use std::fmt::{self, Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

/// A string that is cheap to clone: a `'static` literal, or text shared behind an
/// `Arc`. The backends use it for column and type names, which every row and statement
/// description repeats.
///
/// Both forms compare, hash and print as the text they hold.
#[derive(Clone, Eq)]
pub enum UStr {
    Static(&'static str),
    Shared(Arc<str>),
}

impl UStr {
    /// Shares a copy of `s`.
    pub fn new(s: &str) -> Self {
        UStr::Shared(Arc::from(s))
    }

    /// [`str::strip_prefix`] for a `UStr`; a `'static` string stays borrowed.
    pub fn strip_prefix(this: &Self, prefix: &str) -> Option<Self> {
        match this {
            UStr::Static(s) => s.strip_prefix(prefix).map(Self::Static),
            UStr::Shared(s) => s.strip_prefix(prefix).map(|s| Self::Shared(s.into())),
        }
    }
}

impl Deref for UStr {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        match self {
            UStr::Static(s) => s,
            UStr::Shared(s) => s,
        }
    }
}

// By the text alone: a derived `Hash` would also hash which variant holds it, and the
// two forms of one string must hash alike, as `Borrow<str>` requires.
impl Hash for UStr {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}

impl Borrow<str> for UStr {
    #[inline]
    fn borrow(&self) -> &str {
        self
    }
}

impl PartialEq<UStr> for UStr {
    fn eq(&self, other: &UStr) -> bool {
        (**self).eq(&**other)
    }
}

impl From<&'static str> for UStr {
    #[inline]
    fn from(s: &'static str) -> Self {
        UStr::Static(s)
    }
}

impl<'a> From<&'a UStr> for UStr {
    fn from(value: &'a UStr) -> Self {
        value.clone()
    }
}

impl From<String> for UStr {
    #[inline]
    fn from(s: String) -> Self {
        UStr::Shared(s.into())
    }
}

// Both print the bare text, without quotes.
impl Debug for UStr {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad(self)
    }
}

impl Display for UStr {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad(self)
    }
}
