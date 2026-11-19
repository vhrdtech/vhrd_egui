# ve_theme_benchmarks

Benchmarks of `ve_theme`, kept apart so the library carries no bench code.

`cargo bench -p ve_theme_benchmarks` prints what the first raster of each SVG icon (`ve_theme::icons::own`) costs at 16, 24 and 32 px, split into parsing the SVG and rendering every layer's coverage mask: the work done once per icon and pixel size before the masks are cached (CHR-7).
