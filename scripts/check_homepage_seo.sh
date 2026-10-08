#!/bin/sh
set -eu

site_root="${1:-documentation/dist/site}"
homepage="$site_root/index.html"
sitemap="$site_root/sitemap.xml"
robots="$site_root/robots.txt"

fail() {
  printf 'homepage SEO check failed: %s\n' "$1" >&2
  exit 1
}

test -f "$homepage" || fail "missing $homepage"
test -f "$sitemap" || fail "missing $sitemap"
test -f "$robots" || fail "missing $robots"

grep -Fq '<title>Fission — Rust GUI Framework for Desktop, Mobile &amp; Web</title>' "$homepage" ||
  fail "unexpected title"
grep -Fq '<meta name="description" content="Build desktop, mobile and web apps in Rust with Fission' "$homepage" ||
  fail "missing search-focused description"
grep -Fq '<link rel="canonical" href="https://fission.rs/">' "$homepage" ||
  fail "missing homepage canonical URL"

h1_count="$(grep -o '<h1[ >]' "$homepage" | wc -l | tr -d ' ')"
test "$h1_count" = "1" || fail "expected one H1, found $h1_count"
perl -0777 -e '
  my $html = <>;
  $html =~ m{<h1\b[^>]*>(.*?)</h1>}s or exit 1;
  my $body = $1;
  exit 1 if $body =~ /<div\b/;
  $body =~ s/<[^>]+>/ /g;
  $body =~ s/\s+/ /g;
  exit($body =~ /Build desktop, mobile and web apps in Rust/ ? 0 : 1);
' "$homepage" || fail "H1 is empty, malformed or off-topic"

h2_count="$(grep -o '<h2[ >]' "$homepage" | wc -l | tr -d ' ')"
test "$h2_count" -ge 5 || fail "expected at least five H2 sections, found $h2_count"

grep -Fq '"@type":"Organization"' "$homepage" || fail "missing Organization JSON-LD"
grep -Fq '"@type":"SoftwareApplication"' "$homepage" ||
  fail "missing SoftwareApplication JSON-LD"
grep -Fq '<loc>https://fission.rs/</loc>' "$sitemap" || fail "homepage missing from sitemap"
grep -Fq 'Allow: /' "$robots" || fail "robots policy does not allow crawling"

perl -0777 -e '
  my $html = <>;
  while ($html =~ /<img\b([^>]*)>/sg) {
    exit 1 unless $1 =~ /\balt=/;
  }
' "$homepage" || fail "homepage image is missing alt text"

perl -0777 -e '
  my $html = <>;
  for my $expected (
    [qr/Build your first Rust app/, qr{href="docs/learn/quickstart/"}],
    [qr/Explore Fission as a Rust application framework/, qr{href="product/overview/"}],
    [qr/Explore cross-platform Rust app development/, qr{href="product/cross-platform-apps/"}],
  ) {
    my ($text, $href) = @$expected;
    my $found = 0;
    pos($html) = 0;
    while ($html =~ /<a\b([^>]*)>(.*?)<\/a>/sg) {
      my ($attrs, $body) = ($1, $2);
      $body =~ s/<[^>]+>/ /g;
      if ($attrs =~ $href && $body =~ $text) {
        $found = 1;
        last;
      }
    }
    exit 1 unless $found;
  }
' "$homepage" || fail "missing a descriptive internal homepage link"

printf 'homepage SEO check passed: %s\n' "$homepage"
