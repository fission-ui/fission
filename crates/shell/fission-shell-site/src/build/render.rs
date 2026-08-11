use super::routing::{
    canonical_url_for_route, page_asset_href_for_route, page_elements_for_route,
    search_script_href_for_route, structured_data_for_route, stylesheet_href_for_route,
};
use super::SiteBuildOptions;
use crate::document::{ContentRoute, DocumentationPage, SitePageState};
use crate::html::{
    render_ir_to_html_with_styles, CssVariableMap, HtmlRenderOptions, StyleRegistry,
};
use crate::site::{FissionSite, SitePageElementPlacement, SiteRenderContext, SiteRouteRender};
use anyhow::Result;
use fission_core::internal::{BuildCtx, InternalLoweringCx};
use fission_core::registry::{VideoRegistration, WebRegistration};
use fission_core::ui::{Column, Overlay, ZStack};
use fission_core::{Env, MotionDeclaration, RuntimeState, View, Widget, WidgetId};
use fission_layout::LayoutSize;
use std::path::PathBuf;

fn render_footer_node(
    options: &SiteBuildOptions,
    site: &FissionSite,
    route_path: &str,
    env: &Env,
) -> Result<Option<SiteRouteRender>> {
    let Some(footer) = &site.footer else {
        return Ok(None);
    };
    let ctx = SiteRenderContext {
        project_dir: &options.project_dir,
        route_path,
        theme: &env.theme,
        default_locale: &options.default_locale,
        env,
    };
    Ok(Some(footer(&ctx)?))
}

fn append_footer(node: Widget, footer: Option<Widget>) -> Widget {
    let Some(footer) = footer else {
        return node;
    };
    Column {
        children: vec![node, footer],
        ..Default::default()
    }
    .into()
}

fn compose_portals(node: Widget, portals: Vec<(Option<WidgetId>, Widget)>) -> Widget {
    if portals.is_empty() {
        return node;
    }
    Overlay {
        id: None,
        content: node,
        overlay: ZStack {
            id: None,
            children: portals.into_iter().map(|(_, portal)| portal).collect(),
        }
        .into(),
    }
    .into()
}

pub(super) fn render_custom_routes(
    options: &SiteBuildOptions,
    site: &FissionSite,
    styles: &mut StyleRegistry,
) -> Result<Vec<ContentRoute>> {
    let mut routes = Vec::new();
    for route in &site.custom_routes {
        let env = site_env_for_route(options, site, &route.path, None)?;
        let ctx = SiteRenderContext {
            project_dir: &options.project_dir,
            route_path: &route.path,
            theme: &env.theme,
            default_locale: &options.default_locale,
            env: &env,
        };
        let mut rendered = (route.render)(&ctx)?;
        let footer = render_footer_node(options, site, &route.path, &env)?;
        let footer_widget = footer.as_ref().map(|footer| footer.widget.clone());
        let node = append_footer(rendered.widget, footer_widget);
        if let Some(footer) = footer {
            rendered
                .motion_declarations
                .extend(footer.motion_declarations);
            rendered
                .video_registrations
                .extend(footer.video_registrations);
            rendered.web_registrations.extend(footer.web_registrations);
            rendered.portals.extend(footer.portals);
        }
        let node = compose_portals(node, rendered.portals);
        let html = render_node_to_html(
            node,
            &route.title,
            route
                .description
                .clone()
                .or_else(|| options.site_description.clone()),
            &route.path,
            options,
            site,
            &env,
            styles,
            rendered.motion_declarations,
            rendered.video_registrations,
            rendered.web_registrations,
            route.structured_data.clone(),
        )?;
        routes.push(ContentRoute {
            path: route.path.clone(),
            title: route.title.clone(),
            description: route.description.clone(),
            locale: None,
            body: String::new(),
            headings: Vec::new(),
            sidebar: Vec::new(),
            tags: Vec::new(),
            categories: Vec::new(),
            show_adjacent_posts: false,
            source_path: PathBuf::from("<custom>"),
            rendered: Some(html),
        });
    }
    Ok(routes)
}

pub(super) fn render_route(
    route: &ContentRoute,
    routes: &[ContentRoute],
    options: &SiteBuildOptions,
    site: &FissionSite,
    styles: &mut StyleRegistry,
) -> Result<String> {
    if let Some(rendered) = &route.rendered {
        return Ok(rendered.clone());
    }
    let runtime = RuntimeState::default();
    let env = site_env_for_route(options, site, &route.path, route.locale.as_deref())?;
    let state = SitePageState;
    let view = View::new(&state, &runtime, &env, None);
    let mut build_ctx = BuildCtx::<SitePageState>::new();
    let page = DocumentationPage {
        site_title: &options.site_title,
        site_logo: options.site_logo.as_deref(),
        site_nav: &options.site_nav,
        theme_switching: site.document.theme_switching(),
        search_enabled: options.search.enabled,
        route,
        all_routes: routes,
    };
    let page_node = fission_core::build::enter(&mut build_ctx, &view, || page.into());
    let footer = render_footer_node(options, site, &route.path, &env)?;
    let footer_widget = footer.as_ref().map(|footer| footer.widget.clone());
    let node = append_footer(page_node, footer_widget);
    let mut motion_declarations = build_ctx.take_motion_declarations();
    let mut video_registrations = build_ctx.take_video_registrations();
    let mut web_registrations = build_ctx.take_web_registrations();
    let mut portals = build_ctx.take_portals();
    if let Some(footer) = footer {
        motion_declarations.extend(footer.motion_declarations);
        video_registrations.extend(footer.video_registrations);
        web_registrations.extend(footer.web_registrations);
        portals.extend(footer.portals);
    }
    let node = compose_portals(node, portals);
    render_node_to_html(
        node,
        &format!("{} | {}", route.title, options.site_title),
        route
            .description
            .clone()
            .or_else(|| options.site_description.clone()),
        &route.path,
        options,
        site,
        &env,
        styles,
        motion_declarations,
        video_registrations,
        web_registrations,
        Vec::new(),
    )
}

