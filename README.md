# Pugio

[![Crates.io Version](https://img.shields.io/crates/v/pugio)](https://crates.io/crates/pugio) ![Crates.io MSRV](https://img.shields.io/crates/msrv/pugio) [![GitHub License](https://img.shields.io/github/license/my4ng/pugio)](https://github.com/my4ng/pugio/blob/main/LICENSE)

*Pugio* is a graph visualisation tool for Rust to estimate and present the binary size contributions of a crate and its dependencies. It uses `cargo-tree` and `cargo-bloat` to build the dependency graph where the diameter of each crate node is logarithmic to its size. The resulting graph can then be either exported with `graphviz` and opened as an SVG file, or as a DOT graph file for additional processing.

It is important to note that the sizes is and will always be only an *estimation*. Some information is irrevocably lost during compilation and linkage. In addition, calls to the standard library is not included in the caller's size (although the total size of the standard library can be shown with the `--std` flag). Multiple versions of a dependency is also not distinguishable in the final binary.

## Examples

`pugio --release --gradient blues -t non-zero -o images/pugio.svg`

![pugio](images/pugio.svg)

`pugio --bin rg --release --scheme dep-count --gradient purples -R "grep v0.4.1" -d 2 -t 1KiB --gamma 0.5 --dark-mode --std -o images/ripgrep.svg`

![ripgrep](images/ripgrep.svg)

`pugio --release --node-label-template "{short}\n{value_binary}" -g reds -t non-zero --dark-mode --std -o images/hyperfine.svg`

![hyperfine](images/hyperfine.svg)

## Installation

`cargo install --locked pugio`

## Dependencies

- `cargo`: `cargo-tree` command is now part of the cargo binary
- [`cargo-bloat`](https://crates.io/crates/cargo-bloat)
  - `cargo install cargo-bloat --no-default-features`
- [`dot`](https://graphviz.org/): part of the `graphviz` package; optional, needed for SVG image generation (disabled via the `--dot-only` option)
  - Debian, Ubuntu: `sudo apt install graphviz`
  - Fedora, RHEL-compatible: `sudo dnf install graphviz`
  - Others: [graphviz download](https://graphviz.org/download/)

## Planned Features

- Filter options
- Edge label by dependency features
- Additional style customisation
- Interactive SVG support (function breakdown)

## Usage

```plain
A command-line dependency binary size graph visualisation tool

Usage: pugio [OPTIONS]

Options:
  -p, --package <PACKAGE>
          Package to inspect
      --bin <BINARY>
          Binary to inspect
  -F, --features <FEATURES>
          Space or comma separated list of features to activate
      --all-features
          Activate all available features
      --no-default-features
          Do not activate the `default` feature
      --release
          Build artifacts in release mode, with optimizations
  -R, --root <ROOT>
          Change root to the specified dependency name
           unique prefix is supported
      --std
          Add std standalone node
  -s, --scheme <SCHEME>
          Color scheme of nodes
           - "cum-sum": cumulative sum of the size of a node and its dependencies (default)
           - "dep-count": dependency count; number of transitive dependency relations from a node
           - "rev-dep-count": reverse dependency count; number of paths from the root to a node
           - "none"
  -g, --gradient <GRADIENT>
          Color gradient of nodes
           - "reds" (default), "oranges", "purples", "greens", "blues"
           - custom CSS gradient format, e.g. "#fff, 75%, #00f"
      --gamma <GAMMA>
          Color gamma of nodes, between 0.0 and 1.0
           default is scheme-specific
  -t, --threshold <THRESHOLD>
          Remove nodes that have cumulative sum below threshold
           - human readable byte format, e.g. "21KiB", "69 KB"
           - "non-zero"
  -d, --max-depth <MAX_DEPTH>
          Remove nodes that are more than max depth deep
      --inverse-gradient
          Inverse color gradient
      --dark-mode
          Dark mode for output svg file
      --node-label-template <NODE_LABEL_TEMPLATE>
          Custom node label formatting template
           default: "{short}"
      --node-tooltip-template <NODE_TOOLTIP_TEMPLATE>
          Custom node tooltip formatting template
           default: "{full}\n{size_binary}"
      --edge-label-template <EDGE_LABEL_TEMPLATE>
          Custom edge label formatting template
      --edge-tooltip-template <EDGE_TOOLTIP_TEMPLATE>
          Custom edge tooltip formatting template
           default: "{source} -> {target}"
      --dot-only
          Dot output file only
  -o, --output <OUTPUT>
          Output filename, default is output.*
      --no-open
          Do not open output svg file
  -h, --help
          Print help
  -V, --version
          Print version
```

## License

Pugio is licensed under the BSD-2-Clause Plus Patent license, see [LICENSE](LICENSE) for more details.

*Note: This license is designed to provide: a) a simple permissive license; b) that is compatible with the GNU General Public License (GPL), version 2; and c) which also has an express patent grant included.*
