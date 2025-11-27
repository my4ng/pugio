# Pugio

[![Crates.io Version](https://img.shields.io/crates/v/pugio)](https://crates.io/crates/pugio) ![Crates.io MSRV](https://img.shields.io/crates/msrv/pugio) [![GitHub License](https://img.shields.io/github/license/my4ng/pugio)](https://github.com/my4ng/pugio/blob/main/LICENSE)

*Pugio* is a graph visualisation tool for Rust to estimate and present the binary size contributions of a crate and its dependencies. It uses `cargo-tree` and `cargo-bloat` to build the dependency graph where the diameter of each crate node is logarithmic to its size. The resulting graph can then be either exported with `graphviz` and opened as an SVG file, or as a DOT graph file for additional processing.

It is important to note that the sizes is and will always be only an *estimation*. Some information is irrevocably lost during compilation and linkage. In addition, calls to the standard library is not included in the caller's size (although the total size of the standard library can be shown with the `--std` flag). Multiple versions of a dependency is also not distinguishable in the final binary.

## Examples

`pugio -c examples/config.toml`

![pugio](examples/pugio.svg)

`pugio --bin rg --release --scheme dep-count --gradient purples -R "grep v0.4.1" -d 2 -t 1KiB --dark-mode --highlight rev-dep -o examples/ripgrep.svg`

![ripgrep](examples/ripgrep.svg)

`pugio --release --node-label-template "{short}\n{value_binary}" -E 'clap' -g rd-pu -t non-zero --dark-mode --std -o examples/hyperfine.svg`

![hyperfine](examples/hyperfine.svg)

## Dependencies

- `cargo`: `cargo-tree` command is now part of the cargo binary
- [`cargo-bloat`](https://crates.io/crates/cargo-bloat)
  - `cargo install cargo-bloat --no-default-features`
- [`dot`](https://graphviz.org/): part of the `graphviz` package; optional, needed for SVG image generation (disabled via the `--dot-only` option)
  - Debian, Ubuntu: `sudo apt install graphviz`
  - Fedora, RHEL-compatible: `sudo dnf install graphviz`
  - Others: [graphviz download](https://graphviz.org/download/)

## Installation

`cargo install --locked pugio`

Or install binary directly from GitHub release assets via `cargo-binstall`:

`cargo binstall --locked pugio`

To customise enabled Cargo features, add the options:

`--no-default-features --features="..."`

## Feature flags

- `default`: `regex`, `config`
- `regex`: support regex pattern matching in options: [regex-lite syntax](https://docs.rs/regex-lite/latest/regex_lite/index.html#syntax)
- `config`: support TOML config file

## Planned Features

- Edge label by dependency features
- Interactive SVG support (function breakdown)

## Usage

```plain
A command-line dependency binary size graph visualisation tool for Rust

Usage: pugio [OPTIONS]

Options:
  -c, --config <CONFIG_FILE>
          Config TOML file path, "-" for stdin
           disables all other options
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
  -E, --excludes <EXCLUDES>
          Exclude dependency names matching the regex patterns
  -R, --root <ROOT>
          Change root to the unique dependency name matching the regex pattern
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
           - "bu-pu", "or-rd", "pu-rd", "rd-pu"
           - "viridis", "cividis", "plasma"
      --gamma <GAMMA>
          Color gamma of nodes, between 0.0 and 1.0
           default is scheme-specific
  -t, --threshold <THRESHOLD>
          Remove nodes that have cumulative sum below threshold
           - human readable byte format, e.g. "21KiB", "69 KB"
           - "non-zero"
  -d, --depth <MAX_DEPTH>
          Remove nodes that are more than max depth deep
      --inverse-gradient
          Inverse color gradient
      --dark-mode
          Dark mode for output svg file
      --padding <PADDING>
          Padding for output svg file default: 1.0
      --scale-factor <SCALE_FACTOR>
          Scale factor for output svg file
      --separation-factor <SEPARATION_FACTOR>
          Separation factor for output svg file
      --highlight <HIGHLIGHT>
          Highlight parts of the graph when hovered for output svg file
           - "dep": all dependencies
           - "rev-dep": all reverse dependencies
             requires modern browser for `:has()` CSS pseudo-class support
      --highlight-amount <HIGHLIGHT_AMOUNT>
          Highlight amount for output svg file, between 0.0 and 1.0
           default: 0.5
      --node-label-template <NODE_LABEL_TEMPLATE>
          Custom node label formatting template
           default: "{short}"
      --node-tooltip-template <NODE_TOOLTIP_TEMPLATE>
          Custom node tooltip formatting template
           default: "{full}\n{size_binary}\n{features}"
      --edge-label-template <EDGE_LABEL_TEMPLATE>
          Custom edge label formatting template
           default: "{features}"
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

## Template values

### Node

```plain
{short}, {extra}, {full}
{size}, {size_binary}, {size_decimal}
{value}, {value_binary}, {value_decimal}
{scheme}
{features}
```

### Edge

```plain
{source}, {target}
{features}
```

## License

Pugio is licensed under the BSD-2-Clause Plus Patent license, see [LICENSE](LICENSE) for more details.

*Note: This license is designed to provide: a) a simple permissive license; b) that is compatible with the GNU General Public License (GPL), version 2; and c) which also has an express patent grant included.*
