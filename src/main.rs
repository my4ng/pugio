mod cargo;

use std::{
    collections::HashMap,
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
    visit::{EdgeRef, Topo, Walker},
};

use crate::cargo::{
    CargoOptions, cargo_bloat_output, cargo_tree_output, get_dep_graph, get_size_map,
};

fn cum_sums(graph: &StableGraph<String, ()>, map: &HashMap<String, usize>) -> (Vec<usize>, f32) {
    let mut cum_sums: Vec<_> = graph
        .node_weights()
        .map(|n| map.get(n).copied().unwrap_or_default())
        .collect();

    let mut nodes = Topo::new(&graph).iter(&graph).collect::<Vec<_>>();
    nodes.reverse();

    for node in nodes {
        for child in graph.neighbors(node) {
            cum_sums[node.index()] += cum_sums[child.index()];
        }
    }

    (cum_sums, 0.25)
}

fn dep_counts(graph: &StableGraph<String, ()>) -> (Vec<usize>, f32) {
    let mut dep_counts: Vec<_> = vec![0; graph.node_count()];

    let mut nodes = Topo::new(&graph).iter(&graph).collect::<Vec<_>>();
    nodes.reverse();

    for node in nodes {
        for child in graph.neighbors(node) {
            dep_counts[node.index()] += dep_counts[child.index()] + 1;
        }
    }

    (dep_counts, 0.25)
}

fn rev_dep_counts(graph: &StableGraph<String, ()>) -> (Vec<usize>, f32) {
    let mut rev_dep_counts: Vec<_> = vec![0; graph.node_count()];

    for node in Topo::new(&graph).iter(&graph) {
        for child in graph.neighbors(node) {
            rev_dep_counts[child.index()] += 1;
        }
    }

    (rev_dep_counts, 0.5)
}

fn remove_small_deps(graph: &mut StableGraph<String, ()>, cum_sums: &[usize], threshold: usize) {
    for (idx, sum) in cum_sums.iter().enumerate() {
        if *sum < threshold {
            graph.remove_node(NodeIndex::new(idx));
        }
    }
}

#[derive(Default, Clone, Copy, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
enum NodeColoring {
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

    /// Color scheme of nodes
    #[arg(short, long)]
    coloring: Option<NodeColoring>,

    /// Color gradient of nodes
    #[arg(short, long)]
    gradient: Option<NodeColoringGradient>,

    /// Color gamma of nodes
    #[arg(long)]
    gamma: Option<f32>,

    /// Remove nodes that have cumulative sum below threshold
    #[arg(short, long)]
    threshold: Option<usize>,

    /// Invert color gradient
    #[arg(long)]
    inverse: bool,

    /// Dot output file only
    #[arg(short, long)]
    dot: bool,

    /// Output filename, default is output.*
    #[arg(short, long)]
    output: Option<String>,

    /// Do not open output svg file
    #[arg(long)]
    no_open: bool,
}

fn output_svg(
    dot_output: &str,
    graph: &StableGraph<String, ()>,
    output_filename: &str,
    no_open: bool,
) -> anyhow::Result<()> {
    let node_count_factor = (graph.node_count() as f32 / 32.0).floor();
    let font_size = node_count_factor * 3.0 + 15.0;
    let arrow_size = node_count_factor * 0.2 + 0.8;
    let edge_width = node_count_factor * 0.4 + 1.2;

    // TODO: Customise style
    let mut child = Command::new("dot")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .arg("-Tsvg")
        .arg("-Gpad=1.0")
        .arg("-Nshape=circle")
        .arg("-Nstyle=filled")
        .arg("-Nfixedsize=shape")
        .arg("-Nfontname=monospace")
        .arg(format!("-Nfontsize={font_size}"))
        .arg(format!("-Earrowsize={arrow_size}"))
        .arg("-Ecolor=#0000009F")
        .arg(format!("-Epenwidth={edge_width}"))
        .arg("-Gnodesep=0.35")
        .arg("-Granksep=0.7")
        .spawn()
        .context("failed to execute dot")?;

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
        binary: args.binary,
        features: args.features,
        all_features: args.all_features,
        no_default_features: args.no_default_features,
        release: args.release,
    };

    let tree_output = cargo_tree_output(&options)?;
    let mut graph = get_dep_graph(&tree_output);

    let bloat_output = cargo_bloat_output(&options)?;
    let size_map = get_size_map(&bloat_output)?;
    let cum_sums_vec = cum_sums(&graph, &size_map);

    let node_colouring_values = match args.coloring.unwrap_or_default() {
        NodeColoring::None => None,
        coloring => {
            let (values, mut gamma) = match coloring {
                NodeColoring::CumSum => cum_sums_vec.clone(),
                NodeColoring::DepCount => dep_counts(&graph),
                NodeColoring::RevDepCount => rev_dep_counts(&graph),
                _ => unreachable!(),
            };

            if let Some(gamma_) = args.gamma {
                gamma = gamma_;
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

    let binding = |_, (i, n): (NodeIndex, &String)| {
        let size = size_map.get(n).copied().unwrap_or_default();
        let width = (size as f32 / 4096.0 + 1.0).log10();
        let tooltip = humansize::format_size(size, humansize::BINARY);

        if let Some(NodeColoringValues {
            values,
            gamma,
            max,
            gradient,
        }) = &node_colouring_values
        {
            let mut t = (values[i.index()] as f32 / *max as f32).powf(*gamma);
            if args.inverse {
                t = 1.0 - t;
            }
            let node_color = gradient.at(t).to_css_hex();
            format!(
                r#"label = "{}" tooltip = "{}" width = {} fillcolor= "{}""#,
                n, tooltip, width, node_color
            )
        } else {
            format!(
                r#"label = "{}" tooltip = "{}" width = {} "#,
                n, tooltip, width
            )
        }
    };

    let dot = Dot::with_attr_getters(
        &graph,
        &[Config::EdgeNoLabel, Config::NodeNoLabel],
        &|g, e| {
            let source = g.node_weight(e.source()).unwrap();
            let target = g.node_weight(e.target()).unwrap();
            format!(r#"edgetooltip = "{source} -> {target}""#)
        },
        &binding,
    );

    let dot_output = format!("{dot:?}");
    let output_filename = args.output.as_deref();

    if args.dot {
        std::fs::write(output_filename.unwrap_or("output.gv"), dot_output)
            .context("failed to write output dot file")?;
    } else {
        output_svg(
            &dot_output,
            &graph,
            output_filename.unwrap_or("output.svg"),
            args.no_open,
        )?;
    }

    Ok(())
}
