# Pugio

*Pugio* is a graph visualisation tool for Rust to estimate and present the binary size contributions of a crate and its dependencies. It uses `cargo-tree` and `cargo-bloat` to build the dependency graph where the diameter of each crate node is logarithmic to its size. The resulting graph can then be either exported with `graphviz` and opened as an SVG file, or as a DOT graph file for additional processing.

## Example

`pugio --release --gradient blues -t 1024 -o pugio.svg`

![pugio](pugio.svg)

`pugio --bin rg --release --scheme dep_count --gradient purples -t 4096 --gamma 0.5 --dark-mode -o ripgrep.svg`

![ripgrep](ripgrep.svg)

`pugio --release --gradient reds -t 1 --dark-mode --std -o hyperfine.svg`

![hyperfine](hyperfine.svg)

## Dependencies

- `cargo`: `cargo-tree` command is now part of the cargo binary
- [`cargo-bloat`](https://crates.io/crates/cargo-bloat)
- [`dot`](https://graphviz.org/): part of the `graphviz` package; optional, needed for SVG image generation (disabled via the `--dot-only` option)

## Usage

```plain
Usage: pugio [OPTIONS]

Options:
  -p, --package <PACKAGE>      Package to inspect
      --bin <BINARY>           Binary to inspect
  -F, --features <FEATURES>    Space or comma separated list of features to activate
      --all-features           Activate all available features
      --no-default-features    Do not activate the `default` feature
      --release                Build artifacts in release mode, with optimizations
      --std                    Add std standalone node
  -s, --scheme <SCHEME>        Color scheme of nodes
  -g, --gradient <GRADIENT>    Color gradient of nodes
      --gamma <GAMMA>          Color gamma of nodes
  -t, --threshold <THRESHOLD>  Remove nodes that have cumulative sum below threshold
      --inverse-gradient       Inverse color gradient
      --dark-mode              Dark mode for output svg file
      --dot-only               Dot output file only
  -o, --output <OUTPUT>        Output filename, default is output.*
      --no-open                Do not open output svg file
  -h, --help                   Print help
  -V, --version                Print version
```

## License

Pugio is licensed under the BSD+Patent license, see [LICENSE](LICENSE) for more details.

*Note: This license is designed to provide: a) a simple permissive license; b) that is compatible with the GNU General Public License (GPL), version 2; and c) which also has an express patent grant included.*
