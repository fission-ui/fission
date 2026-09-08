(function () {
  var script = document.currentScript;
  var assetRoot = script ? new URL(".", script.src) : new URL("./search/", location.href);
  var siteRoot = new URL("../", assetRoot);
  var manifestPromise = null;
  var docsPromise = null;
  var shardCache = new Map();
  var state = { open: false, selected: 0, results: [] };

  function fetchJson(url) {
    return fetch(url, { credentials: "same-origin" }).then(function (response) {
      if (!response.ok) throw new Error("failed to fetch " + url);
      return response.json();
    });
  }

  function manifest() {
    if (!manifestPromise) {
      manifestPromise = fetchJson(new URL("manifest.json", assetRoot));
    }
    return manifestPromise;
  }

  function docs() {
    if (!docsPromise) {
      docsPromise = manifest().then(function (m) {
        return fetchJson(new URL(m.documents, assetRoot));
      });
    }
    return docsPromise;
  }

  function tokenise(input) {
    return String(input || "")
      .toLowerCase()
      .split(/[^a-z0-9_+-]+/g)
      .filter(function (token) {
        return token.length >= 2;
      })
      .slice(0, 8);
  }

  function shardId(token) {
    var first = token.charAt(0);
    return /^[a-z0-9]$/.test(first) ? first : "other";
  }

  function shardFor(token) {
    return manifest().then(function (m) {
      var href = m.shards[shardId(token)];
      if (!href) return {};
      if (!shardCache.has(href)) {
        shardCache.set(href, fetchJson(new URL(href, assetRoot)));
      }
      return shardCache.get(href);
    });
  }

  function search(query) {
    var tokens = tokenise(query);
    if (!tokens.length) return Promise.resolve([]);
    return Promise.all([docs(), Promise.all(tokens.map(shardFor))]).then(function (loaded) {
      var documents = loaded[0];
      var shards = loaded[1];
      var scores = new Map();

      tokens.forEach(function (token, index) {
        var shard = shards[index] || {};
        Object.keys(shard).forEach(function (term) {
          if (term !== token && !term.startsWith(token)) return;
          var multiplier = term === token ? 2 : 1;
          shard[term].forEach(function (posting) {
            var id = posting[0];
            var score = posting[1] * multiplier;
            scores.set(id, (scores.get(id) || 0) + score);
          });
        });
      });

      return Array.from(scores.entries())
        .sort(function (a, b) {
          return b[1] - a[1];
        })
        .slice(0, 12)
        .map(function (entry) {
          var doc = documents[entry[0]];
          return doc && Object.assign({ score: entry[1] }, doc);
        })
        .filter(Boolean);
    });
  }

  function ensureUi() {
    var existing = document.querySelector("[data-fission-search-dialog]");
    if (existing) return existing;

    var dialog = document.createElement("div");
    dialog.className = "fission-site-search-dialog";
    dialog.setAttribute("data-fission-search-dialog", "");
    dialog.hidden = true;
    dialog.innerHTML =
      '<div class="fission-site-search-backdrop" data-fission-search-close></div>' +
      '<section class="fission-site-search-panel" role="dialog" aria-modal="true" aria-label="Search this site">' +
      '<div class="fission-site-search-input-wrap">' +
      '<input class="fission-site-search-input" data-fission-search-input placeholder="Search this site" autocomplete="off" spellcheck="false">' +
      '<kbd>Esc</kbd>' +
      '</div>' +
      '<div class="fission-site-search-results" data-fission-search-results></div>' +
      '</section>';
    document.body.appendChild(dialog);
    return dialog;
  }

  function siteHref(path) {
    return new URL(String(path || "").replace(/^\/+/, ""), siteRoot).toString();
  }

  function renderResults(dialog, results, query) {
    var target = dialog.querySelector("[data-fission-search-results]");
    state.results = results;
    state.selected = 0;
    if (!query) {
      target.innerHTML = '<p class="fission-site-search-empty">Start typing to search this site.</p>';
      return;
    }
    if (!results.length) {
      target.innerHTML = '<p class="fission-site-search-empty">No results for <strong></strong>.</p>';
      target.querySelector("strong").textContent = query;
      return;
    }
    target.innerHTML = results
      .map(function (result, index) {
        var section = result.section ? '<span class="fission-site-search-section"></span>' : "";
        return (
          '<a class="fission-site-search-result" data-fission-search-result="' +
          index +
          '" href="' +
          siteHref(result.href) +
          '">' +
          '<strong></strong>' +
          section +
          '<span class="fission-site-search-excerpt"></span>' +
          "</a>"
        );
      })
      .join("");
    results.forEach(function (result, index) {
      var row = target.querySelector('[data-fission-search-result="' + index + '"]');
      row.querySelector("strong").textContent = result.title;
      if (result.section) row.querySelector(".fission-site-search-section").textContent = result.section;
      row.querySelector(".fission-site-search-excerpt").textContent = result.excerpt || "";
    });
    markSelected(target);
  }

  function markSelected(root) {
    root.querySelectorAll("[data-fission-search-result]").forEach(function (row, index) {
      row.toggleAttribute("aria-selected", index === state.selected);
    });
  }

  function openSearch() {
    var dialog = ensureUi();
    state.open = true;
    dialog.hidden = false;
    document.documentElement.classList.add("fission-site-search-open");
    var input = dialog.querySelector("[data-fission-search-input]");
    renderResults(dialog, [], input.value);
    setTimeout(function () {
      input.focus();
      input.select();
    }, 0);
  }

  function closeSearch() {
    var dialog = ensureUi();
    state.open = false;
    dialog.hidden = true;
    document.documentElement.classList.remove("fission-site-search-open");
  }

  function submitSelected() {
    if (!state.results.length) return;
    location.href = siteHref(state.results[state.selected].href);
  }

  function bind() {
    document.addEventListener("click", function (event) {
      if (event.target.closest("[data-fission-search-trigger]")) {
        event.preventDefault();
        openSearch();
      }
      if (event.target.closest("[data-fission-search-close]")) {
        event.preventDefault();
        closeSearch();
      }
    });

    document.addEventListener("keydown", function (event) {
      var modK = (event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k";
      if (modK) {
        event.preventDefault();
        openSearch();
        return;
      }
      if (!state.open) return;
      var dialog = ensureUi();
      if (event.key === "Escape") {
        event.preventDefault();
        closeSearch();
      } else if (event.key === "ArrowDown") {
        event.preventDefault();
        state.selected = Math.min(state.selected + 1, state.results.length - 1);
        markSelected(dialog);
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        state.selected = Math.max(state.selected - 1, 0);
        markSelected(dialog);
      } else if (event.key === "Enter" && document.activeElement === dialog.querySelector("[data-fission-search-input]")) {
        event.preventDefault();
        submitSelected();
      }
    });

    document.addEventListener("input", function (event) {
      if (!event.target.matches("[data-fission-search-input]")) return;
      var dialog = ensureUi();
      var value = event.target.value;
      search(value)
        .then(function (results) {
          if (event.target.value === value) renderResults(dialog, results, value);
        })
        .catch(function () {
          dialog.querySelector("[data-fission-search-results]").innerHTML =
            '<p class="fission-site-search-empty">Search index could not be loaded.</p>';
        });
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", bind, { once: true });
  } else {
    bind();
  }
})();
