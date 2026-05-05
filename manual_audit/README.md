# Manual audit artifacts

This directory stores screenshot-driven manual audit passes for the example apps.

## Naming
- One directory per pass: `YYYY-MM-DD-passN`
- Each pass contains:
  - one subdirectory per example app
  - ordered screenshots for each interaction sequence
  - a `findings.md` file summarizing visual, functional, and performance issues

## Required pass contents
Every substantive UI/platform change must end with a full pass covering every app in `examples/`:
- `animation-gallery`
- `chart-gallery`
- `counter`
- `editor`
- `icons_gallery`
- `inbox`
- `text-lab`
- `widget-gallery`

## Audit method
For each app:
1. Launch the real app via the desktop shell.
2. Drive it through the test-control endpoint with real user-like actions.
3. Take screenshots after each meaningful interaction.
4. Inspect the screenshots manually.
5. Record visual bugs, functional bugs, and performance observations in `findings.md`.

The examples are not the ground truth. They are high-value consumers of the platform. Findings from these audits must be translated into core/platform regression tests wherever possible.
