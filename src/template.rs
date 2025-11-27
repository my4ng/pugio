use serde::Serialize;
use tinytemplate::TinyTemplate;

use crate::{
    NodeColoringScheme,
    config::Config,
    graph::{EdgeWeight, NodeWeight},
};

pub fn get_templates(config: &Config) -> anyhow::Result<TinyTemplate<'_>> {
    let mut templates = TinyTemplate::new();
    templates.add_template(
        "node_label",
        config.node_label_template.as_deref().unwrap_or("{short}"),
    )?;
    templates.add_template(
        "node_tooltip",
        config
            .node_tooltip_template
            .as_deref()
            .unwrap_or("{full}\n{size_binary}\n{features}"),
    )?;
    templates.add_template(
        "edge_label",
        config
            .edge_label_template
            .as_deref()
            .unwrap_or("{features}"),
    )?;
    templates.add_template(
        "edge_tooltip",
        config
            .edge_tooltip_template
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
    features: String,
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
            features: node_features(weight),
        }
    }
}

#[derive(Serialize)]
pub struct EdgeContext<'a> {
    source: &'a str,
    target: &'a str,
    features: String,
}

impl<'a> EdgeContext<'a> {
    pub fn new(edge: &EdgeWeight, source: &'a NodeWeight, target: &'a NodeWeight) -> Self {
        Self {
            source: source.short(),
            target: target.short(),
            features: edge_features(edge),
        }
    }
}

fn node_features(node_weight: &NodeWeight) -> String {
    node_weight
        .features
        .iter()
        .map(|(f, d)| {
            if d.is_empty() {
                f.clone()
            } else {
                format!("{f}({})", d.join(","))
            }
        })
        .collect::<Vec<String>>()
        .join(",")
}

fn edge_features(edge_weight: &EdgeWeight) -> String {
    edge_weight
        .features
        .iter()
        .map(|(f, d)| {
            if d.is_empty() {
                f.clone()
            } else {
                format!("{f}({})", d.join(","))
            }
        })
        .collect::<Vec<String>>()
        .join(",\n")
}
