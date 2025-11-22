use serde::Serialize;
use tinytemplate::TinyTemplate;

use crate::{Args, NodeColoringScheme, graph::NodeWeight};

pub fn get_templates(args: &Args) -> anyhow::Result<TinyTemplate<'_>> {
    let mut templates = TinyTemplate::new();
    templates.add_template(
        "node_label",
        args.node_label_template.as_deref().unwrap_or("{short}"),
    )?;
    templates.add_template(
        "node_tooltip",
        args.node_tooltip_template
            .as_deref()
            .unwrap_or("{full}\n{size_binary}"),
    )?;
    templates.add_template(
        "edge_label",
        args.edge_label_template.as_deref().unwrap_or(""),
    )?;
    templates.add_template(
        "edge_tooltip",
        args.edge_tooltip_template
            .as_deref()
            .unwrap_or("{source} -> {target}"),
    )?;
    Ok(templates)
}

#[derive(Serialize)]
pub struct NodeContext<'a> {
    short: &'a str,
    extra: &'a str,
    full: &'a str,
    size: usize,
    size_binary: String,
    size_decimal: String,
    value: Option<usize>,
    value_binary: Option<String>,
    value_decimal: Option<String>,
    scheme: Option<&'static str>,
}

impl<'a> NodeContext<'a> {
    pub fn new(
        weight: &'a NodeWeight,
        size: usize,
        value: Option<usize>,
        scheme: Option<NodeColoringScheme>,
    ) -> Self {
        Self {
            short: weight.short(),
            extra: weight.extra(),
            full: weight.full(),
            size,
            size_binary: humansize::format_size(size, humansize::BINARY),
            size_decimal: humansize::format_size(size, humansize::DECIMAL),
            value,
            value_binary: value.map(|v| humansize::format_size(v, humansize::BINARY)),
            value_decimal: value.map(|v| humansize::format_size(v, humansize::DECIMAL)),
            scheme: scheme.map(NodeColoringScheme::into),
        }
    }
}

#[derive(Serialize)]
pub struct EdgeContext<'a> {
    source: &'a str,
    target: &'a str,
}

impl<'a> EdgeContext<'a> {
    pub fn new(source: &'a NodeWeight, target: &'a NodeWeight) -> Self {
        Self {
            source: source.short(),
            target: target.short(),
        }
    }
}
