//! Public runtime client configuration.
//!
//! `GET /api/v1/client-config` exposes the Connect endpoints the browser
//! client needs, resolved from `WF_CONNECT_*` (see
//! [`crate::config::connect_client_config`]). This is what lets self-hosters
//! repoint the connector/auth host by editing `wealthfolio.env` + restarting,
//! with no rebuild. Only publishable material is served here — never secrets.
//!
//! Unset means "no runtime override": the frontend falls back to its baked-in
//! build values, then to the cloud defaults, exactly like upstream.

use axum::{routing::get, Json, Router};
use serde::Serialize;

use crate::config::connect_client_config;

/// CSP `connect-src` origins derived from the runtime Connect URLs, cached
/// after first use. The static [`super::SERVER_CSP`] allowlists the cloud
/// hosts; a self-hosted auth/API host would otherwise be blocked in the
/// browser. Computed lazily (env is fully loaded before serving starts).
pub fn extra_csp_connect_sources() -> Vec<String> {
    use std::sync::OnceLock;
    static EXTRA: OnceLock<Vec<String>> = OnceLock::new();
    EXTRA
        .get_or_init(|| {
            let cfg = connect_client_config();
            [cfg.api_url, cfg.auth_url, cfg.oauth_callback_url]
                .into_iter()
                .flatten()
                .filter_map(|url| csp_source(&url))
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect()
        })
        .clone()
}

/// Reduce a URL to its CSP origin (`scheme://host[:port]`). Returns `None`
/// for unparseable values, which fail closed (no extra allowance).
fn csp_source(url: &str) -> Option<String> {
    let url = url.trim();
    let (scheme, rest) = url.split_once("://")?;
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let host = rest.split('/').next()?.split('@').next_back()?;
    if host.is_empty() {
        return None;
    }
    Some(format!("{scheme}://{host}"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectClientShape {
    #[serde(skip_serializing_if = "Option::is_none")]
    api_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_publishable_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    oauth_callback_url: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientConfigShape {
    #[serde(skip_serializing_if = "Option::is_none")]
    connect: Option<ConnectClientShape>,
}

/// Serve the runtime client configuration. Public (the login screen needs it
/// pre-auth) and read-only.
async fn get_client_config() -> Json<ClientConfigShape> {
    let cfg = connect_client_config();
    let connect = cfg.has_any().then(|| ConnectClientShape {
        api_url: cfg.api_url,
        auth_url: cfg.auth_url,
        auth_publishable_key: cfg.auth_publishable_key,
        oauth_callback_url: cfg.oauth_callback_url,
    });
    Json(ClientConfigShape { connect })
}

pub fn router<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new().route("/client-config", get(get_client_config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csp_source_keeps_origin_only() {
        assert_eq!(
            csp_source("https://auth.example.com:8443/some/path?q=1"),
            Some("https://auth.example.com:8443".to_string())
        );
        assert_eq!(
            csp_source("http://127.0.0.1:8088"),
            Some("http://127.0.0.1:8088".to_string())
        );
        assert_eq!(csp_source("not a url"), None);
        assert_eq!(csp_source("ftp://x.example"), None);
        assert_eq!(csp_source("https://"), None);
    }
}
