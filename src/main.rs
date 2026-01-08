mod command;
mod config;

use command::{cargo_bloat_output, cargo_tree_output};

use anyhow::{Context, bail};
use clap::Parser;
use pugio_lib::{
    coloring::{NodeColoringScheme, NodeColoringValues},
    graph::{DotOptions, Graph},
    template::{Template, TemplateOptions},
};

use crate::command::{CargoOptions, SvgOptions, output_svg};
use crate::config::Config;

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

fn get_matched_node_indices(graph: &Graph, pattern: &str) -> anyhow::Result<Vec<usize>> {
    #[cfg(feature = "regex")]
    let regex = regex_lite::Regex::new(pattern)?;

    let filter = |i: &usize| -> bool {
        let name = graph.node_weight(*i).full();
        cfg_if::cfg_if! {
            if #[cfg(feature = "regex")] {
                regex.is_match(name)
            } else {
                name.starts_with(pattern)
            }
        }
    };

    Ok(graph.node_indices().filter(filter).collect::<Vec<_>>())
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

    let options = CargoOptions {
        package: config.package.clone(),
        bin: config.bin.clone(),
        features: config.features.clone(),
        all_features: config.all_features,
        no_default_features: config.no_default_features,
        release: config.release,
    };

    let cargo_tree_output = cargo_tree_output(&options)?;
    if cargo_tree_output.contains("\n\n") || cargo_tree_output.contains("\r\n\r\n") {
        bail!("one and only one package must be specified");
    }
    let cargo_bloat_output = cargo_bloat_output(&options)?;

    let mut graph = Graph::new(
        &cargo_tree_output,
        &cargo_bloat_output,
        config.std,
        config.bin.as_deref(),
    );

    if let Some(root) = &config.root {
        let indices = get_matched_node_indices(&graph, root)?;
        if indices.is_empty() {
            bail!("dependency name pattern not found");
        } else if indices.len() > 1 {
            bail!("dependency name pattern not unique");
        } else {
            graph.change_root(indices[0]);
        }
    }

    let gradient = config.gradient.unwrap_or_default();
    let node_values = match config.scheme {
        None => None,
        Some(scheme) => {
            let mut node_values = NodeColoringValues::new(&graph, scheme);

            if let Some(gamma) = config.gamma {
                node_values.set_gamma(gamma);
            }

            Some(node_values)
        }
    };

    if let Some(threshold) = config.threshold {
        let cum_sums = NodeColoringValues::new(&graph, NodeColoringScheme::CumSum);
        let std = graph.std();

        let iter = cum_sums
            .indices_values()
            .filter(|(i, s)| *s < threshold && Some(*i) != std)
            .map(|(i, _)| i);

        graph.remove_indices(iter);
    }

    if let Some(excludes) = &config.excludes {
        let indices = excludes.iter().try_fold(Vec::new(), |mut v, e| {
            let indices = get_matched_node_indices(&graph, e)?;
            v.extend(indices);
            Ok::<_, anyhow::Error>(v)
        })?;
        graph.remove_indices(indices.into_iter());
    }

    if let Some(depth) = config.depth {
        graph.remove_deep_deps(depth);
    }

    let output_filename = config.output.as_deref();

    let template_options = TemplateOptions {
        node_label_template: config.node_label_template,
        node_tooltip_template: config.node_tooltip_template,
        edge_label_template: config.edge_label_template,
        edge_tooltip_template: config.edge_tooltip_template,
    };
    let template = Template::new(&template_options).context("failed to parse templates")?;

    let dot_options = DotOptions {
        highlight: config.highlight,
        bin: config.bin,
        inverse_gradient: config.inverse_gradient,
        dark_mode: config.dark_mode,
    };

    let dot = graph.output_dot(&dot_options, &template, &node_values, &gradient);

    if config.dot_only {
        std::fs::write(output_filename.unwrap_or("output.gv"), dot)
            .context("failed to write output dot file")?;
    } else {
        let svg_options = SvgOptions {
            scale_factor: config.scale_factor,
            separation_factor: config.separation_factor,
            padding: config.padding,
            dark_mode: config.dark_mode,
            highlight: config.highlight,
            highlight_amount: config.highlight_amount,
            no_open: config.no_open,
        };

        output_svg(
            &dot,
            &graph,
            output_filename.unwrap_or("output.svg"),
            &svg_options,
        )?;
    }

    Ok(())
}
