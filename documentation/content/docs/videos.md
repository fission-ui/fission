---
title: Video calendar
sidebar_label: Video calendar
description: A two-year publishing calendar for practical Fission videos, example apps, and tutorial series.
---

# Fission video calendar

This calendar is the public plan for Fission video content. The goal is simple: ship at least one useful video every week, and make each video leave behind a real Fission example that viewers can run, inspect, and adapt.

The videos are free on YouTube. The value for Fission comes from consistent proof: every week should make another part of the framework easier to trust. A viewer should be able to watch the video, open the matching example, and understand how to build that piece of an app themselves.

## Format for each episode

Every episode should produce more than a video. The weekly package should include:

| Asset | Purpose |
| --- | --- |
| Video | A clear walkthrough with the finished app visible early, then the implementation explained step by step. |
| Example app | A production-style Fission example in the repository, structured with clear modules, components, state, and tests. |
| Documentation link | A short guide or reference update when the video covers a widget, capability, shell, CLI feature, or workflow. |
| Screenshot | One or more real screenshots from the Fission app for the website, README, and video thumbnail planning. |
| Tests | Unit, smoke, or integration tests that prove the app or feature keeps working. |

The examples should not be toy code unless the video is explicitly about a tiny concept. When the video is about a widget, show the widget in a realistic screen. When the video is about a capability, show the complete host setup needed to use it. When the video is about an app, structure the code the way we want Fission developers to structure production apps.

## Content pillars

The two-year plan rotates through these pillars so the channel does not become repetitive:

| Pillar | What it proves |
| --- | --- |
| Core Fission | The state model, widget tree, reducers, resources, services, testing, and rendering pipeline are understandable. |
| Widgets | Fission has a broad, polished widget catalog and each widget can be used in real layouts. |
| Apps | Fission can build complete products, not only isolated controls. |
| Charts | Fission can handle serious dashboards and data-heavy UI. |
| Platform capabilities | Desktop, web, Android, and iOS apps can access host capabilities through a consistent Rust API. |
| Shells | Desktop, mobile, web, terminal, static site, and server-rendered site targets are first-class outcomes. |
| Production lifecycle | The CLI supports setup, running, testing, packaging, release, and publishing workflows. |

## Two-year schedule

The dates assume a weekly cadence starting in June 2026. If a week slips, keep the order unless a release, bug fix, or new framework feature makes a later topic more useful to ship first.

