use fission_core::{Hyperlink, LinkTarget, NavigationCommand, RouteLocation};

/// How a Web application represents logical routes in the browser URL.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WebRouteStrategy {
    /// Accept both `/route` and `#/route`, preserving the representation used
    /// by the initial location for subsequent application navigation.
    #[default]
    Auto,
    /// Store the logical route in `window.location.pathname`.
    Path,
    /// Store the logical route after `#/` in `window.location.hash`.
    Hash,
}

/// Web route parsing and deployment-base configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebNavigationConfig {
    /// URL representation used by application navigation.
    pub strategy: WebRouteStrategy,
    /// Deployment path removed before exposing the logical application route.
    pub base_path: String,
}

impl Default for WebNavigationConfig {
    fn default() -> Self {
        Self {
            strategy: WebRouteStrategy::Auto,
            base_path: "/".into(),
        }
    }
}

impl WebNavigationConfig {
    pub fn new(strategy: WebRouteStrategy) -> Self {
        Self {
            strategy,
            ..Self::default()
        }
    }

    pub fn base_path(mut self, base_path: impl Into<String>) -> Self {
        self.base_path = normalize_base_path(&base_path.into());
        self
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResolvedWebRouteStrategy {
    Path,
    Hash,
}

#[derive(Debug)]
pub(crate) struct NavigationHistory {
    entries: Vec<RouteLocation>,
    index: usize,
}

impl NavigationHistory {
    pub(crate) fn new(initial: RouteLocation) -> Self {
        Self {
            entries: vec![initial],
            index: 0,
        }
    }

    pub(crate) fn apply(&mut self, command: &NavigationCommand) -> Option<RouteLocation> {
        match command {
            NavigationCommand::Push(path) => {
                self.entries.truncate(self.index + 1);
                self.entries.push(RouteLocation::from_route(path));
                self.index += 1;
            }
            NavigationCommand::Replace(path) => {
                self.entries[self.index] = RouteLocation::from_route(path);
            }
            NavigationCommand::Open(Hyperlink {
                href,
                target: LinkTarget::Current,
                ..
            }) if is_internal_route(href) => {
                self.entries.truncate(self.index + 1);
                self.entries.push(RouteLocation::from_route(href));
                self.index += 1;
            }
            NavigationCommand::Back => {
                self.index = self.index.saturating_sub(1);
            }
            NavigationCommand::Forward => {
                self.index = (self.index + 1).min(self.entries.len() - 1);
            }
            NavigationCommand::Go(delta) => {
                let index = (self.index as i64 + i64::from(*delta))
                    .clamp(0, self.entries.len().saturating_sub(1) as i64);
                self.index = index as usize;
            }
            NavigationCommand::Reload | NavigationCommand::Open(_) => return None,
        }
        Some(self.entries[self.index].clone())
    }
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn normalize_route(path: &str) -> String {
    let path = path.trim();
    let path = path.strip_prefix('#').unwrap_or(path);
    let path = path.split('?').next().unwrap_or(path);
    if path.is_empty() {
        "/".into()
    } else if path.starts_with('/') {
        path.into()
    } else {
        format!("/{path}")
    }
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn route_from_href(href: &str) -> String {
    normalize_route(href)
}

pub(crate) fn is_internal_route(href: &str) -> bool {
    href.starts_with('/') || href.starts_with("#/")
}

fn normalize_base_path(path: &str) -> String {
    let trimmed = path.trim().trim_matches('/');
    if trimmed.is_empty() {
        "/".into()
    } else {
        format!("/{trimmed}/")
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn current_browser_location(
    config: &WebNavigationConfig,
) -> Result<(RouteLocation, ResolvedWebRouteStrategy), String> {
    let window = web_sys::window().ok_or_else(|| "browser window is unavailable".to_string())?;
    let location = window.location();
    let browser_pathname = location.pathname().map_err(js_error)?;
    let raw_hash = location.hash().map_err(js_error)?;
    let browser_search = location.search().map_err(js_error)?;
    let hash_route = raw_hash.strip_prefix("#/");
    let resolved = match config.strategy {
        WebRouteStrategy::Path => ResolvedWebRouteStrategy::Path,
        WebRouteStrategy::Hash => ResolvedWebRouteStrategy::Hash,
        WebRouteStrategy::Auto if hash_route.is_some() => ResolvedWebRouteStrategy::Hash,
        WebRouteStrategy::Auto => ResolvedWebRouteStrategy::Path,
    };
    let (pathname, search) = match resolved {
        ResolvedWebRouteStrategy::Path => (
            strip_base_path(&browser_pathname, &config.base_path),
            optional_location_part(browser_search.clone()),
        ),
        ResolvedWebRouteStrategy::Hash => {
            let encoded = hash_route.unwrap_or_default();
            let (path, query) = encoded.split_once('?').unwrap_or((encoded, ""));
            (
                normalize_route(path),
                (!query.is_empty()).then(|| format!("?{query}")),
            )
        }
    };
    Ok((
        RouteLocation {
            pathname,
            host: optional_location_part(location.host().map_err(js_error)?),
            hash: optional_location_part(raw_hash),
            hostname: optional_location_part(location.hostname().map_err(js_error)?),
            href: optional_location_part(location.href().map_err(js_error)?),
            origin: optional_location_part(location.origin().map_err(js_error)?),
            port: optional_location_part(location.port().map_err(js_error)?),
            protocol: optional_location_part(location.protocol().map_err(js_error)?),
            search,
        },
        resolved,
    ))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn apply_browser_navigation(
    command: &NavigationCommand,
    config: &WebNavigationConfig,
    active_strategy: ResolvedWebRouteStrategy,
) -> Result<bool, String> {
    use wasm_bindgen::JsValue;

    let window = web_sys::window().ok_or_else(|| "browser window is unavailable".to_string())?;
    let history = window.history().map_err(js_error)?;
    match command {
        NavigationCommand::Push(path) => history
            .push_state_with_url(
                &JsValue::NULL,
                "",
                Some(&browser_url(path, config, active_strategy)),
            )
            .map_err(js_error)?,
        NavigationCommand::Replace(path) => history
            .replace_state_with_url(
                &JsValue::NULL,
                "",
                Some(&browser_url(path, config, active_strategy)),
            )
            .map_err(js_error)?,
        NavigationCommand::Open(link) if matches!(link.target, LinkTarget::Current) => {
            if is_internal_route(&link.href) {
                history
                    .push_state_with_url(
                        &JsValue::NULL,
                        "",
                        Some(&browser_url(&link.href, config, active_strategy)),
                    )
                    .map_err(js_error)?;
            } else if link.href.starts_with('#') {
                window
                    .location()
                    .set_hash(link.href.trim_start_matches('#'))
                    .map_err(js_error)?;
                return Ok(false);
            } else {
                window.location().set_href(&link.href).map_err(js_error)?;
                return Ok(false);
            }
        }
        NavigationCommand::Open(link) => {
            window
                .open_with_url_and_target(&link.href, link.target.as_html_target())
                .map_err(js_error)?;
            return Ok(false);
        }
        NavigationCommand::Back => {
            history.back().map_err(js_error)?;
            return Ok(false);
        }
        NavigationCommand::Forward => {
            history.forward().map_err(js_error)?;
            return Ok(false);
        }
        NavigationCommand::Go(delta) => {
            history.go_with_delta(*delta).map_err(js_error)?;
            return Ok(false);
        }
        NavigationCommand::Reload => {
            window.location().reload().map_err(js_error)?;
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(any(target_arch = "wasm32", test))]
fn browser_url(
    route: &str,
    config: &WebNavigationConfig,
    strategy: ResolvedWebRouteStrategy,
) -> String {
    let route = if route.starts_with("#/") {
        &route[1..]
    } else {
        route
    };
    let route = if route.starts_with('/') {
        route.to_string()
    } else {
        format!("/{route}")
    };
    match strategy {
        ResolvedWebRouteStrategy::Path => {
            if config.base_path == "/" {
                route
            } else {
                format!("{}{}", config.base_path.trim_end_matches('/'), route)
            }
        }
        ResolvedWebRouteStrategy::Hash => format!("{}#{}", config.base_path, route),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn strip_base_path(path: &str, base_path: &str) -> String {
    if base_path == "/" {
        return normalize_route(path);
    }
    let base_without_slash = base_path.trim_end_matches('/');
    if path == base_without_slash {
        return "/".into();
    }
    path.strip_prefix(base_path)
        .map(normalize_route)
        .unwrap_or_else(|| normalize_route(path))
}

#[cfg(target_arch = "wasm32")]
fn optional_location_part(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

#[cfg(target_arch = "wasm32")]
fn js_error(error: wasm_bindgen::JsValue) -> String {
    error
        .as_string()
        .unwrap_or_else(|| format!("JavaScript navigation error: {error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_history_supports_push_replace_and_traversal() {
        let mut history = NavigationHistory::new(RouteLocation::new("/"));
        assert_eq!(
            history
                .apply(&NavigationCommand::Push("/one".into()))
                .unwrap()
                .pathname,
            "/one"
        );
        history.apply(&NavigationCommand::Push("/two".into()));
        assert_eq!(
            history.apply(&NavigationCommand::Back).unwrap().pathname,
            "/one"
        );
        history.apply(&NavigationCommand::Replace("/changed".into()));
        assert_eq!(
            history.apply(&NavigationCommand::Forward).unwrap().pathname,
            "/two"
        );
        assert_eq!(
            history.apply(&NavigationCommand::Back).unwrap().pathname,
            "/changed"
        );
    }

    #[test]
    fn hash_and_path_hrefs_share_a_logical_route() {
        assert_eq!(route_from_href("/my/path?tab=one"), "/my/path");
        assert_eq!(route_from_href("#/my/path?tab=one"), "/my/path");
    }

    #[test]
    fn browser_urls_preserve_query_and_deployment_strategy() {
        let root = WebNavigationConfig::default();
        assert_eq!(
            browser_url(
                "/projects/42?tab=activity",
                &root,
                ResolvedWebRouteStrategy::Path
            ),
            "/projects/42?tab=activity"
        );
        let nested = WebNavigationConfig::default().base_path("/app");
        assert_eq!(
            browser_url("/projects/42", &nested, ResolvedWebRouteStrategy::Path),
            "/app/projects/42"
        );
        assert_eq!(
            browser_url("/projects/42", &nested, ResolvedWebRouteStrategy::Hash),
            "/app/#/projects/42"
        );
        assert_eq!(strip_base_path("/app/projects/42", "/app/"), "/projects/42");
        assert_eq!(
            strip_base_path("/application/42", "/app/"),
            "/application/42"
        );
    }
}
