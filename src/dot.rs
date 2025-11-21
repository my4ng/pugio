use std::{
    collections::HashMap,
    io::Write,
    process::{Command, Stdio},
};

use anyhow::Context;
use colorgrad::Gradient;
use petgraph::{
    dot::{Config, Dot},
    graph::NodeIndex,
    prelude::StableGraph,
    visit::EdgeRef,
};

use crate::{Args, NodeColoringValues, graph::NodeWeight};

pub fn output_svg(
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

pub fn output_dot(
    graph: &StableGraph<NodeWeight, ()>,
    size_map: &HashMap<String, usize>,
    args: &Args,
    node_colouring_values: Option<NodeColoringValues>,
) -> String {
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

    format!("{dot:?}")
}
