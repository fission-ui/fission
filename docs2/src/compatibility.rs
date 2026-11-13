//! Incoming-link compatibility belongs to this website, not the Fission runtime.
//! The captured route list is a migration contract, not a generated site index.

use crate::blog::BlogPost;
use crate::navigation;
use crate::state::DocsState;
use anyhow::{bail, Result};
use fission::prelude::*;
use fission::site::{list_site_routes, FissionSite, SiteBuildOptions, SitePageElement};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub fn legacy_routes() -> impl Iterator<Item = &'static str> {
    include_str!("../data/legacy-routes.txt")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
}

fission_component! {
    RedirectPage<DocsState> {
        props { destination: String, title: String }
    }
    view(page, _state, _cx) =>
        ProtectedRoute::new(
            RouteRedirect::replace(page.destination.clone()).into(),
            Spacer::default(),
        ).pending(Link::to(page.title.clone(), page.destination.clone()))
}

/// Register aliases through the existing route-outcome API. Static output gets
/// Fission's normal redirect metadata and a working link without JavaScript.
/// Refuse the cutover if any captured incoming URL has been forgotten.
pub fn preserve_legacy_routes(
    mut site: FissionSite,
    project: &Path,
    posts: &[BlogPost],
) -> Result<FissionSite> {
    let options = SiteBuildOptions::from_project_dir(project, "Fission")?;
    let mut available = list_site_routes(&options, &site)?
        .into_iter()
        .map(|route| route.path)
        .collect::<BTreeSet<_>>();
    let mut redirects = posts
        .iter()
        .map(|post| {
            (
                post.legacy_path.clone(),
                (post.path.clone(), post.title.clone()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    redirects.insert(
        "/example-showcase/".into(),
        (
            navigation::TRY_IN_BROWSER.into(),
            "Try Fission in your browser".into(),
        ),
    );
    for old in legacy_routes() {
        if available.contains(old) {
            continue;
        }
        let Some((destination, title)) = redirects.get(old) else {
            bail!("legacy documentation URL `{old}` has no preserved page or redirect");
        };
        if !available.contains(destination) {
            bail!("legacy documentation URL `{old}` redirects to missing `{destination}`");
        }
        site = site
            .route_widget::<DocsState, _>(
                old,
                title.clone(),
                None,
                RedirectPage::new(destination.clone(), title.clone()),
            )
            .page_element(
                SitePageElement::head("<meta name=\"robots\" content=\"noindex, follow\">")
                    .only_route(old),
            );
        available.insert(old.to_owned());
    }
    Ok(site)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_legacy_urls_are_unique_and_all_remain_routable() {
        let captured = legacy_routes().collect::<Vec<_>>();
        assert_eq!(
            captured.len(),
            851,
            "do not regenerate the contract from the replacement site"
        );
        assert_eq!(
            captured.iter().collect::<BTreeSet<_>>().len(),
            captured.len()
        );
        let project = Path::new(env!("CARGO_MANIFEST_DIR"));
        let options = SiteBuildOptions::from_project_dir(project, "Fission").unwrap();
        let routes = list_site_routes(&options, &crate::app::site_app().unwrap()).unwrap();
        let available = routes
            .iter()
            .map(|route| route.path.as_str())
            .collect::<BTreeSet<_>>();
        let missing = captured
            .into_iter()
            .filter(|route| !available.contains(route))
            .collect::<Vec<_>>();
        assert!(missing.is_empty(), "missing public URLs: {missing:?}");
    }

    #[test]
    fn filename_alias_survives_an_explicit_canonical_slug() {
        let posts = crate::blog::load_blog_posts(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("content/blog"),
        )
        .unwrap();
        let post = posts
            .iter()
            .find(|post| post.slug == "rust-would-have-avoided-php2shell")
            .unwrap();
        assert_eq!(post.path, "/blog/rust-would-have-avoided-php2shell/");
        assert_eq!(
            post.legacy_path,
            "/blog/2026-10-22-rust-would-have-avoided-php2shell/"
        );
    }
}