| Week | Target date | Working title | Example app or artifact | Viewer should learn | Status |
| --- | --- | --- | --- | --- | --- |
| 1 | 2026-06-01 | Build your first Fission app | Simple counter | How a Fission project starts, how state changes, and how to run the app on desktop. | Planned |
| 2 | 2026-06-08 | The Fission app structure that scales | Counter, reorganised | Where state, actions, reducers, widgets, routes, and tests belong in a real project. | Planned |
| 3 | 2026-06-15 | Actions and reducers without boilerplate | Counter with reducer macros | How user intent becomes typed actions and predictable state updates. | Planned |
| 4 | 2026-06-22 | Layout that adapts to real screens | Responsive profile card | How rows, columns, grids, constraints, spacing, and breakpoints work together. | Planned |
| 5 | 2026-06-29 | Buttons, text input, and form state | Sign-in form | How to wire inputs, validation state, submit buttons, and disabled states. | Planned |
| 6 | 2026-07-06 | Build a useful todo app | Todo list | How to combine lists, reducers, forms, filtering, empty states, and tests. | Planned |
| 7 | 2026-07-13 | Modal dialogs and overlays | Settings modal | How modals, backdrop dismissal, focus, and layered UI fit into app state. | Planned |
| 8 | 2026-07-20 | Theme a Fission app with a design system | Design-system todo | How generated design systems feed themes, buttons, text fields, spacing, and dark mode. | Planned |
| 9 | 2026-07-27 | Internationalisation and readable UI | Locale switcher | What internationalisation means, where strings live, and how locale affects layout. | Planned |
| 10 | 2026-08-03 | Accessibility from the first screen | Accessible settings app | How labels, roles, focus order, and keyboard navigation make a screen usable. | Planned |
| 11 | 2026-08-10 | Navigation and routes in Fission | Mini mail router | How to model routes, route parameters, not-found screens, and navigation actions. | Planned |
| 12 | 2026-08-17 | Resources and async work | Weather lookup | How to request outside work without putting side effects inside component conversion. | Planned |
| 13 | 2026-08-24 | Commands, jobs, and services | Import progress app | When to use commands, long-running jobs, services, progress, cancellation, and resume. | Planned |
| 14 | 2026-08-31 | Test a Fission UI like a product | Counter and todo tests | How to assert state, semantics, hit testing, layout, and rendered behavior. | Planned |
| 15 | 2026-09-07 | Lists that stay fast | Inbox list | How lazy lists, row identity, selection, and scroll state work in a real list. | Planned |
| 16 | 2026-09-14 | Tables, sorting, and dense data | Customer table | How to build sortable rows, headers, empty states, and responsive table fallbacks. | Planned |
| 17 | 2026-09-21 | Select, combobox, and menus | Filter toolbar | How selection widgets differ and how to choose the right one for a task. | Planned |
| 18 | 2026-09-28 | Date and time picking | Scheduler form | How date pickers, time pickers, formatting, and validation fit together. | Planned |
| 19 | 2026-10-05 | Tabs, accordions, and progressive disclosure | Account settings | How to reduce visual noise by revealing the right controls at the right time. | Planned |
| 20 | 2026-10-12 | Drawers and split views | Responsive mail shell | How sidebars collapse, drawers open, and split panes adapt from desktop to mobile. | Planned |
| 21 | 2026-10-19 | Tooltips, popovers, and flyouts | Help overlay | How transient UI should be anchored, dismissed, and tested. | Planned |
| 22 | 2026-10-26 | Drag and drop in a Fission app | Kanban board | How draggable items, drop targets, visual feedback, and state updates work. | Planned |
| 23 | 2026-11-02 | File upload and drop zones | Attachment uploader | How to present file input, dropped files, upload progress, and error states. | Planned |
| 24 | 2026-11-09 | Images, SVG, and media cards | Gallery browser | How media widgets participate in layout, sizing, fitting, and accessibility. | Planned |
| 25 | 2026-11-16 | Video and web embeds | Media article | How embedded surfaces are represented, updated, and tested inside Fission UI. | Planned |
| 26 | 2026-11-23 | Animation that helps the user | Onboarding flow | How transitions communicate state changes without becoming noise. | Planned |
| 27 | 2026-11-30 | Build your first Fission chart | Revenue line chart | How chart widgets, axes, series, tooltips, and themes fit into normal UI. | Planned |
| 28 | 2026-12-07 | Bar charts for real dashboards | Sales comparison | How grouped, stacked, horizontal, and sorted bars communicate comparison. | Planned |
| 29 | 2026-12-14 | Pie, donut, and rose charts | Market share view | When circular charts are useful and when a bar chart is clearer. | Planned |
| 30 | 2026-12-21 | Scatter and bubble charts | Product portfolio | How to show correlation, outliers, labels, and multiple dimensions. | Planned |
| 31 | 2026-12-28 | Heatmaps and calendar heatmaps | Activity tracker | How to present density, missing values, legends, and time-based patterns. | Planned |
| 32 | 2027-01-04 | Finance charts | Portfolio screen | How candlestick, volume, moving average, and range context fit together. | Planned |
| 33 | 2027-01-11 | Maps and route lines | Delivery map | How GeoJSON-backed maps, regions, markers, and routes are modelled. | Planned |
| 34 | 2027-01-18 | Graphs, sankey, and flow | Dependency explorer | How node-link and flow diagrams explain relationships. | Planned |
| 35 | 2027-01-25 | Gauges, funnels, and operational charts | Support dashboard | How to show targets, progress, conversion, and SLA risk clearly. | Planned |
| 36 | 2027-02-01 | Trees, treemaps, and sunbursts | Storage explorer | How hierarchical charts help users understand nested data. | Planned |
| 37 | 2027-02-08 | Data zoom, brush, and chart interaction | Analytics explorer | How users inspect dense data with zooming, brushing, selection, and tooltips. | Planned |
| 38 | 2027-02-15 | 3D charts without a second app model | Capacity scene | How 3D chart scenes are still Fission widgets with normal state and tests. | Planned |
| 39 | 2027-02-22 | Build a complete analytics dashboard | Executive dashboard | How charts, filters, cards, tables, and responsive layout form one screen. | Planned |
| 40 | 2027-03-01 | Notifications and deep links | Reminder app | How apps request notifications, handle taps, and route from deep links. | Planned |
| 41 | 2027-03-08 | Clipboard and file picking | Snippet manager | How to copy, paste, pick files, and display platform capability errors. | Planned |
| 42 | 2027-03-15 | Camera and barcode scanning | Inventory scanner | How camera capture, flashlight, barcode results, permissions, and fallbacks work. | Planned |
| 43 | 2027-03-22 | Geolocation in a cross-platform app | Check-in app | How location permission, accuracy, errors, and user trust affect app design. | Planned |
| 44 | 2027-03-29 | Bluetooth, Wi-Fi, and NFC | Device setup app | How host capabilities expose nearby device workflows consistently. | Planned |
| 45 | 2027-04-05 | Biometrics and passkeys | Secure notes | How user presence, passkeys, and secure authentication fit into an app flow. | Planned |
| 46 | 2027-04-12 | Microphone and audio capture | Voice memo app | How capture requests, permissions, duration, and result handling work. | Planned |
| 47 | 2027-04-19 | Haptics and volume control | Focus timer | How device feedback should support interaction rather than distract. | Planned |
| 48 | 2027-04-26 | Desktop tray apps | Menu bar timer | How tray icons, close behavior, menus, and exit actions work. | Planned |
| 49 | 2027-05-03 | Web and WASM target | Browser counter | How the web target runs, fills the page, handles input, and fits deployment. | Planned |
| 50 | 2027-05-10 | Android and iOS from one app model | Mobile todo | How mobile shell setup, safe areas, screen size, and package targets work. | Planned |
| 51 | 2027-05-17 | Terminal UI with Fission | CLI dashboard | How terminal rendering differs while still using Fission widgets and state. | Planned |
| 52 | 2027-05-24 | Static sites with Fission | Documentation site | How content, templates, generated HTML, search, metadata, and assets are built. | Planned |
| 53 | 2027-05-31 | Build a habit tracker | Habit tracker | How to structure a small daily-use app with history, streaks, and charts. | Planned |
| 54 | 2027-06-07 | Build a personal finance app | Budget planner | How forms, categories, charts, filters, and local state form a practical app. | Planned |
| 55 | 2027-06-14 | Build a recipe manager | Recipe book | How search, tags, images, detail pages, and editable forms work together. | Planned |
| 56 | 2027-06-21 | Build a chat interface | Team chat mock | How message lists, compose boxes, avatars, attachments, and scrollback behave. | Planned |
| 57 | 2027-06-28 | Build a kanban board | Project board | How drag/drop, columns, card editing, dialogs, and keyboard access combine. | Planned |
| 58 | 2027-07-05 | Build a calendar app | Weekly planner | How date navigation, event editing, recurring items, and layout density work. | Planned |
| 59 | 2027-07-12 | Build a notes app | Markdown notes | How markdown, split views, search, autosave, and keyboard shortcuts fit together. | Planned |
| 60 | 2027-07-19 | Build a media library | Movie catalog | How grids, metadata, images, filters, ratings, and responsive cards work. | Planned |
| 61 | 2027-07-26 | Build a photo journal | Travel journal | How camera import, image layout, captions, maps, and timeline views combine. | Planned |
| 62 | 2027-08-02 | Build an IoT dashboard | Sensor console | How live data, charts, alerts, device lists, and historical inspection work. | Planned |
| 63 | 2027-08-09 | Build a point-of-sale screen | Cafe POS | How touch-first layouts, totals, modifiers, receipts, and fast input work. | Planned |
| 64 | 2027-08-16 | Build an issue tracker | Bug tracker | How filters, tables, detail panes, comments, and state transitions work. | Planned |
| 65 | 2027-08-23 | Build a lightweight CRM | Customer CRM | How relationships, notes, tasks, and chart summaries fit into one app. | Planned |
| 66 | 2027-08-30 | Debugging a Fission app | Broken todo, fixed live | How to inspect state, actions, layout, semantics, and render output. | Planned |
| 67 | 2027-09-06 | Widget inspector workflow | Inspector demo | How developer tools should reveal the widget tree, layout, state, and events. | Planned |
| 68 | 2027-09-13 | Network and host work inspector | API dashboard | How requests, resources, jobs, retries, and errors can be made visible. | Planned |
| 69 | 2027-09-20 | Logging and tracing for UI bugs | Trace viewer | How to collect useful traces without turning the app into noise. | Planned |
| 70 | 2027-09-27 | Performance profiling | Large dashboard | How to spot slow reconversions, heavy layouts, expensive paint, and unnecessary work. | Planned |
| 71 | 2027-10-04 | Golden, smoke, and interaction tests | Widget test suite | How to choose the right test type for each UI risk. | Planned |
| 72 | 2027-10-11 | CI for Fission apps | GitHub workflow | How to run checks for desktop, web, mobile package smoke, and static sites. | Planned |
| 73 | 2027-10-18 | Package for Linux | Linux release demo | How readiness checks, metadata, icons, artifacts, and distribution fit together. | Planned |
| 74 | 2027-10-25 | Package for macOS | macOS release demo | How app bundles, signing readiness, icons, package output, and verification work. | Planned |
| 75 | 2027-11-01 | Package for Windows | Windows release demo | How installer metadata, icons, MSIX readiness, and app identity work. | Planned |
| 76 | 2027-11-08 | Android release workflow | Android release demo | How APK/AAB output, signing config, store metadata, screenshots, and checks work. | Planned |
| 77 | 2027-11-15 | iOS release workflow | iOS release demo | How simulator/device builds, signing config, metadata, screenshots, and checks work. | Planned |
| 78 | 2027-11-22 | GitHub releases and static hosting | Release publisher | How the Fission CLI prepares assets, uploads releases, and publishes static sites. | Planned |
| 79 | 2027-11-29 | Material-style design system | Material task app | How to reproduce a design system through tokens, components, states, and typography. | Planned |
| 80 | 2027-12-06 | Fluent-style design system | Enterprise dashboard | How density, command bars, navigation, and desktop-first polish are themed. | Planned |
| 81 | 2027-12-13 | Liquid glass style exploration | Media control app | How depth, translucency, blur, shadows, and contrast need to be handled carefully. | Planned |
| 82 | 2027-12-20 | Cupertino-style mobile UI | Mobile settings app | How mobile conventions, lists, navigation, switches, and safe areas affect design. | Planned |
| 83 | 2027-12-27 | Complex data tables | Operations table | How sticky headers, sorting, filtering, pagination, selection, and density work. | Planned |
| 84 | 2028-01-03 | Offline-first app design | Field notes | How local state, sync queues, conflicts, and visible sync status should be modelled. | Planned |
| 85 | 2028-01-10 | Plugin-ready desktop apps | Editor plugin demo | How extension points, commands, services, and safe boundaries can be designed. | Planned |
| 86 | 2028-01-17 | Build a better text editor surface | Code editor slice | How text storage, cursor movement, selection, syntax color, and huge files are handled. | Planned |
| 87 | 2028-01-24 | Terminal inside a desktop app | Dev console | How an embedded terminal differs from a terminal shell app. | Planned |
| 88 | 2028-01-31 | Data storytelling with charts | Product metrics story | How animation, annotation, mark lines, and chart sequencing explain a result. | Planned |
| 89 | 2028-02-07 | Accessibility audit from scratch | Audit checklist app | How to inspect keyboard flow, labels, contrast, focus, motion, and error text. | Planned |
| 90 | 2028-02-14 | Keyboard-first productivity UI | Command palette app | How shortcuts, focus scopes, menus, and discoverability make power-user flows work. | Planned |
| 91 | 2028-02-21 | Write a custom widget | Rating widget | How to design a reusable widget API, state boundary, semantics, and tests. | Planned |
| 92 | 2028-02-28 | Start a capstone app | Product launch planner | How to plan a larger Fission app before writing screens. | Planned |
| 93 | 2028-03-06 | Capstone: app shell and navigation | Product launch planner | How to create the route structure, shell, sidebar, mobile drawer, and empty states. | Planned |
| 94 | 2028-03-13 | Capstone: tasks and reducers | Product launch planner | How to model core product state, actions, reducers, and validation. | Planned |
| 95 | 2028-03-20 | Capstone: timeline and calendar | Product launch planner | How to build scheduling views, date navigation, and deadline risk indicators. | Planned |
| 96 | 2028-03-27 | Capstone: dashboard and charts | Product launch planner | How to build a useful status dashboard with charts and summary cards. | Planned |
| 97 | 2028-04-03 | Capstone: files and comments | Product launch planner | How attachments, notes, comments, dialogs, and empty states improve the product. | Planned |
| 98 | 2028-04-10 | Capstone: notifications and deep links | Product launch planner | How reminders and deep links connect platform behavior back to app routes. | Planned |
| 99 | 2028-04-17 | Capstone: mobile layout | Product launch planner | How the same app adapts to phone-size screens without becoming a separate product. | Planned |
| 100 | 2028-04-24 | Capstone: web and static companion site | Product launch planner | How the app target and static documentation/landing target can share Fission concepts. | Planned |
| 101 | 2028-05-01 | Capstone: tests and diagnostics | Product launch planner | How to add coverage for reducers, widgets, routing, semantics, and flows. | Planned |
| 102 | 2028-05-08 | Capstone: package every target | Product launch planner | How to prepare desktop, web, Android, iOS, terminal, static, and server-rendered outputs. | Planned |
| 103 | 2028-05-15 | Capstone: release and publish | Product launch planner | How to run readiness checks, generate artifacts, publish releases, and verify output. | Planned |
| 104 | 2028-05-22 | Two years of Fission apps | Showcase reel | What the example library proves and where new viewers should start. | Planned |

