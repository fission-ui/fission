# Fission website

This folder contains the Docusaurus-based product and documentation site.

## Development

```bash
cd website
yarn install

yarn start
```

The site runs at http://localhost:3000 and uses hot reload.

## Build

```bash
cd website
yarn build
```

The static output is written to `website/build`.

## Deploy

```bash
cd website
yarn deploy
```

## Current structure

- **Marketing / landing page:** `/`
- **Narrative docs:** `/docs`
- **API and framework reference:** `/reference`
- **Examples:** `/examples`
- **Playground:** `/playground`
- **Showcase:** `/showcase`

We are intentionally presenting the framework as a production-ready destination for
desktop and web platforms (including responsive mobile web), while calling out remaining
ecosystem work in the footer note.
