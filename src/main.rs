mod cargo;
mod graph;

use std::{
    io::Write,
    process::{Command, Stdio},
};

use anyhow::Context;
use clap::Parser;
use colorgrad::{BasisGradient, Gradient};
use petgraph::{
    dot::{Config, Dot},
    graph::NodeIndex,
    prelude::StableGraph,
    visit::EdgeRef,
};

use crate::{
    cargo::{CargoOptions, cargo_bloat_output, cargo_tree_output, get_dep_graph, get_size_map},
    graph::{
        NodeWeight, change_root, cum_sums, dep_counts, remove_deep_deps, remove_small_deps,
        rev_dep_counts,
    },
};

#[derive(Default, Clone, Copy, strum::EnumString)]
#[strum(serialize_all = "kebab-case")]
enum NodeColoringScheme {
    #[default]
    CumSum,
    DepCount,
    RevDepCount,
    None,
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
    #[arg(long, verbatim_doc_comment)]
    root: Option<String>,

    /// Add std standalone node
    #[arg(long = "std")]
    has_std: bool,

    /// Color scheme of nodes
    ///  - "cum-sum": cumulative sum of the size of a node and its dependencies (default)
    ///  - "dep-count": dependency count; number of transitive dependency relations from a node
    ///  - "rev-dep-count": reverse dependency count; number of paths from the root to a node
    ///  - "none"
    #[arg(short, long, verbatim_doc_comment)]
    scheme: Option<NodeColoringScheme>,

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
    ///  support human readable byte format, e.g. "21KiB", "69 KB"
    #[arg(short, long, value_parser = |s: &str| parse_size::parse_size(s).map(|b| b as usize), verbatim_doc_comment)]
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

fn output_svg(
    dot_output: &str,
    graph: &StableGraph<NodeWeight, ()>,
    output_filename: &str,
    dark_mode: bool,
    no_open: bool,
) -> anyhow::Result<()> {
    let node_count_factor = (graph.node_count() as f32 / 32.0).floor();
    let font_size = node_count_factor * 3.0 + 15.0;
    let arrow_size = node_count_factor * 0.2 + 0.8;
    let edge_width = node_count_factor * 0.4 + 1.2;
    let node_border_width = edge_width * 0.75;

    // TODO: Customise style
    let mut command = Command::new("dot");
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .arg("-Tsvg")
        .arg("-Gpad=1.0")
        .arg("-Nshape=circle")
        .arg(format!("-Npenwidth={node_border_width}"))
        .arg("-Nstyle=filled")
        .arg("-Nfixedsize=shape")
        .arg("-Nfontname=monospace")
        .arg(format!("-Nfontsize={font_size}"))
        .arg(format!("-Earrowsize={arrow_size}"))
        .arg("-Earrowhead=onormal")
        .arg(format!("-Epenwidth={edge_width}"))
        .arg("-Gnodesep=0.35")
        .arg("-Granksep=0.7");

    if dark_mode {
        command
            .arg("-Ncolor=#FFFFFF9F")
            .arg("-Ecolor=#FFFFFF9F")
            .arg("-Gbgcolor=#000000")
            .arg("-Nfontcolor=#FFFFFF");
    } else {
        command.arg("-Ncolor=#0000009F").arg("-Ecolor=#0000009F");
    }

    let mut child = command.spawn().context("failed to execute dot")?;

    let stdin = child.stdin.as_mut().context("failed to get stdin")?;
    stdin
        .write_all(dot_output.as_bytes())
        .context("failed to write into stdin")?;

    let output = child.wait_with_output().context("failed to wait on dot")?;
    std::fs::write(output_filename, output.stdout).context("failed to write output svg file")?;
    if !no_open {
        open::that_detached(output_filename).context("failed to open output svg")?;
    }
    Ok(())
}

struct NodeColoringValues {
    values: Vec<usize>,
    gamma: f32,
    max: usize,
    gradient: BasisGradient,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let options = CargoOptions {
        package: args.package,
        binary: args.binary.clone(),
        features: args.features,
        all_features: args.all_features,
        no_default_features: args.no_default_features,
        release: args.release,
    };

