mod cargo;
mod dot;
mod graph;
mod template;

use std::str::FromStr;

use crate::{
    cargo::{CargoOptions, cargo_bloat_output, cargo_tree_output, get_dep_graph, get_size_map},
    dot::{output_dot, output_svg},
    graph::{
        NodeWeight, change_root, cum_sums, dep_counts, remove_deep_deps, remove_small_deps,
        rev_dep_counts,
    },
    template::get_templates,
};
use anyhow::Context;
use clap::Parser;
use colorgrad::BasisGradient;

#[derive(Clone, Copy, strum::EnumString)]
#[strum(serialize_all = "kebab-case")]
enum NodeColoringScheme {
    CumSum,
    DepCount,
    RevDepCount,
}

impl From<NodeColoringScheme> for &'static str {
    fn from(value: NodeColoringScheme) -> Self {
        match value {
            NodeColoringScheme::CumSum => "cumulative sum",
            NodeColoringScheme::DepCount => "dependency count",
            NodeColoringScheme::RevDepCount => "reverse dependency count",
        }
    }
}

#[derive(Default, Clone)]
enum NodeColoringGradient {
    #[default]
    Reds,
    Oranges,
    Purples,
    Greens,
    Blues,
    Custom(BasisGradient),
}

impl std::str::FromStr for NodeColoringGradient {
    type Err = colorgrad::GradientBuilderError;

    fn from_str(s: &str) -> Result<NodeColoringGradient, Self::Err> {
        match s {
            "reds" => Ok(Self::Reds),
            "oranges" => Ok(Self::Oranges),
            "purples" => Ok(Self::Purples),
            "greens" => Ok(Self::Greens),
            "blues" => Ok(Self::Blues),
            _ => colorgrad::GradientBuilder::new()
                .css(s)
                .build()
                .map(Self::Custom),
        }
    }
}

impl From<NodeColoringGradient> for BasisGradient {
    fn from(value: NodeColoringGradient) -> Self {
        use colorgrad::preset::*;
        match value {
            NodeColoringGradient::Reds => reds(),
            NodeColoringGradient::Oranges => oranges(),
            NodeColoringGradient::Purples => purples(),
            NodeColoringGradient::Greens => greens(),
            NodeColoringGradient::Blues => blues(),
            NodeColoringGradient::Custom(gradient) => gradient,
        }
    }
}

type OptScheme = Option<NodeColoringScheme>;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Package to inspect
    #[arg(short, long)]
    package: Option<String>,

    /// Binary to inspect
    #[arg(long = "bin")]
    binary: Option<String>,

    /// Space or comma separated list of features to activate
    #[arg(short = 'F', long)]
    features: Option<String>,

    /// Activate all available features
    #[arg(long)]
    all_features: bool,

    /// Do not activate the `default` feature
    #[arg(long)]
    no_default_features: bool,

    /// Build artifacts in release mode, with optimizations
    #[arg(long)]
    release: bool,

    /// Change root to the specified dependency name
    ///  unique prefix is supported
    #[arg(short = 'R', long, verbatim_doc_comment)]
    root: Option<String>,

    /// Add std standalone node
    #[arg(long)]
    std: bool,

    /// Color scheme of nodes
    ///  - "cum-sum": cumulative sum of the size of a node and its dependencies (default)
    ///  - "dep-count": dependency count; number of transitive dependency relations from a node
    ///  - "rev-dep-count": reverse dependency count; number of paths from the root to a node
    ///  - "none"
    #[arg(short, long, default_value = "cum-sum", hide_default_value = true, value_parser = parse_scheme, verbatim_doc_comment)]
    scheme: OptScheme,

    /// Color gradient of nodes
    ///  - "reds" (default), "oranges", "purples", "greens", "blues"
    ///  - custom CSS gradient format, e.g. "#fff, 75%, #00f"
    #[arg(short, long, verbatim_doc_comment)]
    gradient: Option<NodeColoringGradient>,

    /// Color gamma of nodes, between 0.0 and 1.0
    ///  default is scheme-specific
    #[arg(long, verbatim_doc_comment)]
    gamma: Option<f32>,

    /// Remove nodes that have cumulative sum below threshold
    ///  - human readable byte format, e.g. "21KiB", "69 KB"
    ///  - "non-zero"
    #[arg(short, long, value_parser = parse_threshold, verbatim_doc_comment)]
    threshold: Option<usize>,

    /// Remove nodes that are more than max depth deep
    #[arg(short = 'd', long)]
    max_depth: Option<usize>,

    /// Inverse color gradient
    #[arg(long)]
    inverse_gradient: bool,

    /// Dark mode for output svg file
    #[arg(long)]
    dark_mode: bool,

    /// Custom node label formatting template
    ///  default: "{short}"
    #[arg(long, verbatim_doc_comment)]
    node_label_template: Option<String>,

    /// Custom node tooltip formatting template
    ///  default: "{full}\n{size_binary}"
    #[arg(long, verbatim_doc_comment)]
    node_tooltip_template: Option<String>,

    /// Custom edge label formatting template
    #[arg(long)]
    edge_label_template: Option<String>,

    /// Custom edge tooltip formatting template
    ///  default: "{source} -> {target}"
    #[arg(long)]
    edge_tooltip_template: Option<String>,

    /// Dot output file only
    #[arg(long)]
    dot_only: bool,

    /// Output filename, default is output.*
    #[arg(short, long)]
    output: Option<String>,

    /// Do not open output svg file
    #[arg(long)]
    no_open: bool,
    // TODO: Add filter option
}

