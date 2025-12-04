use std::collections::BTreeMap;

use serde::Serialize;
use tinytemplate::TinyTemplate;

use crate::{
    coloring::NodeColoringScheme,
    error::TemplateError,
    graph::{EdgeWeight, NodeWeight},
};

pub trait Templating {
    type Context;
    type Value;

    fn node(
        &self,
        node: &NodeWeight,
        size: usize,
        value: Self::Value,
        context: Self::Context,
    ) -> (String, String);
    fn edge(&self, source: &NodeWeight, target: &NodeWeight, edge: &EdgeWeight)
    -> (String, String);
}

#[derive(Debug)]
pub struct TemplateOptions {
    pub node_label_template: Option<String>,
    pub node_tooltip_template: Option<String>,
    pub edge_label_template: Option<String>,
    pub edge_tooltip_template: Option<String>,
}

pub struct Template<'a>(pub(crate) TinyTemplate<'a>);

impl<'a> Template<'a> {
    pub fn new(template_options: &'a TemplateOptions) -> Result<Self, TemplateError> {
        let mut template = TinyTemplate::new();
        template.add_template(
            "node_label",
            template_options
                .node_label_template
                .as_deref()
                .unwrap_or("{short}"),
        )?;
        template.add_template(
            "node_tooltip",
            template_options
                .node_tooltip_template
                .as_deref()
                .unwrap_or("{full}\n{size_binary}\n{features}"),
        )?;
        template.add_template(
            "edge_label",
            template_options
                .edge_label_template
                .as_deref()
                .unwrap_or("{features}"),
        )?;
        template.add_template(
            "edge_tooltip",
            template_options
                .edge_tooltip_template
                .as_deref()
                .unwrap_or("{source} -> {target}"),
        )?;
        Ok(Template(template))
    }
}

impl<'a> Templating for Template<'a> {
    type Context = Option<NodeColoringScheme>;
    type Value = Option<usize>;

    fn node(
        &self,
        node: &NodeWeight,
        size: usize,
        value: Self::Value,
        context: Self::Context,
    ) -> (String, String) {
        #[derive(Serialize)]
        struct NodeContext<'a> {
            short: &'a str,
            extra: &'a str,
            full: &'a str,
            size: usize,
            size_binary: String,
            size_decimal: String,
            value: Option<usize>,
            value_binary: Option<String>,
            value_decimal: Option<String>,
            features: String,
            scheme: Option<&'static str>,
        }

        let context = NodeContext {
            short: node.short(),
            extra: node.extra(),
            full: node.full(),
            size,
            size_binary: humansize::format_size(size, humansize::BINARY),
            size_decimal: humansize::format_size(size, humansize::DECIMAL),
            value,
            value_binary: value.map(|v| humansize::format_size(v, humansize::BINARY)),
            value_decimal: value.map(|v| humansize::format_size(v, humansize::DECIMAL)),
            features: features(&node.features),
            scheme: context.map(Into::into),
        };

        let label = self
            .0
            .render("node_label", &context)
            .unwrap_or_else(|e| e.to_string());
        let tooltip = self
            .0
            .render("node_tooltip", &context)
            .unwrap_or_else(|e| e.to_string());
        (label, tooltip)
    }

    fn edge(
        &self,
        source: &NodeWeight,
        target: &NodeWeight,
        edge: &EdgeWeight,
    ) -> (String, String) {
        #[derive(Serialize)]
        struct EdgeContext<'a> {
            source: &'a str,
            target: &'a str,
            features: String,
        }

        let context = EdgeContext {
            source: source.short(),
            target: target.short(),
            features: features(&edge.features),
        };

        let label = self
            .0
            .render("edge_label", &context)
            .unwrap_or_else(|e| e.to_string());
        let tooltip = self
            .0
            .render("edge_tooltip", &context)
            .unwrap_or_else(|e| e.to_string());
        (label, tooltip)
    }
}

fn features(features: &BTreeMap<String, Vec<String>>) -> String {
    features
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
