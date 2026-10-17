use crate::{Body, Error, Result};
use std::{
    borrow::Cow,
    ops::{Deref, DerefMut},
    str::FromStr,
};

pub struct Request {
    inner: http::Request<Body>,
    route: Cow<'static, str>,
}
impl Request {
    pub fn new(inner: http::Request<Body>) -> Self {
        Self {
            inner,
            route: Cow::Borrowed(""),
        }
    }
    pub fn set_route(&mut self, pattern: impl Into<Cow<'static, str>>) {
        self.route = pattern.into();
    }
    pub fn into_inner(self) -> http::Request<Body> {
        self.inner
    }
    pub fn into_body(self) -> Body {
        self.inner.into_body()
    }
    pub fn path<T: FromStr>(&self, name: &str) -> Result<T> {
        for (pattern, segment) in self.route.split('/').zip(self.uri().path().split('/')) {
            if pattern.strip_prefix('{').and_then(|s| s.strip_suffix('}')) == Some(name) {
                let value = decode(segment, false)?;
                if value.contains('/') || value.contains('\\') || value.contains('\0') {
                    return Err(Error::bad_request("invalid path segment"));
                }
                return value
                    .parse()
                    .map_err(|_| Error::bad_request("invalid path parameter"));
            }
        }
        Err(Error::bad_request("missing path parameter"))
    }
    pub fn query<T: FromStr>(&self, name: &str) -> Result<T> {
        let mut found = None;
        for pair in self
            .uri()
            .query()
            .unwrap_or("")
            .split('&')
            .filter(|p| !p.is_empty())
        {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            if decode(key, true)? == name {
                if found.is_some() {
                    return Err(Error::bad_request("duplicate query parameter"));
                }
                found = Some(decode(value, true)?);
            }
        }
        found
            .ok_or_else(|| Error::bad_request("missing query parameter"))?
            .parse()
            .map_err(|_| Error::bad_request("invalid query parameter"))
    }
    pub fn header<T: FromStr>(&self, name: &str) -> Result<T> {
        self.header_str(name)?
            .parse()
            .map_err(|_| Error::bad_request("invalid header"))
    }
    pub(crate) fn header_str(&self, name: &str) -> Result<&str> {
        let mut values = self.headers().get_all(name).iter();
        let first = values
            .next()
            .ok_or_else(|| Error::bad_request("missing header"))?;
        if values.next().is_some() {
            return Err(Error::bad_request("duplicate header"));
        }
        first
            .to_str()
            .map_err(|_| Error::bad_request("invalid header"))
    }
}
impl Deref for Request {
    type Target = http::Request<Body>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl DerefMut for Request {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// Percent decoding borrows the input when no decoding is required.
pub fn decode(value: &str, form: bool) -> Result<Cow<'_, str>> {
    if !(value.contains('%') || form && value.contains('+')) {
        return Ok(Cow::Borrowed(value));
    }
    let mut output = Vec::with_capacity(value.len());
    let mut input = value.bytes();
    while let Some(byte) = input.next() {
        match byte {
            b'%' => {
                let a = input.next().and_then(|b| (b as char).to_digit(16));
                let b = input.next().and_then(|b| (b as char).to_digit(16));
                let (Some(a), Some(b)) = (a, b) else {
                    return Err(Error::bad_request("invalid percent encoding"));
                };
                output.push((a * 16 + b) as u8);
            }
            b'+' if form => output.push(b' '),
            byte => output.push(byte),
        }
    }
    String::from_utf8(output)
        .map(Cow::Owned)
        .map_err(|_| Error::bad_request("invalid UTF-8"))
}
