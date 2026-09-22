pub use http::header::{HeaderMap, HeaderName, HeaderValue};

#[cfg(target_env = "p2")]
use super::{Error, error::Context};
#[cfg(target_env = "p2")]
use wasip2::http::types::Fields;

#[cfg(target_env = "p2")]
pub(crate) fn header_map_from_wasi(wasi_fields: Fields) -> Result<HeaderMap, Error> {
    let mut output = HeaderMap::new();
    #[cfg(target_env = "p2")]
    let entries = wasi_fields.entries();
    #[cfg(target_env = "p3")]
    let entries = wasi_fields.copy_all();

    for (key, value) in entries {
        let key =
            HeaderName::from_bytes(key.as_bytes()).with_context(|| format!("header name {key}"))?;
        let value =
            HeaderValue::from_bytes(&value).with_context(|| format!("header value for {key}"))?;
        output.append(key, value);
    }
    Ok(output)
}

#[cfg(target_env = "p2")]
pub(crate) fn header_map_to_wasi(header_map: &HeaderMap) -> Result<Fields, Error> {
    let wasi_fields = Fields::new();
    for (key, value) in header_map {
        // Unwrap because `HeaderMap` has already validated the headers.
        wasi_fields
            .append(key.as_str(), value.as_bytes())
            .with_context(|| format!("wasi rejected header `{key}: {value:?}`"))?
    }
    Ok(wasi_fields)
}
