use std::{
    io::Write,
    process::{Command, Stdio},
};

use anyhow::Context;
use pugio_lib::graph::Graph;

#[derive(Debug, Default)]
pub struct CargoOptions {
    pub package: Option<String>,
    pub bin: Option<String>,
    pub features: Option<String>,
    pub all_features: bool,
    pub no_default_features: bool,
    pub release: bool,
}

pub fn cargo_tree_output(options: &CargoOptions) -> anyhow::Result<String> {
    let mut command = Command::new("cargo");
    command
        .stdout(Stdio::piped())
        .arg("tree")
        .arg("--edges=no-build,no-proc-macro,no-dev,features")
        .arg("--prefix=depth")
        .arg("--color=never");

    if let Some(package) = &options.package {
        command.arg(format!("--package={package}"));
    }

    if let Some(features) = &options.features {
        command.arg(format!("--features={features}"));
    }

    if options.all_features {
        command.arg("--all-features");
    }

    if options.no_default_features {
        command.arg("--no-default-features");
    }

    command
        .spawn()
        .context("failed to execute cargo-tree")?
        .wait_with_output()
        .map(|o| String::from_utf8(o.stdout).unwrap())
        .context("failed to wait on cargo-tree")
}

pub fn cargo_bloat_output(options: &CargoOptions) -> anyhow::Result<String> {
    let mut command = Command::new("cargo");
    command
        .stdout(Stdio::piped())
        .arg("bloat")
        .arg("-n0")
        .arg("--message-format=json")
        .arg("--crates");

    if let Some(package) = &options.package {
        command.arg(format!("--package={package}"));
    }

    if let Some(binary) = &options.bin {
        command.arg(format!("--bin={binary}"));
    }

    if let Some(features) = &options.features {
        command.arg(format!("--features={features}"));
    }

    if options.all_features {
        command.arg("--all-features");
    }

    if options.no_default_features {
        command.arg("--no-default-features");
    }

    if options.release {
        command.arg("--release");
    }

    command
        .spawn()
        .context("failed to execute cargo-bloat")?
        .wait_with_output()
        .map(|o| String::from_utf8(o.stdout).unwrap())
        .context("failed to wait on cargo-bloat")
}

#[derive(Debug, Default)]
pub struct SvgOptions {
    pub scale_factor: Option<f32>,
    pub separation_factor: Option<f32>,
    pub padding: Option<f32>,
    pub dark_mode: bool,
    pub highlight: Option<bool>,
    pub highlight_amount: Option<f32>,
    pub no_open: bool,
}

pub fn output_svg(
    dot_output: &str,
    graph: &Graph,
    output_filename: &str,
    svg_options: &SvgOptions,
) -> anyhow::Result<()> {
    let node_count_factor = (graph.node_count() as f32 / 32.0).floor();
    let scale_factor = svg_options.scale_factor.unwrap_or(1.0);
    let node_font_size = (node_count_factor * 3.0 + 15.0) * scale_factor;
    let edge_font_size = node_font_size * 0.75;
    let arrow_size = (node_count_factor * 0.2 + 0.6) * scale_factor;
    let edge_width = arrow_size * 2.0;
    let node_border_width = edge_width * 0.75;

    let sep_factor = svg_options.separation_factor.unwrap_or(1.0);
    let node_sep = 0.35 * sep_factor;
    let rank_sep = node_sep * 2.0;
    let padding = svg_options.padding.unwrap_or(1.0);

    let mut command = Command::new("dot");
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .arg("-Tsvg")
        .arg(format!("-Gpad={padding}"))
        .arg("-Nshape=circle")
        .arg(format!("-Npenwidth={node_border_width}"))
        .arg("-Nstyle=filled")
        .arg("-Nfixedsize=shape")
        .arg("-Nfontname=monospace")
        .arg(format!("-Nfontsize={node_font_size}"))
        .arg("-Efontname=monospace")
        .arg(format!("-Efontsize={edge_font_size}"))
        .arg(format!("-Earrowsize={arrow_size}"))
        .arg("-Earrowhead=onormal")
        .arg(format!("-Epenwidth={edge_width}"))
        .arg(format!("-Gnodesep={node_sep}"))
        .arg(format!("-Granksep={rank_sep}"));

    if svg_options.dark_mode {
        command
            .arg("-Gbgcolor=#000000")
            .arg("-Ncolor=#FFFFFF")
            .arg("-Ecolor=#FFFFFF9F")
            .arg("-Efontcolor=#FFFFFFFF")
            .arg("-Nfontcolor=#FFFFFF");
    } else {
        command
            .arg("-Ncolor=#000000")
            .arg("-Nfontcolor=#000000")
            .arg("-Ecolor=#0000009F")
            .arg("-Efontcolor=#000000");
    }

    let mut child = command.spawn().context("failed to execute dot")?;

    let stdin = child.stdin.as_mut().context("failed to get stdin")?;
    stdin
        .write_all(dot_output.as_bytes())
        .context("failed to write into stdin")?;

    let output = child.wait_with_output().context("failed to wait on dot")?;
    let mut svg =
        String::from_utf8(output.stdout).context("failed to convert dot output to string")?;

    if svg_options.highlight.is_some() {
        let index = svg
            .find("<g id=\"graph0\"")
            .context("failed to find graph start")?;

        let highlight_amount = 1.0 - svg_options.highlight_amount.unwrap_or(0.5).clamp(0.0, 1.0);
        let rules = graph
            .node_indices()
            .map(|i| {
                format!(
                    ".graph:has(.node{i}:hover) > g:not(.node{i}) {{ opacity: {highlight_amount} }}"
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let style = format!("<style>\n{rules}\n</style>\n");
        svg.insert_str(index, &style);
    }

    std::fs::write(output_filename, svg).context("failed to write output svg file")?;
    if !svg_options.no_open {
        open::that_detached(output_filename).context("failed to open output svg")?;
    }
    Ok(())
}