    let tree_output = cargo_tree_output(&options)?;
    let mut graph =
        get_dep_graph(&tree_output, args.has_std).context("failed to parse cargo-tree output")?;

    let bloat_output = cargo_bloat_output(&options)?;
    let size_map = get_size_map(&bloat_output).context("failed to parse cargo-bloat output")?;

    let mut root_idx = NodeIndex::new(0);

    if let Some(root) = args.root {
        root_idx = change_root(&mut graph, &root)?;
    }

    let cum_sums_vec = cum_sums(&graph, &size_map);

    let node_colouring_values = match args.scheme.unwrap_or_default() {
        NodeColoringScheme::None => None,
        scheme => {
            let (values, mut gamma) = match scheme {
                NodeColoringScheme::CumSum => cum_sums_vec.clone(),
                NodeColoringScheme::DepCount => dep_counts(&graph),
                NodeColoringScheme::RevDepCount => rev_dep_counts(&graph),
                _ => unreachable!(),
            };

            if let Some(gamma_) = args.gamma {
                gamma = gamma_.clamp(0.0, 1.0);
            }

            let max = values.iter().copied().max().unwrap();
            let gradient = args.gradient.unwrap_or_default().into();

            Some(NodeColoringValues {
                values,
                gamma,
                max,
                gradient,
            })
        }
    };

    if let Some(threshold) = args.threshold {
        remove_small_deps(&mut graph, &cum_sums_vec.0, threshold);
    }

    if let Some(max_depth) = args.max_depth {
        remove_deep_deps(&mut graph, root_idx, max_depth);
    }

    let binding = |_, (i, n): (NodeIndex, &NodeWeight)| {
        let short_name = n.short_name();
        let mut size = size_map.get(short_name).copied().unwrap_or_default();
        if let Some(bin) = args.binary.as_ref()
            && i.index() == 0
        {
            size += size_map.get(bin).copied().unwrap_or_default();
        }
        let width = (size as f32 / 4096.0 + 1.0).log10();

        let human_size = humansize::format_size(size, humansize::BINARY);
        let tooltip = format!("{}\n{human_size}", n.name);

        let node_color = if let Some(NodeColoringValues {
            values,
            gamma,
            max,
            gradient,
        }) = &node_colouring_values
        {
            let mut t = (values[i.index()] as f32 / *max as f32).powf(*gamma);
            if args.inverse_gradient {
                t = 1.0 - t;
            }

            let mut node_color = gradient.at(t);
            if args.dark_mode {
                let mut hsla = node_color.to_hsla();
                hsla[2] = 1.0 - hsla[2];
                node_color = colorgrad::Color::from_hsla(hsla[0], hsla[1], hsla[2], hsla[3])
            }
            node_color
        } else {
            colorgrad::Color::new(1.0, 1.0, 1.0, 1.0)
        };

        let node_color = node_color.to_css_hex();
        format!(
            r#"label = "{short_name}" tooltip = "{tooltip}" width = {width} fillcolor= "{node_color}""#,
        )
    };

    let dot = Dot::with_attr_getters(
        &graph,
        &[Config::EdgeNoLabel, Config::NodeNoLabel],
        &|g, e| {
            let source = g.node_weight(e.source()).unwrap().short_name();
            let target = g.node_weight(e.target()).unwrap().short_name();
            format!(r#"edgetooltip = "{source} -> {target}""#)
        },
        &binding,
    );

    let dot_output = format!("{dot:?}");
    let output_filename = args.output.as_deref();

    if args.dot_only {
        std::fs::write(output_filename.unwrap_or("output.gv"), dot_output)
            .context("failed to write output dot file")?;
    } else {
        output_svg(
            &dot_output,
            &graph,
            output_filename.unwrap_or("output.svg"),
            args.dark_mode,
            args.no_open,
        )?;
    }

    Ok(())
}
