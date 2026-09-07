# WVST identity

The WVST mark combines two angled signal paths into an interlocking WV shape.
The dark ribbon represents the browser side; the copper ribbon represents the
local native path. Both the mark and the WVST wordmark are SVG geometry, so
exported logos do not depend on fonts installed on the viewer's device.

Assets in `static/brand`:

| Asset | Use |
| --- | --- |
| `wvst-logo.svg` | Complete logo on light backgrounds, transparent SVG |
| `wvst-logo-dark.svg` | Complete logo on dark backgrounds, transparent SVG |
| `wvst-mark.svg` | Standalone mark on light backgrounds |
| `wvst-mark-dark.svg` | Standalone mark on dark backgrounds |
| `wvst-mark-mono.svg` | Single-color mark |
| `wvst-brand-sheet.svg` | Overview of the logo, icon sizes and palette |

`static/favicon.svg` uses the same mark on a graphite tile for visibility at
16–64 px. Keep the mark's proportions and allow clear space around it; use the
complete logo at 100 px or wider and the icon for smaller placements.

The custom `WVSTBrand.svelte` theme component selects the light/dark asset using
the site's active theme and keeps the localized home URL and accessible WVST
link label. The full logo replaces the default icon-plus-text brand.

Documentation typography uses separate spacing for section transitions and
heading padding: H2 sections have 52 px above the divider and 24 px below it
on desktop, reduced to 40 px / 20 px on mobile. H3/H4 have independent margins.
An explicit Markdown rule before H2 suppresses the heading's automatic divider.
