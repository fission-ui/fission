use crate::registry::RegistryCrate;
use crate::state::DocsState;
use crate::widgets::{CrateCard, SiteFooter, SiteHeader};
use fission::prelude::*;

fission_component! {
    pub CrateDirectoryPage<DocsState> {
        props { packages: Vec<RegistryCrate> }
    }
    view(page, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Column(children: [
        SiteHeader(),
        SemanticsRegion(
            identifier: Some("crate-directory-main".into()),
            child: Column(gap: tokens.spacing.l, children: [
                SemanticsRegion(
                    Text(TextContent::Key("crates.title".into())),
                    identifier: Some("site-heading-1:crate-directory".into()),
                ),
                Text(TextContent::Key("crates.introduction".into())).wrap(true),
                if page.packages.is_empty() {
                    Text(TextContent::Key("crates.unavailable".into())).wrap(true)
                } else {
                    Grid(
                        columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                            GridTrack::Points(280.0), GridTrack::Fr(1.0),
                        ))],
                        column_gap: tokens.spacing.l,
                        row_gap: tokens.spacing.l,
                        children: [
                            for package in &page.packages {
                                CrateCard(package.clone(), key: package.name.clone())
                            },
                        ],
                    )
                },
                Link::to(cx.view.env().tr("crates.registry"), "https://crates.io/keywords/fission"),
            ]),
        ),
        SiteFooter(),
    ])
}

fission_component! {
    pub CrateDetailPage<DocsState> {
        props { package: RegistryCrate }
    }
    view(page, _state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Column(children: [
        SiteHeader(),
        SemanticsRegion(
            identifier: Some("crate-detail-main".into()),
            child: Column(gap: tokens.spacing.l, children: [
                Link::to(cx.view.env().tr("crates.back"),
                    if cx.view.env().locale.0.starts_with("es") { "/es/crates/" } else { "/crates/" }),
                SemanticsRegion(
                    Text(page.package.name.clone()),
                    identifier: Some("site-heading-1:crate-detail".into()),
                ),
                Text(page.package.description.clone()).wrap(true),
                Text(format!("{} · {}", page.package.version, page.package.platforms.join(" · "))).wrap(true),
                if page.package.is_prerelease() || page.package.api_status == "alpha" {
                    Text(TextContent::Key("crates.alpha".into()))
                },
                Link::to(cx.view.env().tr("crates.registry"), format!("https://crates.io/crates/{}", page.package.name)),
                if let Some(documentation) = &page.package.documentation {
                    Link::to(cx.view.env().tr("crates.documentation"), documentation.clone())
                },
                if let Some(repository) = &page.package.repository {
                    Link::to(cx.view.env().tr("crates.repository"), repository.clone())
                },
                MarkdownContent::new(page.package.readme_markdown.clone()),
            ]),
        ),
        SiteFooter(),
    ])
}
