import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const websiteRoot = path.resolve(__dirname, '..');
const imageDir = path.join(websiteRoot, 'static', 'img', 'charts');
const dataFile = path.join(websiteRoot, 'src', 'data', 'chartCatalog.ts');

const entries = [
  e('line-basic', 'Basic line', 'Cartesian', 'line', 'available', 'A single ordered numeric series with a clear trend line.', 'Vec<f32> on a category, value, or time axis.', 'Use it when the shape of change matters more than individual bars.', ['line', 'axis']),
  e('line-smooth', 'Smooth line', 'Cartesian', 'line-smooth', 'available', 'A continuous line with curve interpolation for softer trend reading.', 'Vec<f32> with smooth interpolation enabled.', 'Use it for dashboards where the series is sampled often enough to justify interpolation.', ['line', 'smooth']),
  e('line-step', 'Step line', 'Cartesian', 'line-step', 'available', 'A line that changes in discrete jumps instead of continuous slopes.', 'Vec<f32> with start, middle, or end step behavior.', 'Use it for counters, states, quotas, and event-driven changes.', ['line', 'step']),
  e('line-area', 'Area line', 'Cartesian', 'area', 'available', 'A line with filled area to emphasize magnitude over time.', 'Vec<f32> plus area style.', 'Use it when total volume is as important as the outline.', ['line', 'area']),
  e('line-stacked-area', 'Stacked area', 'Cartesian', 'stacked-area', 'available', 'Multiple area series stacked into one total.', 'Several line series sharing one stack key.', 'Use it to show composition over time without losing the total.', ['line', 'stack']),
  e('line-time', 'Time series', 'Cartesian', 'line-time', 'next', 'A line whose horizontal axis is time rather than a small category list.', 'Timestamp dimension plus one or more numeric measures.', 'Use it for telemetry, finance, operations, and event history.', ['line', 'time']),
  e('line-log', 'Log line', 'Cartesian', 'line-log', 'next', 'A line drawn against a logarithmic value axis.', 'Positive numeric values on a log scale.', 'Use it when values span several orders of magnitude.', ['line', 'scale']),
  e('line-large', 'Large line', 'Cartesian', 'line-large', 'planned', 'A decimated line path for very large datasets.', 'Tens of thousands to millions of ordered points.', 'Use it when the chart must stay interactive with dense data.', ['line', 'performance']),
  e('bar-basic', 'Basic bar', 'Cartesian', 'bar', 'available', 'A category comparison using rectangular bars.', 'Vec<f32> aligned to category labels.', 'Use it when individual values need easy comparison.', ['bar', 'category']),
  e('bar-grouped', 'Grouped bar', 'Cartesian', 'grouped-bar', 'available', 'Several bar series shown side by side for each category.', 'Multiple bar series sharing the same category axis.', 'Use it to compare related measures within each group.', ['bar', 'grouped']),
  e('bar-stacked', 'Stacked bar', 'Cartesian', 'stacked-bar', 'available', 'Bars stacked into a cumulative total for each category.', 'Multiple bar series sharing one stack key.', 'Use it to show composition and total at the same time.', ['bar', 'stack']),
  e('bar-horizontal', 'Horizontal bar', 'Cartesian', 'horizontal-bar', 'next', 'A bar chart rotated so long category labels stay readable.', 'Categories on the vertical axis and values on the horizontal axis.', 'Use it for rankings, surveys, and tables converted into charts.', ['bar', 'ranking']),
  e('bar-background', 'Bar with background', 'Cartesian', 'bar-background', 'next', 'Bars drawn against a faint maximum guide.', 'Value bars plus maximum/reference background.', 'Use it for progress-like comparisons across categories.', ['bar', 'reference']),
  e('bar-waterfall', 'Waterfall', 'Cartesian', 'waterfall', 'planned', 'Floating bars that show how positive and negative changes build a total.', 'Ordered deltas with intermediate totals.', 'Use it for revenue bridges, budget movement, and attribution.', ['bar', 'waterfall']),
  e('bar-race', 'Bar race', 'Cartesian', 'bar-race', 'planned', 'An animated ranking chart over time.', 'Category values across ordered frames.', 'Use it when rank movement is the story.', ['bar', 'animation']),
  e('pictorial-bar', 'Pictorial bar', 'Cartesian', 'pictorial-bar', 'available', 'Bars represented by repeated symbols or a custom path.', 'Numeric values plus symbol choice.', 'Use it when the chart should feel branded without giving up scale.', ['bar', 'symbol']),

  e('pie-basic', 'Pie', 'Radial and polar', 'pie', 'available', 'A circular part-to-whole chart.', 'Label/value pairs.', 'Use it for a small number of categories where the whole matters.', ['pie', 'part-to-whole']),
  e('pie-donut', 'Donut', 'Radial and polar', 'donut', 'available', 'A pie chart with an open center for a total or primary label.', 'Label/value pairs plus inner radius.', 'Use it when the whole needs a central value or status.', ['pie', 'donut']),
  e('pie-rose-radius', 'Rose by radius', 'Radial and polar', 'rose', 'available', 'Slices use both angle and radius to make differences more expressive.', 'Label/value pairs plus rose radius mode.', 'Use it for presentation-heavy part-to-whole views.', ['pie', 'rose']),
  e('pie-rose-area', 'Rose by area', 'Radial and polar', 'rose-area', 'next', 'Slices vary by area so large categories stand out strongly.', 'Label/value pairs plus area-based rose mode.', 'Use it when the visual hierarchy should be more dramatic than a normal pie.', ['pie', 'rose']),
  e('pie-nested', 'Nested donut', 'Radial and polar', 'nested-donut', 'planned', 'Two or more donut rings that compare related part-to-whole layers.', 'Several label/value series with ring placement.', 'Use it for inner/outer category breakdowns.', ['pie', 'nested']),
  e('radar-basic', 'Radar', 'Radial and polar', 'radar', 'available', 'Multiple metrics plotted around a circular axis set.', 'Vec<Vec<f32>> where each row is one profile.', 'Use it for profile comparison across a fixed set of dimensions.', ['radar', 'profile']),
  e('radar-filled', 'Filled radar', 'Radial and polar', 'radar-filled', 'available', 'A radar chart with filled polygons for easier shape comparison.', 'Metric profiles plus fill style.', 'Use it when profile area and overlap should be visible.', ['radar', 'profile']),
  e('polar-line', 'Polar line', 'Radial and polar', 'polar-line', 'planned', 'A line plotted in angle/radius space.', 'Angle and radius values.', 'Use it for cyclic data such as direction, season, or clock position.', ['polar', 'line']),
  e('polar-bar', 'Polar bar', 'Radial and polar', 'polar-bar', 'planned', 'Bars arranged around a circle.', 'Category or angle buckets with numeric values.', 'Use it for cyclic category comparisons.', ['polar', 'bar']),
  e('radial-bar', 'Radial progress bars', 'Radial and polar', 'radial-bar', 'planned', 'Concentric arcs used as compact progress indicators.', 'One or more percentages or bounded values.', 'Use it for dashboards with several bounded measures.', ['polar', 'progress']),
  e('gauge-basic', 'Gauge', 'Radial and polar', 'gauge', 'available', 'A dial-style chart for one bounded measure.', 'One label/value pair with an expected range.', 'Use it when the value is read as a current instrument state.', ['gauge', 'status']),
  e('gauge-progress', 'Progress gauge', 'Radial and polar', 'gauge-progress', 'available', 'A gauge emphasizing completed amount rather than a needle.', 'One or more bounded values.', 'Use it for operational progress and service health panels.', ['gauge', 'progress']),
  e('liquid-fill', 'Liquid fill', 'Radial and polar', 'liquid', 'available', 'A circular fill indicator with a wave-shaped level.', 'One or more percentages.', 'Use it for capacity, completion, or quota states.', ['liquid', 'status']),

  e('scatter-basic', 'Scatter', 'Statistical and finance', 'scatter', 'available', 'Points plotted by two numeric dimensions.', 'Vec<(f32, f32)>.', 'Use it to find relationship, clustering, or outliers.', ['scatter', 'correlation']),
  e('scatter-bubble', 'Bubble scatter', 'Statistical and finance', 'bubble', 'next', 'A scatter chart where point size carries a third measure.', 'x, y, and size dimensions.', 'Use it when two-dimensional position is not enough.', ['scatter', 'bubble']),
  e('scatter-effect', 'Effect scatter', 'Statistical and finance', 'effect-scatter', 'available', 'Scatter points with emphasis rings for important observations.', 'Vec<(f32, f32)> plus emphasis styling.', 'Use it to mark active locations, alerts, or selected results.', ['scatter', 'effect']),
  e('boxplot-basic', 'Boxplot', 'Statistical and finance', 'boxplot', 'available', 'A distribution summary showing min, quartiles, median, and max.', 'Rows of five-number summaries or raw groups.', 'Use it when distribution matters more than a single average.', ['statistics', 'distribution']),
  e('violin-plot', 'Violin plot', 'Statistical and finance', 'violin', 'planned', 'A distribution chart showing density shape around a center line.', 'Raw samples or precomputed density estimates.', 'Use it when the full distribution shape is important.', ['statistics', 'density']),
  e('histogram', 'Histogram', 'Statistical and finance', 'histogram', 'planned', 'Binned counts over a continuous numeric range.', 'Raw numeric samples plus bin strategy.', 'Use it to explain the shape of a single variable.', ['statistics', 'bins']),
  e('candlestick-basic', 'Candlestick', 'Statistical and finance', 'candlestick', 'available', 'Open, close, low, and high values drawn as market candles.', 'Rows of open, close, low, high values.', 'Use it for finance and other range-over-time data.', ['finance', 'ohlc']),
  e('ohlc-bar', 'OHLC bar', 'Statistical and finance', 'ohlc', 'planned', 'Financial range data drawn with open and close ticks.', 'Rows of open, high, low, close values.', 'Use it when the compact OHLC convention is preferred over candles.', ['finance', 'ohlc']),
  e('volume-underlay', 'Price with volume', 'Statistical and finance', 'volume', 'planned', 'A financial price chart with aligned volume bars.', 'OHLC rows plus volume series.', 'Use it for trading and market analysis surfaces.', ['finance', 'volume']),
  e('heatmap-cartesian', 'Cartesian heatmap', 'Statistical and finance', 'heatmap', 'available', 'A rectangular value matrix rendered with a color scale.', 'x index, y index, and value triples.', 'Use it for density, activity, and matrix-style comparison.', ['heatmap', 'matrix']),
  e('heatmap-calendar', 'Calendar heatmap', 'Statistical and finance', 'calendar-heatmap', 'planned', 'A date grid where each day is colored by value.', 'Date/value pairs.', 'Use it for habits, activity, incidents, and availability.', ['heatmap', 'calendar']),
  e('matrix-bubble', 'Matrix bubble', 'Statistical and finance', 'matrix', 'planned', 'A matrix where cell size and color both encode data.', 'Row, column, size, and color dimensions.', 'Use it for correlation and comparison matrices.', ['matrix', 'bubble']),

  e('graph-force', 'Force graph', 'Relationships and hierarchy', 'graph', 'available', 'Nodes and edges arranged into a readable network.', 'Node list plus edge list.', 'Use it for dependency, social, and topology diagrams.', ['graph', 'network']),
  e('graph-circular', 'Circular graph', 'Relationships and hierarchy', 'graph-circular', 'next', 'A network arranged around a circle for stable comparison.', 'Node list plus edge list and circular layout settings.', 'Use it when node order is meaningful or stability matters.', ['graph', 'network']),
  e('tree-basic', 'Tree', 'Relationships and hierarchy', 'tree', 'planned', 'A parent-child hierarchy laid out as a tree.', 'Nested nodes.', 'Use it for organization charts, file trees, and decision paths.', ['tree', 'hierarchy']),
  e('tree-radial', 'Radial tree', 'Relationships and hierarchy', 'radial-tree', 'planned', 'A hierarchy expanded from the center outward.', 'Nested nodes with radial layout.', 'Use it for dense hierarchies where breadth matters.', ['tree', 'radial']),
  e('treemap-basic', 'Treemap', 'Relationships and hierarchy', 'treemap', 'available', 'Hierarchical values packed into rectangles.', 'Nested nodes with values.', 'Use it for storage, budgets, or part-to-whole hierarchy.', ['treemap', 'hierarchy']),
  e('sunburst-basic', 'Sunburst', 'Relationships and hierarchy', 'sunburst', 'available', 'A hierarchy drawn as concentric rings.', 'Nested nodes with values.', 'Use it when hierarchy depth should remain visible.', ['sunburst', 'hierarchy']),
  e('sankey-basic', 'Sankey', 'Relationships and hierarchy', 'sankey', 'available', 'Flow between stages using bands with width.', 'Node list plus weighted edges.', 'Use it for energy, revenue, and conversion flows.', ['sankey', 'flow']),
  e('funnel-basic', 'Funnel', 'Relationships and hierarchy', 'funnel', 'available', 'Stage values drawn as narrowing bands.', 'Ordered label/value pairs.', 'Use it for conversion stages and pipeline health.', ['funnel', 'conversion']),
  e('theme-river', 'Stream river', 'Relationships and hierarchy', 'theme-river', 'available', 'Stacked flowing bands over time.', 'Time, value, and category tuples.', 'Use it when composition changes continuously over time.', ['stream', 'time']),
  e('parallel-basic', 'Parallel coordinates', 'Relationships and hierarchy', 'parallel', 'available', 'Rows drawn across multiple vertical axes.', 'Vec<Vec<f32>> with one row per observation.', 'Use it for high-dimensional filtering and comparison.', ['parallel', 'dimensions']),

  e('map-choropleth', 'Choropleth map', 'Geographic and calendar', 'map', 'available', 'Regions colored by a numeric value.', 'Region identifiers plus values and GeoJSON geometry.', 'Use it for geography-first comparison.', ['map', 'geo']),
  e('map-bubble', 'Map bubble', 'Geographic and calendar', 'map-bubble', 'planned', 'Geographic points sized by value.', 'Longitude, latitude, and value triples.', 'Use it for city, branch, event, and sensor maps.', ['map', 'scatter']),
  e('geo-lines', 'Geo lines', 'Geographic and calendar', 'geo-lines', 'planned', 'Curved paths between geographic points.', 'Origin/destination coordinate pairs.', 'Use it for flights, routes, and network flow.', ['map', 'routes']),
  e('geo-heatmap', 'Geo heatmap', 'Geographic and calendar', 'geo-heatmap', 'planned', 'Density over a geographic surface.', 'Coordinate/value samples plus smoothing settings.', 'Use it for activity, risk, or demand concentration.', ['map', 'heatmap']),
  e('route-map', 'Route map', 'Geographic and calendar', 'route-map', 'planned', 'A path over geographic geometry with milestones.', 'Polyline coordinates and optional events.', 'Use it for logistics, tracking, and trip playback.', ['map', 'route']),
  e('calendar-basic', 'Calendar', 'Geographic and calendar', 'calendar', 'planned', 'Date cells arranged into weeks and months.', 'Date/value pairs.', 'Use it when the date grid itself is the primary structure.', ['calendar', 'time']),
  e('timeline-events', 'Timeline events', 'Geographic and calendar', 'timeline', 'planned', 'Events placed on a time axis with duration or point markers.', 'Timestamped events.', 'Use it for incident, release, and audit timelines.', ['time', 'events']),
  e('gantt-basic', 'Gantt', 'Geographic and calendar', 'gantt', 'planned', 'Tasks with start/end spans across time.', 'Task rows plus start and end dates.', 'Use it for plans, schedules, and project tracking.', ['time', 'schedule']),
  e('calendar-range', 'Calendar range', 'Geographic and calendar', 'calendar-range', 'planned', 'Date ranges highlighted inside a calendar grid.', 'Start/end date pairs.', 'Use it for availability, bookings, and campaigns.', ['calendar', 'range']),
  e('map-small-multiples', 'Map small multiples', 'Geographic and calendar', 'map-multiples', 'planned', 'Several compact maps sharing one scale.', 'Several region/value datasets.', 'Use it to compare geography over time or scenario.', ['map', 'multiples']),

  e('dataset-encoded', 'Encoded dataset', 'Data pipeline and interaction', 'dataset', 'available', 'Line and bar series bound to named dataset dimensions.', 'Dataset rows, dimensions, and encode mappings.', 'Use it when chart code should name fields instead of copying arrays.', ['dataset', 'encode']),
  e('visual-map', 'Visual map', 'Data pipeline and interaction', 'visual-map', 'available', 'Color encodes a numeric range consistently across a chart.', 'Numeric values plus a color scale.', 'Use it for heatmap and scatter intensity.', ['visual-map', 'color']),
  e('data-zoom', 'Data zoom', 'Data pipeline and interaction', 'data-zoom', 'next', 'A viewport selector for large ordered data.', 'Ordered categories or time values.', 'Use it when users need to inspect a slice of a larger series.', ['zoom', 'interaction']),
  e('tooltip-axis', 'Axis tooltip', 'Data pipeline and interaction', 'tooltip-axis', 'next', 'A crosshair and tooltip tied to the nearest axis value.', 'Pointer position plus resolved series values.', 'Use it for multi-series readout.', ['tooltip', 'interaction']),
  e('brush-select', 'Brush selection', 'Data pipeline and interaction', 'brush', 'planned', 'A rectangular or lasso selection over plotted marks.', 'Pointer gesture and selected data keys.', 'Use it for exploration and dashboard filtering.', ['brush', 'interaction']),
  e('mark-line-point', 'Mark line and point', 'Data pipeline and interaction', 'mark-line', 'planned', 'Reference lines, thresholds, and annotated points.', 'Reference values and annotation labels.', 'Use it to explain targets, alerts, and key events.', ['annotation', 'reference']),
  e('custom-render-hook', 'Typed custom render', 'Data pipeline and interaction', 'custom-render', 'planned', 'A typed extension hook for bespoke marks.', 'Typed data plus a Fission render hook.', 'Use it when a product needs a domain-specific visualization.', ['custom', 'typed']),
  e('animated-transition', 'Animated transition', 'Data pipeline and interaction', 'animation', 'planned', 'Stateful chart transitions between data frames.', 'Old and new model snapshots.', 'Use it when change should be explained rather than replaced instantly.', ['animation', 'state']),

  e('scatter3d-basic', '3D scatter', '3D and GL', 'scatter3d', 'planned', 'Points in three-dimensional space.', 'x, y, z triples plus point styling.', 'Use it for spatial data and multi-dimensional exploration.', ['3d', 'scatter']),
  e('bar3d-basic', '3D bar', '3D and GL', 'bar3d', 'planned', 'Bars rising from a 3D grid.', 'x, y, z category/value triples.', 'Use it for dense matrix data where height adds useful shape.', ['3d', 'bar']),
  e('line3d-basic', '3D line', '3D and GL', 'line3d', 'planned', 'A path through three-dimensional space.', 'Ordered x, y, z points.', 'Use it for trajectories, paths, and parametric curves.', ['3d', 'line']),
  e('surface3d-basic', '3D surface', '3D and GL', 'surface3d', 'planned', 'A continuous surface over a grid.', 'x/y grid values with z height.', 'Use it for functions, terrain, and response surfaces.', ['3d', 'surface']),
  e('globe-basic', 'Globe', '3D and GL', 'globe', 'planned', 'A rotatable planet surface for geographic data.', 'Texture, camera, and geographic overlays.', 'Use it when world-scale context matters.', ['3d', 'globe']),
  e('globe-bars', 'Bars on globe', '3D and GL', 'globe-bars', 'planned', 'Extruded bars placed on a globe.', 'Longitude, latitude, and value triples.', 'Use it for population, traffic, and global metrics.', ['3d', 'globe']),
  e('globe-flights', 'Globe flight lines', '3D and GL', 'globe-lines', 'planned', 'Animated arcs between points on a globe.', 'Origin/destination coordinates plus value.', 'Use it for travel, logistics, and network traffic.', ['3d', 'globe']),
  e('map3d-basic', '3D map', '3D and GL', 'map3d', 'planned', 'Extruded regions with camera controls.', 'GeoJSON regions plus numeric values.', 'Use it for geographic data where elevation helps comparison.', ['3d', 'map']),
  e('graph3d-basic', '3D graph', '3D and GL', 'graph3d', 'planned', 'A network laid out in three dimensions.', 'Nodes and edges with force or fixed positions.', 'Use it for large topology exploration.', ['3d', 'graph']),
  e('terrain-surface', 'Terrain surface', '3D and GL', 'terrain', 'planned', 'A shaded terrain mesh from elevation data.', 'Heightmap or triangulated mesh.', 'Use it for maps, simulation, and environmental data.', ['3d', 'terrain']),
  e('point-cloud', 'Point cloud', '3D and GL', 'point-cloud', 'planned', 'Many colored points rendered efficiently.', 'Large x/y/z point sets with optional color.', 'Use it for scans, telemetry, and dense spatial samples.', ['3d', 'performance']),
  e('volume-render', 'Volume render', '3D and GL', 'volume', 'planned', 'A scalar field rendered through slices or ray marching.', '3D grid of scalar values.', 'Use it for medical, scientific, and simulation data.', ['3d', 'volume']),
  e('vector-field', 'Vector field', '3D and GL', 'vector-field', 'planned', 'Arrows or streamlines showing direction and magnitude.', 'Positions plus vector components.', 'Use it for weather, physics, and flow analysis.', ['3d', 'vector']),
  e('mesh-render', 'Mesh render', '3D and GL', 'mesh', 'planned', 'A lit mesh with data-bound colors.', 'Vertices, indices, normals, and optional values.', 'Use it when the data is already a 3D object.', ['3d', 'mesh']),
  e('flow-field', 'Flow field', '3D and GL', 'flow', 'planned', 'Animated particles or paths through vector data.', 'Vector field plus seed/animation configuration.', 'Use it for fluid, wind, and traffic direction.', ['3d', 'flow']),
  e('large-scatter-gl', 'Large scatter acceleration', '3D and GL', 'large-scatter', 'planned', 'GPU-accelerated point rendering for very large datasets.', 'Large point buffers and direct GPU upload path.', 'Use it when normal scene nodes are too expensive.', ['3d', 'performance']),
];

