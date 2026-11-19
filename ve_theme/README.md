# ve_theme

The VHRD design system for egui: brand tokens applied as dark and light egui styles, the brand fonts and type
scale, `UiExt` helpers, the property-panel widgets, the perforated status light and icons. `ve_theme::setup(ctx)`
turns it on; the crate docs (`cargo doc -p ve_theme --open`) have the whole API and
`cargo run -p ve_theme --example gallery` shows every part, in dark and light.

## Features

| Feature | Default | Turns on |
|---|---|---|
| `fonts` | yes | IBM Plex Sans and Mono (and a Noto Emoji subset) installed by `setup` |
| `phosphor` | yes, through `icons` | the Phosphor regular icon font installed by `setup`, its glyphs as `icons::ph` |
| `icons` | yes | SVG icons with colour slots (`icons::SvgIcon`, `IconImage`), rasterised by resvg |
| `serde` | no | `ColorSource` as a setting |

## Icons

Phosphor regular is the base set, a font tinted like text. SVG icons name a colour slot per shape instead of
a colour (`class="f-ink"` fills, `class="s-accent"` strokes), so they follow the theme and the desktop accent;
the rules are in the `icons` module docs. Each icon is rasterised once per pixel size into cached masks and
tinted at paint. `cargo bench -p ve_theme_benchmarks` times the first raster.

## Licences

The crate is MIT OR Apache-2.0. The bundled fonts keep their own licences, in `fonts/`:

- IBM Plex Sans and Mono, © IBM Corp., SIL Open Font License 1.1: `fonts/LICENSE.txt`.
- Noto Emoji (subset), © Google, SIL Open Font License 1.1: `fonts/LICENSE-NotoEmoji.txt`.
- Phosphor Icons regular font (through the `egui-phosphor` crate), © Phosphor Icons, MIT:
  `fonts/LICENSE-Phosphor.txt`, from github.com/phosphor-icons/web.