## Maintenance notes

As videos are produced, update the matching row with the final title, YouTube link, repository path, and status. If a topic exposes a missing feature or poor API, keep the episode honest: explain the limitation, open the follow-up issue, and use the next suitable episode to show the improvement.

A good weekly rhythm is:

1. Pick the example and define the viewer outcome.
2. Build or improve the example app.
3. Add tests and screenshots.
4. Update documentation.
5. Record the video.
6. Publish the video and update this calendar.

The calendar should remain flexible, but the standard should not. Every shipped video should make Fission more understandable, more trustworthy, and easier to adopt.

## Lifecycle fit and verification

This page belongs to the setup, learn, build, test, and publish lifecycle. Use it to decide the next concrete action, then verify the action before moving to the next stage.

| Stage question | Verification |
| --- | --- |
| What file or command changes? | The page should point to the exact `fission` command, `fission.toml` section, Rust component, or generated artifact involved. |
| What proves it worked? | Prefer a command output, generated file, screenshot, test assertion, package artifact, or deployed URL over a vague statement. |
| What can fail safely? | Permission prompts, missing tools, unsupported hosts, invalid config, and expired credentials should produce diagnosable errors that can be retried after the cause is fixed. |
| What should I read next? | Continue to the linked guide for step-by-step work or the reference page for exact fields and contracts. |