fn site_env_for_route(
    options: &SiteBuildOptions,
    site: &FissionSite,
    route_path: &str,
    declared_locale: Option<&str>,
) -> Result<Env> {
    let mut env = site.document.env().clone();
    env.theme = site.document.theme().clone();
    env.viewport_size = LayoutSize::new(1280.0, 900.0);
    let default_locale = site
        .default_locale
        .as_ref()
        .map(|locale| locale.0.as_str())
        .unwrap_or(options.default_locale.as_str());
    env.locale = declared_locale.unwrap_or(default_locale).into();
    if let Some(resolve_locale) = &site.locale_resolver {
        env.locale = resolve_locale(&crate::SiteLocaleContext {
            project_dir: &options.project_dir,
            route_path,
            theme: &env.theme,
            default_locale,
            declared_locale,
        })?;
    }
    Ok(env)
}

fn render_node_to_html(
    node: Widget,
    title: &str,
    description: Option<String>,
    route_path: &str,
    options: &SiteBuildOptions,
    site: &FissionSite,
    env: &Env,
    styles: &mut StyleRegistry,
    motion_declarations: Vec<MotionDeclaration>,
    video_registrations: Vec<VideoRegistration>,
    web_registrations: Vec<WebRegistration>,
    route_structured_data: Vec<String>,
) -> Result<String> {
    let runtime = RuntimeState::default();
    let mut lowering = InternalLoweringCx::new(env, &runtime, None, None);
    let root = fission_core::internal::lower_widget(&node, &mut lowering);
    lowering.ir.set_root(root);

    let defaults = crate::DocumentMetadata::new(title, description.clone());
    let metadata = if let Some(resolve_metadata) = &site.document_metadata_resolver {
        resolve_metadata(
            &SiteRenderContext {
                project_dir: &options.project_dir,
                route_path,
                theme: &env.theme,
                default_locale: site
                    .default_locale
                    .as_ref()
                    .map(|locale| locale.0.as_str())
                    .unwrap_or(options.default_locale.as_str()),
                env,
            },
            &defaults,
        )?
    } else {
        defaults
    };

    let render_options = HtmlRenderOptions {
        lang: env.locale.0.clone(),
        document_title: metadata.title.clone(),
        description: metadata.description.clone(),
        canonical_url: canonical_url_for_route(options, route_path),
        site_name: Some(options.site_title.clone()),
        favicon_href: site
            .document
            .favicon_href()
            .or(options.site_favicon.as_deref())
            .map(|href| page_asset_href_for_route(route_path, href)),
        stylesheet_href: stylesheet_href_for_route(route_path),
        current_route_path: route_path.to_string(),
        css_variables: CssVariableMap::from_theme(site.document.theme()),
        default_theme_mode: site.document.default_theme_mode(),
        theme_switching: site.document.theme_switching(),
        code_highlighting: site
            .document
            .code_highlighting()
            .cloned()
            .unwrap_or_else(|| options.code_highlighting.clone()),
        search_script_href: options
            .search
            .enabled
            .then(|| search_script_href_for_route(route_path, &options.search.output_path)),
        structured_data: structured_data_for_route(
            options,
            &metadata.title,
            metadata.description.as_deref(),
            route_path,
        )
        .into_iter()
        .chain(route_structured_data)
        .collect(),
        head_start_html: page_elements_for_route(
            options,
            site,
            route_path,
            SitePageElementPlacement::HeadStart,
        ),
        head_end_html: page_elements_for_route(
            options,
            site,
            route_path,
            SitePageElementPlacement::HeadEnd,
        ),
        body_start_html: page_elements_for_route(
            options,
            site,
            route_path,
            SitePageElementPlacement::BodyStart,
        ),
        body_end_html: page_elements_for_route(
            options,
            site,
            route_path,
            SitePageElementPlacement::BodyEnd,
        ),
        motion_declarations,
        video_registrations: video_registrations
            .into_iter()
            .map(|registration| (registration.node_id, registration))
            .collect(),
        web_registrations: web_registrations
            .into_iter()
            .map(|registration| (registration.node_id, registration))
            .collect(),
        font_faces: site.document.font_faces(),
        ..Default::default()
    };
    Ok(render_ir_to_html_with_styles(&lowering.ir, &render_options, styles)?.html)
}
