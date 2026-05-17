import React, {useCallback, useEffect, useMemo, useRef, useState} from 'react';
import useBaseUrl from '@docusaurus/useBaseUrl';
import styles from './styles.module.css';

type PagefindSearchResult = {
  data: () => Promise<PagefindResultData>;
};

type PagefindSearchResponse = {
  results: PagefindSearchResult[];
};

type PagefindModule = {
  options?: (options: {basePath: string}) => Promise<void> | void;
  init?: () => Promise<void> | void;
  debouncedSearch: (
    query: string,
    options?: Record<string, unknown>,
    debounceTimeout?: number,
  ) => Promise<PagefindSearchResponse | null>;
};

type PagefindSubResult = {
  title?: string;
  url: string;
  excerpt?: string;
  plain_excerpt?: string;
};

type PagefindResultData = {
  url: string;
  excerpt?: string;
  plain_excerpt?: string;
  meta?: {
    title?: string;
  };
  sub_results?: PagefindSubResult[];
};

type SearchStatus = 'idle' | 'loading' | 'ready' | 'error';

const MIN_QUERY_LENGTH = 2;
const RESULT_LIMIT = 8;

function isApplePlatform(): boolean {
  if (typeof navigator === 'undefined') {
    return false;
  }

  return /Mac|iPhone|iPad|iPod/.test(navigator.platform);
}

function withBaseUrl(url: string, baseUrl: string): string {
  if (/^https?:\/\//.test(url)) {
    return url;
  }

  const normalizedBase = baseUrl.endsWith('/') ? baseUrl.slice(0, -1) : baseUrl;
  const normalizedUrl = url.startsWith('/') ? url : `/${url}`;

  if (!normalizedBase || normalizedBase === '/') {
    return normalizedUrl;
  }

  if (normalizedUrl === normalizedBase || normalizedUrl.startsWith(`${normalizedBase}/`)) {
    return normalizedUrl;
  }

  return `${normalizedBase}${normalizedUrl}`;
}

function resultTitle(result: PagefindResultData): string {
  return result.meta?.title?.trim() || 'Untitled page';
}

function resultExcerpt(result: PagefindResultData): string {
  return result.excerpt || result.plain_excerpt || '';
}

export default function SearchBar(): React.JSX.Element {
  const siteBaseUrl = useBaseUrl('/');
  const pagefindScriptUrl = useBaseUrl('/pagefind/pagefind.js');
  const pagefindBundlePath = useMemo(
    () => pagefindScriptUrl.replace(/pagefind\.js$/, ''),
    [pagefindScriptUrl],
  );
  const shortcutLabel = isApplePlatform() ? 'Cmd K' : 'Ctrl K';
  const pagefindRef = useRef<Promise<PagefindModule> | null>(null);
  const searchRunRef = useRef(0);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [status, setStatus] = useState<SearchStatus>('idle');
  const [results, setResults] = useState<PagefindResultData[]>([]);
  const [error, setError] = useState<string | null>(null);

  const loadPagefind = useCallback((): Promise<PagefindModule> => {
    if (!pagefindRef.current) {
      pagefindRef.current = import(/* webpackIgnore: true */ pagefindScriptUrl).then(
        async (module: PagefindModule) => {
          await module.options?.({basePath: pagefindBundlePath});
          await module.init?.();
          return module;
        },
      );
    }

    return pagefindRef.current;
  }, [pagefindBundlePath, pagefindScriptUrl]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setOpen(true);
      }

      if (event.key === 'Escape') {
        setOpen(false);
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  useEffect(() => {
    if (open) {
      window.setTimeout(() => inputRef.current?.focus(), 0);
      void loadPagefind().catch(() => {
        setStatus('error');
        setError('Search index is not available. Run `yarn build` to generate Pagefind assets.');
      });
    }
  }, [loadPagefind, open]);

  useEffect(() => {
    const trimmedQuery = query.trim();
    const runId = searchRunRef.current + 1;
    searchRunRef.current = runId;

    if (trimmedQuery.length < MIN_QUERY_LENGTH) {
      setResults([]);
      setError(null);
      setStatus('idle');
      return undefined;
    }

    setStatus('loading');
    setError(null);

    const timeout = window.setTimeout(() => {
      void loadPagefind()
        .then((pagefind) => pagefind.debouncedSearch(trimmedQuery, {}, 120))
        .then(async (search) => {
          if (searchRunRef.current !== runId || search === null) {
            return;
          }

          const loadedResults = await Promise.all(
            search.results.slice(0, RESULT_LIMIT).map((result) => result.data()),
          );

          if (searchRunRef.current === runId) {
            setResults(loadedResults);
            setStatus('ready');
          }
        })
        .catch(() => {
          if (searchRunRef.current === runId) {
            setResults([]);
            setStatus('error');
            setError('Search index is not available. Run `yarn build` to generate Pagefind assets.');
          }
        });
    }, 80);

    return () => window.clearTimeout(timeout);
  }, [loadPagefind, query]);

  const closeSearch = () => {
    setOpen(false);
  };

  return (
    <>
      <button
        type="button"
        className={styles.searchButton}
        onClick={() => setOpen(true)}
        onFocus={() => void loadPagefind().catch(() => undefined)}>
        <span className={styles.searchIcon} aria-hidden="true">
          
        </span>
        <span className={styles.searchLabel}>Search</span>
        <kbd className={styles.shortcut}>{shortcutLabel}</kbd>
      </button>

      {open && (
        <div className={styles.overlay} role="presentation" onMouseDown={closeSearch}>
          <section
            className={styles.dialog}
            role="dialog"
            aria-modal="true"
            aria-label="Search Fission documentation"
            onMouseDown={(event) => event.stopPropagation()}>
            <div className={styles.inputRow}>
              <span className={styles.dialogIcon} aria-hidden="true">
                
              </span>
              <input
                ref={inputRef}
                className={styles.input}
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="Search docs, guides, and reference"
                aria-label="Search docs, guides, and reference"
              />
              <button type="button" className={styles.closeButton} onClick={closeSearch}>
                Esc
              </button>
            </div>

            <div className={styles.results}>
              {status === 'idle' && (
                <p className={styles.empty}>Type at least two characters to search the local index.</p>
              )}

              {status === 'loading' && <p className={styles.empty}>Searching...</p>}

              {status === 'error' && <p className={styles.empty}>{error}</p>}

              {status === 'ready' && results.length === 0 && (
                <p className={styles.empty}>No results for "{query.trim()}".</p>
              )}

              {status === 'ready' && results.length > 0 && (
                <ol className={styles.resultList}>
                  {results.map((result) => (
                    <li key={result.url} className={styles.resultItem}>
                      <a
                        className={styles.resultLink}
                        href={withBaseUrl(result.url, siteBaseUrl)}
                        onClick={closeSearch}>
                        <span className={styles.resultTitle}>{resultTitle(result)}</span>
                        {resultExcerpt(result) && (
                          <span
                            className={styles.resultExcerpt}
                            dangerouslySetInnerHTML={{__html: resultExcerpt(result)}}
                          />
                        )}
                      </a>
                      {result.sub_results && result.sub_results.length > 0 && (
                        <div className={styles.subResults}>
                          {result.sub_results.slice(0, 3).map((subResult) => (
                            <a
                              key={subResult.url}
                              className={styles.subResultLink}
                              href={withBaseUrl(subResult.url, siteBaseUrl)}
                              onClick={closeSearch}>
                              {subResult.title || 'Section'}
                            </a>
                          ))}
                        </div>
                      )}
                    </li>
                  ))}
                </ol>
              )}
            </div>
          </section>
        </div>
      )}
    </>
  );
}
