// Real Fission chart screenshot catalog. Images are captured from examples/chart-gallery in FISSION_CHART_DOC_SLUG mode.

export interface ChartCatalogEntry {
  slug: string;
  title: string;
  family: string;
  description: string;
  dataShape: string;
  useWhen: string;
  tags: string[];
  image: string;
}

export const chartCatalog: ChartCatalogEntry[] =
[
  {
    "slug": "line-basic",
    "title": "Basic line",
    "family": "Cartesian",
    "description": "A single ordered numeric series with a clear trend line.",
    "dataShape": "Vec<f32> on a category, value, or time axis.",
    "useWhen": "Use it when the shape of change matters more than individual bars.",
    "tags": [
      "line",
      "axis"
    ],
    "image": "/img/charts/line-basic.png"
  },
  {
    "slug": "line-smooth",
    "title": "Smooth line",
    "family": "Cartesian",
    "description": "A continuous line with curve interpolation for softer trend reading.",
    "dataShape": "Vec<f32> with smooth interpolation enabled.",
    "useWhen": "Use it for dashboards where the series is sampled often enough to justify interpolation.",
    "tags": [
      "line",
      "smooth"
    ],
    "image": "/img/charts/line-smooth.png"
  },
  {
    "slug": "line-step",
    "title": "Step line",
    "family": "Cartesian",
    "description": "A line that changes in discrete jumps instead of continuous slopes.",
    "dataShape": "Vec<f32> with start, middle, or end step behavior.",
    "useWhen": "Use it for counters, states, quotas, and event-driven changes.",
    "tags": [
      "line",
      "step"
    ],
    "image": "/img/charts/line-step.png"
  },
  {
    "slug": "line-area",
    "title": "Area line",
    "family": "Cartesian",
    "description": "A line with filled area to emphasize magnitude over time.",
    "dataShape": "Vec<f32> plus area style.",
    "useWhen": "Use it when total volume is as important as the outline.",
    "tags": [
      "line",
      "area"
    ],
    "image": "/img/charts/line-area.png"
  },
  {
    "slug": "line-stacked-area",
    "title": "Stacked area",
    "family": "Cartesian",
    "description": "Multiple area series stacked into one total.",
    "dataShape": "Several line series sharing one stack key.",
    "useWhen": "Use it to show composition over time without losing the total.",
    "tags": [
      "line",
      "stack"
    ],
    "image": "/img/charts/line-stacked-area.png"
  },
  {
    "slug": "bar-basic",
    "title": "Basic bar",
    "family": "Cartesian",
    "description": "A category comparison using rectangular bars.",
    "dataShape": "Vec<f32> aligned to category labels.",
    "useWhen": "Use it when individual values need easy comparison.",
    "tags": [
      "bar",
      "category"
    ],
    "image": "/img/charts/bar-basic.png"
  },
  {
    "slug": "bar-grouped",
    "title": "Grouped bar",
    "family": "Cartesian",
    "description": "Several bar series shown side by side for each category.",
    "dataShape": "Multiple bar series sharing the same category axis.",
    "useWhen": "Use it to compare related measures within each group.",
    "tags": [
      "bar",
      "grouped"
    ],
    "image": "/img/charts/bar-grouped.png"
  },
  {
    "slug": "bar-stacked",
    "title": "Stacked bar",
    "family": "Cartesian",
    "description": "Bars stacked into a cumulative total for each category.",
    "dataShape": "Multiple bar series sharing one stack key.",
    "useWhen": "Use it to show composition and total at the same time.",
    "tags": [
      "bar",
      "stack"
    ],
    "image": "/img/charts/bar-stacked.png"
  },
  {
    "slug": "pictorial-bar",
    "title": "Pictorial bar",
    "family": "Cartesian",
    "description": "Bars represented by repeated symbols or a custom path.",
    "dataShape": "Numeric values plus symbol choice.",
    "useWhen": "Use it when the chart should feel branded without giving up scale.",
    "tags": [
      "bar",
      "symbol"
    ],
    "image": "/img/charts/pictorial-bar.png"
  },
  {
    "slug": "pie-basic",
    "title": "Pie",
    "family": "Radial and polar",
    "description": "A circular part-to-whole chart.",
    "dataShape": "Label/value pairs.",
    "useWhen": "Use it for a small number of categories where the whole matters.",
    "tags": [
      "pie",
      "part-to-whole"
    ],
    "image": "/img/charts/pie-basic.png"
  },
  {
    "slug": "pie-donut",
    "title": "Donut",
    "family": "Radial and polar",
    "description": "A pie chart with an open center for a total or primary label.",
    "dataShape": "Label/value pairs plus inner radius.",
    "useWhen": "Use it when the whole needs a central value or status.",
    "tags": [
      "pie",
      "donut"
    ],
    "image": "/img/charts/pie-donut.png"
  },
  {
    "slug": "pie-rose-radius",
    "title": "Rose by radius",
    "family": "Radial and polar",
    "description": "Slices use radius to make category differences more expressive.",
    "dataShape": "Label/value pairs plus radius rose mode.",
    "useWhen": "Use it for presentation-heavy part-to-whole views.",
    "tags": [
      "pie",
      "rose"
    ],
    "image": "/img/charts/pie-rose-radius.png"
  },
  {
    "slug": "pie-rose-area",
    "title": "Rose by area",
    "family": "Radial and polar",
    "description": "Slices use area-oriented radius scaling for a softer rose chart.",
    "dataShape": "Label/value pairs plus area rose mode.",
    "useWhen": "Use it when rose slices should emphasize difference without making small values disappear.",
    "tags": [
      "pie",
      "rose"
    ],
    "image": "/img/charts/pie-rose-area.png"
  },
  {
    "slug": "radar-basic",
    "title": "Radar",
    "family": "Radial and polar",
    "description": "Multiple metrics plotted around a circular axis set.",
    "dataShape": "Vec<Vec<f32>> where each row is one profile.",
    "useWhen": "Use it for profile comparison across a fixed set of dimensions.",
    "tags": [
      "radar",
      "profile"
    ],
    "image": "/img/charts/radar-basic.png"
  },
  {
    "slug": "radar-filled",
    "title": "Filled radar",
    "family": "Radial and polar",
    "description": "A radar chart with filled polygons for easier shape comparison.",
    "dataShape": "Metric profiles plus fill style.",
    "useWhen": "Use it when profile area and overlap should be visible.",
    "tags": [
      "radar",
      "profile"
    ],
    "image": "/img/charts/radar-filled.png"
  },
  {
    "slug": "gauge-basic",
    "title": "Gauge",
    "family": "Radial and polar",
    "description": "A dial-style chart for one bounded measure.",
    "dataShape": "One label/value pair with an expected range.",
    "useWhen": "Use it when the value is read as a current instrument state.",
    "tags": [
      "gauge",
      "status"
    ],
    "image": "/img/charts/gauge-basic.png"
  },
  {
    "slug": "gauge-progress",
    "title": "Progress gauge",
    "family": "Radial and polar",
    "description": "A gauge emphasizing completed amount rather than a raw number.",
    "dataShape": "One or more bounded values.",
    "useWhen": "Use it for operational progress and service health panels.",
    "tags": [
      "gauge",
      "progress"
    ],
    "image": "/img/charts/gauge-progress.png"
  },
  {
    "slug": "liquid-fill",
    "title": "Liquid fill",
    "family": "Radial and polar",
    "description": "A circular fill indicator with a wave-shaped level.",
    "dataShape": "One or more percentages.",
    "useWhen": "Use it for capacity, completion, or quota states.",
    "tags": [
      "liquid",
      "status"
    ],
    "image": "/img/charts/liquid-fill.png"
  },
  {
    "slug": "scatter-basic",
    "title": "Scatter",
    "family": "Statistical and finance",
    "description": "Points plotted by two numeric dimensions.",
    "dataShape": "Vec<(f32, f32)>.",
    "useWhen": "Use it to find relationship, clustering, or outliers.",
    "tags": [
      "scatter",
      "correlation"
    ],
    "image": "/img/charts/scatter-basic.png"
  },
  {
    "slug": "scatter-effect",
    "title": "Effect scatter",
    "family": "Statistical and finance",
    "description": "Scatter points with emphasis rings for important observations.",
    "dataShape": "Vec<(f32, f32)> plus emphasis styling.",
    "useWhen": "Use it to mark active locations, alerts, or selected results.",
    "tags": [
      "scatter",
      "effect"
    ],
    "image": "/img/charts/scatter-effect.png"
  },
  {
    "slug": "boxplot-basic",
    "title": "Boxplot",
    "family": "Statistical and finance",
    "description": "A distribution summary showing min, quartiles, median, and max.",
    "dataShape": "Rows of five-number summaries or raw groups.",
    "useWhen": "Use it when distribution matters more than a single average.",
    "tags": [
      "statistics",
      "distribution"
    ],
    "image": "/img/charts/boxplot-basic.png"
  },
  {
    "slug": "candlestick-basic",
    "title": "Candlestick",
    "family": "Statistical and finance",
    "description": "Open, close, low, and high values drawn as market candles.",
    "dataShape": "Rows of open, close, low, high values.",
    "useWhen": "Use it for finance and other range-over-time data.",
    "tags": [
      "finance",
      "ohlc"
    ],
    "image": "/img/charts/candlestick-basic.png"
  },
  {
    "slug": "heatmap-cartesian",
    "title": "Cartesian heatmap",
    "family": "Statistical and finance",
    "description": "A rectangular value matrix rendered with a color scale.",
    "dataShape": "x index, y index, and value triples.",
    "useWhen": "Use it for density, activity, and matrix-style comparison.",
    "tags": [
      "heatmap",
      "matrix"
    ],
    "image": "/img/charts/heatmap-cartesian.png"
  },
  {
    "slug": "graph-force",
    "title": "Force graph",
    "family": "Relationships and hierarchy",
    "description": "Nodes and edges arranged into a readable network.",
    "dataShape": "Node list plus edge list.",
    "useWhen": "Use it for dependency, social, and topology diagrams.",
    "tags": [
      "graph",
      "network"
    ],
    "image": "/img/charts/graph-force.png"
  },
  {
    "slug": "tree-basic",
    "title": "Tree",
    "family": "Relationships and hierarchy",
    "description": "A rooted hierarchy drawn with parent-child links.",
    "dataShape": "Nested nodes with optional values.",
    "useWhen": "Use it for navigation structures, ownership trees, and dependency drilldown.",
    "tags": [
      "tree",
      "hierarchy"
    ],
    "image": "/img/charts/tree-basic.png"
  },
  {
    "slug": "tree-radial",
    "title": "Radial tree",
    "family": "Relationships and hierarchy",
    "description": "A tree layout arranged around a circle.",
    "dataShape": "Nested nodes with optional values.",
    "useWhen": "Use it when the hierarchy is shallow and symmetry matters more than linear reading.",
    "tags": [
      "tree",
      "radial"
    ],
    "image": "/img/charts/tree-radial.png"
  },
  {
    "slug": "treemap-basic",
    "title": "Treemap",
    "family": "Relationships and hierarchy",
    "description": "Hierarchical values packed into rectangles.",
    "dataShape": "Nested nodes with values.",
    "useWhen": "Use it for storage, budgets, or part-to-whole hierarchy.",
    "tags": [
      "treemap",
      "hierarchy"
    ],
    "image": "/img/charts/treemap-basic.png"
  },
  {
    "slug": "sunburst-basic",
    "title": "Sunburst",
    "family": "Relationships and hierarchy",
    "description": "A hierarchy drawn as concentric rings.",
    "dataShape": "Nested nodes with values.",
    "useWhen": "Use it when hierarchy depth should remain visible.",
    "tags": [
      "sunburst",
      "hierarchy"
    ],
    "image": "/img/charts/sunburst-basic.png"
  },
  {
    "slug": "sankey-basic",
    "title": "Sankey",
    "family": "Relationships and hierarchy",
    "description": "Flow between stages using bands with width.",
    "dataShape": "Node list plus weighted edges.",
    "useWhen": "Use it for energy, revenue, and conversion flows.",
    "tags": [
      "sankey",
      "flow"
    ],
    "image": "/img/charts/sankey-basic.png"
  },
  {
    "slug": "funnel-basic",
    "title": "Funnel",
    "family": "Relationships and hierarchy",
    "description": "Stage values drawn as narrowing bands.",
    "dataShape": "Ordered label/value pairs.",
    "useWhen": "Use it for conversion stages and pipeline health.",
    "tags": [
      "funnel",
      "conversion"
    ],
    "image": "/img/charts/funnel-basic.png"
  },
  {
    "slug": "theme-river",
    "title": "Theme river",
    "family": "Relationships and hierarchy",
    "description": "Stacked flowing bands over time.",
    "dataShape": "Time, value, and category tuples.",
    "useWhen": "Use it when composition changes continuously over time.",
    "tags": [
      "stream",
      "time"
    ],
    "image": "/img/charts/theme-river.png"
  },
  {
    "slug": "parallel-basic",
    "title": "Parallel coordinates",
    "family": "Relationships and hierarchy",
    "description": "Rows drawn across multiple vertical axes.",
    "dataShape": "Vec<Vec<f32>> with one row per observation.",
    "useWhen": "Use it for high-dimensional filtering and comparison.",
    "tags": [
      "parallel",
      "dimensions"
    ],
    "image": "/img/charts/parallel-basic.png"
  },
  {
    "slug": "lines-basic",
    "title": "Lines",
    "family": "Geographic and route",
    "description": "Curved line segments with direction and optional emphasis.",
    "dataShape": "Line segments with from/to points and a value.",
    "useWhen": "Use it for routes, movement, and connection flow.",
    "tags": [
      "lines",
      "routes"
    ],
    "image": "/img/charts/lines-basic.png"
  },
  {
    "slug": "geo-lines",
    "title": "Geo lines",
    "family": "Geographic and route",
    "description": "Route lines drawn over a GeoJSON-backed map.",
    "dataShape": "GeoJSON regions plus route line segments.",
    "useWhen": "Use it when geography and movement need to be read together.",
    "tags": [
      "lines",
      "geo"
    ],
    "image": "/img/charts/geo-lines.png"
  },
  {
    "slug": "route-map",
    "title": "Route map",
    "family": "Geographic and route",
    "description": "A map-focused route view with animated-looking line emphasis.",
    "dataShape": "Regions plus route segments.",
    "useWhen": "Use it for logistics, traffic, or service coverage views.",
    "tags": [
      "map",
      "routes"
    ],
    "image": "/img/charts/route-map.png"
  },
  {
    "slug": "map-choropleth",
    "title": "Choropleth map",
    "family": "Geographic and route",
    "description": "Regions colored by a numeric value.",
    "dataShape": "Region identifiers plus values and GeoJSON geometry.",
    "useWhen": "Use it for geography-first comparison.",
    "tags": [
      "map",
      "geo"
    ],
    "image": "/img/charts/map-choropleth.png"
  },
  {
    "slug": "dataset-encoded",
    "title": "Encoded dataset",
    "family": "Data pipeline and interaction",
    "description": "Line and bar series bound to named dataset dimensions.",
    "dataShape": "Dataset rows, dimensions, and encode mappings.",
    "useWhen": "Use it when chart code should name fields instead of copying arrays.",
    "tags": [
      "dataset",
      "encode"
    ],
    "image": "/img/charts/dataset-encoded.png"
  },
  {
    "slug": "visual-map",
    "title": "Visual map",
    "family": "Data pipeline and interaction",
    "description": "Color encodes a numeric range consistently across a chart.",
    "dataShape": "Numeric values plus a color scale.",
    "useWhen": "Use it for heatmap and scatter intensity.",
    "tags": [
      "visual-map",
      "color"
    ],
    "image": "/img/charts/visual-map.png"
  },
  {
    "slug": "wordcloud",
    "title": "Word cloud",
    "family": "Data pipeline and interaction",
    "description": "Words sized by weight inside the chart area.",
    "dataShape": "Label/value pairs where the label is the word and the value controls size.",
    "useWhen": "Use it for qualitative summaries where exact numeric comparison is less important than emphasis.",
    "tags": [
      "wordcloud",
      "text"
    ],
    "image": "/img/charts/wordcloud.png"
  },
  {
    "slug": "line-large",
    "title": "Large line",
    "family": "Cartesian",
    "description": "A dense ordered series rendered as a continuous trend without changing the app model.",
    "dataShape": "A longer Vec<f32> aligned to ordered category samples.",
    "useWhen": "Use it for telemetry, monitoring, and sampled metrics where the trend matters more than every label.",
    "tags": [
      "line",
      "large"
    ],
    "image": "/img/charts/line-large.png"
  },
  {
    "slug": "bar-horizontal",
    "title": "Horizontal bar",
    "family": "Cartesian",
    "description": "A category comparison with value length running left to right.",
    "dataShape": "BarSeries values with a category y-axis and value x-axis.",
    "useWhen": "Use it when category labels are long or ranking order is more important than time order.",
    "tags": [
      "bar",
      "horizontal"
    ],
    "image": "/img/charts/bar-horizontal.png"
  },
  {
    "slug": "bar-background",
    "title": "Bar with background",
    "family": "Cartesian",
    "description": "Rounded bars drawn over a full-range background track.",
    "dataShape": "BarSeries values plus background and border-radius styling.",
    "useWhen": "Use it for progress-like category comparisons where the maximum should remain visible.",
    "tags": [
      "bar",
      "background"
    ],
    "image": "/img/charts/bar-background.png"
  },
  {
    "slug": "bar-negative",
    "title": "Positive and negative bar",
    "family": "Cartesian",
    "description": "Bars extending above and below the zero baseline.",
    "dataShape": "BarSeries values that may be positive or negative.",
    "useWhen": "Use it for deltas, profit/loss, variance, and month-over-month movement.",
    "tags": [
      "bar",
      "negative"
    ],
    "image": "/img/charts/bar-negative.png"
  },
  {
    "slug": "scatter-bubble",
    "title": "Bubble scatter",
    "family": "Statistical and finance",
    "description": "Scatter points where bubble size carries a third numeric dimension.",
    "dataShape": "Vec<(x, y, size)>.",
    "useWhen": "Use it when two axes are not enough and size can encode importance or volume.",
    "tags": [
      "scatter",
      "bubble"
    ],
    "image": "/img/charts/scatter-bubble.png"
  },
  {
    "slug": "mark-line-point",
    "title": "Mark line and point",
    "family": "Components and interaction",
    "description": "Target bands, threshold lines, and named points layered over cartesian data.",
    "dataShape": "MarkArea, MarkLine, and MarkPoint records attached to a chart.",
    "useWhen": "Use it to explain thresholds, goals, anomalies, or operational bands directly on the chart.",
    "tags": [
      "markPoint",
      "markLine",
      "markArea"
    ],
    "image": "/img/charts/mark-line-point.png"
  },
  {
    "slug": "data-zoom",
    "title": "Data zoom",
    "family": "Components and interaction",
    "description": "A slider selection that limits the visible portion of ordered data.",
    "dataShape": "DataZoom start and end percentages plus ordered series data.",
    "useWhen": "Use it when the user needs to inspect a slice of a longer series without leaving context.",
    "tags": [
      "dataZoom",
      "interaction"
    ],
    "image": "/img/charts/data-zoom.png"
  },
  {
    "slug": "tooltip-axis",
    "title": "Axis tooltip",
    "family": "Components and interaction",
    "description": "Axis-oriented tooltip configuration and hit-tested chart events.",
    "dataShape": "Tooltip::axis_trigger plus series data under shared axes.",
    "useWhen": "Use it when several series share one x position and the user needs exact values.",
    "tags": [
      "tooltip",
      "axisPointer"
    ],
    "image": "/img/charts/tooltip-axis.png"
  },
  {
    "slug": "timeline-events",
    "title": "Timeline",
    "family": "Components and interaction",
    "description": "A timeline control rendered with active state and labeled stops.",
    "dataShape": "ChartTimeline labels and current index.",
    "useWhen": "Use it to show snapshots, scenario years, deployment phases, or step-based playback.",
    "tags": [
      "timeline",
      "component"
    ],
    "image": "/img/charts/timeline-events.png"
  },
  {
    "slug": "toolbox-actions",
    "title": "Toolbox actions",
    "family": "Components and interaction",
    "description": "Chart-level tool buttons for restore, save, zoom, and brush actions.",
    "dataShape": "ChartInteraction toolbox action list.",
    "useWhen": "Use it when a chart has explicit utility operations that should remain close to the data.",
    "tags": [
      "toolbox",
      "actions"
    ],
    "image": "/img/charts/toolbox-actions.png"
  },
  {
    "slug": "brush-select",
    "title": "Brush selection",
    "family": "Components and interaction",
    "description": "A visible brush region over a scatter plot.",
    "dataShape": "ChartBrush configuration plus hit-testable series data.",
    "useWhen": "Use it when users need to select or inspect a region of points.",
    "tags": [
      "brush",
      "selection"
    ],
    "image": "/img/charts/brush-select.png"
  },
  {
    "slug": "graphic-overlay",
    "title": "Graphic overlay",
    "family": "Components and interaction",
    "description": "Typed graphic shapes and text layered over the chart plot area.",
    "dataShape": "ChartGraphic rect, text, line, or circle records.",
    "useWhen": "Use it for callouts, annotations, labels, and lightweight explanatory overlays.",
    "tags": [
      "graphic",
      "annotation"
    ],
    "image": "/img/charts/graphic-overlay.png"
  },
  {
    "slug": "polar-bar",
    "title": "Polar bar",
    "family": "Coordinates",
    "description": "Radial bars placed around a polar coordinate system.",
    "dataShape": "Label/value pairs rendered as radial bar segments.",
    "useWhen": "Use it when cyclical comparison benefits from circular layout.",
    "tags": [
      "polar",
      "bar"
    ],
    "image": "/img/charts/polar-bar.png"
  },
  {
    "slug": "polar-line",
    "title": "Polar line",
    "family": "Coordinates",
    "description": "A line series mapped by angle and radius.",
    "dataShape": "Vec<(angle_degrees, radius)>.",
    "useWhen": "Use it for wind, direction, periodic signals, and angular measurements.",
    "tags": [
      "polar",
      "line"
    ],
    "image": "/img/charts/polar-line.png"
  },
  {
    "slug": "calendar-heatmap",
    "title": "Calendar heatmap",
    "family": "Coordinates",
    "description": "Date values placed into a week-by-day calendar grid.",
    "dataShape": "Date/value pairs using YYYY-MM-DD dates.",
    "useWhen": "Use it for contribution, activity, incident, and habit patterns over weeks or months.",
    "tags": [
      "calendar",
      "heatmap"
    ],
    "image": "/img/charts/calendar-heatmap.png"
  },
  {
    "slug": "single-axis",
    "title": "Single axis",
    "family": "Coordinates",
    "description": "Events or weighted points arranged along one numeric axis.",
    "dataShape": "Vec<(value, size)> on a single horizontal scale.",
    "useWhen": "Use it for timelines, distributions, and event density when a second axis would add noise.",
    "tags": [
      "singleAxis",
      "events"
    ],
    "image": "/img/charts/single-axis.png"
  },
  {
    "slug": "bar3d-basic",
    "title": "3D bar",
    "family": "3D and GL",
    "description": "Bar values represented as retained native 3D scene primitives.",
    "dataShape": "A grid of values lowered into Scene3D mesh cuboids.",
    "useWhen": "Use it when depth and spatial grouping are part of the story, not as a default bar replacement.",
    "tags": [
      "3d",
      "bar"
    ],
    "image": "/img/charts/bar3d-basic.png"
  },
  {
    "slug": "scatter3d-basic",
    "title": "3D scatter",
    "family": "3D and GL",
    "description": "Points positioned in three dimensions with native scene primitives.",
    "dataShape": "Vec<(x, y, z, radius)> lowered into Scene3D spheres.",
    "useWhen": "Use it when all three dimensions are meaningful and the app benefits from spatial inspection.",
    "tags": [
      "3d",
      "scatter"
    ],
    "image": "/img/charts/scatter3d-basic.png"
  },
  {
    "slug": "surface3d-basic",
    "title": "3D surface",
    "family": "3D and GL",
    "description": "A gridded surface lowered into a native 3D mesh.",
    "dataShape": "Mesh vertices and triangle indices generated from sampled z values.",
    "useWhen": "Use it for terrain, response surfaces, and continuous two-variable functions.",
    "tags": [
      "3d",
      "surface"
    ],
    "image": "/img/charts/surface3d-basic.png"
  },
  {
    "slug": "line3d-basic",
    "title": "3D line",
    "family": "3D and GL",
    "description": "A trajectory drawn through a native 3D scene using sampled points.",
    "dataShape": "Ordered 3D positions lowered into Scene3D primitives.",
    "useWhen": "Use it for paths, movement, and ordered spatial traces.",
    "tags": [
      "3d",
      "line"
    ],
    "image": "/img/charts/line3d-basic.png"
  },
  {
    "slug": "point-cloud",
    "title": "Point cloud",
    "family": "3D and GL",
    "description": "Many small points distributed in three-dimensional space.",
    "dataShape": "A collection of x, y, z positions with point radii.",
    "useWhen": "Use it for spatial samples, scan data, and dense 3D observations.",
    "tags": [
      "3d",
      "point-cloud"
    ],
    "image": "/img/charts/point-cloud.png"
  },
  {
    "slug": "globe-basic",
    "title": "Globe",
    "family": "3D and GL",
    "description": "A spherical world-style chart primitive with highlighted locations.",
    "dataShape": "Scene3D sphere primitives plus marker primitives.",
    "useWhen": "Use it when global shape and spatial orientation matter to the reader.",
    "tags": [
      "3d",
      "globe"
    ],
    "image": "/img/charts/globe-basic.png"
  },
  {
    "slug": "graph3d-basic",
    "title": "3D graph",
    "family": "3D and GL",
    "description": "Relationship nodes placed in a native 3D scene.",
    "dataShape": "Node positions and values lowered into Scene3D primitives.",
    "useWhen": "Use it when topology benefits from depth or when the graph is part of a 3D product surface.",
    "tags": [
      "3d",
      "graph"
    ],
    "image": "/img/charts/graph3d-basic.png"
  },
  {
    "slug": "terrain-surface",
    "title": "Terrain surface",
    "family": "3D and GL",
    "description": "A raised mesh surface for terrain-like continuous data.",
    "dataShape": "Grid-generated mesh vertices and triangle indices.",
    "useWhen": "Use it for terrain, elevation, and other continuous spatial fields.",
    "tags": [
      "3d",
      "terrain"
    ],
    "image": "/img/charts/terrain-surface.png"
  },
  {
    "slug": "line-gradient-area",
    "title": "Gradient area line",
    "family": "Line",
    "description": "Line chart with translucent area fill for volume emphasis.",
    "dataShape": "Vec<f32> with area style.",
    "useWhen": "Use it when magnitude and trend should be read together.",
    "tags": [
      "line",
      "area"
    ],
    "image": "/img/charts/line-gradient-area.png"
  },
  {
    "slug": "line-threshold",
    "title": "Line with threshold",
    "family": "Line",
    "description": "Line chart with a target line and highlighted operating band.",
    "dataShape": "Vec<f32> plus MarkLine and MarkArea.",
    "useWhen": "Use it for service levels, quotas, and alert thresholds.",
    "tags": [
      "line",
      "markLine"
    ],
    "image": "/img/charts/line-threshold.png"
  },
  {
    "slug": "line-forecast-band",
    "title": "Forecast band",
    "family": "Line",
    "description": "Line chart with an expected range behind the observed values.",
    "dataShape": "Vec<f32> plus MarkArea.",
    "useWhen": "Use it when forecasts need uncertainty context.",
    "tags": [
      "line",
      "forecast"
    ],
    "image": "/img/charts/line-forecast-band.png"
  },
  {
    "slug": "line-weekly-cycle",
    "title": "Weekly cycle line",
    "family": "Line",
    "description": "Ordered samples over a repeating weekly cycle.",
    "dataShape": "Vec<f32> aligned to weekdays.",
    "useWhen": "Use it for operational rhythms and weekly dashboards.",
    "tags": [
      "line",
      "cycle"
    ],
    "image": "/img/charts/line-weekly-cycle.png"
  },
  {
    "slug": "line-spark-dense",
    "title": "Dense spark line",
    "family": "Line",
    "description": "Compact dense line with many telemetry samples.",
    "dataShape": "Long Vec<f32> on ordered categories.",
    "useWhen": "Use it for monitoring and telemetry surfaces.",
    "tags": [
      "line",
      "large"
    ],
    "image": "/img/charts/line-spark-dense.png"
  },
  {
    "slug": "line-step-start",
    "title": "Step start line",
    "family": "Line",
    "description": "Discrete state changes drawn as step segments.",
    "dataShape": "Vec<f32> with start step behavior.",
    "useWhen": "Use it when values jump at the start of each interval.",
    "tags": [
      "line",
      "step"
    ],
    "image": "/img/charts/line-step-start.png"
  },
  {
    "slug": "line-step-end",
    "title": "Step end line",
    "family": "Line",
    "description": "Discrete state changes drawn at interval ends.",
    "dataShape": "Vec<f32> with end step behavior.",
    "useWhen": "Use it when values settle at the end of each interval.",
    "tags": [
      "line",
      "step"
    ],
    "image": "/img/charts/line-step-end.png"
  },
  {
    "slug": "line-dual-series",
    "title": "Dual line comparison",
    "family": "Line",
    "description": "Two related line series sharing one chart.",
    "dataShape": "Two Vec<f32> series on shared axes.",
    "useWhen": "Use it to compare two trends without changing units.",
    "tags": [
      "line",
      "comparison"
    ],
    "image": "/img/charts/line-dual-series.png"
  },
  {
    "slug": "line-stacked-stream",
    "title": "Stacked stream area",
    "family": "Line",
    "description": "Stacked area series showing total and composition.",
    "dataShape": "Multiple LineSeries with one stack key.",
    "useWhen": "Use it for composition over time.",
    "tags": [
      "line",
      "stack"
    ],
    "image": "/img/charts/line-stacked-stream.png"
  },
  {
    "slug": "line-minmax-band",
    "title": "Min/max band",
    "family": "Line",
    "description": "Trend line with upper and lower guide marks.",
    "dataShape": "Vec<f32> plus mark lines.",
    "useWhen": "Use it to show expected bounds around a metric.",
    "tags": [
      "line",
      "bounds"
    ],
    "image": "/img/charts/line-minmax-band.png"
  },
  {
    "slug": "line-seasonal",
    "title": "Seasonal line",
    "family": "Line",
    "description": "Trend samples shaped as a seasonal curve.",
    "dataShape": "Vec<f32> over ordered periods.",
    "useWhen": "Use it for seasonal demand and capacity planning.",
    "tags": [
      "line",
      "seasonal"
    ],
    "image": "/img/charts/line-seasonal.png"
  },
  {
    "slug": "line-annotation",
    "title": "Annotated line",
    "family": "Line",
    "description": "Line chart with a callout annotation in the plot area.",
    "dataShape": "LineSeries plus ChartGraphic.",
    "useWhen": "Use it to explain a visible change directly on the chart.",
    "tags": [
      "line",
      "graphic"
    ],
    "image": "/img/charts/line-annotation.png"
  },
  {
    "slug": "line-rolling-average",
    "title": "Rolling average line",
    "family": "Line",
    "description": "Raw metric and smoothed average on one chart.",
    "dataShape": "Two Vec<f32> series.",
    "useWhen": "Use it when readers need both movement and trend.",
    "tags": [
      "line",
      "average"
    ],
    "image": "/img/charts/line-rolling-average.png"
  },
  {
    "slug": "line-zoomed-window",
    "title": "Zoomed line window",
    "family": "Line",
    "description": "Ordered line data filtered through a data zoom window.",
    "dataShape": "LineSeries plus DataZoom.",
    "useWhen": "Use it for long series with local inspection.",
    "tags": [
      "line",
      "dataZoom"
    ],
    "image": "/img/charts/line-zoomed-window.png"
  },
  {
    "slug": "line-alert-events",
    "title": "Line with alert events",
    "family": "Line",
    "description": "Line chart with named event points.",
    "dataShape": "LineSeries plus MarkPoint.",
    "useWhen": "Use it to label incidents or releases on a metric.",
    "tags": [
      "line",
      "markPoint"
    ],
    "image": "/img/charts/line-alert-events.png"
  },
  {
    "slug": "line-log-shaped",
    "title": "Log-shaped line",
    "family": "Line",
    "description": "A line with rapid early growth and slower later growth.",
    "dataShape": "Vec<f32> with nonlinear values.",
    "useWhen": "Use it for adoption curves and saturation effects.",
    "tags": [
      "line",
      "growth"
    ],
    "image": "/img/charts/line-log-shaped.png"
  },
  {
    "slug": "bar-ranked",
    "title": "Ranked bar",
    "family": "Bar",
    "description": "Horizontal bars sorted by value.",
    "dataShape": "BarSeries with horizontal orientation.",
    "useWhen": "Use it for rankings with long labels.",
    "tags": [
      "bar",
      "ranked"
    ],
    "image": "/img/charts/bar-ranked.png"
  },
  {
    "slug": "bar-diverging",
    "title": "Diverging bar",
    "family": "Bar",
    "description": "Horizontal bars extending in both directions from zero.",
    "dataShape": "Positive and negative BarSeries values.",
    "useWhen": "Use it for sentiment, deltas, and balance views.",
    "tags": [
      "bar",
      "diverging"
    ],
    "image": "/img/charts/bar-diverging.png"
  },
  {
    "slug": "bar-waterfall",
    "title": "Waterfall bar",
    "family": "Bar",
    "description": "Sequential changes displayed as rising and falling bars.",
    "dataShape": "Delta values over ordered categories.",
    "useWhen": "Use it for profit bridges and cumulative change explanations.",
    "tags": [
      "bar",
      "waterfall"
    ],
    "image": "/img/charts/bar-waterfall.png"
  },
  {
    "slug": "bar-rounded",
    "title": "Rounded bar",
    "family": "Bar",
    "description": "Bars with rounded corners for dashboard presentation.",
    "dataShape": "BarSeries with border radius.",
    "useWhen": "Use it when the chart sits in a polished product dashboard.",
    "tags": [
      "bar",
      "rounded"
    ],
    "image": "/img/charts/bar-rounded.png"
  },
  {
    "slug": "bar-track-progress",
    "title": "Track progress bars",
    "family": "Bar",
    "description": "Bars shown against full-range tracks.",
    "dataShape": "BarSeries with background color.",
    "useWhen": "Use it for completion and capacity comparisons.",
    "tags": [
      "bar",
      "progress"
    ],
    "image": "/img/charts/bar-track-progress.png"
  },
  {
    "slug": "bar-grouped-quarter",
    "title": "Grouped quarterly bars",
    "family": "Bar",
    "description": "Grouped bars comparing several periods per category.",
    "dataShape": "Multiple BarSeries on the same categories.",
    "useWhen": "Use it for side-by-side period comparisons.",
    "tags": [
      "bar",
      "grouped"
    ],
    "image": "/img/charts/bar-grouped-quarter.png"
  },
  {
    "slug": "bar-stacked-revenue",
    "title": "Stacked revenue bars",
    "family": "Bar",
    "description": "Stacked bars showing total and contribution.",
    "dataShape": "Multiple BarSeries sharing a stack key.",
    "useWhen": "Use it when totals and composition matter together.",
    "tags": [
      "bar",
      "stack"
    ],
    "image": "/img/charts/bar-stacked-revenue.png"
  },
  {
    "slug": "bar-negative-delta",
    "title": "Negative delta bars",
    "family": "Bar",
    "description": "Vertical bars with positive and negative changes.",
    "dataShape": "BarSeries values crossing zero.",
    "useWhen": "Use it for variance and profit/loss dashboards.",
    "tags": [
      "bar",
      "negative"
    ],
    "image": "/img/charts/bar-negative-delta.png"
  },
  {
    "slug": "bar-compact",
    "title": "Compact category bars",
    "family": "Bar",
    "description": "Small category comparison with a clean vertical bar layout.",
    "dataShape": "Vec<f32> aligned to categories.",
    "useWhen": "Use it for short lists and cards.",
    "tags": [
      "bar",
      "category"
    ],
    "image": "/img/charts/bar-compact.png"
  },
  {
    "slug": "bar-wide-labels",
    "title": "Wide-label horizontal bars",
    "family": "Bar",
    "description": "Horizontal layout preserving long category labels.",
    "dataShape": "BarSeries with category y-axis.",
    "useWhen": "Use it when labels would collide on the x-axis.",
    "tags": [
      "bar",
      "horizontal"
    ],
    "image": "/img/charts/bar-wide-labels.png"
  },
  {
    "slug": "bar-pictorial-units",
    "title": "Pictorial units",
    "family": "Bar",
    "description": "Repeated symbols representing category values.",
    "dataShape": "PictorialBarSeries values.",
    "useWhen": "Use it for branded but still quantitative category displays.",
    "tags": [
      "bar",
      "pictorial"
    ],
    "image": "/img/charts/bar-pictorial-units.png"
  },
  {
    "slug": "bar-capacity",
    "title": "Capacity bars",
    "family": "Bar",
    "description": "Bars drawn inside visible capacity tracks.",
    "dataShape": "BarSeries plus maximum axis and background.",
    "useWhen": "Use it for quota and inventory screens.",
    "tags": [
      "bar",
      "capacity"
    ],
    "image": "/img/charts/bar-capacity.png"
  },
  {
    "slug": "bar-small-multiples-a",
    "title": "Small multiple bars A",
    "family": "Bar",
    "description": "A focused bar chart variant for compact dashboards.",
    "dataShape": "BarSeries values on a category axis.",
    "useWhen": "Use it in repeated dashboard cards.",
    "tags": [
      "bar",
      "small-multiple"
    ],
    "image": "/img/charts/bar-small-multiples-a.png"
  },
  {
    "slug": "bar-small-multiples-b",
    "title": "Small multiple bars B",
    "family": "Bar",
    "description": "A second compact bar chart with different distribution.",
    "dataShape": "BarSeries values on a category axis.",
    "useWhen": "Use it when multiple products use the same visual scale.",
    "tags": [
      "bar",
      "small-multiple"
    ],
    "image": "/img/charts/bar-small-multiples-b.png"
  },
  {
    "slug": "bar-sorted-horizontal",
    "title": "Sorted horizontal bars",
    "family": "Bar",
    "description": "Horizontal ranking sorted largest to smallest.",
    "dataShape": "BarSeries with category y-axis.",
    "useWhen": "Use it for top-N views.",
    "tags": [
      "bar",
      "sorted"
    ],
    "image": "/img/charts/bar-sorted-horizontal.png"
  },
  {
    "slug": "bar-budget-stack",
    "title": "Budget stack",
    "family": "Bar",
    "description": "Stacked bars for budget contribution by department.",
    "dataShape": "Stacked BarSeries values.",
    "useWhen": "Use it for departmental totals and breakdowns.",
    "tags": [
      "bar",
      "budget"
    ],
    "image": "/img/charts/bar-budget-stack.png"
  },
  {
    "slug": "bar-kpi-background",
    "title": "KPI background bars",
    "family": "Bar",
    "description": "KPI bars with background tracks.",
    "dataShape": "BarSeries with background styling.",
    "useWhen": "Use it for KPI completion panels.",
    "tags": [
      "bar",
      "kpi"
    ],
    "image": "/img/charts/bar-kpi-background.png"
  },
  {
    "slug": "bar-region-comparison",
    "title": "Region comparison bars",
    "family": "Bar",
    "description": "Grouped regional values over shared categories.",
    "dataShape": "Multiple BarSeries.",
    "useWhen": "Use it for region-by-period comparisons.",
    "tags": [
      "bar",
      "region"
    ],
    "image": "/img/charts/bar-region-comparison.png"
  },
  {
    "slug": "pie-two-level",
    "title": "Two-level donut",
    "family": "Pie and radial",
    "description": "Donut chart for a small part-to-whole breakdown.",
    "dataShape": "Label/value pairs plus inner radius.",
    "useWhen": "Use it for compact composition summaries.",
    "tags": [
      "pie",
      "donut"
    ],
    "image": "/img/charts/pie-two-level.png"
  },
  {
    "slug": "pie-nested-like",
    "title": "Nested-style donut",
    "family": "Pie and radial",
    "description": "Donut-style chart with more categories and central whitespace.",
    "dataShape": "Label/value pairs plus inner radius.",
    "useWhen": "Use it when a central status label will be added by surrounding UI.",
    "tags": [
      "pie",
      "nested"
    ],
    "image": "/img/charts/pie-nested-like.png"
  },
  {
    "slug": "pie-rose-presentation",
    "title": "Presentation rose",
    "family": "Pie and radial",
    "description": "Rose chart emphasizing category differences with radius.",
    "dataShape": "Label/value pairs with rose radius mode.",
    "useWhen": "Use it for presentation views with few categories.",
    "tags": [
      "pie",
      "rose"
    ],
    "image": "/img/charts/pie-rose-presentation.png"
  },
  {
    "slug": "pie-area-rose",
    "title": "Area rose",
    "family": "Pie and radial",
    "description": "Rose chart using area-oriented radius scaling.",
    "dataShape": "Label/value pairs with rose area mode.",
    "useWhen": "Use it when small values should remain visible.",
    "tags": [
      "pie",
      "rose"
    ],
    "image": "/img/charts/pie-area-rose.png"
  },
  {
    "slug": "pie-market-share",
    "title": "Market share pie",
    "family": "Pie and radial",
    "description": "Part-to-whole chart for a small market mix.",
    "dataShape": "Label/value pairs.",
    "useWhen": "Use it for small composition snapshots.",
    "tags": [
      "pie",
      "share"
    ],
    "image": "/img/charts/pie-market-share.png"
  },
  {
    "slug": "pie-device-mix",
    "title": "Device mix donut",
    "family": "Pie and radial",
    "description": "Donut chart for device or channel mix.",
    "dataShape": "Label/value pairs plus inner radius.",
    "useWhen": "Use it when the total belongs in the center of the chart.",
    "tags": [
      "pie",
      "device"
    ],
    "image": "/img/charts/pie-device-mix.png"
  },
  {
    "slug": "gauge-score",
    "title": "Score gauge",
    "family": "Pie and radial",
    "description": "Bounded score shown as an instrument state.",
    "dataShape": "One label/value pair.",
    "useWhen": "Use it for health or readiness scores.",
    "tags": [
      "gauge",
      "score"
    ],
    "image": "/img/charts/gauge-score.png"
  },
  {
    "slug": "gauge-capacity",
    "title": "Capacity gauge",
    "family": "Pie and radial",
    "description": "Gauge showing capacity consumed.",
    "dataShape": "One bounded value.",
    "useWhen": "Use it for operational capacity panels.",
    "tags": [
      "gauge",
      "capacity"
    ],
    "image": "/img/charts/gauge-capacity.png"
  },
  {
    "slug": "polar-cyclic-bar",
    "title": "Cyclic polar bars",
    "family": "Pie and radial",
    "description": "Radial bars around a cycle.",
    "dataShape": "Label/value pairs in polar coordinates.",
    "useWhen": "Use it for cyclical categories.",
    "tags": [
      "polar",
      "bar"
    ],
    "image": "/img/charts/polar-cyclic-bar.png"
  },
  {
    "slug": "polar-wind-line",
    "title": "Wind polar line",
    "family": "Pie and radial",
    "description": "Directional values on a polar line.",
    "dataShape": "Angle/radius samples.",
    "useWhen": "Use it for direction and magnitude data.",
    "tags": [
      "polar",
      "line"
    ],
    "image": "/img/charts/polar-wind-line.png"
  },
  {
    "slug": "radar-profile-a",
    "title": "Radar profile A",
    "family": "Pie and radial",
    "description": "Profile comparison across fixed dimensions.",
    "dataShape": "Vec<Vec<f32>> metric profiles.",
    "useWhen": "Use it for capability or budget profiles.",
    "tags": [
      "radar",
      "profile"
    ],
    "image": "/img/charts/radar-profile-a.png"
  },
  {
    "slug": "radar-profile-b",
    "title": "Radar profile B",
    "family": "Pie and radial",
    "description": "Filled radar chart for overlapping profile comparison.",
    "dataShape": "Vec<Vec<f32>> metric profiles.",
    "useWhen": "Use it when the profile shape matters.",
    "tags": [
      "radar",
      "profile"
    ],
    "image": "/img/charts/radar-profile-b.png"
  },
  {
    "slug": "scatter-clusters",
    "title": "Scatter clusters",
    "family": "Scatter and statistical",
    "description": "Point cloud showing clustered observations.",
    "dataShape": "Vec<(f32, f32)>.",
    "useWhen": "Use it for correlation and clustering.",
    "tags": [
      "scatter",
      "cluster"
    ],
    "image": "/img/charts/scatter-clusters.png"
  },
  {
    "slug": "scatter-outliers",
    "title": "Scatter outliers",
    "family": "Scatter and statistical",
    "description": "Scatter chart with emphasized outlier points.",
    "dataShape": "Vec<(f32, f32)> plus effect scatter styling.",
    "useWhen": "Use it to make exceptions visible.",
    "tags": [
      "scatter",
      "outlier"
    ],
    "image": "/img/charts/scatter-outliers.png"
  },
  {
    "slug": "scatter-bubble-market",
    "title": "Market bubble chart",
    "family": "Scatter and statistical",
    "description": "Bubble chart encoding market size as radius.",
    "dataShape": "Vec<(x, y, size)>.",
    "useWhen": "Use it for three-dimensional business comparisons.",
    "tags": [
      "scatter",
      "bubble"
    ],
    "image": "/img/charts/scatter-bubble-market.png"
  },
  {
    "slug": "scatter-risk-return",
    "title": "Risk return scatter",
    "family": "Scatter and statistical",
    "description": "Scatter chart with bubble size for exposure.",
    "dataShape": "Vec<(risk, return, exposure)>.",
    "useWhen": "Use it for portfolio-style comparisons.",
    "tags": [
      "scatter",
      "risk"
    ],
    "image": "/img/charts/scatter-risk-return.png"
  },
  {
    "slug": "boxplot-latency",
    "title": "Latency boxplot",
    "family": "Scatter and statistical",
    "description": "Distribution summary for latency groups.",
    "dataShape": "Five-number summary rows.",
    "useWhen": "Use it when distribution matters more than averages.",
    "tags": [
      "boxplot",
      "latency"
    ],
    "image": "/img/charts/boxplot-latency.png"
  },
  {
    "slug": "boxplot-quality",
    "title": "Quality boxplot",
    "family": "Scatter and statistical",
    "description": "Boxplot comparing quality ranges.",
    "dataShape": "Five-number summary rows.",
    "useWhen": "Use it for process quality and batch comparisons.",
    "tags": [
      "boxplot",
      "quality"
    ],
    "image": "/img/charts/boxplot-quality.png"
  },
  {
    "slug": "candlestick-volume",
    "title": "Candlestick with movement",
    "family": "Scatter and statistical",
    "description": "Open-close-low-high financial movement.",
    "dataShape": "Rows of open, close, low, high values.",
    "useWhen": "Use it for market and range-over-time data.",
    "tags": [
      "candlestick",
      "finance"
    ],
    "image": "/img/charts/candlestick-volume.png"
  },
  {
    "slug": "candlestick-intraday",
    "title": "Intraday candlestick",
    "family": "Scatter and statistical",
    "description": "Candlestick variant with tighter movement.",
    "dataShape": "OHLC rows.",
    "useWhen": "Use it for intraday or short-range market views.",
    "tags": [
      "candlestick",
      "finance"
    ],
    "image": "/img/charts/candlestick-intraday.png"
  },
  {
    "slug": "funnel-conversion",
    "title": "Conversion funnel",
    "family": "Scatter and statistical",
    "description": "Stages drawn as a narrowing funnel.",
    "dataShape": "Ordered label/value pairs.",
    "useWhen": "Use it for sales or onboarding funnels.",
    "tags": [
      "funnel",
      "conversion"
    ],
    "image": "/img/charts/funnel-conversion.png"
  },
  {
    "slug": "funnel-recruiting",
    "title": "Recruiting funnel",
    "family": "Scatter and statistical",
    "description": "Funnel chart for hiring stages.",
    "dataShape": "Ordered label/value pairs.",
    "useWhen": "Use it when stage drop-off matters.",
    "tags": [
      "funnel",
      "pipeline"
    ],
    "image": "/img/charts/funnel-recruiting.png"
  },
  {
    "slug": "parallel-products",
    "title": "Product parallel coordinates",
    "family": "Scatter and statistical",
    "description": "Rows crossing multiple dimensions.",
    "dataShape": "Vec<Vec<f32>> observations.",
    "useWhen": "Use it for multidimensional comparison.",
    "tags": [
      "parallel",
      "dimensions"
    ],
    "image": "/img/charts/parallel-products.png"
  },
  {
    "slug": "parallel-quality",
    "title": "Quality parallel coordinates",
    "family": "Scatter and statistical",
    "description": "Parallel coordinates for quality dimensions.",
    "dataShape": "Vec<Vec<f32>> observations.",
    "useWhen": "Use it for filtering and comparing profiles.",
    "tags": [
      "parallel",
      "quality"
    ],
    "image": "/img/charts/parallel-quality.png"
  },
  {
    "slug": "single-axis-events",
    "title": "Event single axis",
    "family": "Scatter and statistical",
    "description": "Weighted events along a single timeline.",
    "dataShape": "Vec<(value, size)>.",
    "useWhen": "Use it for event density and distributions.",
    "tags": [
      "singleAxis",
      "events"
    ],
    "image": "/img/charts/single-axis-events.png"
  },
  {
    "slug": "single-axis-distribution",
    "title": "Distribution single axis",
    "family": "Scatter and statistical",
    "description": "Weighted samples on one numeric scale.",
    "dataShape": "Vec<(value, size)>.",
    "useWhen": "Use it when a second axis adds noise.",
    "tags": [
      "singleAxis",
      "distribution"
    ],
    "image": "/img/charts/single-axis-distribution.png"
  },
  {
    "slug": "heatmap-weekday-hour",
    "title": "Weekday hour heatmap",
    "family": "Heatmap and calendar",
    "description": "Matrix heatmap for activity by hour and weekday.",
    "dataShape": "x index, y index, value triples.",
    "useWhen": "Use it for operational intensity matrices.",
    "tags": [
      "heatmap",
      "matrix"
    ],
    "image": "/img/charts/heatmap-weekday-hour.png"
  },
  {
    "slug": "heatmap-resource-load",
    "title": "Resource load heatmap",
    "family": "Heatmap and calendar",
    "description": "Heatmap showing resource load across services.",
    "dataShape": "Matrix value triples.",
    "useWhen": "Use it for capacity and utilization panels.",
    "tags": [
      "heatmap",
      "load"
    ],
    "image": "/img/charts/heatmap-resource-load.png"
  },
  {
    "slug": "heatmap-risk-grid",
    "title": "Risk grid heatmap",
    "family": "Heatmap and calendar",
    "description": "Risk values in a two-axis matrix.",
    "dataShape": "Matrix value triples.",
    "useWhen": "Use it for risk and priority matrices.",
    "tags": [
      "heatmap",
      "risk"
    ],
    "image": "/img/charts/heatmap-risk-grid.png"
  },
  {
    "slug": "visual-map-density",
    "title": "Visual map density",
    "family": "Heatmap and calendar",
    "description": "Heatmap with explicit visual map color scale.",
    "dataShape": "HeatmapSeries plus VisualMap.",
    "useWhen": "Use it when color needs a visible numeric range.",
    "tags": [
      "visual-map",
      "heatmap"
    ],
    "image": "/img/charts/visual-map-density.png"
  },
  {
    "slug": "calendar-quarter",
    "title": "Quarter calendar heatmap",
    "family": "Heatmap and calendar",
    "description": "Calendar heatmap for a quarter of activity.",
    "dataShape": "Date/value pairs.",
    "useWhen": "Use it for contribution and activity patterns.",
    "tags": [
      "calendar",
      "heatmap"
    ],
    "image": "/img/charts/calendar-quarter.png"
  },
  {
    "slug": "calendar-incidents",
    "title": "Incident calendar",
    "family": "Heatmap and calendar",
    "description": "Calendar heatmap for incident counts.",
    "dataShape": "Date/value pairs.",
    "useWhen": "Use it for reliability and support reporting.",
    "tags": [
      "calendar",
      "incidents"
    ],
    "image": "/img/charts/calendar-incidents.png"
  },
  {
    "slug": "calendar-retention",
    "title": "Retention calendar",
    "family": "Heatmap and calendar",
    "description": "Calendar heatmap for retained activity.",
    "dataShape": "Date/value pairs.",
    "useWhen": "Use it for habit and retention surfaces.",
    "tags": [
      "calendar",
      "retention"
    ],
    "image": "/img/charts/calendar-retention.png"
  },
  {
    "slug": "calendar-builds",
    "title": "Build calendar",
    "family": "Heatmap and calendar",
    "description": "Calendar heatmap for build volume.",
    "dataShape": "Date/value pairs.",
    "useWhen": "Use it for engineering operations.",
    "tags": [
      "calendar",
      "builds"
    ],
    "image": "/img/charts/calendar-builds.png"
  },
  {
    "slug": "heatmap-small-matrix",
    "title": "Small matrix heatmap",
    "family": "Heatmap and calendar",
    "description": "Compact heatmap for a small matrix.",
    "dataShape": "Matrix value triples.",
    "useWhen": "Use it for dashboards with limited space.",
    "tags": [
      "heatmap",
      "matrix"
    ],
    "image": "/img/charts/heatmap-small-matrix.png"
  },
  {
    "slug": "heatmap-large-matrix",
    "title": "Large matrix heatmap",
    "family": "Heatmap and calendar",
    "description": "Larger heatmap with more cells.",
    "dataShape": "Dense matrix value triples.",
    "useWhen": "Use it for activity grids with many columns.",
    "tags": [
      "heatmap",
      "large"
    ],
    "image": "/img/charts/heatmap-large-matrix.png"
  },
  {
    "slug": "heatmap-correlation",
    "title": "Correlation heatmap",
    "family": "Heatmap and calendar",
    "description": "Square-style heatmap for correlation-like data.",
    "dataShape": "Matrix value triples.",
    "useWhen": "Use it for relationship strength matrices.",
    "tags": [
      "heatmap",
      "correlation"
    ],
    "image": "/img/charts/heatmap-correlation.png"
  },
  {
    "slug": "heatmap-availability",
    "title": "Availability heatmap",
    "family": "Heatmap and calendar",
    "description": "Availability values across services and periods.",
    "dataShape": "Matrix value triples.",
    "useWhen": "Use it for SRE and operations dashboards.",
    "tags": [
      "heatmap",
      "availability"
    ],
    "image": "/img/charts/heatmap-availability.png"
  },
  {
    "slug": "tree-org",
    "title": "Org tree",
    "family": "Hierarchy and flow",
    "description": "Parent-child hierarchy as a tidy tree.",
    "dataShape": "Nested nodes.",
    "useWhen": "Use it for organization and ownership charts.",
    "tags": [
      "tree",
      "hierarchy"
    ],
    "image": "/img/charts/tree-org.png"
  },
  {
    "slug": "tree-file-system",
    "title": "File-system tree",
    "family": "Hierarchy and flow",
    "description": "Nested file-style hierarchy.",
    "dataShape": "Nested nodes with values.",
    "useWhen": "Use it for product and resource hierarchies.",
    "tags": [
      "tree",
      "files"
    ],
    "image": "/img/charts/tree-file-system.png"
  },
  {
    "slug": "tree-radial-taxonomy",
    "title": "Radial taxonomy",
    "family": "Hierarchy and flow",
    "description": "Radial layout for a shallow taxonomy.",
    "dataShape": "Nested nodes.",
    "useWhen": "Use it when hierarchy symmetry matters.",
    "tags": [
      "tree",
      "radial"
    ],
    "image": "/img/charts/tree-radial-taxonomy.png"
  },
  {
    "slug": "treemap-budget",
    "title": "Budget treemap",
    "family": "Hierarchy and flow",
    "description": "Hierarchical values packed into rectangles.",
    "dataShape": "Nested value nodes.",
    "useWhen": "Use it for budget or storage breakdowns.",
    "tags": [
      "treemap",
      "budget"
    ],
    "image": "/img/charts/treemap-budget.png"
  },
  {
    "slug": "treemap-storage",
    "title": "Storage treemap",
    "family": "Hierarchy and flow",
    "description": "Treemap for storage allocation.",
    "dataShape": "Nested value nodes.",
    "useWhen": "Use it for disk and memory allocation.",
    "tags": [
      "treemap",
      "storage"
    ],
    "image": "/img/charts/treemap-storage.png"
  },
  {
    "slug": "sunburst-product",
    "title": "Product sunburst",
    "family": "Hierarchy and flow",
    "description": "Hierarchical values in concentric rings.",
    "dataShape": "Nested value nodes.",
    "useWhen": "Use it for hierarchy depth and composition.",
    "tags": [
      "sunburst",
      "hierarchy"
    ],
    "image": "/img/charts/sunburst-product.png"
  },
  {
    "slug": "sunburst-revenue",
    "title": "Revenue sunburst",
    "family": "Hierarchy and flow",
    "description": "Revenue hierarchy as radial rings.",
    "dataShape": "Nested value nodes.",
    "useWhen": "Use it for nested part-to-whole data.",
    "tags": [
      "sunburst",
      "revenue"
    ],
    "image": "/img/charts/sunburst-revenue.png"
  },
  {
    "slug": "sankey-energy",
    "title": "Energy sankey",
    "family": "Hierarchy and flow",
    "description": "Flow bands between stages.",
    "dataShape": "Node list plus edges.",
    "useWhen": "Use it for material, energy, or revenue flow.",
    "tags": [
      "sankey",
      "flow"
    ],
    "image": "/img/charts/sankey-energy.png"
  },
  {
    "slug": "sankey-user-flow",
    "title": "User-flow sankey",
    "family": "Hierarchy and flow",
    "description": "Sankey chart for user movement.",
    "dataShape": "Node list plus edges.",
    "useWhen": "Use it for conversion path analysis.",
    "tags": [
      "sankey",
      "flow"
    ],
    "image": "/img/charts/sankey-user-flow.png"
  },
  {
    "slug": "theme-river-traffic",
    "title": "Traffic theme river",
    "family": "Hierarchy and flow",
    "description": "Flowing stacked categories over time.",
    "dataShape": "Time/value/category tuples.",
    "useWhen": "Use it for composition changing over time.",
    "tags": [
      "themeRiver",
      "time"
    ],
    "image": "/img/charts/theme-river-traffic.png"
  },
  {
    "slug": "theme-river-demand",
    "title": "Demand theme river",
    "family": "Hierarchy and flow",
    "description": "Theme river chart for demand sources.",
    "dataShape": "Time/value/category tuples.",
    "useWhen": "Use it for campaign or demand mix.",
    "tags": [
      "themeRiver",
      "demand"
    ],
    "image": "/img/charts/theme-river-demand.png"
  },
  {
    "slug": "graph-dependencies",
    "title": "Dependency graph",
    "family": "Hierarchy and flow",
    "description": "Network nodes and edges.",
    "dataShape": "Graph nodes plus edges.",
    "useWhen": "Use it for dependency and topology diagrams.",
    "tags": [
      "graph",
      "network"
    ],
    "image": "/img/charts/graph-dependencies.png"
  },
  {
    "slug": "graph-services",
    "title": "Service graph",
    "family": "Hierarchy and flow",
    "description": "Service topology network.",
    "dataShape": "Graph nodes plus edges.",
    "useWhen": "Use it for systems maps and service graphs.",
    "tags": [
      "graph",
      "service"
    ],
    "image": "/img/charts/graph-services.png"
  },
  {
    "slug": "graph-circular-like",
    "title": "Circular-like graph",
    "family": "Hierarchy and flow",
    "description": "Network layout with larger central nodes.",
    "dataShape": "Graph nodes plus edges.",
    "useWhen": "Use it for relationship overview surfaces.",
    "tags": [
      "graph",
      "circular"
    ],
    "image": "/img/charts/graph-circular-like.png"
  },
  {
    "slug": "map-region-values",
    "title": "Region value map",
    "family": "Geo and route",
    "description": "GeoJSON-backed choropleth map.",
    "dataShape": "GeoJSON regions plus values.",
    "useWhen": "Use it for geography-first comparisons.",
    "tags": [
      "map",
      "geo"
    ],
    "image": "/img/charts/map-region-values.png"
  },
  {
    "slug": "map-service-coverage",
    "title": "Service coverage map",
    "family": "Geo and route",
    "description": "Map with coverage values by region.",
    "dataShape": "GeoJSON regions plus values.",
    "useWhen": "Use it for coverage and rollout screens.",
    "tags": [
      "map",
      "coverage"
    ],
    "image": "/img/charts/map-service-coverage.png"
  },
  {
    "slug": "geo-route-arcs",
    "title": "Route arcs",
    "family": "Geo and route",
    "description": "Curved route lines with direction.",
    "dataShape": "Line segments with values.",
    "useWhen": "Use it for logistics and movement.",
    "tags": [
      "lines",
      "routes"
    ],
    "image": "/img/charts/geo-route-arcs.png"
  },
  {
    "slug": "geo-route-map",
    "title": "Route map overlay",
    "family": "Geo and route",
    "description": "Routes drawn over geographic regions.",
    "dataShape": "GeoJSON regions plus route segments.",
    "useWhen": "Use it when geography and movement combine.",
    "tags": [
      "map",
      "routes"
    ],
    "image": "/img/charts/geo-route-map.png"
  },
  {
    "slug": "geo-migration",
    "title": "Migration routes",
    "family": "Geo and route",
    "description": "Route lines representing movement intensity.",
    "dataShape": "LineSegment values.",
    "useWhen": "Use it for movement and transfer flows.",
    "tags": [
      "lines",
      "migration"
    ],
    "image": "/img/charts/geo-migration.png"
  },
  {
    "slug": "geo-network",
    "title": "Geo network",
    "family": "Geo and route",
    "description": "Map with network-like route overlays.",
    "dataShape": "GeoJSON plus lines.",
    "useWhen": "Use it for network coverage and paths.",
    "tags": [
      "map",
      "network"
    ],
    "image": "/img/charts/geo-network.png"
  },
  {
    "slug": "map-risk",
    "title": "Risk map",
    "family": "Geo and route",
    "description": "Regions colored by risk score.",
    "dataShape": "GeoJSON regions plus numeric values.",
    "useWhen": "Use it for geographic risk dashboards.",
    "tags": [
      "map",
      "risk"
    ],
    "image": "/img/charts/map-risk.png"
  },
  {
    "slug": "map-sales",
    "title": "Sales map",
    "family": "Geo and route",
    "description": "Region sales values on a map.",
    "dataShape": "GeoJSON regions plus values.",
    "useWhen": "Use it for regional business metrics.",
    "tags": [
      "map",
      "sales"
    ],
    "image": "/img/charts/map-sales.png"
  },
  {
    "slug": "lines-traffic",
    "title": "Traffic lines",
    "family": "Geo and route",
    "description": "Curved lines with different values.",
    "dataShape": "LineSegment values.",
    "useWhen": "Use it for traffic or flow intensity.",
    "tags": [
      "lines",
      "traffic"
    ],
    "image": "/img/charts/lines-traffic.png"
  },
  {
    "slug": "lines-flight-like",
    "title": "Flight-like lines",
    "family": "Geo and route",
    "description": "Directional curved lines with effect points.",
    "dataShape": "LineSegment values plus effect.",
    "useWhen": "Use it for route animation-style visuals.",
    "tags": [
      "lines",
      "flight"
    ],
    "image": "/img/charts/lines-flight-like.png"
  },
  {
    "slug": "component-mark-area",
    "title": "Mark area component",
    "family": "Components and interaction",
    "description": "Line chart with a highlighted range.",
    "dataShape": "MarkArea plus LineSeries.",
    "useWhen": "Use it for safe operating zones.",
    "tags": [
      "markArea",
      "component"
    ],
    "image": "/img/charts/component-mark-area.png"
  },
  {
    "slug": "component-mark-line",
    "title": "Mark line component",
    "family": "Components and interaction",
    "description": "Line chart with target threshold line.",
    "dataShape": "MarkLine plus LineSeries.",
    "useWhen": "Use it for goals and alert thresholds.",
    "tags": [
      "markLine",
      "component"
    ],
    "image": "/img/charts/component-mark-line.png"
  },
  {
    "slug": "component-mark-point",
    "title": "Mark point component",
    "family": "Components and interaction",
    "description": "Line chart with named event points.",
    "dataShape": "MarkPoint plus LineSeries.",
    "useWhen": "Use it for incidents and milestones.",
    "tags": [
      "markPoint",
      "component"
    ],
    "image": "/img/charts/component-mark-point.png"
  },
  {
    "slug": "component-data-zoom-short",
    "title": "Data zoom short",
    "family": "Components and interaction",
    "description": "Small data zoom window over ordered data.",
    "dataShape": "DataZoom plus LineSeries.",
    "useWhen": "Use it for local inspection of long series.",
    "tags": [
      "dataZoom",
      "component"
    ],
    "image": "/img/charts/component-data-zoom-short.png"
  },
  {
    "slug": "component-data-zoom-long",
    "title": "Data zoom long",
    "family": "Components and interaction",
    "description": "Wide data zoom window over ordered data.",
    "dataShape": "DataZoom plus LineSeries.",
    "useWhen": "Use it for dashboards with focus ranges.",
    "tags": [
      "dataZoom",
      "component"
    ],
    "image": "/img/charts/component-data-zoom-long.png"
  },
  {
    "slug": "component-tooltip-axis",
    "title": "Tooltip axis component",
    "family": "Components and interaction",
    "description": "Axis tooltip over shared bar and line data.",
    "dataShape": "Tooltip plus ChartInteraction.",
    "useWhen": "Use it for exact values across series.",
    "tags": [
      "tooltip",
      "component"
    ],
    "image": "/img/charts/component-tooltip-axis.png"
  },
  {
    "slug": "component-tooltip-item",
    "title": "Tooltip item component",
    "family": "Components and interaction",
    "description": "Item tooltip over single series data.",
    "dataShape": "Tooltip item trigger.",
    "useWhen": "Use it for exact item readout.",
    "tags": [
      "tooltip",
      "item"
    ],
    "image": "/img/charts/component-tooltip-item.png"
  },
  {
    "slug": "component-toolbox-zoom",
    "title": "Toolbox zoom",
    "family": "Components and interaction",
    "description": "Toolbox actions rendered near the chart.",
    "dataShape": "ChartInteraction toolbox actions.",
    "useWhen": "Use it for chart utility operations.",
    "tags": [
      "toolbox",
      "component"
    ],
    "image": "/img/charts/component-toolbox-zoom.png"
  },
  {
    "slug": "component-toolbox-full",
    "title": "Toolbox full",
    "family": "Components and interaction",
    "description": "Full toolbox action set.",
    "dataShape": "ChartInteraction toolbox actions.",
    "useWhen": "Use it when a chart supports several operations.",
    "tags": [
      "toolbox",
      "actions"
    ],
    "image": "/img/charts/component-toolbox-full.png"
  },
  {
    "slug": "component-brush-rect",
    "title": "Brush rectangle",
    "family": "Components and interaction",
    "description": "Rectangular brush preview over a scatter plot.",
    "dataShape": "ChartBrush plus ScatterSeries.",
    "useWhen": "Use it for region selection.",
    "tags": [
      "brush",
      "selection"
    ],
    "image": "/img/charts/component-brush-rect.png"
  },
  {
    "slug": "component-brush-horizontal",
    "title": "Brush horizontal",
    "family": "Components and interaction",
    "description": "Horizontal brush preview over scatter data.",
    "dataShape": "ChartBrush plus ScatterSeries.",
    "useWhen": "Use it for x-range selection.",
    "tags": [
      "brush",
      "selection"
    ],
    "image": "/img/charts/component-brush-horizontal.png"
  },
  {
    "slug": "component-graphic-callout",
    "title": "Graphic callout",
    "family": "Components and interaction",
    "description": "Graphic annotation overlay on a line chart.",
    "dataShape": "ChartGraphic plus LineSeries.",
    "useWhen": "Use it to explain a change in context.",
    "tags": [
      "graphic",
      "annotation"
    ],
    "image": "/img/charts/component-graphic-callout.png"
  },
  {
    "slug": "component-graphic-band",
    "title": "Graphic band",
    "family": "Components and interaction",
    "description": "Graphic highlight and label overlay.",
    "dataShape": "ChartGraphic plus LineSeries.",
    "useWhen": "Use it for release windows and custom callouts.",
    "tags": [
      "graphic",
      "annotation"
    ],
    "image": "/img/charts/component-graphic-band.png"
  },
  {
    "slug": "component-visual-map",
    "title": "Visual map component",
    "family": "Components and interaction",
    "description": "Visible color scale for heatmap values.",
    "dataShape": "VisualMap plus HeatmapSeries.",
    "useWhen": "Use it where color represents a numeric value.",
    "tags": [
      "visualMap",
      "component"
    ],
    "image": "/img/charts/component-visual-map.png"
  },
  {
    "slug": "component-timeline-year",
    "title": "Timeline years",
    "family": "Components and interaction",
    "description": "Timeline component selecting a year.",
    "dataShape": "ChartTimeline plus series.",
    "useWhen": "Use it for step-based snapshots.",
    "tags": [
      "timeline",
      "component"
    ],
    "image": "/img/charts/component-timeline-year.png"
  },
  {
    "slug": "component-timeline-release",
    "title": "Timeline releases",
    "family": "Components and interaction",
    "description": "Timeline component for release phases.",
    "dataShape": "ChartTimeline plus series.",
    "useWhen": "Use it for phase or scenario playback.",
    "tags": [
      "timeline",
      "release"
    ],
    "image": "/img/charts/component-timeline-release.png"
  },
  {
    "slug": "bar3d-grid",
    "title": "3D grid bars",
    "family": "3D and GL",
    "description": "3D bars over a grid.",
    "dataShape": "Scene3D mesh cuboids.",
    "useWhen": "Use it when depth and grouping are part of the data.",
    "tags": [
      "3d",
      "bar"
    ],
    "image": "/img/charts/bar3d-grid.png"
  },
  {
    "slug": "bar3d-capacity",
    "title": "3D capacity bars",
    "family": "3D and GL",
    "description": "3D bar scene for capacity values.",
    "dataShape": "Scene3D mesh cuboids.",
    "useWhen": "Use it for spatial capacity displays.",
    "tags": [
      "3d",
      "bar"
    ],
    "image": "/img/charts/bar3d-capacity.png"
  },
  {
    "slug": "scatter3d-cluster",
    "title": "3D scatter cluster",
    "family": "3D and GL",
    "description": "3D scatter points in a native scene.",
    "dataShape": "Scene3D spheres.",
    "useWhen": "Use it for spatial sample clusters.",
    "tags": [
      "3d",
      "scatter"
    ],
    "image": "/img/charts/scatter3d-cluster.png"
  },
  {
    "slug": "scatter3d-outliers",
    "title": "3D scatter outliers",
    "family": "3D and GL",
    "description": "3D scatter variant with separated points.",
    "dataShape": "Scene3D spheres.",
    "useWhen": "Use it for spatial outlier inspection.",
    "tags": [
      "3d",
      "scatter"
    ],
    "image": "/img/charts/scatter3d-outliers.png"
  },
  {
    "slug": "line3d-trajectory",
    "title": "3D trajectory",
    "family": "3D and GL",
    "description": "3D trajectory through sampled points.",
    "dataShape": "Scene3D mesh segments plus spheres.",
    "useWhen": "Use it for movement paths.",
    "tags": [
      "3d",
      "line"
    ],
    "image": "/img/charts/line3d-trajectory.png"
  },
  {
    "slug": "line3d-spiral",
    "title": "3D spiral line",
    "family": "3D and GL",
    "description": "Spiral-like 3D line path.",
    "dataShape": "Scene3D mesh segments plus points.",
    "useWhen": "Use it for ordered spatial signals.",
    "tags": [
      "3d",
      "line"
    ],
    "image": "/img/charts/line3d-spiral.png"
  },
  {
    "slug": "surface3d-wave",
    "title": "3D wave surface",
    "family": "3D and GL",
    "description": "Smooth mesh surface.",
    "dataShape": "Scene3D mesh vertices and indices.",
    "useWhen": "Use it for sampled surfaces.",
    "tags": [
      "3d",
      "surface"
    ],
    "image": "/img/charts/surface3d-wave.png"
  },
  {
    "slug": "surface3d-terrain",
    "title": "3D terrain mesh",
    "family": "3D and GL",
    "description": "Terrain-like green surface mesh.",
    "dataShape": "Scene3D mesh vertices and indices.",
    "useWhen": "Use it for elevation and terrain fields.",
    "tags": [
      "3d",
      "terrain"
    ],
    "image": "/img/charts/surface3d-terrain.png"
  },
  {
    "slug": "point-cloud-dense",
    "title": "Dense point cloud",
    "family": "3D and GL",
    "description": "Dense cloud of spatial samples.",
    "dataShape": "Scene3D spheres.",
    "useWhen": "Use it for scan-like data.",
    "tags": [
      "3d",
      "point-cloud"
    ],
    "image": "/img/charts/point-cloud-dense.png"
  },
  {
    "slug": "point-cloud-sparse",
    "title": "Sparse point cloud",
    "family": "3D and GL",
    "description": "Sparse point cloud in 3D space.",
    "dataShape": "Scene3D spheres.",
    "useWhen": "Use it for sampled spatial observations.",
    "tags": [
      "3d",
      "point-cloud"
    ],
    "image": "/img/charts/point-cloud-sparse.png"
  },
  {
    "slug": "globe-markers",
    "title": "Globe markers",
    "family": "3D and GL",
    "description": "Globe primitive with marker locations.",
    "dataShape": "Scene3D spheres.",
    "useWhen": "Use it for global context.",
    "tags": [
      "3d",
      "globe"
    ],
    "image": "/img/charts/globe-markers.png"
  },
  {
    "slug": "globe-coverage",
    "title": "Globe coverage",
    "family": "3D and GL",
    "description": "Globe variant for coverage displays.",
    "dataShape": "Scene3D spheres.",
    "useWhen": "Use it for global product surfaces.",
    "tags": [
      "3d",
      "globe"
    ],
    "image": "/img/charts/globe-coverage.png"
  },
  {
    "slug": "graph3d-network",
    "title": "3D network",
    "family": "3D and GL",
    "description": "3D graph nodes and links.",
    "dataShape": "Scene3D nodes and segment meshes.",
    "useWhen": "Use it for topology in depth.",
    "tags": [
      "3d",
      "graph"
    ],
    "image": "/img/charts/graph3d-network.png"
  },
  {
    "slug": "graph3d-topology",
    "title": "3D topology",
    "family": "3D and GL",
    "description": "3D topology graph variant.",
    "dataShape": "Scene3D nodes and segment meshes.",
    "useWhen": "Use it for spatial relationship maps.",
    "tags": [
      "3d",
      "graph"
    ],
    "image": "/img/charts/graph3d-topology.png"
  },
  {
    "slug": "mesh-surface",
    "title": "Mesh surface",
    "family": "3D and GL",
    "description": "Generic mesh-rendered surface.",
    "dataShape": "Scene3D mesh primitive.",
    "useWhen": "Use it for custom mesh data.",
    "tags": [
      "3d",
      "mesh"
    ],
    "image": "/img/charts/mesh-surface.png"
  },
  {
    "slug": "volume-style",
    "title": "Volume-style point field",
    "family": "3D and GL",
    "description": "Dense point field suggesting volume data.",
    "dataShape": "Scene3D spheres.",
    "useWhen": "Use it as the current native path toward volume-style visualization.",
    "tags": [
      "3d",
      "volume"
    ],
    "image": "/img/charts/volume-style.png"
  },
  {
    "slug": "dataset-encoded-bars",
    "title": "Encoded bars dataset",
    "family": "Dataset and dynamic",
    "description": "Grouped bars generated from named encoded fields.",
    "dataShape": "Records with category, group, and numeric value fields.",
    "useWhen": "Use it when chart code should name fields once and reuse them across bar variants.",
    "tags": [
      "dataset",
      "bar"
    ],
    "image": "/img/charts/dataset-encoded-bars.png"
  },
  {
    "slug": "dataset-encoded-lines",
    "title": "Encoded lines dataset",
    "family": "Dataset and dynamic",
    "description": "Line series generated from named encoded fields.",
    "dataShape": "Records with ordered label, measure, and series fields.",
    "useWhen": "Use it when the same dataset powers several line views.",
    "tags": [
      "dataset",
      "line"
    ],
    "image": "/img/charts/dataset-encoded-lines.png"
  },
  {
    "slug": "dataset-stack-area",
    "title": "Dataset stacked area",
    "family": "Dataset and dynamic",
    "description": "Stacked area view built from reusable dataset-like series.",
    "dataShape": "Several named measures sharing ordered categories.",
    "useWhen": "Use it when the data pipeline should stay visible instead of hidden in chart-specific arrays.",
    "tags": [
      "dataset",
      "stack"
    ],
    "image": "/img/charts/dataset-stack-area.png"
  },
  {
    "slug": "visual-map-scatter",
    "title": "Visual map scatter",
    "family": "Dataset and dynamic",
    "description": "Bubble scatter with value-driven size and color scale.",
    "dataShape": "Triples of x, y, and magnitude values.",
    "useWhen": "Use it when density and magnitude both matter.",
    "tags": [
      "visualMap",
      "scatter"
    ],
    "image": "/img/charts/visual-map-scatter.png"
  },
  {
    "slug": "visual-map-heatmap",
    "title": "Visual map heatmap",
    "family": "Dataset and dynamic",
    "description": "Heatmap with a continuous value scale.",
    "dataShape": "Grid cell coordinates with numeric intensity.",
    "useWhen": "Use it for dense comparisons where color communicates rank faster than labels.",
    "tags": [
      "visualMap",
      "heatmap"
    ],
    "image": "/img/charts/visual-map-heatmap.png"
  },
  {
    "slug": "visual-map-calendar",
    "title": "Visual map calendar",
    "family": "Dataset and dynamic",
    "description": "Calendar heatmap with a continuous value scale.",
    "dataShape": "Date/value pairs over a calendar range.",
    "useWhen": "Use it to show daily volume, incidents, or activity in a compact year-like shape.",
    "tags": [
      "visualMap",
      "calendar"
    ],
    "image": "/img/charts/visual-map-calendar.png"
  },
  {
    "slug": "dynamic-gauge-speed",
    "title": "Dynamic speed gauge",
    "family": "Dataset and dynamic",
    "description": "Gauge configured for a changing bounded measure.",
    "dataShape": "Single label/value pair in a known range.",
    "useWhen": "Use it for operational status values where one number dominates the view.",
    "tags": [
      "dynamic",
      "gauge"
    ],
    "image": "/img/charts/dynamic-gauge-speed.png"
  },
  {
    "slug": "dynamic-gauge-score",
    "title": "Dynamic score gauge",
    "family": "Dataset and dynamic",
    "description": "Gauge configured for a score or quality metric.",
    "dataShape": "Single label/value pair scaled into a gauge arc.",
    "useWhen": "Use it for scorecards where the main result should be immediately visible.",
    "tags": [
      "dynamic",
      "gauge"
    ],
    "image": "/img/charts/dynamic-gauge-score.png"
  },
  {
    "slug": "dynamic-effect-alerts",
    "title": "Dynamic alert scatter",
    "family": "Dataset and dynamic",
    "description": "Effect scatter highlighting important points over numeric axes.",
    "dataShape": "Small list of highlighted x/y samples.",
    "useWhen": "Use it when outliers or active alerts need immediate attention.",
    "tags": [
      "dynamic",
      "scatter"
    ],
    "image": "/img/charts/dynamic-effect-alerts.png"
  },
  {
    "slug": "dynamic-pictorial-units",
    "title": "Dynamic pictorial units",
    "family": "Dataset and dynamic",
    "description": "Pictorial bar using symbolic marks for count-like values.",
    "dataShape": "Category labels plus numeric values and a symbol choice.",
    "useWhen": "Use it for unit counts that benefit from a branded or icon-like visual form.",
    "tags": [
      "dynamic",
      "pictorial"
    ],
    "image": "/img/charts/dynamic-pictorial-units.png"
  },
  {
    "slug": "dynamic-funnel-sales",
    "title": "Dynamic sales funnel",
    "family": "Dataset and dynamic",
    "description": "Funnel showing staged conversion through a pipeline.",
    "dataShape": "Stage/value pairs ordered by process position.",
    "useWhen": "Use it when the loss between stages matters more than exact axis measurement.",
    "tags": [
      "dynamic",
      "funnel"
    ],
    "image": "/img/charts/dynamic-funnel-sales.png"
  },
  {
    "slug": "dynamic-polar-score",
    "title": "Dynamic polar score",
    "family": "Dataset and dynamic",
    "description": "Polar bars arranged around a radial scale.",
    "dataShape": "Label/value pairs mapped around a circle.",
    "useWhen": "Use it for cyclic or radial score comparisons.",
    "tags": [
      "dynamic",
      "polar"
    ],
    "image": "/img/charts/dynamic-polar-score.png"
  },
  {
    "slug": "dynamic-radar-health",
    "title": "Dynamic radar health",
    "family": "Dataset and dynamic",
    "description": "Radar profile with multiple dimensions on one shape.",
    "dataShape": "Several same-length vectors across named indicators.",
    "useWhen": "Use it for multidimensional status summaries.",
    "tags": [
      "dynamic",
      "radar"
    ],
    "image": "/img/charts/dynamic-radar-health.png"
  },
  {
    "slug": "dynamic-single-axis-events",
    "title": "Dynamic single-axis events",
    "family": "Dataset and dynamic",
    "description": "Single-axis event distribution with value-coded marks.",
    "dataShape": "Position/value pairs on one continuous axis.",
    "useWhen": "Use it for timelines or compact event strips.",
    "tags": [
      "dynamic",
      "singleAxis"
    ],
    "image": "/img/charts/dynamic-single-axis-events.png"
  },
  {
    "slug": "dynamic-brush-scatter",
    "title": "Dynamic brush scatter",
    "family": "Dataset and dynamic",
    "description": "Scatter chart with a visible brush selection preview.",
    "dataShape": "Point samples plus a brush rectangle.",
    "useWhen": "Use it when users need to select a region before drilling into data.",
    "tags": [
      "dynamic",
      "brush"
    ],
    "image": "/img/charts/dynamic-brush-scatter.png"
  },
  {
    "slug": "dynamic-toolbox-line",
    "title": "Dynamic toolbox line",
    "family": "Dataset and dynamic",
    "description": "Line chart with explicit chart utility actions.",
    "dataShape": "Ordered samples plus toolbox action configuration.",
    "useWhen": "Use it when zoom, brush, restore, and export controls belong with the chart.",
    "tags": [
      "dynamic",
      "toolbox"
    ],
    "image": "/img/charts/dynamic-toolbox-line.png"
  },
  {
    "slug": "line-service-latency",
    "title": "Service latency trend",
    "family": "Line",
    "description": "Service latency trend uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Latency samples over ordered time buckets.",
    "useWhen": "Use it for operations screens that need drift and spikes in one glance.",
    "tags": [
      "line",
      "latency"
    ],
    "image": "/img/charts/line-service-latency.png"
  },
  {
    "slug": "line-error-budget",
    "title": "Error budget burn",
    "family": "Line",
    "description": "Error budget burn uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Budget percentage samples plus threshold marks.",
    "useWhen": "Use it when teams need to see whether a service is inside its operating band.",
    "tags": [
      "line",
      "slo"
    ],
    "image": "/img/charts/line-error-budget.png"
  },
  {
    "slug": "line-capacity-forecast",
    "title": "Capacity forecast band",
    "family": "Line",
    "description": "Capacity forecast band uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Observed values with a highlighted expected range.",
    "useWhen": "Use it when a forecast needs both the line and the safe range.",
    "tags": [
      "line",
      "forecast"
    ],
    "image": "/img/charts/line-capacity-forecast.png"
  },
  {
    "slug": "line-release-window",
    "title": "Release window annotation",
    "family": "Line",
    "description": "Release window annotation uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Ordered samples with a graphic callout over the plot.",
    "useWhen": "Use it when a chart needs to explain why a trend changed.",
    "tags": [
      "line",
      "annotation"
    ],
    "image": "/img/charts/line-release-window.png"
  },
  {
    "slug": "line-api-throughput",
    "title": "API throughput line",
    "family": "Line",
    "description": "API throughput line uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Dense ordered samples from a service counter.",
    "useWhen": "Use it for telemetry panels with many points.",
    "tags": [
      "line",
      "large"
    ],
    "image": "/img/charts/line-api-throughput.png"
  },
  {
    "slug": "line-signup-cohorts",
    "title": "Signup cohort comparison",
    "family": "Line",
    "description": "Signup cohort comparison uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Two ordered series sharing the same axis.",
    "useWhen": "Use it when users need to compare cohorts over the same time range.",
    "tags": [
      "line",
      "comparison"
    ],
    "image": "/img/charts/line-signup-cohorts.png"
  },
  {
    "slug": "line-conversion-stack",
    "title": "Conversion stack area",
    "family": "Line",
    "description": "Conversion stack area uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Several ordered series sharing a stack key.",
    "useWhen": "Use it to explain how categories make up a total over time.",
    "tags": [
      "area",
      "stack"
    ],
    "image": "/img/charts/line-conversion-stack.png"
  },
  {
    "slug": "line-operational-steps",
    "title": "Operational step series",
    "family": "Line",
    "description": "Operational step series uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Discrete states represented as stepped values.",
    "useWhen": "Use it for state changes, quotas, or inventory counts.",
    "tags": [
      "line",
      "step"
    ],
    "image": "/img/charts/line-operational-steps.png"
  },
  {
    "slug": "line-inventory-end-step",
    "title": "Inventory end-step",
    "family": "Line",
    "description": "Inventory end-step uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Discrete samples that update at the end of each interval.",
    "useWhen": "Use it when the value changes after the period closes.",
    "tags": [
      "line",
      "step"
    ],
    "image": "/img/charts/line-inventory-end-step.png"
  },
  {
    "slug": "line-retention-window",
    "title": "Retention zoom window",
    "family": "Line",
    "description": "Retention zoom window uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Long ordered samples with a visible zoom window.",
    "useWhen": "Use it when the primary view should focus on one time range.",
    "tags": [
      "line",
      "zoom"
    ],
    "image": "/img/charts/line-retention-window.png"
  },
  {
    "slug": "line-alert-annotations",
    "title": "Alert annotations",
    "family": "Line",
    "description": "Alert annotations uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Ordered values with event markers.",
    "useWhen": "Use it when incidents or milestones need to stay attached to the trend.",
    "tags": [
      "line",
      "markers"
    ],
    "image": "/img/charts/line-alert-annotations.png"
  },
  {
    "slug": "line-revenue-seasonality",
    "title": "Revenue seasonality",
    "family": "Line",
    "description": "Revenue seasonality uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Ordered samples over a repeated seasonal interval.",
    "useWhen": "Use it when weekly or monthly rhythm is the main signal.",
    "tags": [
      "line",
      "seasonal"
    ],
    "image": "/img/charts/line-revenue-seasonality.png"
  },
  {
    "slug": "line-quality-band",
    "title": "Quality operating band",
    "family": "Line",
    "description": "Quality operating band uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Samples with a target and acceptable range.",
    "useWhen": "Use it for production quality metrics with explicit guardrails.",
    "tags": [
      "line",
      "band"
    ],
    "image": "/img/charts/line-quality-band.png"
  },
  {
    "slug": "line-traffic-rolling-average",
    "title": "Traffic rolling average",
    "family": "Line",
    "description": "Traffic rolling average uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Raw and smoothed ordered series.",
    "useWhen": "Use it when the user needs the current signal and the trend baseline.",
    "tags": [
      "line",
      "average"
    ],
    "image": "/img/charts/line-traffic-rolling-average.png"
  },
  {
    "slug": "line-demand-spark",
    "title": "Demand sparkline",
    "family": "Line",
    "description": "Demand sparkline uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Compact dense ordered values.",
    "useWhen": "Use it inside dense dashboard cards.",
    "tags": [
      "line",
      "dense"
    ],
    "image": "/img/charts/line-demand-spark.png"
  },
  {
    "slug": "line-market-index",
    "title": "Market index line",
    "family": "Line",
    "description": "Market index line uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Indexed ordered values on a value axis.",
    "useWhen": "Use it when direction and volatility matter more than individual samples.",
    "tags": [
      "line",
      "finance"
    ],
    "image": "/img/charts/line-market-index.png"
  },
  {
    "slug": "line-support-volume",
    "title": "Support volume area",
    "family": "Line",
    "description": "Support volume area uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Support counts with an area fill.",
    "useWhen": "Use it when volume should be visible at a glance.",
    "tags": [
      "area",
      "support"
    ],
    "image": "/img/charts/line-support-volume.png"
  },
  {
    "slug": "line-deployment-events",
    "title": "Deployment event line",
    "family": "Line",
    "description": "Deployment event line uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Trend values with deployment markers.",
    "useWhen": "Use it to connect product changes to metric movement.",
    "tags": [
      "line",
      "deployment"
    ],
    "image": "/img/charts/line-deployment-events.png"
  },
  {
    "slug": "line-region-comparison",
    "title": "Regional trend comparison",
    "family": "Line",
    "description": "Regional trend comparison uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Two regional series on shared axes.",
    "useWhen": "Use it when teams compare regions over the same period.",
    "tags": [
      "line",
      "region"
    ],
    "image": "/img/charts/line-region-comparison.png"
  },
  {
    "slug": "line-product-mix-area",
    "title": "Product mix area",
    "family": "Line",
    "description": "Product mix area uses Fission Charts typed Rust data to render a production-ready line view.",
    "dataShape": "Stacked product series over ordered buckets.",
    "useWhen": "Use it when the total and category mix both matter.",
    "tags": [
      "area",
      "product"
    ],
    "image": "/img/charts/line-product-mix-area.png"
  },
  {
    "slug": "bar-region-rank",
    "title": "Region ranking",
    "family": "Bar",
    "description": "Region ranking uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Category/value pairs sorted for comparison.",
    "useWhen": "Use it when exact rank is the product question.",
    "tags": [
      "bar",
      "ranking"
    ],
    "image": "/img/charts/bar-region-rank.png"
  },
  {
    "slug": "bar-profit-loss",
    "title": "Profit and loss bars",
    "family": "Bar",
    "description": "Profit and loss bars uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Positive and negative values on one value axis.",
    "useWhen": "Use it when gains and losses must share one baseline.",
    "tags": [
      "bar",
      "negative"
    ],
    "image": "/img/charts/bar-profit-loss.png"
  },
  {
    "slug": "bar-quarter-waterfall",
    "title": "Quarter waterfall",
    "family": "Bar",
    "description": "Quarter waterfall uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Sequential deltas that explain a final value.",
    "useWhen": "Use it for financial bridge and variance analysis.",
    "tags": [
      "bar",
      "waterfall"
    ],
    "image": "/img/charts/bar-quarter-waterfall.png"
  },
  {
    "slug": "bar-sales-target-track",
    "title": "Sales target track",
    "family": "Bar",
    "description": "Sales target track uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Values against a fixed target background.",
    "useWhen": "Use it when completion against capacity is more important than raw count.",
    "tags": [
      "bar",
      "progress"
    ],
    "image": "/img/charts/bar-sales-target-track.png"
  },
  {
    "slug": "bar-store-comparison",
    "title": "Store comparison",
    "family": "Bar",
    "description": "Store comparison uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Grouped category values across periods.",
    "useWhen": "Use it to compare periods inside each category.",
    "tags": [
      "bar",
      "grouped"
    ],
    "image": "/img/charts/bar-store-comparison.png"
  },
  {
    "slug": "bar-channel-stack",
    "title": "Channel stack",
    "family": "Bar",
    "description": "Channel stack uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Multiple series stacked per category.",
    "useWhen": "Use it when total and contribution both matter.",
    "tags": [
      "bar",
      "stack"
    ],
    "image": "/img/charts/bar-channel-stack.png"
  },
  {
    "slug": "bar-return-deltas",
    "title": "Return deltas",
    "family": "Bar",
    "description": "Return deltas uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Category deltas with negative values.",
    "useWhen": "Use it for change analysis around zero.",
    "tags": [
      "bar",
      "delta"
    ],
    "image": "/img/charts/bar-return-deltas.png"
  },
  {
    "slug": "bar-priority-queue",
    "title": "Priority queue bars",
    "family": "Bar",
    "description": "Priority queue bars uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Rounded bars for a compact queue view.",
    "useWhen": "Use it when bars sit inside a polished app surface.",
    "tags": [
      "bar",
      "rounded"
    ],
    "image": "/img/charts/bar-priority-queue.png"
  },
  {
    "slug": "bar-funnel-units",
    "title": "Funnel unit pictorial bars",
    "family": "Bar",
    "description": "Funnel unit pictorial bars uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Values represented by repeated symbols.",
    "useWhen": "Use it when unit counts should feel more tactile than rectangles.",
    "tags": [
      "bar",
      "symbol"
    ],
    "image": "/img/charts/bar-funnel-units.png"
  },
  {
    "slug": "bar-utilization-capacity",
    "title": "Utilization capacity",
    "family": "Bar",
    "description": "Utilization capacity uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Usage bars with capacity tracks.",
    "useWhen": "Use it for infrastructure and quota dashboards.",
    "tags": [
      "bar",
      "capacity"
    ],
    "image": "/img/charts/bar-utilization-capacity.png"
  },
  {
    "slug": "bar-customer-segments",
    "title": "Customer segment bars",
    "family": "Bar",
    "description": "Customer segment bars uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Segment values grouped by period.",
    "useWhen": "Use it to compare segments without splitting the page.",
    "tags": [
      "bar",
      "segment"
    ],
    "image": "/img/charts/bar-customer-segments.png"
  },
  {
    "slug": "bar-budget-variance",
    "title": "Budget variance",
    "family": "Bar",
    "description": "Budget variance uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Signed budget variances on a horizontal axis.",
    "useWhen": "Use it when over and under budget must be symmetric.",
    "tags": [
      "bar",
      "variance"
    ],
    "image": "/img/charts/bar-budget-variance.png"
  },
  {
    "slug": "bar-ticket-age",
    "title": "Ticket age distribution",
    "family": "Bar",
    "description": "Ticket age distribution uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Buckets with counts per age range.",
    "useWhen": "Use it when the reader needs the shape of a queue.",
    "tags": [
      "bar",
      "distribution"
    ],
    "image": "/img/charts/bar-ticket-age.png"
  },
  {
    "slug": "bar-performance-bands",
    "title": "Performance bands",
    "family": "Bar",
    "description": "Performance bands uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Values shown against a visual capacity track.",
    "useWhen": "Use it for scorecards where progress is the dominant signal.",
    "tags": [
      "bar",
      "band"
    ],
    "image": "/img/charts/bar-performance-bands.png"
  },
  {
    "slug": "bar-retail-waterfall",
    "title": "Retail waterfall",
    "family": "Bar",
    "description": "Retail waterfall uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Sequential sales and cost changes.",
    "useWhen": "Use it for retail contribution analysis.",
    "tags": [
      "bar",
      "retail"
    ],
    "image": "/img/charts/bar-retail-waterfall.png"
  },
  {
    "slug": "bar-team-load",
    "title": "Team load bars",
    "family": "Bar",
    "description": "Team load bars uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Team categories ranked by workload.",
    "useWhen": "Use it when managers need quick capacity comparison.",
    "tags": [
      "bar",
      "team"
    ],
    "image": "/img/charts/bar-team-load.png"
  },
  {
    "slug": "bar-product-stack",
    "title": "Product stack bars",
    "family": "Bar",
    "description": "Product stack bars uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Product values stacked into category totals.",
    "useWhen": "Use it for portfolio composition.",
    "tags": [
      "bar",
      "product"
    ],
    "image": "/img/charts/bar-product-stack.png"
  },
  {
    "slug": "bar-weekday-shape",
    "title": "Weekday shape bars",
    "family": "Bar",
    "description": "Weekday shape bars uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Seven ordered weekday values.",
    "useWhen": "Use it when the weekly rhythm is easier as bars than a line.",
    "tags": [
      "bar",
      "weekday"
    ],
    "image": "/img/charts/bar-weekday-shape.png"
  },
  {
    "slug": "bar-benchmark-comparison",
    "title": "Benchmark comparison",
    "family": "Bar",
    "description": "Benchmark comparison uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Actual and benchmark values side by side.",
    "useWhen": "Use it when each category needs a direct benchmark.",
    "tags": [
      "bar",
      "benchmark"
    ],
    "image": "/img/charts/bar-benchmark-comparison.png"
  },
  {
    "slug": "bar-sla-breach-delta",
    "title": "SLA breach delta",
    "family": "Bar",
    "description": "SLA breach delta uses Fission Charts typed Rust data to render a production-ready bar view.",
    "dataShape": "Deltas above and below a service baseline.",
    "useWhen": "Use it for operational exception reporting.",
    "tags": [
      "bar",
      "sla"
    ],
    "image": "/img/charts/bar-sla-breach-delta.png"
  },
  {
    "slug": "boxplot-api-latency",
    "title": "API latency boxplot",
    "family": "Statistical",
    "description": "API latency boxplot uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "Five-number summaries across service groups.",
    "useWhen": "Use it to compare distributions instead of averages.",
    "tags": [
      "boxplot",
      "latency"
    ],
    "image": "/img/charts/boxplot-api-latency.png"
  },
  {
    "slug": "boxplot-quality-spread",
    "title": "Quality spread boxplot",
    "family": "Statistical",
    "description": "Quality spread boxplot uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "Five-number summaries for quality scores.",
    "useWhen": "Use it when spread and outliers matter.",
    "tags": [
      "boxplot",
      "quality"
    ],
    "image": "/img/charts/boxplot-quality-spread.png"
  },
  {
    "slug": "candlestick-equity-session",
    "title": "Equity session candlestick",
    "family": "Statistical",
    "description": "Equity session candlestick uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "Open, close, low, and high values by period.",
    "useWhen": "Use it for financial price movement.",
    "tags": [
      "candlestick",
      "finance"
    ],
    "image": "/img/charts/candlestick-equity-session.png"
  },
  {
    "slug": "candlestick-crypto-session",
    "title": "Crypto session candlestick",
    "family": "Statistical",
    "description": "Crypto session candlestick uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "OHLC values over intraday buckets.",
    "useWhen": "Use it when price direction and range must be visible together.",
    "tags": [
      "candlestick",
      "finance"
    ],
    "image": "/img/charts/candlestick-crypto-session.png"
  },
  {
    "slug": "scatter-quality-outliers",
    "title": "Quality outlier scatter",
    "family": "Statistical",
    "description": "Quality outlier scatter uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "Highlighted x/y points over value axes.",
    "useWhen": "Use it when outliers should attract attention immediately.",
    "tags": [
      "scatter",
      "outlier"
    ],
    "image": "/img/charts/scatter-quality-outliers.png"
  },
  {
    "slug": "scatter-risk-bubbles",
    "title": "Risk bubble matrix",
    "family": "Statistical",
    "description": "Risk bubble matrix uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "X, y, and magnitude triples.",
    "useWhen": "Use it when size carries a third measure.",
    "tags": [
      "scatter",
      "bubble"
    ],
    "image": "/img/charts/scatter-risk-bubbles.png"
  },
  {
    "slug": "scatter-portfolio-return",
    "title": "Portfolio risk return",
    "family": "Statistical",
    "description": "Portfolio risk return uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "Risk, return, and exposure values.",
    "useWhen": "Use it to compare assets or initiatives with three measures.",
    "tags": [
      "scatter",
      "portfolio"
    ],
    "image": "/img/charts/scatter-portfolio-return.png"
  },
  {
    "slug": "scatter-lab-samples",
    "title": "Lab sample scatter",
    "family": "Statistical",
    "description": "Lab sample scatter uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "Independent x/y samples.",
    "useWhen": "Use it for correlation and clustering questions.",
    "tags": [
      "scatter",
      "samples"
    ],
    "image": "/img/charts/scatter-lab-samples.png"
  },
  {
    "slug": "funnel-activation",
    "title": "Activation funnel",
    "family": "Statistical",
    "description": "Activation funnel uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "Ordered conversion stage values.",
    "useWhen": "Use it when conversion loss matters more than a shared axis.",
    "tags": [
      "funnel",
      "conversion"
    ],
    "image": "/img/charts/funnel-activation.png"
  },
  {
    "slug": "funnel-support-resolution",
    "title": "Support resolution funnel",
    "family": "Statistical",
    "description": "Support resolution funnel uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "Stage/value pairs through a process.",
    "useWhen": "Use it for operational process attrition.",
    "tags": [
      "funnel",
      "support"
    ],
    "image": "/img/charts/funnel-support-resolution.png"
  },
  {
    "slug": "parallel-device-quality",
    "title": "Device quality parallel",
    "family": "Statistical",
    "description": "Device quality parallel uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "Rows of comparable numeric dimensions.",
    "useWhen": "Use it when each item has several independent measures.",
    "tags": [
      "parallel",
      "quality"
    ],
    "image": "/img/charts/parallel-device-quality.png"
  },
  {
    "slug": "parallel-plan-comparison",
    "title": "Plan comparison parallel",
    "family": "Statistical",
    "description": "Plan comparison parallel uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "Multi-dimensional rows on parallel axes.",
    "useWhen": "Use it when tradeoffs across measures matter.",
    "tags": [
      "parallel",
      "comparison"
    ],
    "image": "/img/charts/parallel-plan-comparison.png"
  },
  {
    "slug": "single-axis-release-events",
    "title": "Release event strip",
    "family": "Statistical",
    "description": "Release event strip uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "Position/value pairs on one axis.",
    "useWhen": "Use it for compact event timelines.",
    "tags": [
      "singleAxis",
      "events"
    ],
    "image": "/img/charts/single-axis-release-events.png"
  },
  {
    "slug": "single-axis-job-runtime",
    "title": "Job runtime strip",
    "family": "Statistical",
    "description": "Job runtime strip uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "Job positions with duration or intensity values.",
    "useWhen": "Use it for queue and schedule views.",
    "tags": [
      "singleAxis",
      "runtime"
    ],
    "image": "/img/charts/single-axis-job-runtime.png"
  },
  {
    "slug": "scatter-alert-hotspots",
    "title": "Alert hotspot scatter",
    "family": "Statistical",
    "description": "Alert hotspot scatter uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "Selected alert points over numeric axes.",
    "useWhen": "Use it when only important events should pulse visually.",
    "tags": [
      "scatter",
      "alert"
    ],
    "image": "/img/charts/scatter-alert-hotspots.png"
  },
  {
    "slug": "boxplot-region-spread",
    "title": "Region spread boxplot",
    "family": "Statistical",
    "description": "Region spread boxplot uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "Five-number summaries by region.",
    "useWhen": "Use it for distribution comparison across regions.",
    "tags": [
      "boxplot",
      "region"
    ],
    "image": "/img/charts/boxplot-region-spread.png"
  },
  {
    "slug": "candlestick-volume-shift",
    "title": "Volume shift candlestick",
    "family": "Statistical",
    "description": "Volume shift candlestick uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "OHLC-like movement values by period.",
    "useWhen": "Use it when directional change and range both matter.",
    "tags": [
      "candlestick",
      "volume"
    ],
    "image": "/img/charts/candlestick-volume-shift.png"
  },
  {
    "slug": "scatter-efficiency-frontier",
    "title": "Efficiency frontier",
    "family": "Statistical",
    "description": "Efficiency frontier uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "Efficiency points over value axes.",
    "useWhen": "Use it to identify leading and lagging points.",
    "tags": [
      "scatter",
      "efficiency"
    ],
    "image": "/img/charts/scatter-efficiency-frontier.png"
  },
  {
    "slug": "bubble-customer-value",
    "title": "Customer value bubbles",
    "family": "Statistical",
    "description": "Customer value bubbles uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "X, y, and value triples for customers or segments.",
    "useWhen": "Use it when business value should influence point size.",
    "tags": [
      "bubble",
      "customer"
    ],
    "image": "/img/charts/bubble-customer-value.png"
  },
  {
    "slug": "parallel-risk-score",
    "title": "Risk score parallel",
    "family": "Statistical",
    "description": "Risk score parallel uses Fission Charts typed Rust data to render a production-ready statistical view.",
    "dataShape": "Rows of risk dimensions on parallel axes.",
    "useWhen": "Use it for multidimensional risk assessment.",
    "tags": [
      "parallel",
      "risk"
    ],
    "image": "/img/charts/parallel-risk-score.png"
  },
  {
    "slug": "pie-plan-share",
    "title": "Plan share pie",
    "family": "Radial",
    "description": "Plan share pie uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "Label/value pairs for a small whole.",
    "useWhen": "Use it when part-to-whole reading is the main task.",
    "tags": [
      "pie",
      "share"
    ],
    "image": "/img/charts/pie-plan-share.png"
  },
  {
    "slug": "pie-device-donut",
    "title": "Device donut",
    "family": "Radial",
    "description": "Device donut uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "Label/value pairs with an open center.",
    "useWhen": "Use it when the center can carry a total or primary label.",
    "tags": [
      "donut",
      "device"
    ],
    "image": "/img/charts/pie-device-donut.png"
  },
  {
    "slug": "pie-market-rose",
    "title": "Market rose",
    "family": "Radial",
    "description": "Market rose uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "Label/value pairs with radius emphasis.",
    "useWhen": "Use it when presentation value is high and categories are few.",
    "tags": [
      "rose",
      "market"
    ],
    "image": "/img/charts/pie-market-rose.png"
  },
  {
    "slug": "pie-exposure-rose",
    "title": "Exposure area rose",
    "family": "Radial",
    "description": "Exposure area rose uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "Label/value pairs with area-oriented rose layout.",
    "useWhen": "Use it when relative shape is more important than a precise angle.",
    "tags": [
      "rose",
      "area"
    ],
    "image": "/img/charts/pie-exposure-rose.png"
  },
  {
    "slug": "gauge-availability",
    "title": "Availability gauge",
    "family": "Radial",
    "description": "Availability gauge uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "One bounded value mapped to an arc.",
    "useWhen": "Use it when a single current status dominates the screen.",
    "tags": [
      "gauge",
      "availability"
    ],
    "image": "/img/charts/gauge-availability.png"
  },
  {
    "slug": "gauge-deploy-health",
    "title": "Deploy health gauge",
    "family": "Radial",
    "description": "Deploy health gauge uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "Single score value in a bounded range.",
    "useWhen": "Use it for health and readiness summaries.",
    "tags": [
      "gauge",
      "health"
    ],
    "image": "/img/charts/gauge-deploy-health.png"
  },
  {
    "slug": "polar-hourly-load",
    "title": "Hourly load polar bars",
    "family": "Radial",
    "description": "Hourly load polar bars uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "Label/value pairs around a circle.",
    "useWhen": "Use it when cyclic position is meaningful.",
    "tags": [
      "polar",
      "bar"
    ],
    "image": "/img/charts/polar-hourly-load.png"
  },
  {
    "slug": "polar-wind-speed",
    "title": "Wind speed polar line",
    "family": "Radial",
    "description": "Wind speed polar line uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "Angle/radius samples.",
    "useWhen": "Use it for directional or cyclic measures.",
    "tags": [
      "polar",
      "line"
    ],
    "image": "/img/charts/polar-wind-speed.png"
  },
  {
    "slug": "radar-product-fit",
    "title": "Product fit radar",
    "family": "Radial",
    "description": "Product fit radar uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "Several same-length dimensional vectors.",
    "useWhen": "Use it to compare shapes across a few dimensions.",
    "tags": [
      "radar",
      "profile"
    ],
    "image": "/img/charts/radar-product-fit.png"
  },
  {
    "slug": "radar-service-health",
    "title": "Service health radar",
    "family": "Radial",
    "description": "Service health radar uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "Health dimensions on a radial profile.",
    "useWhen": "Use it for compact status summaries.",
    "tags": [
      "radar",
      "health"
    ],
    "image": "/img/charts/radar-service-health.png"
  },
  {
    "slug": "pie-revenue-mix",
    "title": "Revenue mix donut",
    "family": "Radial",
    "description": "Revenue mix donut uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "Revenue categories as label/value pairs.",
    "useWhen": "Use it when contribution to total revenue matters.",
    "tags": [
      "donut",
      "revenue"
    ],
    "image": "/img/charts/pie-revenue-mix.png"
  },
  {
    "slug": "pie-source-mix",
    "title": "Source mix pie",
    "family": "Radial",
    "description": "Source mix pie uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "Traffic or source categories with values.",
    "useWhen": "Use it for small source distributions.",
    "tags": [
      "pie",
      "source"
    ],
    "image": "/img/charts/pie-source-mix.png"
  },
  {
    "slug": "gauge-build-confidence",
    "title": "Build confidence gauge",
    "family": "Radial",
    "description": "Build confidence gauge uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "One confidence score on a gauge arc.",
    "useWhen": "Use it when a status panel needs a primary quality number.",
    "tags": [
      "gauge",
      "build"
    ],
    "image": "/img/charts/gauge-build-confidence.png"
  },
  {
    "slug": "polar-seasonality-bars",
    "title": "Seasonality polar bars",
    "family": "Radial",
    "description": "Seasonality polar bars uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "Cyclic label/value pairs.",
    "useWhen": "Use it when the shape wraps around a repeated cycle.",
    "tags": [
      "polar",
      "seasonal"
    ],
    "image": "/img/charts/polar-seasonality-bars.png"
  },
  {
    "slug": "radar-team-balance",
    "title": "Team balance radar",
    "family": "Radial",
    "description": "Team balance radar uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "Team capability vectors.",
    "useWhen": "Use it when balance across dimensions is the main reading.",
    "tags": [
      "radar",
      "team"
    ],
    "image": "/img/charts/radar-team-balance.png"
  },
  {
    "slug": "pie-expense-donut",
    "title": "Expense donut",
    "family": "Radial",
    "description": "Expense donut uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "Expense categories as a small whole.",
    "useWhen": "Use it for budget composition.",
    "tags": [
      "donut",
      "expense"
    ],
    "image": "/img/charts/pie-expense-donut.png"
  },
  {
    "slug": "pie-risk-rose",
    "title": "Risk rose",
    "family": "Radial",
    "description": "Risk rose uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "Risk category values in radial form.",
    "useWhen": "Use it for executive risk snapshots where visual emphasis helps.",
    "tags": [
      "rose",
      "risk"
    ],
    "image": "/img/charts/pie-risk-rose.png"
  },
  {
    "slug": "polar-cycle-line",
    "title": "Cycle polar line",
    "family": "Radial",
    "description": "Cycle polar line uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "Angle/radius samples around a cycle.",
    "useWhen": "Use it for circular process metrics.",
    "tags": [
      "polar",
      "cycle"
    ],
    "image": "/img/charts/polar-cycle-line.png"
  },
  {
    "slug": "gauge-capacity-headroom",
    "title": "Capacity headroom gauge",
    "family": "Radial",
    "description": "Capacity headroom gauge uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "Single capacity value in a bounded range.",
    "useWhen": "Use it when headroom is the only number that matters.",
    "tags": [
      "gauge",
      "capacity"
    ],
    "image": "/img/charts/gauge-capacity-headroom.png"
  },
  {
    "slug": "radar-platform-readiness",
    "title": "Platform readiness radar",
    "family": "Radial",
    "description": "Platform readiness radar uses Fission Charts typed Rust data to render a production-ready radial view.",
    "dataShape": "Platform dimensions as comparable vectors.",
    "useWhen": "Use it to summarize readiness across product areas.",
    "tags": [
      "radar",
      "platform"
    ],
    "image": "/img/charts/radar-platform-readiness.png"
  },
  {
    "slug": "heatmap-deployment-hours",
    "title": "Deployment hour heatmap",
    "family": "Heatmap",
    "description": "Deployment hour heatmap uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Grid coordinates with numeric intensity.",
    "useWhen": "Use it to find concentration by hour and day.",
    "tags": [
      "heatmap",
      "deployment"
    ],
    "image": "/img/charts/heatmap-deployment-hours.png"
  },
  {
    "slug": "heatmap-support-load",
    "title": "Support load heatmap",
    "family": "Heatmap",
    "description": "Support load heatmap uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Two-dimensional grid cells with values.",
    "useWhen": "Use it for workload concentration.",
    "tags": [
      "heatmap",
      "support"
    ],
    "image": "/img/charts/heatmap-support-load.png"
  },
  {
    "slug": "heatmap-service-risk",
    "title": "Service risk matrix",
    "family": "Heatmap",
    "description": "Service risk matrix uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Larger matrix of risk values.",
    "useWhen": "Use it when dense matrix comparison matters.",
    "tags": [
      "heatmap",
      "risk"
    ],
    "image": "/img/charts/heatmap-service-risk.png"
  },
  {
    "slug": "heatmap-correlation-grid",
    "title": "Correlation grid",
    "family": "Heatmap",
    "description": "Correlation grid uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Grid of pairwise values.",
    "useWhen": "Use it for correlation-like analysis.",
    "tags": [
      "heatmap",
      "correlation"
    ],
    "image": "/img/charts/heatmap-correlation-grid.png"
  },
  {
    "slug": "calendar-commit-activity",
    "title": "Commit activity calendar",
    "family": "Heatmap",
    "description": "Commit activity calendar uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Date/value pairs over a calendar range.",
    "useWhen": "Use it to show daily activity patterns.",
    "tags": [
      "calendar",
      "activity"
    ],
    "image": "/img/charts/calendar-commit-activity.png"
  },
  {
    "slug": "calendar-incident-volume",
    "title": "Incident volume calendar",
    "family": "Heatmap",
    "description": "Incident volume calendar uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Daily incident counts over a date range.",
    "useWhen": "Use it when the calendar shape carries meaning.",
    "tags": [
      "calendar",
      "incident"
    ],
    "image": "/img/charts/calendar-incident-volume.png"
  },
  {
    "slug": "calendar-retention-daily",
    "title": "Daily retention calendar",
    "family": "Heatmap",
    "description": "Daily retention calendar uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Date/value pairs for retention events.",
    "useWhen": "Use it to scan day-level consistency.",
    "tags": [
      "calendar",
      "retention"
    ],
    "image": "/img/charts/calendar-retention-daily.png"
  },
  {
    "slug": "calendar-release-burndown",
    "title": "Release burndown calendar",
    "family": "Heatmap",
    "description": "Release burndown calendar uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Daily values across a release period.",
    "useWhen": "Use it for calendar-driven delivery views.",
    "tags": [
      "calendar",
      "release"
    ],
    "image": "/img/charts/calendar-release-burndown.png"
  },
  {
    "slug": "visual-map-load-grid",
    "title": "Load grid visual map",
    "family": "Heatmap",
    "description": "Load grid visual map uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Numeric grid values mapped to color.",
    "useWhen": "Use it when color should explain intensity.",
    "tags": [
      "visualMap",
      "heatmap"
    ],
    "image": "/img/charts/visual-map-load-grid.png"
  },
  {
    "slug": "visual-map-density-grid",
    "title": "Density visual map",
    "family": "Heatmap",
    "description": "Density visual map uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Dense grid with continuous color scale.",
    "useWhen": "Use it for density and occupancy surfaces.",
    "tags": [
      "visualMap",
      "density"
    ],
    "image": "/img/charts/visual-map-density-grid.png"
  },
  {
    "slug": "heatmap-availability-window",
    "title": "Availability window",
    "family": "Heatmap",
    "description": "Availability window uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Grid values representing availability.",
    "useWhen": "Use it to compare uptime windows.",
    "tags": [
      "heatmap",
      "availability"
    ],
    "image": "/img/charts/heatmap-availability-window.png"
  },
  {
    "slug": "heatmap-resource-saturation",
    "title": "Resource saturation",
    "family": "Heatmap",
    "description": "Resource saturation uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Resource and time values on a matrix.",
    "useWhen": "Use it for infrastructure saturation.",
    "tags": [
      "heatmap",
      "resource"
    ],
    "image": "/img/charts/heatmap-resource-saturation.png"
  },
  {
    "slug": "calendar-sales-daily",
    "title": "Daily sales calendar",
    "family": "Heatmap",
    "description": "Daily sales calendar uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Daily sales values over a date range.",
    "useWhen": "Use it when weekday and date position matter.",
    "tags": [
      "calendar",
      "sales"
    ],
    "image": "/img/charts/calendar-sales-daily.png"
  },
  {
    "slug": "calendar-quality-gates",
    "title": "Quality gates calendar",
    "family": "Heatmap",
    "description": "Quality gates calendar uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Daily quality values.",
    "useWhen": "Use it for quality gate consistency over time.",
    "tags": [
      "calendar",
      "quality"
    ],
    "image": "/img/charts/calendar-quality-gates.png"
  },
  {
    "slug": "heatmap-feature-usage",
    "title": "Feature usage heatmap",
    "family": "Heatmap",
    "description": "Feature usage heatmap uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Feature/category values on a two-dimensional grid.",
    "useWhen": "Use it to find usage hotspots.",
    "tags": [
      "heatmap",
      "usage"
    ],
    "image": "/img/charts/heatmap-feature-usage.png"
  },
  {
    "slug": "heatmap-access-matrix",
    "title": "Access matrix",
    "family": "Heatmap",
    "description": "Access matrix uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Permission-like matrix values.",
    "useWhen": "Use it for dense access and security matrices.",
    "tags": [
      "heatmap",
      "security"
    ],
    "image": "/img/charts/heatmap-access-matrix.png"
  },
  {
    "slug": "visual-map-calendar-builds",
    "title": "Visual map build calendar",
    "family": "Heatmap",
    "description": "Visual map build calendar uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Daily build values with a color scale.",
    "useWhen": "Use it when calendar and intensity both matter.",
    "tags": [
      "visualMap",
      "calendar"
    ],
    "image": "/img/charts/visual-map-calendar-builds.png"
  },
  {
    "slug": "heatmap-queue-depth",
    "title": "Queue depth matrix",
    "family": "Heatmap",
    "description": "Queue depth matrix uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Queue depth by queue and time bucket.",
    "useWhen": "Use it for operational queues.",
    "tags": [
      "heatmap",
      "queue"
    ],
    "image": "/img/charts/heatmap-queue-depth.png"
  },
  {
    "slug": "heatmap-regression-risk",
    "title": "Regression risk matrix",
    "family": "Heatmap",
    "description": "Regression risk matrix uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Risk values by component and area.",
    "useWhen": "Use it before releases to focus attention.",
    "tags": [
      "heatmap",
      "regression"
    ],
    "image": "/img/charts/heatmap-regression-risk.png"
  },
  {
    "slug": "calendar-user-activity",
    "title": "User activity calendar",
    "family": "Heatmap",
    "description": "User activity calendar uses Fission Charts typed Rust data to render a production-ready heatmap view.",
    "dataShape": "Date/value activity pairs.",
    "useWhen": "Use it to show consistency and spikes over time.",
    "tags": [
      "calendar",
      "user"
    ],
    "image": "/img/charts/calendar-user-activity.png"
  },
  {
    "slug": "tree-platform-modules",
    "title": "Platform module tree",
    "family": "Hierarchy",
    "description": "Platform module tree uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Nested named nodes with values.",
    "useWhen": "Use it for ownership and dependency hierarchy.",
    "tags": [
      "tree",
      "platform"
    ],
    "image": "/img/charts/tree-platform-modules.png"
  },
  {
    "slug": "tree-product-taxonomy",
    "title": "Product taxonomy tree",
    "family": "Hierarchy",
    "description": "Product taxonomy tree uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Nested product categories.",
    "useWhen": "Use it when users browse hierarchical structure.",
    "tags": [
      "tree",
      "taxonomy"
    ],
    "image": "/img/charts/tree-product-taxonomy.png"
  },
  {
    "slug": "tree-radial-services",
    "title": "Radial service tree",
    "family": "Hierarchy",
    "description": "Radial service tree uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Nested nodes arranged radially.",
    "useWhen": "Use it when a compact hierarchy is needed.",
    "tags": [
      "tree",
      "radial"
    ],
    "image": "/img/charts/tree-radial-services.png"
  },
  {
    "slug": "treemap-cost-centers",
    "title": "Cost center treemap",
    "family": "Hierarchy",
    "description": "Cost center treemap uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Hierarchical values sized by area.",
    "useWhen": "Use it for part-to-whole hierarchy analysis.",
    "tags": [
      "treemap",
      "cost"
    ],
    "image": "/img/charts/treemap-cost-centers.png"
  },
  {
    "slug": "treemap-storage-classes",
    "title": "Storage class treemap",
    "family": "Hierarchy",
    "description": "Storage class treemap uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Nested values by storage class.",
    "useWhen": "Use it when area should communicate magnitude.",
    "tags": [
      "treemap",
      "storage"
    ],
    "image": "/img/charts/treemap-storage-classes.png"
  },
  {
    "slug": "sunburst-feature-areas",
    "title": "Feature area sunburst",
    "family": "Hierarchy",
    "description": "Feature area sunburst uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Nested values in radial layers.",
    "useWhen": "Use it for hierarchical composition with depth.",
    "tags": [
      "sunburst",
      "feature"
    ],
    "image": "/img/charts/sunburst-feature-areas.png"
  },
  {
    "slug": "sunburst-org-revenue",
    "title": "Org revenue sunburst",
    "family": "Hierarchy",
    "description": "Org revenue sunburst uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Hierarchy values as radial rings.",
    "useWhen": "Use it when hierarchy and whole composition both matter.",
    "tags": [
      "sunburst",
      "revenue"
    ],
    "image": "/img/charts/sunburst-org-revenue.png"
  },
  {
    "slug": "sankey-lead-flow",
    "title": "Lead flow sankey",
    "family": "Hierarchy",
    "description": "Lead flow sankey uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Nodes and directed weighted links.",
    "useWhen": "Use it when movement between stages is the question.",
    "tags": [
      "sankey",
      "flow"
    ],
    "image": "/img/charts/sankey-lead-flow.png"
  },
  {
    "slug": "sankey-energy-balance",
    "title": "Energy balance sankey",
    "family": "Hierarchy",
    "description": "Energy balance sankey uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Flow nodes and links.",
    "useWhen": "Use it for transfer and loss diagrams.",
    "tags": [
      "sankey",
      "energy"
    ],
    "image": "/img/charts/sankey-energy-balance.png"
  },
  {
    "slug": "theme-river-support",
    "title": "Support theme river",
    "family": "Hierarchy",
    "description": "Support theme river uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Stacked stream values by time and category.",
    "useWhen": "Use it for shifting composition over time.",
    "tags": [
      "themeRiver",
      "support"
    ],
    "image": "/img/charts/theme-river-support.png"
  },
  {
    "slug": "graph-service-dependencies",
    "title": "Service dependencies",
    "family": "Hierarchy",
    "description": "Service dependencies uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Nodes and links with values.",
    "useWhen": "Use it to explain relationship topology.",
    "tags": [
      "graph",
      "dependencies"
    ],
    "image": "/img/charts/graph-service-dependencies.png"
  },
  {
    "slug": "graph-customer-journey",
    "title": "Customer journey graph",
    "family": "Hierarchy",
    "description": "Customer journey graph uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Relationship nodes and edges.",
    "useWhen": "Use it when paths and relationships matter more than axes.",
    "tags": [
      "graph",
      "journey"
    ],
    "image": "/img/charts/graph-customer-journey.png"
  },
  {
    "slug": "graph-alert-correlation",
    "title": "Alert correlation graph",
    "family": "Hierarchy",
    "description": "Alert correlation graph uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Related alert nodes and links.",
    "useWhen": "Use it for incident analysis.",
    "tags": [
      "graph",
      "alert"
    ],
    "image": "/img/charts/graph-alert-correlation.png"
  },
  {
    "slug": "tree-file-ownership",
    "title": "File ownership tree",
    "family": "Hierarchy",
    "description": "File ownership tree uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Nested file or team ownership nodes.",
    "useWhen": "Use it for repository and ownership views.",
    "tags": [
      "tree",
      "ownership"
    ],
    "image": "/img/charts/tree-file-ownership.png"
  },
  {
    "slug": "treemap-budget-allocation",
    "title": "Budget allocation treemap",
    "family": "Hierarchy",
    "description": "Budget allocation treemap uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Nested budget values sized by area.",
    "useWhen": "Use it for budget share exploration.",
    "tags": [
      "treemap",
      "budget"
    ],
    "image": "/img/charts/treemap-budget-allocation.png"
  },
  {
    "slug": "sunburst-customer-segments",
    "title": "Customer segment sunburst",
    "family": "Hierarchy",
    "description": "Customer segment sunburst uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Nested customer groups by value.",
    "useWhen": "Use it for segment hierarchy and share.",
    "tags": [
      "sunburst",
      "customer"
    ],
    "image": "/img/charts/sunburst-customer-segments.png"
  },
  {
    "slug": "sankey-resolution-path",
    "title": "Resolution path sankey",
    "family": "Hierarchy",
    "description": "Resolution path sankey uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Support stages as flow links.",
    "useWhen": "Use it to analyze process movement.",
    "tags": [
      "sankey",
      "support"
    ],
    "image": "/img/charts/sankey-resolution-path.png"
  },
  {
    "slug": "theme-river-channel-mix",
    "title": "Channel mix river",
    "family": "Hierarchy",
    "description": "Channel mix river uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Channel values over time.",
    "useWhen": "Use it to show mix changes without losing continuity.",
    "tags": [
      "themeRiver",
      "channel"
    ],
    "image": "/img/charts/theme-river-channel-mix.png"
  },
  {
    "slug": "graph-platform-topology",
    "title": "Platform topology graph",
    "family": "Hierarchy",
    "description": "Platform topology graph uses Fission Charts typed Rust data to render a production-ready hierarchy view.",
    "dataShape": "Platform nodes and dependency edges.",
    "useWhen": "Use it for system overview screens.",
    "tags": [
      "graph",
      "platform"
    ],
    "image": "/img/charts/graph-platform-topology.png"
  },
  {
    "slug": "map-market-regions",
    "title": "Market regions map",
    "family": "Geo",
    "description": "Market regions map uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Named regions with values.",
    "useWhen": "Use it for regional comparisons where shape matters.",
    "tags": [
      "map",
      "region"
    ],
    "image": "/img/charts/map-market-regions.png"
  },
  {
    "slug": "map-risk-regions",
    "title": "Risk regions map",
    "family": "Geo",
    "description": "Risk regions map uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Region values mapped to color.",
    "useWhen": "Use it to scan geographic risk.",
    "tags": [
      "map",
      "risk"
    ],
    "image": "/img/charts/map-risk-regions.png"
  },
  {
    "slug": "map-service-health",
    "title": "Service health map",
    "family": "Geo",
    "description": "Service health map uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Region health values.",
    "useWhen": "Use it for operational geography.",
    "tags": [
      "map",
      "health"
    ],
    "image": "/img/charts/map-service-health.png"
  },
  {
    "slug": "map-sales-territory",
    "title": "Sales territory map",
    "family": "Geo",
    "description": "Sales territory map uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Territory values over named regions.",
    "useWhen": "Use it when territory is the mental model.",
    "tags": [
      "map",
      "sales"
    ],
    "image": "/img/charts/map-sales-territory.png"
  },
  {
    "slug": "lines-supply-routes",
    "title": "Supply routes",
    "family": "Geo",
    "description": "Supply routes uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Coordinate pairs representing routes.",
    "useWhen": "Use it for route and movement diagrams.",
    "tags": [
      "lines",
      "route"
    ],
    "image": "/img/charts/lines-supply-routes.png"
  },
  {
    "slug": "lines-network-traffic",
    "title": "Network traffic lines",
    "family": "Geo",
    "description": "Network traffic lines uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Route lines with effect markers.",
    "useWhen": "Use it for flow over a spatial surface.",
    "tags": [
      "lines",
      "traffic"
    ],
    "image": "/img/charts/lines-network-traffic.png"
  },
  {
    "slug": "geo-dispatch-routes",
    "title": "Dispatch routes",
    "family": "Geo",
    "description": "Dispatch routes uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Map regions plus route lines.",
    "useWhen": "Use it when routes need region context.",
    "tags": [
      "map",
      "routes"
    ],
    "image": "/img/charts/geo-dispatch-routes.png"
  },
  {
    "slug": "geo-migration-flow",
    "title": "Migration flow",
    "family": "Geo",
    "description": "Migration flow uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Region map plus movement routes.",
    "useWhen": "Use it for source-to-destination stories.",
    "tags": [
      "geo",
      "migration"
    ],
    "image": "/img/charts/geo-migration-flow.png"
  },
  {
    "slug": "geo-data-center-links",
    "title": "Data center links",
    "family": "Geo",
    "description": "Data center links uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Route-like links between points.",
    "useWhen": "Use it for infrastructure network views.",
    "tags": [
      "geo",
      "network"
    ],
    "image": "/img/charts/geo-data-center-links.png"
  },
  {
    "slug": "map-capacity-regions",
    "title": "Capacity regions map",
    "family": "Geo",
    "description": "Capacity regions map uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Region values with visual scale.",
    "useWhen": "Use it for capacity by geography.",
    "tags": [
      "map",
      "capacity"
    ],
    "image": "/img/charts/map-capacity-regions.png"
  },
  {
    "slug": "map-coverage-score",
    "title": "Coverage score map",
    "family": "Geo",
    "description": "Coverage score map uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Coverage scores by region.",
    "useWhen": "Use it for service coverage surfaces.",
    "tags": [
      "map",
      "coverage"
    ],
    "image": "/img/charts/map-coverage-score.png"
  },
  {
    "slug": "lines-flight-density",
    "title": "Flight density lines",
    "family": "Geo",
    "description": "Flight density lines uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Dense route lines between points.",
    "useWhen": "Use it for movement patterns.",
    "tags": [
      "lines",
      "flight"
    ],
    "image": "/img/charts/lines-flight-density.png"
  },
  {
    "slug": "geo-route-overlay",
    "title": "Route overlay map",
    "family": "Geo",
    "description": "Route overlay map uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Map and line series together.",
    "useWhen": "Use it when geography and movement are inseparable.",
    "tags": [
      "map",
      "overlay"
    ],
    "image": "/img/charts/geo-route-overlay.png"
  },
  {
    "slug": "map-support-demand",
    "title": "Support demand map",
    "family": "Geo",
    "description": "Support demand map uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Demand values over regions.",
    "useWhen": "Use it to allocate support capacity geographically.",
    "tags": [
      "map",
      "support"
    ],
    "image": "/img/charts/map-support-demand.png"
  },
  {
    "slug": "map-incident-severity",
    "title": "Incident severity map",
    "family": "Geo",
    "description": "Incident severity map uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Severity values mapped to region color.",
    "useWhen": "Use it for incident command views.",
    "tags": [
      "map",
      "incident"
    ],
    "image": "/img/charts/map-incident-severity.png"
  },
  {
    "slug": "lines-courier-routes",
    "title": "Courier routes",
    "family": "Geo",
    "description": "Courier routes uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Point-to-point route lines.",
    "useWhen": "Use it for logistics routing.",
    "tags": [
      "lines",
      "courier"
    ],
    "image": "/img/charts/lines-courier-routes.png"
  },
  {
    "slug": "geo-network-overlay",
    "title": "Network overlay",
    "family": "Geo",
    "description": "Network overlay uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Region values with connection lines.",
    "useWhen": "Use it for linked geographic systems.",
    "tags": [
      "geo",
      "network"
    ],
    "image": "/img/charts/geo-network-overlay.png"
  },
  {
    "slug": "map-expansion-plan",
    "title": "Expansion plan map",
    "family": "Geo",
    "description": "Expansion plan map uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Planning values by region.",
    "useWhen": "Use it for geographic planning dashboards.",
    "tags": [
      "map",
      "planning"
    ],
    "image": "/img/charts/map-expansion-plan.png"
  },
  {
    "slug": "lines-incident-routing",
    "title": "Incident routing lines",
    "family": "Geo",
    "description": "Incident routing lines uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Routes representing response movement.",
    "useWhen": "Use it for dispatch and response analytics.",
    "tags": [
      "lines",
      "incident"
    ],
    "image": "/img/charts/lines-incident-routing.png"
  },
  {
    "slug": "geo-sales-routes",
    "title": "Sales route map",
    "family": "Geo",
    "description": "Sales route map uses Fission Charts typed Rust data to render a production-ready geo view.",
    "dataShape": "Sales territories with route overlays.",
    "useWhen": "Use it for field team planning.",
    "tags": [
      "geo",
      "sales"
    ],
    "image": "/img/charts/geo-sales-routes.png"
  },
  {
    "slug": "interaction-marked-slo",
    "title": "Marked SLO chart",
    "family": "Interaction",
    "description": "Marked SLO chart uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Trend data plus mark lines and shaded bands.",
    "useWhen": "Use it to keep thresholds in the chart instead of external notes.",
    "tags": [
      "markLine",
      "slo"
    ],
    "image": "/img/charts/interaction-marked-slo.png"
  },
  {
    "slug": "interaction-marked-deploy",
    "title": "Deployment marks",
    "family": "Interaction",
    "description": "Deployment marks uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Trend data with event mark points.",
    "useWhen": "Use it when specific moments explain the data.",
    "tags": [
      "markPoint",
      "deploy"
    ],
    "image": "/img/charts/interaction-marked-deploy.png"
  },
  {
    "slug": "interaction-datazoom-overview",
    "title": "Data zoom overview",
    "family": "Interaction",
    "description": "Data zoom overview uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Ordered data with a selected zoom range.",
    "useWhen": "Use it for long series where one window is active.",
    "tags": [
      "dataZoom",
      "line"
    ],
    "image": "/img/charts/interaction-datazoom-overview.png"
  },
  {
    "slug": "interaction-datazoom-telemetry",
    "title": "Telemetry zoom",
    "family": "Interaction",
    "description": "Telemetry zoom uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Long ordered telemetry values with zoom.",
    "useWhen": "Use it for monitoring and investigation.",
    "tags": [
      "dataZoom",
      "telemetry"
    ],
    "image": "/img/charts/interaction-datazoom-telemetry.png"
  },
  {
    "slug": "interaction-tooltip-axis",
    "title": "Axis tooltip",
    "family": "Interaction",
    "description": "Axis tooltip uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Series data with axis-oriented tooltip intent.",
    "useWhen": "Use it when the whole axis position is the hover target.",
    "tags": [
      "tooltip",
      "axis"
    ],
    "image": "/img/charts/interaction-tooltip-axis.png"
  },
  {
    "slug": "interaction-tooltip-item",
    "title": "Item tooltip",
    "family": "Interaction",
    "description": "Item tooltip uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Series data with item-level tooltip intent.",
    "useWhen": "Use it when individual marks carry detail.",
    "tags": [
      "tooltip",
      "item"
    ],
    "image": "/img/charts/interaction-tooltip-item.png"
  },
  {
    "slug": "interaction-toolbox-analysis",
    "title": "Analysis toolbox",
    "family": "Interaction",
    "description": "Analysis toolbox uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Chart data with explicit utility actions.",
    "useWhen": "Use it when analysis actions belong close to the chart.",
    "tags": [
      "toolbox",
      "analysis"
    ],
    "image": "/img/charts/interaction-toolbox-analysis.png"
  },
  {
    "slug": "interaction-toolbox-export",
    "title": "Export toolbox",
    "family": "Interaction",
    "description": "Export toolbox uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Chart data with restore, brush, zoom, and save actions.",
    "useWhen": "Use it when chart controls need a consistent built-in place.",
    "tags": [
      "toolbox",
      "export"
    ],
    "image": "/img/charts/interaction-toolbox-export.png"
  },
  {
    "slug": "interaction-brush-region",
    "title": "Brush region",
    "family": "Interaction",
    "description": "Brush region uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Scatter points plus brush rectangle configuration.",
    "useWhen": "Use it when users select data before drilling in.",
    "tags": [
      "brush",
      "selection"
    ],
    "image": "/img/charts/interaction-brush-region.png"
  },
  {
    "slug": "interaction-brush-outliers",
    "title": "Brush outliers",
    "family": "Interaction",
    "description": "Brush outliers uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Point data with visible brush preview.",
    "useWhen": "Use it for exploratory selection.",
    "tags": [
      "brush",
      "outlier"
    ],
    "image": "/img/charts/interaction-brush-outliers.png"
  },
  {
    "slug": "interaction-graphic-note",
    "title": "Graphic note",
    "family": "Interaction",
    "description": "Graphic note uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Trend line with typed graphic overlay.",
    "useWhen": "Use it to add explanations without leaving the chart model.",
    "tags": [
      "graphic",
      "annotation"
    ],
    "image": "/img/charts/interaction-graphic-note.png"
  },
  {
    "slug": "interaction-graphic-band",
    "title": "Graphic band",
    "family": "Interaction",
    "description": "Graphic band uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Chart plus typed rect, text, and line graphics.",
    "useWhen": "Use it for callouts and shaded product ranges.",
    "tags": [
      "graphic",
      "band"
    ],
    "image": "/img/charts/interaction-graphic-band.png"
  },
  {
    "slug": "interaction-timeline-years",
    "title": "Timeline years",
    "family": "Interaction",
    "description": "Timeline years uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Chart data with a timeline control.",
    "useWhen": "Use it when the same view switches between periods.",
    "tags": [
      "timeline",
      "years"
    ],
    "image": "/img/charts/interaction-timeline-years.png"
  },
  {
    "slug": "interaction-timeline-releases",
    "title": "Timeline releases",
    "family": "Interaction",
    "description": "Timeline releases uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Chart data with release timeline options.",
    "useWhen": "Use it for versioned or period-based views.",
    "tags": [
      "timeline",
      "release"
    ],
    "image": "/img/charts/interaction-timeline-releases.png"
  },
  {
    "slug": "interaction-mark-area-breach",
    "title": "Breach mark area",
    "family": "Interaction",
    "description": "Breach mark area uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Line values with highlighted breach band.",
    "useWhen": "Use it when unsafe ranges need visible boundaries.",
    "tags": [
      "markArea",
      "breach"
    ],
    "image": "/img/charts/interaction-mark-area-breach.png"
  },
  {
    "slug": "interaction-annotation-callout",
    "title": "Annotation callout",
    "family": "Interaction",
    "description": "Annotation callout uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Chart data with a typed text callout.",
    "useWhen": "Use it for guided dashboards and reports.",
    "tags": [
      "graphic",
      "callout"
    ],
    "image": "/img/charts/interaction-annotation-callout.png"
  },
  {
    "slug": "interaction-select-scatter",
    "title": "Selectable scatter",
    "family": "Interaction",
    "description": "Selectable scatter uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Scatter points with selection configuration.",
    "useWhen": "Use it when users inspect clusters interactively.",
    "tags": [
      "brush",
      "scatter"
    ],
    "image": "/img/charts/interaction-select-scatter.png"
  },
  {
    "slug": "interaction-tooltip-grouped",
    "title": "Grouped tooltip",
    "family": "Interaction",
    "description": "Grouped tooltip uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Grouped series with tooltip intent.",
    "useWhen": "Use it when comparison needs hover detail.",
    "tags": [
      "tooltip",
      "grouped"
    ],
    "image": "/img/charts/interaction-tooltip-grouped.png"
  },
  {
    "slug": "interaction-toolbox-restore",
    "title": "Restore toolbox",
    "family": "Interaction",
    "description": "Restore toolbox uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Chart data with reset action included.",
    "useWhen": "Use it when user exploration needs a safe reset.",
    "tags": [
      "toolbox",
      "restore"
    ],
    "image": "/img/charts/interaction-toolbox-restore.png"
  },
  {
    "slug": "interaction-timeline-capacity",
    "title": "Capacity timeline",
    "family": "Interaction",
    "description": "Capacity timeline uses Fission Charts typed Rust data to render a production-ready interaction view.",
    "dataShape": "Period labels with changing chart state.",
    "useWhen": "Use it for capacity changes across time.",
    "tags": [
      "timeline",
      "capacity"
    ],
    "image": "/img/charts/interaction-timeline-capacity.png"
  },
  {
    "slug": "dataset-products-by-year",
    "title": "Products by year dataset",
    "family": "Dataset",
    "description": "Products by year dataset uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Tabular product/year/value records.",
    "useWhen": "Use it when several chart views should share one dataset.",
    "tags": [
      "dataset",
      "bar"
    ],
    "image": "/img/charts/dataset-products-by-year.png"
  },
  {
    "slug": "dataset-product-trends",
    "title": "Product trend dataset",
    "family": "Dataset",
    "description": "Product trend dataset uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Tabular data encoded into line series.",
    "useWhen": "Use it when named fields should drive series values.",
    "tags": [
      "dataset",
      "line"
    ],
    "image": "/img/charts/dataset-product-trends.png"
  },
  {
    "slug": "dataset-filtered-pie",
    "title": "Filtered composition",
    "family": "Dataset",
    "description": "Filtered composition uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Filtered label/value rows.",
    "useWhen": "Use it when the chart represents a selected dataset slice.",
    "tags": [
      "dataset",
      "filter"
    ],
    "image": "/img/charts/dataset-filtered-pie.png"
  },
  {
    "slug": "dataset-ranked-bars",
    "title": "Ranked dataset bars",
    "family": "Dataset",
    "description": "Ranked dataset bars uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Rows sorted into category/value pairs.",
    "useWhen": "Use it when rank comes from data transformation.",
    "tags": [
      "dataset",
      "rank"
    ],
    "image": "/img/charts/dataset-ranked-bars.png"
  },
  {
    "slug": "dataset-stacked-revenue",
    "title": "Stacked revenue dataset",
    "family": "Dataset",
    "description": "Stacked revenue dataset uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Multiple encoded measures over one category axis.",
    "useWhen": "Use it when stack series share the same source table.",
    "tags": [
      "dataset",
      "stack"
    ],
    "image": "/img/charts/dataset-stacked-revenue.png"
  },
  {
    "slug": "dataset-visual-heatmap",
    "title": "Dataset visual heatmap",
    "family": "Dataset",
    "description": "Dataset visual heatmap uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Encoded grid cells with values.",
    "useWhen": "Use it when heatmaps come from generic records.",
    "tags": [
      "dataset",
      "heatmap"
    ],
    "image": "/img/charts/dataset-visual-heatmap.png"
  },
  {
    "slug": "dynamic-live-line",
    "title": "Live line update",
    "family": "Dataset",
    "description": "Live line update uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Append-like ordered telemetry samples.",
    "useWhen": "Use it for live monitoring views.",
    "tags": [
      "dynamic",
      "line"
    ],
    "image": "/img/charts/dynamic-live-line.png"
  },
  {
    "slug": "dynamic-live-bars",
    "title": "Live bar update",
    "family": "Dataset",
    "description": "Live bar update uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Changing category values.",
    "useWhen": "Use it for updating status summaries.",
    "tags": [
      "dynamic",
      "bar"
    ],
    "image": "/img/charts/dynamic-live-bars.png"
  },
  {
    "slug": "dynamic-status-gauge",
    "title": "Live status gauge",
    "family": "Dataset",
    "description": "Live status gauge uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Single changing status value.",
    "useWhen": "Use it when a live number needs a strong visual shape.",
    "tags": [
      "dynamic",
      "gauge"
    ],
    "image": "/img/charts/dynamic-status-gauge.png"
  },
  {
    "slug": "dynamic-alert-scatter",
    "title": "Live alert scatter",
    "family": "Dataset",
    "description": "Live alert scatter uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Highlighted alert points.",
    "useWhen": "Use it for active events and outliers.",
    "tags": [
      "dynamic",
      "scatter"
    ],
    "image": "/img/charts/dynamic-alert-scatter.png"
  },
  {
    "slug": "dynamic-funnel-activation",
    "title": "Live activation funnel",
    "family": "Dataset",
    "description": "Live activation funnel uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Changing stage values.",
    "useWhen": "Use it when pipeline stages update during the session.",
    "tags": [
      "dynamic",
      "funnel"
    ],
    "image": "/img/charts/dynamic-funnel-activation.png"
  },
  {
    "slug": "dynamic-brush-telemetry",
    "title": "Brush telemetry",
    "family": "Dataset",
    "description": "Brush telemetry uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Point samples with brush selection.",
    "useWhen": "Use it for interactive telemetry exploration.",
    "tags": [
      "dynamic",
      "brush"
    ],
    "image": "/img/charts/dynamic-brush-telemetry.png"
  },
  {
    "slug": "dataset-calendar-activity",
    "title": "Calendar dataset activity",
    "family": "Dataset",
    "description": "Calendar dataset activity uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Date/value rows mapped to a calendar.",
    "useWhen": "Use it when dates are a first-class data field.",
    "tags": [
      "dataset",
      "calendar"
    ],
    "image": "/img/charts/dataset-calendar-activity.png"
  },
  {
    "slug": "dataset-risk-bubbles",
    "title": "Risk bubbles dataset",
    "family": "Dataset",
    "description": "Risk bubbles dataset uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Rows encoded into x, y, and size.",
    "useWhen": "Use it when one table feeds bubble visualization.",
    "tags": [
      "dataset",
      "bubble"
    ],
    "image": "/img/charts/dataset-risk-bubbles.png"
  },
  {
    "slug": "dataset-parallel-quality",
    "title": "Quality dataset parallel",
    "family": "Dataset",
    "description": "Quality dataset parallel uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Rows with several numeric dimensions.",
    "useWhen": "Use it when the same table powers multidimensional analysis.",
    "tags": [
      "dataset",
      "parallel"
    ],
    "image": "/img/charts/dataset-parallel-quality.png"
  },
  {
    "slug": "dynamic-toolbox-telemetry",
    "title": "Telemetry toolbox",
    "family": "Dataset",
    "description": "Telemetry toolbox uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Telemetry chart with built-in actions.",
    "useWhen": "Use it when live charts need controlled exploration.",
    "tags": [
      "dynamic",
      "toolbox"
    ],
    "image": "/img/charts/dynamic-toolbox-telemetry.png"
  },
  {
    "slug": "dynamic-timeline-quarters",
    "title": "Quarter timeline",
    "family": "Dataset",
    "description": "Quarter timeline uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Period labels switching chart state.",
    "useWhen": "Use it for period-over-period analysis.",
    "tags": [
      "dynamic",
      "timeline"
    ],
    "image": "/img/charts/dynamic-timeline-quarters.png"
  },
  {
    "slug": "dataset-map-coverage",
    "title": "Coverage dataset map",
    "family": "Dataset",
    "description": "Coverage dataset map uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Region/value rows.",
    "useWhen": "Use it when geographic views come from named records.",
    "tags": [
      "dataset",
      "map"
    ],
    "image": "/img/charts/dataset-map-coverage.png"
  },
  {
    "slug": "dataset-flow-sankey",
    "title": "Flow dataset sankey",
    "family": "Dataset",
    "description": "Flow dataset sankey uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Node and edge records.",
    "useWhen": "Use it when flow data is modeled explicitly.",
    "tags": [
      "dataset",
      "sankey"
    ],
    "image": "/img/charts/dataset-flow-sankey.png"
  },
  {
    "slug": "dynamic-radar-score",
    "title": "Live radar score",
    "family": "Dataset",
    "description": "Live radar score uses Fission Charts typed Rust data to render a production-ready dataset view.",
    "dataShape": "Changing dimension vectors.",
    "useWhen": "Use it for live multidimensional scorecards.",
    "tags": [
      "dynamic",
      "radar"
    ],
    "image": "/img/charts/dynamic-radar-score.png"
  },
  {
    "slug": "scene3d-bar-capacity",
    "title": "3D capacity bars",
    "family": "3D and GL",
    "description": "3D capacity bars uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "Scene3D cuboids representing numeric height.",
    "useWhen": "Use it when depth is part of the visual story.",
    "tags": [
      "3d",
      "bar"
    ],
    "image": "/img/charts/scene3d-bar-capacity.png"
  },
  {
    "slug": "scene3d-bar-grid",
    "title": "3D grid bars",
    "family": "3D and GL",
    "description": "3D grid bars uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "Grid of cuboids in a native 3D scene.",
    "useWhen": "Use it for spatial bar comparisons.",
    "tags": [
      "3d",
      "grid"
    ],
    "image": "/img/charts/scene3d-bar-grid.png"
  },
  {
    "slug": "scene3d-scatter-clusters",
    "title": "3D cluster scatter",
    "family": "3D and GL",
    "description": "3D cluster scatter uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "Spheres positioned in 3D space.",
    "useWhen": "Use it for spatial point clusters.",
    "tags": [
      "3d",
      "scatter"
    ],
    "image": "/img/charts/scene3d-scatter-clusters.png"
  },
  {
    "slug": "scene3d-scatter-outliers",
    "title": "3D outlier scatter",
    "family": "3D and GL",
    "description": "3D outlier scatter uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "3D points with varied position and radius.",
    "useWhen": "Use it to show outliers in a point cloud.",
    "tags": [
      "3d",
      "outlier"
    ],
    "image": "/img/charts/scene3d-scatter-outliers.png"
  },
  {
    "slug": "scene3d-surface-response",
    "title": "3D response surface",
    "family": "3D and GL",
    "description": "3D response surface uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "Mesh vertices and indices forming a surface.",
    "useWhen": "Use it for continuous spatial fields.",
    "tags": [
      "3d",
      "surface"
    ],
    "image": "/img/charts/scene3d-surface-response.png"
  },
  {
    "slug": "scene3d-surface-terrain",
    "title": "3D terrain response",
    "family": "3D and GL",
    "description": "3D terrain response uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "Raised terrain-like mesh.",
    "useWhen": "Use it for elevation and field surfaces.",
    "tags": [
      "3d",
      "terrain"
    ],
    "image": "/img/charts/scene3d-surface-terrain.png"
  },
  {
    "slug": "scene3d-line-path",
    "title": "3D line path",
    "family": "3D and GL",
    "description": "3D line path uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "Spheres and segment meshes forming a path.",
    "useWhen": "Use it for trajectories and movement.",
    "tags": [
      "3d",
      "line"
    ],
    "image": "/img/charts/scene3d-line-path.png"
  },
  {
    "slug": "scene3d-line-spiral",
    "title": "3D spiral path",
    "family": "3D and GL",
    "description": "3D spiral path uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "Line path through depth.",
    "useWhen": "Use it for trajectory demos and spatial flows.",
    "tags": [
      "3d",
      "spiral"
    ],
    "image": "/img/charts/scene3d-line-spiral.png"
  },
  {
    "slug": "scene3d-point-cloud-dense",
    "title": "Dense 3D point cloud",
    "family": "3D and GL",
    "description": "Dense 3D point cloud uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "Many small spheres in 3D space.",
    "useWhen": "Use it for point fields and early volume-style views.",
    "tags": [
      "3d",
      "pointCloud"
    ],
    "image": "/img/charts/scene3d-point-cloud-dense.png"
  },
  {
    "slug": "scene3d-point-cloud-sparse",
    "title": "Sparse 3D point cloud",
    "family": "3D and GL",
    "description": "Sparse 3D point cloud uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "Sparse spatial points.",
    "useWhen": "Use it when individual 3D observations need separation.",
    "tags": [
      "3d",
      "pointCloud"
    ],
    "image": "/img/charts/scene3d-point-cloud-sparse.png"
  },
  {
    "slug": "scene3d-globe-status",
    "title": "3D globe status",
    "family": "3D and GL",
    "description": "3D globe status uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "Globe sphere with visible markers.",
    "useWhen": "Use it when global context matters.",
    "tags": [
      "3d",
      "globe"
    ],
    "image": "/img/charts/scene3d-globe-status.png"
  },
  {
    "slug": "scene3d-globe-coverage",
    "title": "3D globe coverage",
    "family": "3D and GL",
    "description": "3D globe coverage uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "Globe primitive with highlighted locations.",
    "useWhen": "Use it for coverage and status over a globe.",
    "tags": [
      "3d",
      "globe"
    ],
    "image": "/img/charts/scene3d-globe-coverage.png"
  },
  {
    "slug": "scene3d-network",
    "title": "3D network scene",
    "family": "3D and GL",
    "description": "3D network scene uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "Node spheres connected by segment meshes.",
    "useWhen": "Use it for spatial network views.",
    "tags": [
      "3d",
      "network"
    ],
    "image": "/img/charts/scene3d-network.png"
  },
  {
    "slug": "scene3d-topology",
    "title": "3D topology scene",
    "family": "3D and GL",
    "description": "3D topology scene uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "3D graph nodes and links.",
    "useWhen": "Use it when topology benefits from depth.",
    "tags": [
      "3d",
      "topology"
    ],
    "image": "/img/charts/scene3d-topology.png"
  },
  {
    "slug": "scene3d-mesh-field",
    "title": "3D mesh field",
    "family": "3D and GL",
    "description": "3D mesh field uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "Mesh surface primitive.",
    "useWhen": "Use it for custom mesh data.",
    "tags": [
      "3d",
      "mesh"
    ],
    "image": "/img/charts/scene3d-mesh-field.png"
  },
  {
    "slug": "scene3d-volume-points",
    "title": "3D volume points",
    "family": "3D and GL",
    "description": "3D volume points uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "Point field suggesting volume data.",
    "useWhen": "Use it as the native point-based volume path.",
    "tags": [
      "3d",
      "volume"
    ],
    "image": "/img/charts/scene3d-volume-points.png"
  },
  {
    "slug": "scene3d-operations-bars",
    "title": "3D operations bars",
    "family": "3D and GL",
    "description": "3D operations bars uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "3D bars over an operational grid.",
    "useWhen": "Use it when spatial placement helps compare operational metrics.",
    "tags": [
      "3d",
      "operations"
    ],
    "image": "/img/charts/scene3d-operations-bars.png"
  },
  {
    "slug": "scene3d-service-cloud",
    "title": "3D service cloud",
    "family": "3D and GL",
    "description": "3D service cloud uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "Service points in 3D space.",
    "useWhen": "Use it to visualize clusters of service observations.",
    "tags": [
      "3d",
      "service"
    ],
    "image": "/img/charts/scene3d-service-cloud.png"
  },
  {
    "slug": "scene3d-terrain-risk",
    "title": "3D risk terrain",
    "family": "3D and GL",
    "description": "3D risk terrain uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "Terrain surface using native mesh rendering.",
    "useWhen": "Use it for risk and elevation-like surfaces.",
    "tags": [
      "3d",
      "risk"
    ],
    "image": "/img/charts/scene3d-terrain-risk.png"
  },
  {
    "slug": "scene3d-surface-wave",
    "title": "3D wave surface",
    "family": "3D and GL",
    "description": "3D wave surface uses Fission Charts typed Rust data to render a production-ready 3d and gl view.",
    "dataShape": "Surface mesh with wave-like height variation.",
    "useWhen": "Use it for continuous response fields.",
    "tags": [
      "3d",
      "wave"
    ],
    "image": "/img/charts/scene3d-surface-wave.png"
  },
  {
    "slug": "monitoring-overview-line",
    "title": "Monitoring overview line",
    "family": "Monitoring",
    "description": "Monitoring overview line uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "Dense telemetry samples.",
    "useWhen": "Use it for top-level operational dashboards.",
    "tags": [
      "monitoring",
      "line"
    ],
    "image": "/img/charts/monitoring-overview-line.png"
  },
  {
    "slug": "monitoring-error-band",
    "title": "Monitoring error band",
    "family": "Monitoring",
    "description": "Monitoring error band uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "Error values with threshold band.",
    "useWhen": "Use it when alert ranges must be visible.",
    "tags": [
      "monitoring",
      "markLine"
    ],
    "image": "/img/charts/monitoring-error-band.png"
  },
  {
    "slug": "monitoring-traffic-bars",
    "title": "Monitoring traffic bars",
    "family": "Monitoring",
    "description": "Monitoring traffic bars uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "Traffic counts by category.",
    "useWhen": "Use it for quick categorical traffic summaries.",
    "tags": [
      "monitoring",
      "bar"
    ],
    "image": "/img/charts/monitoring-traffic-bars.png"
  },
  {
    "slug": "monitoring-service-rank",
    "title": "Service rank",
    "family": "Monitoring",
    "description": "Service rank uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "Services ranked by a value.",
    "useWhen": "Use it to focus on the largest contributors.",
    "tags": [
      "monitoring",
      "rank"
    ],
    "image": "/img/charts/monitoring-service-rank.png"
  },
  {
    "slug": "monitoring-error-scatter",
    "title": "Error scatter",
    "family": "Monitoring",
    "description": "Error scatter uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "Highlighted error points.",
    "useWhen": "Use it for outlier-heavy operational views.",
    "tags": [
      "monitoring",
      "scatter"
    ],
    "image": "/img/charts/monitoring-error-scatter.png"
  },
  {
    "slug": "monitoring-load-heatmap",
    "title": "Load heatmap",
    "family": "Monitoring",
    "description": "Load heatmap uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "Resource load matrix.",
    "useWhen": "Use it for time and resource concentration.",
    "tags": [
      "monitoring",
      "heatmap"
    ],
    "image": "/img/charts/monitoring-load-heatmap.png"
  },
  {
    "slug": "monitoring-uptime-calendar",
    "title": "Uptime calendar",
    "family": "Monitoring",
    "description": "Uptime calendar uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "Daily uptime values.",
    "useWhen": "Use it for long-term reliability summaries.",
    "tags": [
      "monitoring",
      "calendar"
    ],
    "image": "/img/charts/monitoring-uptime-calendar.png"
  },
  {
    "slug": "monitoring-capacity-gauge",
    "title": "Capacity gauge",
    "family": "Monitoring",
    "description": "Capacity gauge uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "One capacity value in a bounded range.",
    "useWhen": "Use it for critical status cards.",
    "tags": [
      "monitoring",
      "gauge"
    ],
    "image": "/img/charts/monitoring-capacity-gauge.png"
  },
  {
    "slug": "monitoring-dependency-graph",
    "title": "Dependency graph",
    "family": "Monitoring",
    "description": "Dependency graph uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "Service nodes and dependencies.",
    "useWhen": "Use it for operational topology.",
    "tags": [
      "monitoring",
      "graph"
    ],
    "image": "/img/charts/monitoring-dependency-graph.png"
  },
  {
    "slug": "monitoring-flow-sankey",
    "title": "Operational flow sankey",
    "family": "Monitoring",
    "description": "Operational flow sankey uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "Flow nodes and links.",
    "useWhen": "Use it for request and process flow.",
    "tags": [
      "monitoring",
      "sankey"
    ],
    "image": "/img/charts/monitoring-flow-sankey.png"
  },
  {
    "slug": "monitoring-region-map",
    "title": "Operational region map",
    "family": "Monitoring",
    "description": "Operational region map uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "Region values with color scale.",
    "useWhen": "Use it for geographic operations.",
    "tags": [
      "monitoring",
      "map"
    ],
    "image": "/img/charts/monitoring-region-map.png"
  },
  {
    "slug": "monitoring-route-lines",
    "title": "Operational route lines",
    "family": "Monitoring",
    "description": "Operational route lines uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "Connection lines with effect markers.",
    "useWhen": "Use it for route and link movement.",
    "tags": [
      "monitoring",
      "lines"
    ],
    "image": "/img/charts/monitoring-route-lines.png"
  },
  {
    "slug": "monitoring-brush-investigation",
    "title": "Brush investigation",
    "family": "Monitoring",
    "description": "Brush investigation uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "Scatter points and brush selection.",
    "useWhen": "Use it when incidents require selecting a point range.",
    "tags": [
      "monitoring",
      "brush"
    ],
    "image": "/img/charts/monitoring-brush-investigation.png"
  },
  {
    "slug": "monitoring-toolbox-analysis",
    "title": "Monitoring toolbox",
    "family": "Monitoring",
    "description": "Monitoring toolbox uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "Line chart with analysis actions.",
    "useWhen": "Use it when charts need local analysis controls.",
    "tags": [
      "monitoring",
      "toolbox"
    ],
    "image": "/img/charts/monitoring-toolbox-analysis.png"
  },
  {
    "slug": "monitoring-release-timeline",
    "title": "Release timeline",
    "family": "Monitoring",
    "description": "Release timeline uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "Timeline control over chart state.",
    "useWhen": "Use it for release-based dashboards.",
    "tags": [
      "monitoring",
      "timeline"
    ],
    "image": "/img/charts/monitoring-release-timeline.png"
  },
  {
    "slug": "monitoring-annotation",
    "title": "Monitoring annotation",
    "family": "Monitoring",
    "description": "Monitoring annotation uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "Trend with typed graphic callout.",
    "useWhen": "Use it when dashboards need explanatory context.",
    "tags": [
      "monitoring",
      "annotation"
    ],
    "image": "/img/charts/monitoring-annotation.png"
  },
  {
    "slug": "monitoring-3d-grid",
    "title": "Monitoring 3D grid",
    "family": "Monitoring",
    "description": "Monitoring 3D grid uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "3D grid bars for operational values.",
    "useWhen": "Use it for spatial operational demonstrations.",
    "tags": [
      "monitoring",
      "3d"
    ],
    "image": "/img/charts/monitoring-3d-grid.png"
  },
  {
    "slug": "monitoring-3d-cloud",
    "title": "Monitoring 3D cloud",
    "family": "Monitoring",
    "description": "Monitoring 3D cloud uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "3D point cloud of operational samples.",
    "useWhen": "Use it for dense spatial monitoring views.",
    "tags": [
      "monitoring",
      "3d"
    ],
    "image": "/img/charts/monitoring-3d-cloud.png"
  },
  {
    "slug": "monitoring-risk-radar",
    "title": "Monitoring risk radar",
    "family": "Monitoring",
    "description": "Monitoring risk radar uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "Multidimensional status vector.",
    "useWhen": "Use it for status summaries across dimensions.",
    "tags": [
      "monitoring",
      "radar"
    ],
    "image": "/img/charts/monitoring-risk-radar.png"
  },
  {
    "slug": "monitoring-single-axis-events",
    "title": "Monitoring event strip",
    "family": "Monitoring",
    "description": "Monitoring event strip uses Fission Charts typed Rust data to render a production-ready monitoring view.",
    "dataShape": "Event positions with value-coded marks.",
    "useWhen": "Use it for compact event timeline rows.",
    "tags": [
      "monitoring",
      "singleAxis"
    ],
    "image": "/img/charts/monitoring-single-axis-events.png"
  },
  {
    "slug": "analytics-acquisition-line",
    "title": "Acquisition trend",
    "family": "Analytics",
    "description": "Acquisition trend uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Acquisition values over time.",
    "useWhen": "Use it for growth dashboards.",
    "tags": [
      "analytics",
      "line"
    ],
    "image": "/img/charts/analytics-acquisition-line.png"
  },
  {
    "slug": "analytics-retention-area",
    "title": "Retention area",
    "family": "Analytics",
    "description": "Retention area uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Retention values with area fill.",
    "useWhen": "Use it when volume and trend both matter.",
    "tags": [
      "analytics",
      "area"
    ],
    "image": "/img/charts/analytics-retention-area.png"
  },
  {
    "slug": "analytics-conversion-funnel",
    "title": "Conversion funnel",
    "family": "Analytics",
    "description": "Conversion funnel uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Ordered conversion stage values.",
    "useWhen": "Use it for product funnel analysis.",
    "tags": [
      "analytics",
      "funnel"
    ],
    "image": "/img/charts/analytics-conversion-funnel.png"
  },
  {
    "slug": "analytics-channel-stack",
    "title": "Channel stack",
    "family": "Analytics",
    "description": "Channel stack uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Channel values stacked by category.",
    "useWhen": "Use it for acquisition mix.",
    "tags": [
      "analytics",
      "stack"
    ],
    "image": "/img/charts/analytics-channel-stack.png"
  },
  {
    "slug": "analytics-market-pie",
    "title": "Market share pie",
    "family": "Analytics",
    "description": "Market share pie uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Label/value market share pairs.",
    "useWhen": "Use it for small whole comparisons.",
    "tags": [
      "analytics",
      "pie"
    ],
    "image": "/img/charts/analytics-market-pie.png"
  },
  {
    "slug": "analytics-device-donut",
    "title": "Device mix donut",
    "family": "Analytics",
    "description": "Device mix donut uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Device values in a donut chart.",
    "useWhen": "Use it when device share matters.",
    "tags": [
      "analytics",
      "donut"
    ],
    "image": "/img/charts/analytics-device-donut.png"
  },
  {
    "slug": "analytics-cohort-heatmap",
    "title": "Cohort heatmap",
    "family": "Analytics",
    "description": "Cohort heatmap uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Cohort matrix values.",
    "useWhen": "Use it for retention and cohort intensity.",
    "tags": [
      "analytics",
      "heatmap"
    ],
    "image": "/img/charts/analytics-cohort-heatmap.png"
  },
  {
    "slug": "analytics-calendar-engagement",
    "title": "Engagement calendar",
    "family": "Analytics",
    "description": "Engagement calendar uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Daily engagement values.",
    "useWhen": "Use it for engagement consistency over time.",
    "tags": [
      "analytics",
      "calendar"
    ],
    "image": "/img/charts/analytics-calendar-engagement.png"
  },
  {
    "slug": "analytics-segment-bubbles",
    "title": "Segment bubbles",
    "family": "Analytics",
    "description": "Segment bubbles uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Segment x, y, and value triples.",
    "useWhen": "Use it for multidimensional segment comparison.",
    "tags": [
      "analytics",
      "bubble"
    ],
    "image": "/img/charts/analytics-segment-bubbles.png"
  },
  {
    "slug": "analytics-source-river",
    "title": "Source theme river",
    "family": "Analytics",
    "description": "Source theme river uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Source values over time.",
    "useWhen": "Use it for changing source composition.",
    "tags": [
      "analytics",
      "themeRiver"
    ],
    "image": "/img/charts/analytics-source-river.png"
  },
  {
    "slug": "analytics-journey-graph",
    "title": "Journey graph",
    "family": "Analytics",
    "description": "Journey graph uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Journey nodes and links.",
    "useWhen": "Use it for relationship-heavy behavior views.",
    "tags": [
      "analytics",
      "graph"
    ],
    "image": "/img/charts/analytics-journey-graph.png"
  },
  {
    "slug": "analytics-journey-sankey",
    "title": "Journey sankey",
    "family": "Analytics",
    "description": "Journey sankey uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Journey stages and flow links.",
    "useWhen": "Use it for user movement between stages.",
    "tags": [
      "analytics",
      "sankey"
    ],
    "image": "/img/charts/analytics-journey-sankey.png"
  },
  {
    "slug": "analytics-region-sales",
    "title": "Region sales map",
    "family": "Analytics",
    "description": "Region sales map uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Sales values by named region.",
    "useWhen": "Use it for geographic sales analysis.",
    "tags": [
      "analytics",
      "map"
    ],
    "image": "/img/charts/analytics-region-sales.png"
  },
  {
    "slug": "analytics-route-engagement",
    "title": "Engagement routes",
    "family": "Analytics",
    "description": "Engagement routes uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Map plus route lines.",
    "useWhen": "Use it for spatial engagement flows.",
    "tags": [
      "analytics",
      "routes"
    ],
    "image": "/img/charts/analytics-route-engagement.png"
  },
  {
    "slug": "analytics-radar-product",
    "title": "Product radar",
    "family": "Analytics",
    "description": "Product radar uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Product dimensions as profile vectors.",
    "useWhen": "Use it to compare product health dimensions.",
    "tags": [
      "analytics",
      "radar"
    ],
    "image": "/img/charts/analytics-radar-product.png"
  },
  {
    "slug": "analytics-gauge-score",
    "title": "Analytics score gauge",
    "family": "Analytics",
    "description": "Analytics score gauge uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "One score value in a bounded range.",
    "useWhen": "Use it for primary scorecards.",
    "tags": [
      "analytics",
      "gauge"
    ],
    "image": "/img/charts/analytics-gauge-score.png"
  },
  {
    "slug": "analytics-parallel-segments",
    "title": "Segment parallel",
    "family": "Analytics",
    "description": "Segment parallel uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Segment rows across several dimensions.",
    "useWhen": "Use it for segment tradeoff analysis.",
    "tags": [
      "analytics",
      "parallel"
    ],
    "image": "/img/charts/analytics-parallel-segments.png"
  },
  {
    "slug": "analytics-treemap-features",
    "title": "Feature treemap",
    "family": "Analytics",
    "description": "Feature treemap uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Feature values in a hierarchy.",
    "useWhen": "Use it for feature usage share.",
    "tags": [
      "analytics",
      "treemap"
    ],
    "image": "/img/charts/analytics-treemap-features.png"
  },
  {
    "slug": "analytics-sunburst-portfolio",
    "title": "Portfolio sunburst",
    "family": "Analytics",
    "description": "Portfolio sunburst uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Portfolio hierarchy in radial layers.",
    "useWhen": "Use it for hierarchical portfolio composition.",
    "tags": [
      "analytics",
      "sunburst"
    ],
    "image": "/img/charts/analytics-sunburst-portfolio.png"
  },
  {
    "slug": "analytics-toolbox-report",
    "title": "Report toolbox chart",
    "family": "Analytics",
    "description": "Report toolbox chart uses Fission Charts typed Rust data to render a production-ready analytics view.",
    "dataShape": "Chart with report actions.",
    "useWhen": "Use it when analytics views need built-in controls.",
    "tags": [
      "analytics",
      "toolbox"
    ],
    "image": "/img/charts/analytics-toolbox-report.png"
  }
];

export function chartFamilySlug(family: string): string {
  return family
    .toLowerCase()
    .replace(/&/g, 'and')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');
}

export function chartReferencePath(chart: Pick<ChartCatalogEntry, 'family' | 'slug'>): string {
  return `/reference/charts/${chartFamilySlug(chart.family)}/${chart.slug}`;
}

export function chartFamilyReferencePath(family: string): string {
  return `/reference/charts/${chartFamilySlug(family)}/overview`;
}

export const chartFamilies = Array.from(new Set(chartCatalog.map((chart) => chart.family)));

export const featuredChartPreviews = ['line-gradient-area', 'bar-ranked', 'calendar-quarter', 'sankey-energy', 'surface3d-wave']
  .map((slug) => chartCatalog.find((chart) => chart.slug === slug))
  .filter((chart): chart is ChartCatalogEntry => Boolean(chart));
