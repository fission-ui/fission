# Pass 6 Findings

## Scope
- huge-file backend follow-up after `DocumentMode::Huge`
- embedded editor terminal audit after PTY + wezterm integration
- standalone terminal example spot-check

## Verified
- Huge files now open in a file-backed windowed mode with sparse newline checkpoints and line-oriented window shifting.
- Huge-file edits now go through an overlay journal and save via streamed rewrite instead of full in-memory replacement.
- Embedded editor terminal can execute commands and render alt-screen content after re-showing the terminal panel.
- Standalone terminal example renders command output, paste/copy selection, and alternate screen transitions correctly.

## Screenshots
- Embedded editor terminal output: `.artifacts/screenshots/examples/editor/editor_e2e/24_terminal_output.png`
- Embedded editor terminal alt-screen: `.artifacts/screenshots/examples/editor/editor_e2e/25_terminal_alt_screen.png`
- Standalone terminal commands/copy: `.artifacts/screenshots/examples/terminal/terminal_live/01_terminal_commands_copy.png`
- Standalone terminal alt-screen active: `.artifacts/screenshots/examples/terminal/terminal_live/02_alt_screen_active.png`
- Standalone terminal alt-screen restored: `.artifacts/screenshots/examples/terminal/terminal_live/03_alt_screen_restored.png`

## Remaining issues
- The editor bottom panel still does not reliably land on the terminal view from a cold start if the test only taps the `TERMINAL` tab. Hiding and re-showing the terminal panel via `Ctrl+\`` consistently restores the terminal content. This still needs a dedicated regression and root-cause fix in the tab/content switching path.
- The embedded terminal panel is visually cramped. It is functional, but the default bottom-panel height is small enough that command output, prompt, and editor content compete for space.
- The terminal header title truncates aggressively on the right side (`..ssion/fission`). This is cosmetic but visible in the audit screenshots.
