use std::{
    collections::HashMap,
    io::Write,
    process::{Command, Stdio},
};

use anyhow::Context;
use petgraph::{
    dot::{Config, Dot},
    graph::NodeIndex,
    stable_graph::EdgeReference,
    visit::EdgeRef,
};
use tinytemplate::TinyTemplate;

use crate::{
    NodeColoringValues,
    graph::{EdgeWeight, Graph, NodeWeight, node_classes},
    template::{EdgeContext, NodeContext},
};

pub fn output_svg(
    dot_output: &str,
    graph: &Graph,
    output_filename: &str,
    config: &crate::config::Config,
) -> anyhow::Result<()> {
    let node_count_factor = (graph.node_count() as f32 / 32.0).floor();
    let scale_factor = config.scale_factor.unwrap_or(1.0);
    let node_font_size = (node_count_factor * 3.0 + 15.0) * scale_factor;
    let edge_font_size = node_font_size * 0.75;
    let arrow_size = (node_count_factor * 0.2 + 0.6) * scale_factor;
    let edge_width = arrow_size * 2.0;
    let node_border_width = edge_width * 0.75;

    let sep_factor = config.separation_factor.unwrap_or(1.0);
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
        .arg(format!("-Nfontsize={node_font_size}"))
        .arg("-Efontname=monospace")
        .arg(format!("-Efontsize={edge_font_size}"))
        .arg(format!("-Earrowsize={arrow_size}"))
        .arg("-Earrowhead=onormal")
        .arg(format!("-Epenwidth={edge_width}"))
        .arg(format!("-Gnodesep={node_sep}"))
        .arg(format!("-Granksep={rank_sep}"));

    if config.dark_mode {
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

    if config.highlight.is_some() {
        let idx = svg
            .find("<g id=\"graph0\"")
            .context("failed to find graph start")?;

        let highlight_amount = 1.0 - config.highlight_amount.unwrap_or(0.5).clamp(0.0, 1.0);
        let rules = graph
            .node_indices()
            .map(|i| {
                let i = i.index();
                format!(
                    ".graph:has(.node{i}:hover) > g:not(.node{i}) {{ opacity: {highlight_amount} }}"
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let style = format!("<style>\n{rules}\n</style>\n");
        svg.insert_str(idx, &style);
    }

    std::fs::write(output_filename, svg).context("failed to write output svg file")?;
    if !config.no_open {
        open::that_detached(output_filename).context("failed to open output svg")?;
    }
    Ok(())
}

pub fn output_dot(
    graph: &Graph,
    size_map: &HashMap<String, usize>,
    config: &crate::config::Config,
    templates: &TinyTemplate,
    node_colouring_values: Option<NodeColoringValues>,
) -> String {
    let classes = config
        .highlight
        .map(|is_dir_down| node_classes(graph, is_dir_down));

    let node_binding = |_, (i, n): (NodeIndex, &NodeWeight)| {
        let mut size = size_map.get(n.short()).copied().unwrap_or_default();
        if let Some(bin) = config.bin.as_ref()
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
            let mut t = (value as f64 / *max as f64).powf(*gamma);
            if config.inverse_gradient {
                t = 1.0 - t;
            }

            let mut node_color = gradient.eval_continuous(t);
            if config.dark_mode {
                let mut hsl: colorsys::Hsl =
                    colorsys::Rgb::from(&(node_color.r, node_color.g, node_color.b)).into();
                hsl.set_lightness(100.0 - hsl.lightness());
                let (r, g, b) = colorsys::Rgb::from(hsl).into();
                node_color = colorous::Color { r, g, b };
            }
            (node_color, Some(value))
        } else {
            #[allow(clippy::collapsible_else_if)]
            let node_color = if config.dark_mode {
                colorous::Color {
                    r: 0x00,
                    g: 0x00,
                    b: 0x00,
                }
            } else {
                colorous::Color {
                    r: 0xff,
                    g: 0xff,
                    b: 0xff,
                }
            };
            (node_color, None)
        };
        let node_color = format!("#{node_color:X}");

        let node_context = NodeContext::new(n, size, value, config.scheme);
        let label = templates
            .render("node_label", &node_context)
            .unwrap_or_else(|e| e.to_string());
        let tooltip = templates
            .render("node_tooltip", &node_context)
            .unwrap_or_else(|e| e.to_string());

        let classes = if let Some(classes) = &classes {
            &classes[i.index()]
                .iter()
                .map(|i| format!("node{i}"))
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            ""
        };

        format!(
            r#"class = "{classes}" label = "{label}" tooltip = "{tooltip}" width = {width} fillcolor= "{node_color}""#,
        )
    };

    let edge_binding = |g: &&Graph, e: EdgeReference<'_, EdgeWeight>| {
        let source = g.node_weight(e.source()).unwrap();
        let target = g.node_weight(e.target()).unwrap();

        let edge_context = EdgeContext::new(e.weight(), source, target);
        let label = templates
            .render("edge_label", &edge_context)
            .unwrap_or_else(|e| e.to_string());
        let tooltip = templates
            .render("edge_tooltip", &edge_context)
            .unwrap_or_else(|e| e.to_string());

        let classes = if let Some(classes) = &classes {
            let i = if config.highlight.unwrap() {
                e.source()
            } else {
                e.target()
            };
            &classes[i.index()]
                .iter()
                .map(|i| format!("node{i}"))
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            ""
        };

        format!(
            r#"class = "{classes}" label = "{label}" edgetooltip = "{tooltip}" labeltooltip = "{tooltip}""#
        )
    };

    let dot = Dot::with_attr_getters(
        &graph,
        &[Config::EdgeNoLabel, Config::NodeNoLabel],
        &edge_binding,
        &node_binding,
    );

    format!("{dot:?}")
}
