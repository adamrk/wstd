pub use http::uri::{InvalidUri, Scheme};
#[cfg(target_env = "p2")]
use std::str::FromStr;
#[cfg(target_env = "p2")]
use wasip2::http::types::Scheme as WasiScheme;

#[cfg(target_env = "p2")]
pub(crate) fn to_wasi_scheme(value: &Scheme) -> WasiScheme {
    match value.as_str() {
        "http" => WasiScheme::Http,
        "https" => WasiScheme::Https,
        other => WasiScheme::Other(other.to_owned()),
    }
}

#[cfg(target_env = "p2")]
pub(crate) fn from_wasi_scheme(value: WasiScheme) -> Result<Scheme, InvalidUri> {
    Ok(match value {
        WasiScheme::Http => Scheme::HTTP,
        WasiScheme::Https => Scheme::HTTPS,
        WasiScheme::Other(other) => Scheme::from_str(&other)?,
    })
}