fn parse_scheme(s: &str) -> Result<Option<NodeColoringScheme>, strum::ParseError> {
    match s {
        "none" => Ok(None),
        _ => Ok(Some(NodeColoringScheme::from_str(s)?)),
    }
}

fn parse_threshold(t: &str) -> Result<usize, parse_size::Error> {
    if t == "non-zero" {
        Ok(1)
    } else {
        parse_size::parse_size(t).map(|b| b as usize)
    }
}

struct NodeColoringValues {
    values: Vec<usize>,
    gamma: f32,
    max: usize,
    gradient: BasisGradient,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let options = CargoOptions::from(&args);

    let tree_output = cargo_tree_output(&options)?;
    let mut graph = get_dep_graph(&tree_output).context("failed to parse cargo-tree output")?;

    let bloat_output = cargo_bloat_output(&options)?;
    let size_map = get_size_map(&bloat_output).context("failed to parse cargo-bloat output")?;

    let mut root_idx = petgraph::graph::NodeIndex::new(0);

    if let Some(root) = &args.root {
        root_idx = change_root(&mut graph, root)?;
    }

    let std_idx = if args.std {
        Some(graph.add_node(NodeWeight {
            name: "std ".to_string(),
            short_end: 3,
        }))
    } else {
        None
    };

    let cum_sums_vec = cum_sums(&graph, &size_map);

    let node_colouring_values = match args.scheme {
        None => None,
        Some(scheme) => {
            let (values, mut gamma) = match scheme {
                NodeColoringScheme::CumSum => cum_sums_vec.clone(),
                NodeColoringScheme::DepCount => dep_counts(&graph),
                NodeColoringScheme::RevDepCount => rev_dep_counts(&graph),
            };

            if let Some(gamma_) = args.gamma {
                gamma = gamma_.clamp(0.0, 1.0);
            }

            let max = values.iter().copied().max().unwrap();
            let gradient = args.gradient.clone().unwrap_or_default().into();

            Some(NodeColoringValues {
                values,
                gamma,
                max,
                gradient,
            })
        }
    };

    if let Some(threshold) = args.threshold {
        remove_small_deps(&mut graph, &cum_sums_vec.0, threshold, std_idx);
    }

    if let Some(max_depth) = args.max_depth {
        remove_deep_deps(&mut graph, root_idx, max_depth, std_idx);
    }

    let output_filename = args.output.as_deref();
    let templates = get_templates(&args).context("failed to parse templates")?;
    let dot = output_dot(&graph, &size_map, &args, &templates, node_colouring_values);

    if args.dot_only {
        std::fs::write(output_filename.unwrap_or("output.gv"), dot)
            .context("failed to write output dot file")?;
    } else {
        output_svg(&dot, &graph, output_filename.unwrap_or("output.svg"), &args)?;
    }

    Ok(())
}
