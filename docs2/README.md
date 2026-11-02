# Fission documentation prototype

`docs2` is the isolated, Compose-first implementation of Fission's new
goal-oriented documentation site. It currently contains the proposed homepage
and one learning goal so the information architecture, responsive layout, and
interactive tutorial experience can be reviewed before the existing site is
migrated.

The documentation-specific parser lives in this crate. It does not extend or
special-case Fission's Static site framework. `FissionSite` registers the
homepage and goal routes, while the pages themselves are written with
`fission_component!` and ordinary public Fission widgets.

## Preview

From the Fission workspace root:

```console
fission site serve --project-dir docs2
```

Before Fission renders the site, the command extracts each complete
`fission:file` block from the goal document and compiles every cumulative step
as a Web app. A broken tutorial therefore fails the site build rather than
publishing code that cannot run. Generated tutorial projects and Web assets are
ignored by Git.

## Goal documents

Goal pages live in `content/goals`. Their frontmatter supplies routing and
learning metadata. The body uses four documentation-specific markers:

- `fission:step id="..."` opens a cumulative tutorial stage.
- `fission:file path="..."` makes the following fenced block a complete file.
- `fission:preview` marks where the compiled result belongs conceptually.
- `fission:checkpoint` contains the behaviour the learner should verify.

Ordinary fenced code is explanatory only. A step inherits files from preceding
steps and replaces any complete file it declares again.

## Application structure

- `src/app.rs` configures the Static site, routes, metadata, theme and locale.
- `src/screens` owns route-level composition.
- `src/widgets` contains small semantic Compose components.
- `src/content.rs` parses the documentation-specific frontmatter and markers.
- `src/tutorials.rs` extracts and compiles the interactive Web applications.
- `design` and `i18n` are the checked-in design-system and translation sources.

## Design references

The visual review source files are kept alongside the implementation:

- `design/reference/homepage-seo-v2.png` — search-focused homepage.
- `design/reference/documentation-hub.png` — goal-oriented documentation hub.
- `design/reference/tutorial-login-flow.png` — interactive tutorial page layout.

The executable entrypoint only validates tutorial applications and hands the
configured site to Fission. Page composition, content parsing and build work do
not leak into `main.rs`.
