//! Compile route declarations into direct async calls without boxed handler futures.
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Ident, LitStr, Path, Token, Type,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

struct Route {
    method: Ident,
    pattern: LitStr,
    handler: Path,
}
impl Parse for Route {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let method = input.parse()?;
        let pattern = input.parse()?;
        input.parse::<Token![=>]>()?;
        Ok(Self {
            method,
            pattern,
            handler: input.parse()?,
        })
    }
}
struct Routes(Punctuated<Route, Token![,]>);
impl Parse for Routes {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Ok(Self(Punctuated::parse_terminated(input)?))
    }
}

/// `routes! { GET "/" => index, GET "/users/{id:u64}" => user }`.
/// Typed captures are appended to the handler's arguments after `Request`.
#[proc_macro]
pub fn routes(input: TokenStream) -> TokenStream {
    let routes = syn::parse_macro_input!(input as Routes);
    expand(routes)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
fn expand(routes: Routes) -> syn::Result<proc_macro2::TokenStream> {
    let mut routes: Vec<_> = routes.0.into_iter().collect();
    routes.sort_by_key(|r| {
        (
            r.pattern
                .value()
                .split('/')
                .map(|s| s.starts_with('{'))
                .collect::<Vec<_>>(),
            r.method != "HEAD",
        )
    });
    let mut groups = std::collections::HashMap::new();
    let mut seen = std::collections::HashSet::new();
    let mut allowed = Vec::<String>::new();
    // ponytail: direct branches suit small route sets; add a trie only after large-route measurements.
    let mut branches = Vec::new();
    for route in routes {
        let method = route.method.to_string();
        if ![
            "GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "CONNECT", "TRACE",
        ]
        .contains(&method.as_str())
        {
            return Err(syn::Error::new_spanned(
                route.method,
                "unsupported HTTP method",
            ));
        }
        let value = route.pattern.value();
        if !value.starts_with('/') || value.contains(['?', '#']) {
            return Err(syn::Error::new_spanned(
                route.pattern,
                "route must be an absolute path without a query or fragment",
            ));
        }
        let mut names = std::collections::HashSet::new();
        let mut parts = Vec::new();
        let mut args = Vec::new();
        let mut extracts = Vec::new();
        let mut shape = Vec::new();
        for segment in value.split('/') {
            if let Some(capture) = segment.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                let (name, ty) = capture
                    .split_once(':')
                    .map_or((capture, None), |(n, t)| (n, Some(t)));
                syn::parse_str::<Ident>(name).map_err(|_| {
                    syn::Error::new_spanned(&route.pattern, "invalid parameter name")
                })?;
                if !names.insert(name) {
                    return Err(syn::Error::new_spanned(
                        &route.pattern,
                        "duplicate parameter name",
                    ));
                }
                parts.push(format!("{{{name}}}"));
                shape.push("{}");
                if let Some(ty) = ty {
                    let ty: Type = syn::parse_str(ty)?;
                    let arg = format_ident!("__maximus_capture_{}", args.len());
                    extracts.push(quote! { let #arg: #ty = match request.path::<#ty>(#name) { Ok(value) => value, Err(error) => return ::maximus::IntoResponse::into_response(error) }; });
                    args.push(arg);
                }
            } else {
                if segment.contains(['{', '}']) || segment == "." || segment == ".." {
                    return Err(syn::Error::new_spanned(
                        &route.pattern,
                        "invalid route segment",
                    ));
                }
                parts.push(segment.to_owned());
                shape.push(segment);
            }
        }
        if !seen.insert((method.clone(), shape.join("/"))) {
            return Err(syn::Error::new_spanned(&route.pattern, "duplicate route"));
        }
        let next_group = groups.len();
        let group = *groups.entry(shape.join("/")).or_insert(next_group);
        if group == allowed.len() {
            allowed.push(String::new());
        }
        if !allowed[group].is_empty() {
            allowed[group].push_str(", ");
        }
        allowed[group].push_str(if method == "GET" {
            "GET, HEAD"
        } else {
            &method
        });
        let pattern = parts.join("/");
        let handler = route.handler;
        let condition = if names.is_empty() {
            quote!(request.uri().path() == #pattern)
        } else {
            quote!(::maximus::__private::matches(#pattern, request.uri().path()))
        };
        branches.push(quote! {
            if (selected == usize::MAX || selected == #group) && #condition {
                selected = #group;
                if request.method().as_str() == #method || (request.method() == ::maximus::Method::HEAD && #method == "GET") {
                    request.set_route(#pattern);
                    #(#extracts)*
                    return ::maximus::IntoResponse::into_response(#handler(request, #(#args),*).await);
                }
            }
        });
    }
    Ok(quote! {
        |mut request: ::maximus::Request| async move {
            if let Err(error) = ::maximus::__private::validate_path(request.uri().path()) { return ::maximus::IntoResponse::into_response(error); }
            let allowed: &[&str] = &[#(#allowed),*];
            let mut selected = usize::MAX;
            #(#branches)*
            if selected == usize::MAX { ::maximus::IntoResponse::into_response(::maximus::StatusCode::NOT_FOUND) }
            else {
                let mut response = ::maximus::IntoResponse::into_response(::maximus::StatusCode::METHOD_NOT_ALLOWED);
                response.headers_mut().insert("allow", ::maximus::http::HeaderValue::from_static(allowed[selected]));
                response
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_declarations() {
        for input in [
            r#"GET "relative" => handler"#,
            r#"GET "/{x}/{x}" => handler"#,
            r#"GET "/{x}" => a, GET "/{y}" => b"#,
            r#"GOT "/" => handler"#,
            r#"GET "/{bad" => handler"#,
        ] {
            assert!(expand(syn::parse_str(input).unwrap()).is_err(), "{input}");
        }
    }
}
