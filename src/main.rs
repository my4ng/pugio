mod cargo;
mod config;
mod dot;
mod graph;
mod template;

use crate::{
    cargo::{CargoOptions, cargo_bloat_output, cargo_tree_output, get_dep_graph, get_size_map},
    config::Config,
    dot::{output_dot, output_svg},
    graph::{
        NodeWeight, change_root, cum_sums, dep_counts, remove_deep_deps, remove_excluded_deps,
        remove_small_deps, rev_dep_counts,
    },
    template::get_templates,
};
use anyhow::Context;
use clap::Parser;
use colorgrad::BasisGradient;

#[cfg_attr(feature = "config", derive(serde_with::DeserializeFromStr))]
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

#[cfg_attr(feature = "config", derive(serde_with::DeserializeFromStr))]
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
    /// Config TOML file path, "-" for stdin
    ///  disables all other options
    #[cfg(feature = "config")]
    #[arg(short, long = "config", verbatim_doc_comment)]
    config_file: Option<String>,

    #[command(flatten)]
    config: Config,
}

struct NodeColoringValues {
    values: Vec<usize>,
    gamma: f32,
    max: usize,
    gradient: BasisGradient,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    cfg_if::cfg_if! {
        if #[cfg(feature = "config")] {
            let config = if let Some(config_file) = args.config_file {
                let config = if config_file == "-" {
                    std::io::read_to_string(std::io::stdin()).context("failed to read from stdin")?
                } else {
                    std::fs::read_to_string(config_file).context("failed to read config file")?
                };
                toml::from_str(&config).context("failed to parse config file")?
            } else {
                args.config
            };
        } else {
            let config = args.config;
        }
    }

    let options = CargoOptions::from(&config);

    let tree_output = cargo_tree_output(&options)?;
    let mut graph = get_dep_graph(&tree_output).context("failed to parse cargo-tree output")?;

    let bloat_output = cargo_bloat_output(&options)?;
    let size_map = get_size_map(&bloat_output).context("failed to parse cargo-bloat output")?;

    let mut root_idx = petgraph::graph::NodeIndex::new(0);

    if let Some(root) = &config.root {
        root_idx = change_root(&mut graph, root).context("failed to change root")?;
    }

    let std_idx = if config.std {
        Some(graph.add_node(NodeWeight {
            name: "std ".to_string(),
            short_end: 3,
        }))
    } else {
        None
    };

    let cum_sums_vec = cum_sums(&graph, &size_map);

    let node_colouring_values = match config.scheme {
        None => None,
        Some(scheme) => {
            let (values, mut gamma) = match scheme {
                NodeColoringScheme::CumSum => cum_sums_vec.clone(),
                NodeColoringScheme::DepCount => dep_counts(&graph),
                NodeColoringScheme::RevDepCount => rev_dep_counts(&graph),
            };

            if let Some(gamma_) = config.gamma {
                gamma = gamma_.clamp(0.0, 1.0);
            }

            let max = values.iter().copied().max().unwrap();
            let gradient = config.gradient.clone().unwrap_or_default().into();

            Some(NodeColoringValues {
                values,
                gamma,
                max,
                gradient,
            })
        }
    };

    if let Some(threshold) = config.threshold {
        remove_small_deps(&mut graph, &cum_sums_vec.0, threshold, std_idx);
    }

    if let Some(excludes) = &config.excludes {
        remove_excluded_deps(&mut graph, excludes, root_idx, std_idx)
            .context("failed to exclude dependencies")?;
    }

    if let Some(max_depth) = config.max_depth {
        remove_deep_deps(&mut graph, root_idx, max_depth, std_idx);
    }

    let output_filename = config.output.as_deref();
    let templates = get_templates(&config).context("failed to parse templates")?;
    let dot = output_dot(
        &graph,
        &size_map,
        &config,
        &templates,
        node_colouring_values,
    );

    if config.dot_only {
        std::fs::write(output_filename.unwrap_or("output.gv"), dot)
            .context("failed to write output dot file")?;
    } else {
        output_svg(
            &dot,
            &graph,
            output_filename.unwrap_or("output.svg"),
            &config,
        )?;
    }

    Ok(())
}
