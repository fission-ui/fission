import clsx from 'clsx';
import useBaseUrl from '@docusaurus/useBaseUrl';
import {chartCatalog, chartFamilies, type ChartCatalogEntry, type ChartStatus} from '../../data/chartCatalog';
import styles from './ChartCatalogGrid.module.css';

type ChartCatalogGridProps = {
  families?: string[];
  limit?: number;
  slugs?: string[];
  statuses?: ChartStatus[];
  compact?: boolean;
};

const statusLabels: Record<ChartStatus, string> = {
  available: 'Available now',
  next: 'Next implementation slice',
  planned: 'Planned catalog target',
};

function ChartImage({chart}: {chart: ChartCatalogEntry}) {
  const src = useBaseUrl(chart.image);
  return <img src={src} alt={`${chart.title} chart screenshot`} loading='lazy' />;
}

export function ChartCatalogGrid({families, limit, slugs, statuses, compact = false}: ChartCatalogGridProps) {
  const selectedFamilies = families ?? chartFamilies;
  const slugSet = slugs ? new Set(slugs) : undefined;
  const statusSet = statuses ? new Set(statuses) : undefined;

  let charts = chartCatalog.filter((chart) => {
    if (!selectedFamilies.includes(chart.family)) return false;
    if (slugSet && !slugSet.has(chart.slug)) return false;
    if (statusSet && !statusSet.has(chart.status)) return false;
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
          <article key={chart.slug} className={styles.compactCard}>
            <ChartImage chart={chart} />
            <div>
              <p>{chart.family}</p>
              <h3>{chart.title}</h3>
            </div>
          </article>
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
                <article key={chart.slug} className={styles.chartCard}>
                  <ChartImage chart={chart} />
                  <div className={styles.cardBody}>
                    <div className={styles.cardTitleRow}>
                      <h3>{chart.title}</h3>
                      <span className={clsx(styles.status, styles[chart.status])}>{statusLabels[chart.status]}</span>
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
                </article>
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
        const available = charts.filter((chart) => chart.status === 'available').length;
        return (
          <article key={family} className={styles.summaryCard}>
            <p>{charts.length} variants</p>
            <h3>{family}</h3>
            <span>{available} available now</span>
          </article>
        );
      })}
    </div>
  );
}
