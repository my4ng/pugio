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
    stable_graph::EdgeReference,
    visit::EdgeRef,
};
use tinytemplate::TinyTemplate;

use crate::{
    Args, NodeColoringValues,
    graph::NodeWeight,
    template::{EdgeContext, NodeContext},
};

pub fn output_svg(
    dot_output: &str,
    graph: &StableGraph<NodeWeight, ()>,
    output_filename: &str,
    args: &Args,
) -> anyhow::Result<()> {
    let node_count_factor = (graph.node_count() as f32 / 32.0).floor();
    let scale_factor = args.scale_factor.unwrap_or(1.0);
    let font_size = (node_count_factor * 3.0 + 15.0) * scale_factor;
    let arrow_size = (node_count_factor * 0.2 + 0.8) * scale_factor;
    let edge_width = (node_count_factor * 0.4 + 1.2) * scale_factor;
    let node_border_width = edge_width * 0.75;

    let sep_factor = args.separation_factor.unwrap_or(1.0);
    let node_sep = 0.35 * sep_factor;
    let rank_sep = node_sep * 2.0;

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
        .arg("-Efontname=monospace")
        .arg(format!("-Efontsize={font_size}"))
        .arg(format!("-Earrowsize={arrow_size}"))
        .arg("-Earrowhead=onormal")
        .arg(format!("-Epenwidth={edge_width}"))
        .arg(format!("-Gnodesep={node_sep}"))
        .arg(format!("-Granksep={rank_sep}"));

    if args.dark_mode {
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
    if !args.no_open {
        open::that_detached(output_filename).context("failed to open output svg")?;
    }
    Ok(())
}

pub fn output_dot(
    graph: &StableGraph<NodeWeight, ()>,
    size_map: &HashMap<String, usize>,
    args: &Args,
    templates: &TinyTemplate,
    node_colouring_values: Option<NodeColoringValues>,
) -> String {
    let node_binding = |_, (i, n): (NodeIndex, &NodeWeight)| {
        let mut size = size_map.get(n.short()).copied().unwrap_or_default();
        if let Some(bin) = args.binary.as_ref()
            && i.index() == 0
        {
            size += size_map.get(bin).copied().unwrap_or_default();
        }
        let width = (size as f32 / 4096.0 + 1.0).log10();

        let (node_color, value) = if let Some(NodeColoringValues {
            values,
            gamma,
            max,
            gradient,
        }) = &node_colouring_values
        {
            let value = values[i.index()];
            let mut t = (value as f32 / *max as f32).powf(*gamma);
            if args.inverse_gradient {
                t = 1.0 - t;
            }

            let mut node_color = gradient.at(t);
            if args.dark_mode {
                let mut hsla = node_color.to_hsla();
                hsla[2] = 1.0 - hsla[2];
                node_color = colorgrad::Color::from_hsla(hsla[0], hsla[1], hsla[2], hsla[3])
            }
            (node_color, Some(value))
        } else {
            #[allow(clippy::collapsible_else_if)]
            let node_color = if args.dark_mode {
                colorgrad::Color::new(0.0, 0.0, 0.0, 0.0)
            } else {
                colorgrad::Color::new(1.0, 1.0, 1.0, 1.0)
            };
            (node_color, None)
        };
        let node_color = node_color.to_css_hex();

        let node_context = NodeContext::new(n, size, value, args.scheme);
        let label = templates
            .render("node_label", &node_context)
            .unwrap_or_else(|e| e.to_string());
        let tooltip = templates
            .render("node_tooltip", &node_context)
            .unwrap_or_else(|e| e.to_string());

        format!(
            r#"label = "{label}" tooltip = "{tooltip}" width = {width} fillcolor= "{node_color}""#,
        )
    };

    let edge_binding = |g: &&StableGraph<NodeWeight, ()>, e: EdgeReference<'_, ()>| {
        let source = g.node_weight(e.source()).unwrap();
        let target = g.node_weight(e.target()).unwrap();

        let edge_context = EdgeContext::new(source, target);
        let label = templates
            .render("edge_label", &edge_context)
            .unwrap_or_else(|e| e.to_string());
        let tooltip = templates
            .render("edge_tooltip", &edge_context)
            .unwrap_or_else(|e| e.to_string());

        format!(r#"label = "{label}" edgetooltip = "{tooltip}""#)
    };

    let dot = Dot::with_attr_getters(
        &graph,
        &[Config::EdgeNoLabel, Config::NodeNoLabel],
        &edge_binding,
        &node_binding,
    );

    format!("{dot:?}")
}
