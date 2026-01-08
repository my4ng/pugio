use std::collections::BTreeMap;

use serde::Serialize;
use tinytemplate::TinyTemplate;

use crate::{coloring::NodeColoringScheme, error::TemplateError, graph::Graph};

/// Trait for templating node and edge labels and tooltips.
pub trait Templating {
    /// The type of context shared across all nodes and edges.
    type Context;
    /// The type of value for each node.
    type Value;

    /// Get the label and tooltip for a given node.
    fn node(
        &self,
        graph: &Graph,
        index: usize,
        value: Self::Value,
        context: Self::Context,
    ) -> (String, String);

    /// Get the label and tooltip for a given edge.
    fn edge(&self, graph: &Graph, source: usize, target: usize) -> (String, String);
}

/// Options for [`Template`] creation.
///
/// Internally, [`Template`] uses [`TinyTemplate`](https://docs.rs/tinytemplate/latest/tinytemplate/)
/// for templating, which allows formatting using the `{...}` syntax.
///
/// # Node template values
///
/// - `short`: Short name of the node.
/// - `extra`: Extra information of the node.
/// - `full`: Full name of the node.
/// - `size`: Size of the node in bytes.
/// - `size_binary`: Size of the node in binary format (e.g., "1.0 KiB").
/// - `size_decimal`: Size of the node in decimal format (e.g., "1.0 kB").
/// - `scheme`: Coloring scheme used (if any).
/// - `value`: Value used for coloring (if any).
/// - `value_binary`: Value used for coloring in binary format (if any).
/// - `value_decimal`: Value used for coloring in decimal format (if any).
/// - `features`: Features of the node.
///
/// # Edge template values
/// - `source`: Short name of the source node.
/// - `target`: Short name of the target node.
/// - `features`: Features of the edge.
#[derive(Debug, Default)]
pub struct TemplateOptions {
    pub node_label_template: Option<String>,
    pub node_tooltip_template: Option<String>,
    pub edge_label_template: Option<String>,
    pub edge_tooltip_template: Option<String>,
}

/// Templating system for node and edge labels and tooltips.
///
/// This implements the [`Templating`] trait to be used in conjunction with `Option<NodeColoringValues>`.
pub struct Template<'a>(pub(crate) TinyTemplate<'a>);

impl<'a> Template<'a> {
    /// Create a new [`Template`] from the given [`TemplateOptions`].
    ///
    /// # Defaults
    ///
    /// If a template option field is `None`, its corresponding default is used:
    /// - Node label: `"{short}"`
    /// - Node tooltip: `"{full}\n{size_binary}\n{features}"`
    /// - Edge label: `"{features}"`
    /// - Edge tooltip: `"{source} -> {target}"`
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
        graph: &Graph,
        index: usize,
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
            scheme: Option<&'static str>,
            value: Option<usize>,
            value_binary: Option<String>,
            value_decimal: Option<String>,
            features: String,
        }

        let node = graph.node_weight(index);
        let size = graph.size(index).unwrap_or_default();

        let context = NodeContext {
            short: node.short(),
            extra: node.extra(),
            full: node.full(),
            size,
            size_binary: humansize::format_size(size, humansize::BINARY),
            size_decimal: humansize::format_size(size, humansize::DECIMAL),
            scheme: context.map(Into::into),
            value,
            value_binary: value.map(|v| humansize::format_size(v, humansize::BINARY)),
            value_decimal: value.map(|v| humansize::format_size(v, humansize::DECIMAL)),
            features: features(&node.features),
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

    fn edge(&self, graph: &Graph, source: usize, target: usize) -> (String, String) {
        #[derive(Serialize)]
        struct EdgeContext<'a> {
            source: &'a str,
            target: &'a str,
            features: String,
        }

        let edge = graph.edge_weight(source, target);
        let source = graph.node_weight(source);
        let target = graph.node_weight(target);

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