function e(slug, title, family, visual, status, description, dataShape, useWhen, tags) {
  return {slug, title, family, visual, status, description, dataShape, useWhen, tags};
}

const palette = ['#2563eb', '#14b8a6', '#f59e0b', '#ef4444', '#8b5cf6', '#22c55e', '#0ea5e9', '#f97316'];
const dark = '#0f172a';
const muted = '#64748b';
const grid = '#dbe4ef';

function hash(input) {
  let h = 2166136261;
  for (const ch of input) {
    h ^= ch.charCodeAt(0);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

function rnd(seed) {
  let state = seed >>> 0;
  return () => {
    state = Math.imul(1664525, state) + 1013904223;
    return ((state >>> 0) / 4294967296);
  };
}

function esc(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;');
}

function svg(entry) {
  const seed = hash(entry.slug);
  const id = entry.slug.replaceAll('-', '_');
  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="640" height="420" viewBox="0 0 640 420" role="img" aria-labelledby="title-${id}">
  <title id="title-${id}">${esc(entry.title)} chart preview</title>
  <defs>
    <linearGradient id="bg-${id}" x1="0" x2="1" y1="0" y2="1">
      <stop offset="0" stop-color="#f8fafc"/>
      <stop offset="1" stop-color="#eef6f8"/>
    </linearGradient>
    <linearGradient id="accent-${id}" x1="0" x2="1" y1="0" y2="0">
      <stop offset="0" stop-color="#2563eb"/>
      <stop offset="0.52" stop-color="#14b8a6"/>
      <stop offset="1" stop-color="#f59e0b"/>
    </linearGradient>
    <filter id="shadow-${id}" x="-20%" y="-20%" width="140%" height="140%">
      <feDropShadow dx="0" dy="10" stdDeviation="14" flood-color="#0f172a" flood-opacity="0.12"/>
    </filter>
  </defs>
  <rect width="640" height="420" rx="30" fill="url(#bg-${id})"/>
  <rect x="34" y="28" width="572" height="348" rx="26" fill="#ffffff" filter="url(#shadow-${id})"/>
  <text x="58" y="68" font-family="Inter, ui-sans-serif, system-ui, sans-serif" font-size="22" font-weight="720" fill="#0f172a">${esc(entry.title)}</text>
  <text x="58" y="94" font-family="Inter, ui-sans-serif, system-ui, sans-serif" font-size="13" fill="#64748b">${esc(entry.family)}</text>
  <g transform="translate(58 112)">
    ${draw(entry.visual, seed, id)}
  </g>
</svg>
`;
}

function draw(visual, seed, id) {
  if (visual.includes('3d') || visual.includes('globe') || ['terrain', 'point-cloud', 'volume', 'vector-field', 'mesh', 'flow', 'large-scatter'].includes(visual)) {
    return draw3d(visual, seed, id);
  }
  switch (visual) {
    case 'line':
    case 'line-smooth':
    case 'line-step':
    case 'line-time':
    case 'line-log':
    case 'line-large': return drawLine(visual, seed, false, id);
    case 'area':
    case 'stacked-area': return drawLine(visual, seed, true, id);
    case 'bar':
    case 'grouped-bar':
    case 'stacked-bar':
    case 'bar-background':
    case 'pictorial-bar': return drawBars(visual, seed, id);
    case 'horizontal-bar': return drawHorizontalBars(seed);
    case 'waterfall': return drawWaterfall(seed);
    case 'bar-race': return drawHorizontalBars(seed, true);
    case 'pie':
    case 'donut':
    case 'rose':
    case 'rose-area':
    case 'nested-donut': return drawPie(visual, seed);
    case 'radar':
    case 'radar-filled': return drawRadar(seed);
    case 'polar-line':
    case 'polar-bar':
    case 'radial-bar': return drawPolar(visual, seed);
    case 'gauge':
    case 'gauge-progress': return drawGauge(visual);
    case 'liquid': return drawLiquid(id);
    case 'scatter':
    case 'bubble':
    case 'effect-scatter': return drawScatter(visual, seed);
    case 'boxplot': return drawBoxplot(seed);
    case 'violin': return drawViolin(seed);
    case 'histogram': return drawHistogram(seed);
    case 'candlestick':
    case 'ohlc':
    case 'volume': return drawFinance(visual, seed);
    case 'heatmap':
    case 'calendar-heatmap':
    case 'matrix': return drawHeatmap(visual, seed);
    case 'graph':
    case 'graph-circular': return drawGraph(visual, seed);
    case 'tree':
    case 'radial-tree': return drawTree(visual);
    case 'treemap': return drawTreemap(seed);
    case 'sunburst': return drawSunburst(seed);
    case 'sankey': return drawSankey();
    case 'funnel': return drawFunnel();
    case 'theme-river': return drawRiver(seed, id);
    case 'parallel': return drawParallel(seed);
    case 'map':
    case 'map-bubble':
    case 'geo-lines':
    case 'geo-heatmap':
    case 'route-map':
    case 'map-multiples': return drawMap(visual, seed);
    case 'calendar':
    case 'calendar-range': return drawCalendar(visual, seed);
    case 'timeline':
    case 'gantt': return drawTimeline(visual, seed);
    case 'dataset': return drawBars('grouped-bar', seed, id) + drawLine('line-smooth', seed + 1, false, id);
    case 'visual-map': return drawHeatmap('heatmap', seed);
    case 'data-zoom': return drawLine('line', seed, false, id) + `<rect x="126" y="238" width="280" height="8" rx="4" fill="#dbeafe"/><rect x="186" y="236" width="150" height="12" rx="6" fill="#2563eb" opacity=".72"/>`;
    case 'tooltip-axis': return drawLine('line', seed, false, id) + `<line x1="282" y1="22" x2="282" y2="222" stroke="#0f172a" stroke-dasharray="4 4"/><rect x="296" y="54" width="106" height="52" rx="10" fill="#0f172a" opacity=".9"/><text x="310" y="78" font-size="12" fill="#fff">Fri 24</text><text x="310" y="96" font-size="12" fill="#a7f3d0">Value 82</text>`;
    case 'brush': return drawScatter('scatter', seed) + `<rect x="158" y="64" width="176" height="108" rx="8" fill="#2563eb" opacity=".12" stroke="#2563eb" stroke-dasharray="5 4"/>`;
    case 'mark-line': return drawLine('line', seed, false, id) + `<line x1="52" y1="112" x2="446" y2="112" stroke="#ef4444" stroke-width="2" stroke-dasharray="6 5"/><circle cx="304" cy="92" r="7" fill="#ef4444"/>`;
    case 'custom-render': return drawCustom(seed);
    case 'animation': return drawLine('line-smooth', seed, false, id) + `<path d="M 320 42 l 32 20 l -32 20 z" fill="#14b8a6" opacity=".8"/>`;
    case 'wordcloud': return drawWordcloud(seed);
    default: return drawLine('line', seed, false, id);
  }
}

function chartFrame() {
  const lines = [];
  for (let i = 0; i < 5; i++) {
    const y = 24 + i * 48;
    lines.push(`<line x1="48" y1="${y}" x2="462" y2="${y}" stroke="${grid}" stroke-width="1"/>`);
  }
  return `<g>${lines.join('')}<line x1="48" y1="216" x2="462" y2="216" stroke="#94a3b8"/><line x1="48" y1="24" x2="48" y2="216" stroke="#94a3b8"/></g>`;
}

function points(seed, count = 9) {
  const r = rnd(seed);
  return Array.from({length: count}, (_, i) => [48 + i * (414 / (count - 1)), 54 + r() * 120 + (i % 3) * 16]);
}

function drawLine(visual, seed, area, id) {
  const pts = points(seed, visual === 'line-large' ? 22 : 9).map(([x, y]) => [x, Math.min(216, y)]);
  const line = visual === 'line-step'
    ? pts.slice(1).reduce((d, [x, y], i) => `${d} L ${x - 23} ${pts[i][1]} L ${x - 23} ${y} L ${x} ${y}`, `M ${pts[0][0]} ${pts[0][1]}`)
    : `M ${pts.map((p) => p.join(' ')).join(' L ')}`;
  const areaPath = area ? `<path d="${line} L ${pts.at(-1)[0]} 216 L ${pts[0][0]} 216 Z" fill="url(#accent-${id})" opacity=".18"/>` : '';
  const second = visual === 'stacked-area' ? `<path d="M 48 174 L 100 142 L 152 156 L 204 98 L 256 126 L 308 78 L 360 94 L 414 62 L 462 88 L 462 216 L 48 216 Z" fill="#14b8a6" opacity=".18"/><path d="M 48 174 L 100 142 L 152 156 L 204 98 L 256 126 L 308 78 L 360 94 L 414 62 L 462 88" fill="none" stroke="#14b8a6" stroke-width="4" stroke-linejoin="round"/>` : '';
  return `${chartFrame()}${areaPath}${second}<path d="${line}" fill="none" stroke="#2563eb" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/>${pts.map(([x, y]) => `<circle cx="${x}" cy="${y}" r="4" fill="#fff" stroke="#2563eb" stroke-width="3"/>`).join('')}`;
}

function drawBars(visual, seed) {
  const r = rnd(seed);
  let out = chartFrame();
  const cats = 7;
  for (let i = 0; i < cats; i++) {
    const x = 72 + i * 54;
    if (visual === 'bar-background') out += `<rect x="${x}" y="58" width="28" height="158" rx="8" fill="#e2e8f0"/>`;
    if (visual === 'stacked-bar') {
      const h1 = 40 + r() * 70, h2 = 28 + r() * 62, h3 = 18 + r() * 36;
      out += `<rect x="${x}" y="${216 - h1}" width="30" height="${h1}" rx="7" fill="#2563eb"/><rect x="${x}" y="${216 - h1 - h2}" width="30" height="${h2}" rx="7" fill="#14b8a6"/><rect x="${x}" y="${216 - h1 - h2 - h3}" width="30" height="${h3}" rx="7" fill="#f59e0b"/>`;
    } else if (visual === 'grouped-bar') {
      const h1 = 40 + r() * 120, h2 = 40 + r() * 110;
      out += `<rect x="${x - 8}" y="${216 - h1}" width="18" height="${h1}" rx="5" fill="#2563eb"/><rect x="${x + 13}" y="${216 - h2}" width="18" height="${h2}" rx="5" fill="#14b8a6"/>`;
    } else if (visual === 'pictorial-bar') {
      const count = 3 + Math.floor(r() * 5);
      for (let j = 0; j < count; j++) out += `<circle cx="${x + 14}" cy="${204 - j * 22}" r="9" fill="${palette[i % palette.length]}" opacity=".88"/>`;
    } else {
      const h = 44 + r() * 136;
      out += `<rect x="${x}" y="${216 - h}" width="30" height="${h}" rx="8" fill="${palette[i % palette.length]}"/>`;
    }
  }
  return out;
}

function drawHorizontalBars(seed, race = false) {
  const r = rnd(seed);
  let out = '';
  for (let i = 0; i < 7; i++) {
    const y = 26 + i * 28;
    const w = 120 + r() * 280;
    out += `<text x="48" y="${y + 14}" font-size="11" fill="${muted}">Item ${i + 1}</text><rect x="106" y="${y}" width="${w}" height="18" rx="8" fill="${palette[i % palette.length]}" opacity="${race && i > 3 ? .45 : .9}"/>`;
  }
  return out;
}

function drawWaterfall(seed) {
  const r = rnd(seed);
  let base = 146;
  let out = chartFrame();
  for (let i = 0; i < 7; i++) {
    const delta = (r() - .45) * 78;
    const next = Math.max(42, Math.min(206, base - delta));
    const y = Math.min(base, next);
    out += `<rect x="${72 + i * 54}" y="${y}" width="32" height="${Math.abs(next - base)}" rx="6" fill="${delta >= 0 ? '#14b8a6' : '#ef4444'}"/><line x1="${104 + i * 54}" y1="${next}" x2="${126 + i * 54}" y2="${next}" stroke="#94a3b8" stroke-dasharray="3 3"/>`;
    base = next;
  }
  return out;
}

function polar(cx, cy, r, a) {
  const rad = (a - 90) * Math.PI / 180;
  return [cx + r * Math.cos(rad), cy + r * Math.sin(rad)];
}

function arcPath(cx, cy, r0, r1, start, end) {
  const [a0x, a0y] = polar(cx, cy, r1, start), [a1x, a1y] = polar(cx, cy, r1, end);
  const [b1x, b1y] = polar(cx, cy, r0, end), [b0x, b0y] = polar(cx, cy, r0, start);
  const large = end - start > 180 ? 1 : 0;
  return `M ${a0x} ${a0y} A ${r1} ${r1} 0 ${large} 1 ${a1x} ${a1y} L ${b1x} ${b1y} A ${r0} ${r0} 0 ${large} 0 ${b0x} ${b0y} Z`;
}

function drawPie(visual, seed) {
  const r = rnd(seed);
  const cx = 250, cy = 118;
  const vals = [30 + r() * 80, 40 + r() * 70, 20 + r() * 60, 28 + r() * 60, 14 + r() * 40];
  const sum = vals.reduce((a, b) => a + b, 0);
  let angle = -60, out = '';
  for (let i = 0; i < vals.length; i++) {
    const span = vals[i] / sum * 360;
    const outer = visual.includes('rose') ? 58 + vals[i] / Math.max(...vals) * 72 : 104;
    const inner = visual === 'pie' || visual.startsWith('rose') ? 0 : visual === 'nested-donut' ? 52 + (i % 2) * 18 : 52;
    out += `<path d="${arcPath(cx, cy, inner, outer, angle, angle + span - 2)}" fill="${palette[i]}" opacity=".9"/>`;
    angle += span;
  }
  if (visual === 'nested-donut') out += `<circle cx="${cx}" cy="${cy}" r="42" fill="#fff"/>`;
  return out;
}

function drawRadar(seed) {
  const r = rnd(seed), cx = 250, cy = 120, radius = 94, axes = 6;
  let out = '';
  for (let ring = 1; ring <= 4; ring++) {
    const pts = Array.from({length: axes}, (_, i) => polar(cx, cy, radius * ring / 4, i * 360 / axes)).map((p) => p.join(',')).join(' ');
    out += `<polygon points="${pts}" fill="none" stroke="${grid}"/>`;
  }
  for (let i = 0; i < axes; i++) {
    const [x, y] = polar(cx, cy, radius, i * 360 / axes);
    out += `<line x1="${cx}" y1="${cy}" x2="${x}" y2="${y}" stroke="${grid}"/>`;
  }
  for (const color of ['#2563eb', '#14b8a6']) {
    const pts = Array.from({length: axes}, (_, i) => polar(cx, cy, radius * (.42 + r() * .5), i * 360 / axes)).map((p) => p.join(',')).join(' ');
    out += `<polygon points="${pts}" fill="${color}" fill-opacity=".16" stroke="${color}" stroke-width="3"/>`;
  }
  return out;
}

function drawPolar(visual, seed) {
  const r = rnd(seed), cx = 250, cy = 120;
  let out = `<circle cx="${cx}" cy="${cy}" r="98" fill="none" stroke="${grid}"/><circle cx="${cx}" cy="${cy}" r="62" fill="none" stroke="${grid}"/><circle cx="${cx}" cy="${cy}" r="28" fill="none" stroke="${grid}"/>`;
  if (visual === 'polar-line') {
    const pts = Array.from({length: 14}, (_, i) => polar(cx, cy, 40 + r() * 70, i * 360 / 14)).map((p) => p.join(' '));
    out += `<path d="M ${pts.join(' L ')} Z" fill="none" stroke="#2563eb" stroke-width="4"/>`;
  } else {
    for (let i = 0; i < 10; i++) out += `<path d="${arcPath(cx, cy, 36, 48 + r() * 66, i * 36 + 2, i * 36 + 30)}" fill="${palette[i % palette.length]}" opacity=".85"/>`;
  }
  return out;
}

function drawGauge(visual) {
  const cx = 250, cy = 164;
  const out = `<path d="${arcPath(cx, cy, 76, 96, -120, 120)}" fill="#e2e8f0"/><path d="${arcPath(cx, cy, 76, 96, -120, 42)}" fill="url(#accent-${visual})"/><line x1="${cx}" y1="${cy}" x2="${cx + 74}" y2="${cy - 42}" stroke="#0f172a" stroke-width="5" stroke-linecap="round"/><circle cx="${cx}" cy="${cy}" r="9" fill="#0f172a"/><text x="${cx}" y="${cy + 42}" text-anchor="middle" font-size="28" font-weight="700" fill="#0f172a">72%</text>`;
  return out.replaceAll(`url(#accent-${visual})`, '#14b8a6');
}

function drawLiquid(id) {
  return `<circle cx="250" cy="124" r="92" fill="#dbeafe" stroke="#2563eb" stroke-width="4"/><clipPath id="clip-${id}"><circle cx="250" cy="124" r="88"/></clipPath><g clip-path="url(#clip-${id})"><rect x="156" y="130" width="188" height="90" fill="#2563eb" opacity=".78"/><path d="M 156 132 C 190 108 218 154 252 130 C 288 106 320 150 344 126 L 344 220 L 156 220 Z" fill="#14b8a6" opacity=".62"/></g><text x="250" y="134" text-anchor="middle" font-size="30" font-weight="800" fill="#0f172a">64%</text>`;
}

function drawScatter(visual, seed) {
  const r = rnd(seed);
  let out = chartFrame();
  for (let i = 0; i < 32; i++) {
    const x = 62 + r() * 386, y = 42 + r() * 158;
    const size = visual === 'bubble' ? 5 + r() * 16 : 6;
    out += `<circle cx="${x}" cy="${y}" r="${size}" fill="${palette[i % palette.length]}" opacity=".78"/>`;
    if (visual === 'effect-scatter' && i % 9 === 0) out += `<circle cx="${x}" cy="${y}" r="${size + 10}" fill="none" stroke="#ef4444" stroke-width="2" opacity=".45"/>`;
  }
  return out;
}

function drawBoxplot(seed) {
  const r = rnd(seed);
  let out = chartFrame();
  for (let i = 0; i < 5; i++) {
    const x = 90 + i * 72, y1 = 52 + r() * 42, y2 = 128 + r() * 44, mid = (y1 + y2) / 2;
    out += `<line x1="${x}" y1="${y1 - 22}" x2="${x}" y2="${y2 + 28}" stroke="#0f172a"/><rect x="${x - 20}" y="${y1}" width="40" height="${y2 - y1}" fill="#bfdbfe" stroke="#2563eb" stroke-width="3"/><line x1="${x - 22}" y1="${mid}" x2="${x + 22}" y2="${mid}" stroke="#ef4444" stroke-width="3"/>`;
  }
  return out;
}

function drawViolin(seed) {
  const r = rnd(seed); let out = chartFrame();
  for (let i = 0; i < 4; i++) {
    const x = 118 + i * 88, w = 22 + r() * 18;
    out += `<path d="M ${x} 44 C ${x - w} 76 ${x - w - 8} 126 ${x} 202 C ${x + w + 8} 126 ${x + w} 76 ${x} 44 Z" fill="${palette[i]}" opacity=".35" stroke="${palette[i]}" stroke-width="3"/><line x1="${x}" y1="50" x2="${x}" y2="198" stroke="#0f172a" opacity=".5"/>`;
  }
  return out;
}

function drawHistogram(seed) {
  const r = rnd(seed); let out = chartFrame();
  for (let i = 0; i < 16; i++) {
    const h = 22 + Math.sin(i / 15 * Math.PI) * 134 + r() * 20;
    out += `<rect x="${58 + i * 24}" y="${216 - h}" width="20" height="${h}" fill="#2563eb" opacity=".74"/>`;
  }
  return out;
}

function drawFinance(visual, seed) {
  const r = rnd(seed); let out = chartFrame();
  for (let i = 0; i < 12; i++) {
    const x = 64 + i * 32, high = 42 + r() * 50, low = 142 + r() * 62, open = high + 20 + r() * 60, close = high + 20 + r() * 60;
    const up = close < open;
    if (visual === 'ohlc') out += `<line x1="${x}" y1="${high}" x2="${x}" y2="${low}" stroke="${up ? '#14b8a6' : '#ef4444'}" stroke-width="2"/><line x1="${x - 10}" y1="${open}" x2="${x}" y2="${open}" stroke="${up ? '#14b8a6' : '#ef4444'}" stroke-width="2"/><line x1="${x}" y1="${close}" x2="${x + 10}" y2="${close}" stroke="${up ? '#14b8a6' : '#ef4444'}" stroke-width="2"/>`;
    else out += `<line x1="${x}" y1="${high}" x2="${x}" y2="${low}" stroke="#0f172a"/><rect x="${x - 9}" y="${Math.min(open, close)}" width="18" height="${Math.abs(close - open) + 2}" fill="${up ? '#14b8a6' : '#ef4444'}"/>`;
    if (visual === 'volume') out += `<rect x="${x - 8}" y="${228 - r() * 42}" width="16" height="${24 + r() * 34}" fill="#94a3b8" opacity=".45"/>`;
  }
  return out;
}

function drawHeatmap(visual, seed) {
  const r = rnd(seed); let out = '';
  const rows = visual === 'calendar-heatmap' ? 7 : 6, cols = visual === 'calendar-heatmap' ? 18 : 12;
  for (let y = 0; y < rows; y++) for (let x = 0; x < cols; x++) {
    const v = r();
    const color = v < .33 ? '#dbeafe' : v < .66 ? '#60a5fa' : '#1d4ed8';
    out += `<rect x="${50 + x * 32}" y="${24 + y * 28}" width="24" height="20" rx="5" fill="${color}" opacity="${visual === 'matrix' ? .35 + v * .6 : .9}"/>`;
    if (visual === 'matrix') out += `<circle cx="${62 + x * 32}" cy="${34 + y * 28}" r="${3 + v * 8}" fill="#14b8a6" opacity=".75"/>`;
  }
  return out;
}

function drawGraph(visual, seed) {
  const r = rnd(seed), cx = 250, cy = 120;
  const nodes = Array.from({length: 9}, (_, i) => visual === 'graph-circular' ? polar(cx, cy, 86, i * 40) : [78 + r() * 340, 32 + r() * 170]);
  let out = '';
  for (let i = 0; i < nodes.length; i++) for (let j = i + 1; j < nodes.length; j++) if ((i + j + seed) % 4 === 0) out += `<line x1="${nodes[i][0]}" y1="${nodes[i][1]}" x2="${nodes[j][0]}" y2="${nodes[j][1]}" stroke="#94a3b8" opacity=".55"/>`;
  nodes.forEach(([x, y], i) => out += `<circle cx="${x}" cy="${y}" r="${9 + (i % 3) * 4}" fill="${palette[i % palette.length]}"/>`);
  return out;
}

function drawTree(visual) {
  if (visual === 'radial-tree') return drawGraph('graph-circular', 42);
  return `<line x1="250" y1="34" x2="160" y2="100" stroke="#94a3b8"/><line x1="250" y1="34" x2="340" y2="100" stroke="#94a3b8"/><line x1="160" y1="100" x2="110" y2="176" stroke="#94a3b8"/><line x1="160" y1="100" x2="210" y2="176" stroke="#94a3b8"/><line x1="340" y1="100" x2="300" y2="176" stroke="#94a3b8"/><line x1="340" y1="100" x2="390" y2="176" stroke="#94a3b8"/>${[[250,34],[160,100],[340,100],[110,176],[210,176],[300,176],[390,176]].map(([x,y],i)=>`<rect x="${x-28}" y="${y-14}" width="56" height="28" rx="12" fill="${palette[i%palette.length]}"/>`).join('')}`;
}

function drawTreemap(seed) {
  const rects = [[52,24,190,120,'#2563eb'],[52,150,190,66,'#14b8a6'],[250,24,112,86,'#f59e0b'],[368,24,80,86,'#ef4444'],[250,118,198,98,'#8b5cf6']];
  return rects.map(([x,y,w,h,c])=>`<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="12" fill="${c}" opacity=".86"/><text x="${x+12}" y="${y+24}" font-size="13" fill="#fff" font-weight="700">${Math.round(w*h/100)}</text>`).join('');
}

function drawSunburst(seed) {
  let out = '';
  for (let ring = 0; ring < 3; ring++) for (let i = 0; i < 7 + ring * 2; i++) out += `<path d="${arcPath(250, 120, 26 + ring * 34, 55 + ring * 34, i * (360 / (7 + ring * 2)), (i + .82) * (360 / (7 + ring * 2)))}" fill="${palette[(i + ring) % palette.length]}" opacity=".82"/>`;
  return out;
}

function drawSankey() {
  return `<rect x="60" y="46" width="34" height="148" rx="8" fill="#2563eb"/><rect x="414" y="34" width="34" height="72" rx="8" fill="#14b8a6"/><rect x="414" y="128" width="34" height="82" rx="8" fill="#f59e0b"/><path d="M 94 70 C 190 70 292 54 414 62" fill="none" stroke="#2563eb" stroke-width="34" opacity=".25"/><path d="M 94 150 C 190 150 292 174 414 168" fill="none" stroke="#f59e0b" stroke-width="44" opacity=".28"/>`;
}

function drawFunnel() {
  const widths = [360, 300, 236, 174, 106];
  return widths.map((w, i) => `<path d="M ${250-w/2} ${28+i*38} L ${250+w/2} ${28+i*38} L ${250+widths[Math.min(i+1,4)]/2} ${60+i*38} L ${250-widths[Math.min(i+1,4)]/2} ${60+i*38} Z" fill="${palette[i]}" opacity=".86"/>`).join('');
}

function drawRiver(seed, id) {
  return `<path d="M 52 150 C 130 80 188 170 264 104 C 330 50 380 128 450 78 L 450 188 C 372 216 318 166 262 206 C 176 254 116 192 52 222 Z" fill="url(#accent-${id})" opacity=".55"/><path d="M 52 104 C 128 64 194 132 268 78 C 340 28 392 80 450 48 L 450 96 C 380 126 328 82 268 124 C 188 180 130 104 52 152 Z" fill="#8b5cf6" opacity=".32"/>`;
}

function drawParallel(seed) {
  const r = rnd(seed); let out = '';
  for (let i = 0; i < 5; i++) out += `<line x1="${70+i*90}" y1="28" x2="${70+i*90}" y2="210" stroke="#94a3b8"/>`;
  for (let row = 0; row < 8; row++) {
    const pts = Array.from({length:5},(_,i)=>`${70+i*90},${42+r()*150}`).join(' ');
    out += `<polyline points="${pts}" fill="none" stroke="${palette[row%palette.length]}" stroke-width="2" opacity=".65"/>`;
  }
  return out;
}

function drawMap(visual, seed) {
  let out = `<path d="M 92 86 L 156 42 L 248 58 L 294 104 L 262 164 L 180 188 L 114 154 Z" fill="#bfdbfe" stroke="#fff" stroke-width="3"/><path d="M 286 70 L 380 44 L 440 92 L 410 174 L 326 162 Z" fill="#99f6e4" stroke="#fff" stroke-width="3"/><path d="M 172 194 L 286 176 L 382 210 L 320 244 L 214 232 Z" fill="#fde68a" stroke="#fff" stroke-width="3"/>`;
  if (visual === 'map-bubble' || visual === 'geo-heatmap') out += `<circle cx="190" cy="114" r="24" fill="#ef4444" opacity=".55"/><circle cx="344" cy="104" r="16" fill="#2563eb" opacity=".6"/><circle cx="292" cy="208" r="28" fill="#f59e0b" opacity=".5"/>`;
  if (visual === 'geo-lines' || visual === 'route-map') out += `<path d="M 170 120 C 230 46 324 48 380 120" fill="none" stroke="#2563eb" stroke-width="4" stroke-linecap="round"/><path d="M 202 182 C 260 236 326 234 386 150" fill="none" stroke="#ef4444" stroke-width="4" stroke-linecap="round"/>`;
  if (visual === 'map-multiples') out += `<g transform="translate(300 152) scale(.42)">${out}</g>`;
  return out;
}

function drawCalendar(visual, seed) {
  const r = rnd(seed); let out = '';
  for (let row = 0; row < 6; row++) for (let col = 0; col < 12; col++) {
    const active = visual === 'calendar-range' ? col > 2 && col < 8 && row > 1 && row < 5 : r() > .28;
    out += `<rect x="${58+col*32}" y="${32+row*30}" width="24" height="22" rx="5" fill="${active ? palette[(row+col)%palette.length] : '#e2e8f0'}" opacity="${active ? .82 : .7}"/>`;
  }
  return out;
}

function drawTimeline(visual, seed) {
  const r = rnd(seed); let out = `<line x1="60" y1="118" x2="454" y2="118" stroke="#94a3b8" stroke-width="3"/>`;
  for (let i = 0; i < 8; i++) {
    const x = 74 + i*52;
    if (visual === 'gantt') out += `<rect x="${x}" y="${50+i%4*36}" width="${42+r()*90}" height="20" rx="8" fill="${palette[i%palette.length]}" opacity=".82"/>`;
    else out += `<circle cx="${x}" cy="118" r="9" fill="${palette[i%palette.length]}"/><line x1="${x}" y1="118" x2="${x}" y2="${60+r()*120}" stroke="${palette[i%palette.length]}" opacity=".5"/>`;
  }
  return out;
}

function drawCustom(seed) {
  return `<path d="M 74 176 C 124 48 172 224 236 82 S 350 190 428 64" fill="none" stroke="#2563eb" stroke-width="8" stroke-linecap="round"/><rect x="116" y="92" width="90" height="90" rx="22" fill="#14b8a6" opacity=".36"/><circle cx="330" cy="138" r="62" fill="#f59e0b" opacity=".34"/><path d="M 360 84 l 78 52 l -78 52 z" fill="#8b5cf6" opacity=".5"/>`;
}

function drawWordcloud(seed) {
  const words = [['Rust',36,'#2563eb',146,90],['Fission',34,'#14b8a6',250,132],['Charts',30,'#f59e0b',320,76],['GPU',26,'#ef4444',208,174],['State',22,'#8b5cf6',100,150],['Data',24,'#22c55e',374,172],['Host',18,'#0ea5e9',296,196]];
  return words.map(([w,s,c,x,y])=>`<text x="${x}" y="${y}" text-anchor="middle" font-size="${s}" font-weight="800" fill="${c}">${w}</text>`).join('');
}

function draw3d(visual, seed, id) {
  const r = rnd(seed);
  if (visual === 'scatter3d' || visual === 'point-cloud' || visual === 'large-scatter') {
    let out = perspectiveAxes();
    const count = visual === 'point-cloud' || visual === 'large-scatter' ? 70 : 24;
    for (let i = 0; i < count; i++) {
      const depth = r();
      const x = 92 + r() * 320 + depth * 58;
      const y = 58 + r() * 132 - depth * 34;
      const size = visual === 'large-scatter' ? 2.8 + r() * 3.4 : 4 + depth * 9;
      out += `<circle cx="${x}" cy="${y}" r="${size}" fill="${palette[i % palette.length]}" opacity="${0.42 + depth * 0.48}"/>`;
    }
    return out;
  }
  if (visual === 'line3d') {
    const pts = Array.from({length: 11}, (_, i) => {
      const depth = i / 10;
      return [82 + i * 36 + depth * 42, 164 - Math.sin(i * .82) * 56 - depth * 42];
    });
    return `${perspectiveAxes()}<path d="M ${pts.map((p) => p.join(' ')).join(' L ')}" fill="none" stroke="#2563eb" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/>${pts.map(([x,y],i)=>`<circle cx="${x}" cy="${y}" r="${5+i*.35}" fill="#fff" stroke="${palette[i%palette.length]}" stroke-width="3"/>`).join('')}`;
  }
  if (visual.includes('globe')) {
    let out = `<circle cx="250" cy="126" r="102" fill="#dbeafe" stroke="#2563eb" stroke-width="4"/><ellipse cx="250" cy="126" rx="102" ry="34" fill="none" stroke="#60a5fa"/><path d="M 172 92 C 222 42 288 56 332 92 C 286 118 226 118 172 92 Z" fill="#14b8a6" opacity=".58"/><path d="M 208 160 C 260 138 318 144 356 176 C 308 206 244 202 208 160 Z" fill="#f59e0b" opacity=".5"/>`;
    if (visual === 'globe-bars') for (let i=0;i<10;i++){const [x,y]=polar(250,126,50+r()*44,r()*360); out+=`<line x1="${x}" y1="${y}" x2="${x}" y2="${y-18-r()*46}" stroke="#ef4444" stroke-width="5" stroke-linecap="round"/>`;}
    if (visual === 'globe-lines') out += `<path d="M 174 118 C 230 30 304 28 360 110" fill="none" stroke="#ef4444" stroke-width="4"/><path d="M 184 164 C 252 90 316 98 344 164" fill="none" stroke="#8b5cf6" stroke-width="4"/>`;
    return out;
  }
  if (visual === 'surface3d' || visual === 'terrain') {
    let out = '';
    for (let y=0;y<7;y++) for (let x=0;x<9;x++) {
      const px=80+x*38+y*18, py=58+y*20-r()*18;
      out+=`<polygon points="${px},${py} ${px+38},${py+8} ${px+56},${py+28} ${px+18},${py+20}" fill="${palette[(x+y)%palette.length]}" opacity=".58" stroke="#fff"/>`;
    }
    return out;
  }
  if (visual === 'bar3d' || visual === 'map3d') {
    let out = '';
    for (let y=0;y<4;y++) for (let x=0;x<7;x++) {
      const h=20+r()*86, px=86+x*50+y*22, py=188-y*18;
      out += isoBar(px, py, h, palette[(x+y)%palette.length]);
    }
    return out;
  }
  if (visual === 'graph3d') return drawGraph('graph', seed) + `<path d="M 120 188 L 250 42 L 410 174" fill="none" stroke="#0f172a" stroke-dasharray="4 5" opacity=".25"/>`;
  if (visual === 'volume') return `<rect x="164" y="54" width="156" height="156" fill="#2563eb" opacity=".12" stroke="#2563eb"/><rect x="198" y="28" width="156" height="156" fill="#14b8a6" opacity=".12" stroke="#14b8a6"/><line x1="164" y1="54" x2="198" y2="28" stroke="#94a3b8"/><line x1="320" y1="54" x2="354" y2="28" stroke="#94a3b8"/><line x1="320" y1="210" x2="354" y2="184" stroke="#94a3b8"/>`;
  if (visual === 'vector-field' || visual === 'flow') {
    let out = '';
    for(let y=0;y<6;y++) for(let x=0;x<9;x++){const px=70+x*44,py=44+y*30,ang=r()*Math.PI*2,len=16+r()*14; out+=`<line x1="${px}" y1="${py}" x2="${px+Math.cos(ang)*len}" y2="${py+Math.sin(ang)*len}" stroke="${palette[(x+y)%palette.length]}" stroke-width="3" stroke-linecap="round"/>`;}
    return out;
  }
  if (visual === 'mesh') return `<polygon points="96,184 178,54 286,120 376,42 444,188" fill="#bfdbfe" stroke="#2563eb" stroke-width="3"/><path d="M178 54 L286 120 L96 184 M286 120 L444 188 M286 120 L376 42" fill="none" stroke="#fff" stroke-width="2"/>`;
  return drawScatter('bubble', seed);
}

function perspectiveAxes() {
  return `<path d="M 74 206 L 420 206 L 486 162 M 74 206 L 136 62 M 136 62 L 486 62 L 486 162" fill="none" stroke="#94a3b8" stroke-width="2" opacity=".65"/><path d="M 74 158 L 420 158 L 486 114 M 106 110 L 454 110" fill="none" stroke="#dbe4ef" stroke-width="1"/>`;
}

function isoBar(x, y, h, color) {
  return `<polygon points="${x},${y-h} ${x+18},${y-h-10} ${x+38},${y-h} ${x+20},${y-h+10}" fill="${color}"/><polygon points="${x},${y-h} ${x+20},${y-h+10} ${x+20},${y+10} ${x},${y}" fill="${color}" opacity=".72"/><polygon points="${x+20},${y-h+10} ${x+38},${y-h} ${x+38},${y} ${x+20},${y+10}" fill="${color}" opacity=".5"/>`;
}

fs.mkdirSync(imageDir, {recursive: true});
for (const entry of entries) {
  fs.writeFileSync(path.join(imageDir, `${entry.slug}.svg`), svg(entry));
}

const serializable = entries.map(({visual, ...entry}) => ({
  ...entry,
  image: `/img/charts/${entry.slug}.svg`,
}));

const ts = `// Generated by website/scripts/generate-chart-assets.mjs. Edit the script, then run npm run charts:generate.\n\nexport type ChartStatus = 'available' | 'next' | 'planned';\n\nexport interface ChartCatalogEntry {\n  slug: string;\n  title: string;\n  family: string;\n  status: ChartStatus;\n  description: string;\n  dataShape: string;\n  useWhen: string;\n  tags: string[];\n  image: string;\n}\n\nexport const chartCatalog: ChartCatalogEntry[] = ${JSON.stringify(serializable, null, 2)};\n\nexport const chartFamilies = Array.from(new Set(chartCatalog.map((chart) => chart.family)));\n\nexport const featuredChartPreviews = ['line-stacked-area', 'pie-donut', 'heatmap-cartesian', 'sankey-basic', 'scatter3d-basic']\n  .map((slug) => chartCatalog.find((chart) => chart.slug === slug))\n  .filter((chart): chart is ChartCatalogEntry => Boolean(chart));\n`;

fs.writeFileSync(dataFile, ts);
console.log(`Generated ${entries.length} chart previews and ${path.relative(websiteRoot, dataFile)}`);
