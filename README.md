# Pugio

*Pugio* is a graph visualisation tool for Rust to estimate and present the binary size contributions of a crate and its dependencies. It uses `cargo-tree` and `cargo-bloat` to build the dependency graph where the diameter of each crate node is logarithmic to its size. The resulting graph can then be either exported with `graphviz` and opened as an SVG file, or as a DOT graph file for additional processing.

It is important to note that the sizes is and will always be only an *estimation*. Some information is irrevocably lost during compilation and linkage. In addition, calls to the standard library is not included in the caller's size (although the total size of the standard library can be shown with the `--std` flag). Multiple versions of a dependency is also not distinguishable in the final binary.

## Examples

`pugio --release --gradient blues -t non-zero -o images/pugio.svg`

![pugio](images/pugio.svg)

`pugio --bin rg --release --scheme dep-count --gradient purples -R "grep v0.4.1" -d 2 -t 1KiB --gamma 0.5 --dark-mode --std -o images/ripgrep.svg`

![ripgrep](images/ripgrep.svg)

`pugio --release --gradient reds -t 1 --dark-mode --std -o images/hyperfine.svg`

![hyperfine](images/hyperfine.svg)

## Dependencies

- `cargo`: `cargo-tree` command is now part of the cargo binary
- [`cargo-bloat`](https://crates.io/crates/cargo-bloat)
- [`dot`](https://graphviz.org/): part of the `graphviz` package; optional, needed for SVG image generation (disabled via the `--dot-only` option)

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
  -p, --package <PACKAGE>      Package to inspect
      --bin <BINARY>           Binary to inspect
  -F, --features <FEATURES>    Space or comma separated list of features to activate
      --all-features           Activate all available features
      --no-default-features    Do not activate the `default` feature
      --release                Build artifacts in release mode, with optimizations
  -R, --root <ROOT>            Change root to the specified dependency name
                                unique prefix is supported
      --std                    Add std standalone node
  -s, --scheme <SCHEME>        Color scheme of nodes
                                - "cum-sum": cumulative sum of the size of a node and its dependencies (default)
                                - "dep-count": dependency count; number of transitive dependency relations from a node
                                - "rev-dep-count": reverse dependency count; number of paths from the root to a node
                                - "none"
  -g, --gradient <GRADIENT>    Color gradient of nodes
                                - "reds" (default), "oranges", "purples", "greens", "blues"
                                - custom CSS gradient format, e.g. "#fff, 75%, #00f"
      --gamma <GAMMA>          Color gamma of nodes, between 0.0 and 1.0
                                default is scheme-specific
  -t, --threshold <THRESHOLD>  Remove nodes that have cumulative sum below threshold
                                - human readable byte format, e.g. "21KiB", "69 KB"
                                - "non-zero"
  -d, --max-depth <MAX_DEPTH>  Remove nodes that are more than max depth deep
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
