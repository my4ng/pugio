use std::str::FromStr;

use clap::Args;
#[cfg(feature = "config")]
use serde::de;

use pugio_lib::coloring::{NodeColoringGradient, NodeColoringScheme};

// Obfuscate type for clap
type OptScheme = Option<NodeColoringScheme>;

#[cfg_attr(
    feature = "config",
    derive(serde::Deserialize),
    serde(rename_all = "kebab-case")
)]
#[derive(Args)]
pub struct Config {
    /// Package to inspect
    #[arg(short, long)]
    pub package: Option<String>,

    /// Binary to inspect
    #[arg(long, value_name = "BINARY")]
    pub bin: Option<String>,

    /// Space or comma separated list of features to activate
    #[arg(short = 'F', long)]
    pub features: Option<String>,

    /// Activate all available features
    #[arg(long)]
    #[cfg_attr(feature = "config", serde(default))]
    pub all_features: bool,

    /// Do not activate the `default` feature
    #[arg(long)]
    #[cfg_attr(feature = "config", serde(default))]
    pub no_default_features: bool,

    /// Build artifacts in release mode, with optimizations
    #[arg(long)]
    #[cfg_attr(feature = "config", serde(default))]
    pub release: bool,

    /// Exclude dependency names matching the regex patterns
    #[cfg(feature = "regex")]
    #[arg(short = 'E', long)]
    pub excludes: Option<Vec<String>>,

    /// Exclude dependency names matching the prefixes
    #[cfg(not(feature = "regex"))]
    #[arg(short = 'E', long)]
    pub excludes: Option<Vec<String>>,

    /// Change root to the unique dependency name matching the regex pattern
    #[cfg(feature = "regex")]
    #[arg(short = 'R', long)]
    pub root: Option<String>,

    /// Change root to the unique dependency name matching the prefix
    #[cfg(not(feature = "regex"))]
    #[arg(short = 'R', long)]
    pub root: Option<String>,

    /// Add std standalone node
    #[arg(long)]
    #[cfg_attr(feature = "config", serde(default))]
    pub std: bool,

