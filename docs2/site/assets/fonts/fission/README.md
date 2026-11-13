# Fission documentation fonts

This directory contains local font assets, not a list of typefaces necessarily
used by a page. The current site CSS uses system sans-serif and monospace
families; copying a font into the output does not activate it.

- Space Grotesk Variable 5.3.0 from `@fontsource-variable/space-grotesk`
- DM Mono 5.3.0 from `@fontsource/dm-mono`
- Inter static Regular, Medium, SemiBold and Bold files

These typefaces are distributed under the SIL Open Font License 1.1. Copyright
notices and upstream sources are recorded in `LICENSES.md`.

If the design starts using one of these assets, configure its delivery
explicitly. Prefer a same-origin font URL when browser loading is appropriate;
do not turn the presence of these files into an implicit inline-font policy.
