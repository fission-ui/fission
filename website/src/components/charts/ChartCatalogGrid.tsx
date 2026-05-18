import Link from '@docusaurus/Link';
import useBaseUrl from '@docusaurus/useBaseUrl';
import {
  chartCatalog,
  chartFamilies,
  chartFamilyReferencePath,
  chartReferencePath,
  type ChartCatalogEntry,
} from '../../data/chartCatalog';
import styles from './ChartCatalogGrid.module.css';

type ChartCatalogGridProps = {
  families?: string[];
  limit?: number;
  slugs?: string[];
  compact?: boolean;
};

function ChartImage({chart}: {chart: ChartCatalogEntry}) {
  const src = useBaseUrl(chart.image);
  return <img src={src} alt={`${chart.title} chart screenshot`} loading='lazy' />;
}

export function ChartCatalogGrid({families, limit, slugs, compact = false}: ChartCatalogGridProps) {
  const selectedFamilies = families ?? chartFamilies;
  const slugSet = slugs ? new Set(slugs) : undefined;

  let charts = chartCatalog.filter((chart) => {
    if (!selectedFamilies.includes(chart.family)) return false;
    if (slugSet && !slugSet.has(chart.slug)) return false;
    return true;
  });

  if (slugs) {
    charts = slugs
      .map((slug) => charts.find((chart) => chart.slug === slug))
      .filter((chart): chart is ChartCatalogEntry => Boolean(chart));
  }

  if (limit) charts = charts.slice(0, limit);

  if (compact) {
    return (
      <div className={styles.compactGrid}>
        {charts.map((chart) => (
          <Link key={chart.slug} className={styles.compactCard} to={chartReferencePath(chart)}>
            <ChartImage chart={chart} />
            <div>
              <p>{chart.family}</p>
              <h3>{chart.title}</h3>
            </div>
          </Link>
        ))}
      </div>
    );
  }

  return (
    <div className={styles.catalogGroups}>
      {selectedFamilies.map((family) => {
        const familyCharts = charts.filter((chart) => chart.family === family);
        if (familyCharts.length === 0) return null;
        return (
          <section key={family} className={styles.familyGroup}>
            <div className={styles.familyHeader}>
              <p>Chart family</p>
              <h2>{family}</h2>
            </div>
            <div className={styles.catalogGrid}>
              {familyCharts.map((chart) => (
                <Link key={chart.slug} className={styles.chartCard} to={chartReferencePath(chart)}>
                  <ChartImage chart={chart} />
                  <div className={styles.cardBody}>
                    <div className={styles.cardTitleRow}>
                      <h3>{chart.title}</h3>
                    </div>
                    <p>{chart.description}</p>
                    <dl>
                      <div>
                        <dt>Data</dt>
                        <dd>{chart.dataShape}</dd>
                      </div>
                      <div>
                        <dt>Use when</dt>
                        <dd>{chart.useWhen}</dd>
                      </div>
                    </dl>
                    <div className={styles.tags}>
                      {chart.tags.map((tag) => (
                        <span key={tag}>{tag}</span>
                      ))}
                    </div>
                  </div>
                </Link>
              ))}
            </div>
          </section>
        );
      })}
    </div>
  );
}

export function ChartFamilySummary() {
  return (
    <div className={styles.summaryGrid}>
      {chartFamilies.map((family) => {
        const charts = chartCatalog.filter((chart) => chart.family === family);
        return (
          <Link key={family} className={styles.summaryCard} to={chartFamilyReferencePath(family)}>
            <p>{charts.length} variants</p>
            <h3>{family}</h3>
            <span>Open the family reference</span>
          </Link>
        );
      })}
    </div>
  );
}