    /// Color scheme of nodes
    ///  - "cum-sum": cumulative sum of the size of a node and its dependencies (default)
    ///  - "dep-count": dependency count; number of transitive dependency relations from a node
    ///  - "rev-dep-count": reverse dependency count; number of paths from the root to a node
    ///  - "none"
    #[cfg_attr(
        feature = "config",
        serde(deserialize_with = "de_scheme", default = "default_opt_scheme")
    )]
    #[arg(short, long, default_value = "cum-sum", hide_default_value = true, value_parser = parse_scheme, verbatim_doc_comment)]
    pub scheme: OptScheme,

    /// Color gradient of nodes
    ///  - "reds" (default), "oranges", "purples", "greens", "blues"
    ///  - "bu-pu", "or-rd", "pu-rd", "rd-pu"
    ///  - "viridis", "cividis", "plasma"
    #[arg(short, long, verbatim_doc_comment)]
    pub gradient: Option<NodeColoringGradient>,

    /// Color gamma of nodes, between 0.0 and 1.0
    ///  default is scheme-specific
    #[arg(long, verbatim_doc_comment)]
    pub gamma: Option<f64>,

    /// Remove nodes that have cumulative sum below threshold
    ///  - human readable byte format, e.g. "21KiB", "69 KB"
    ///  - "non-zero"
    #[arg(short, long, value_parser = parse_threshold, verbatim_doc_comment)]
    #[cfg_attr(feature = "config", serde(deserialize_with = "de_threshold", default))]
    pub threshold: Option<usize>,

    /// Remove nodes that are more than max depth deep
    #[arg(short = 'd', long = "depth", value_name = "MAX_DEPTH")]
    pub depth: Option<usize>,

    /// Inverse color gradient
    #[arg(long)]
    #[cfg_attr(feature = "config", serde(default))]
    pub inverse_gradient: bool,

    /// Dark mode for output svg file
    #[arg(long)]
    #[cfg_attr(feature = "config", serde(default))]
    pub dark_mode: bool,

    /// Padding for output svg file
    ///  default: 1.0
    #[arg(long)]
    pub padding: Option<f32>,

    /// Scale factor for output svg file
    #[arg(long)]
    pub scale_factor: Option<f32>,

    /// Separation factor for output svg file
    #[arg(long)]
    pub separation_factor: Option<f32>,

    /// Highlight parts of the graph when hovered for output svg file
    ///  - "dep": all dependencies
    ///  - "rev-dep": all reverse dependencies
    ///    requires modern browser for `:has()` CSS pseudo-class support
    #[arg(long, value_parser = parse_highlight, verbatim_doc_comment)]
    #[cfg_attr(feature = "config", serde(deserialize_with = "de_highlight", default))]
    pub highlight: Option<bool>,

    /// Highlight amount for output svg file, between 0.0 and 1.0
    ///  default: 0.5
    #[arg(long, verbatim_doc_comment)]
    pub highlight_amount: Option<f32>,

    /// Custom node label formatting template
    ///  default: "{short}"
    #[arg(long, verbatim_doc_comment)]
    pub node_label_template: Option<String>,

    /// Custom node tooltip formatting template
    ///  default: "{full}\n{size_binary}\n{features}"
    #[arg(long, verbatim_doc_comment)]
    pub node_tooltip_template: Option<String>,

    /// Custom edge label formatting template
    ///  default: "{features}"
    #[arg(long, verbatim_doc_comment)]
    pub edge_label_template: Option<String>,

    /// Custom edge tooltip formatting template
    ///  default: "{source} -> {target}"
    #[arg(long, verbatim_doc_comment)]
    pub edge_tooltip_template: Option<String>,

    /// Dot output file only
    #[arg(long)]
    #[cfg_attr(feature = "config", serde(default))]
    pub dot_only: bool,

    /// Output filename, default is output.*
    #[arg(short, long)]
    pub output: Option<String>,

    /// Do not open output svg file
    #[arg(long)]
    #[cfg_attr(feature = "config", serde(default))]
    pub no_open: bool,
}

#[cfg(feature = "config")]
fn default_opt_scheme() -> OptScheme {
    Some(NodeColoringScheme::CumSum)
}

#[cfg(feature = "config")]
fn de_scheme<'de, D: de::Deserializer<'de>>(d: D) -> Result<Option<NodeColoringScheme>, D::Error> {
    let str: String = de::Deserialize::deserialize(d)?;
    parse_scheme(&str).map_err(de::Error::custom)
}

#[cfg(feature = "config")]
fn de_highlight<'de, D: de::Deserializer<'de>>(d: D) -> Result<Option<bool>, D::Error> {
    let str: String = de::Deserialize::deserialize(d)?;
    parse_highlight(&str)
        .map(Option::Some)
        .map_err(de::Error::custom)
}

#[cfg(feature = "config")]
fn de_threshold<'de, D: de::Deserializer<'de>>(d: D) -> Result<Option<usize>, D::Error> {
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Threshold {
        Usize(usize),
        String(String),
    }

    let threshold: Threshold = de::Deserialize::deserialize(d)?;
    match threshold {
        Threshold::Usize(u) => Ok(Some(u)),
        Threshold::String(s) => parse_threshold(&s)
            .map(Option::Some)
            .map_err(de::Error::custom),
    }
}

fn parse_scheme(
    s: &str,
) -> Result<Option<NodeColoringScheme>, <NodeColoringScheme as FromStr>::Err> {
    match s {
        "none" => Ok(None),
        _ => Ok(Some(NodeColoringScheme::from_str(s)?)),
    }
}

fn parse_highlight(h: &str) -> Result<bool, &'static str> {
    match h {
        "dep" => Ok(true),
        "rev-dep" => Ok(false),
        _ => Err("invalid highlight value"),
    }
}

fn parse_threshold(t: &str) -> Result<usize, parse_size::Error> {
    if t == "non-zero" {
        Ok(1)
    } else {
        parse_size::parse_size(t).map(|b| b as usize)
    }
}
