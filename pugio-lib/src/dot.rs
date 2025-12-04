use petgraph::{
    dot::{Config, Dot},
    graph::NodeIndex,
    stable_graph::{EdgeReference, StableGraph},
    visit::EdgeRef,
};

use crate::{
    coloring::{Gradient, Values},
    graph::{EdgeWeight, Graph, NodeWeight},
    template::Templating,
};

#[derive(Debug)]
pub struct DotOptions {
    pub highlight: Option<bool>,
    pub bin: Option<String>,
    pub inverse_gradient: bool,
    pub dark_mode: bool,
}

pub fn output_dot<T, V, C, R, S, G>(
    graph: &Graph,
    dot_options: &DotOptions,
    templating: &R,
    values: &S,
    gradient: &G,
) -> String
where
    R: Templating<Context = C, Value = V>,
    S: Values<Context = C, Value = V, Output = T>,
    G: Gradient<Input = T>,
{
    let classes = dot_options
        .highlight
        .map(|is_dir_down| graph.node_classes(is_dir_down));

    let size_map = &graph.size_map;

    let node_binding = |_, (i, n): (NodeIndex, &NodeWeight)| {
        let index = i.index();
        let mut size = size_map.get(n.short()).copied().unwrap_or_default();
        if let Some(bin) = dot_options.bin.as_ref()
            && index == 0
        {
            size += size_map.get(bin).copied().unwrap_or_default();
        }
        let width = (size as f32 / 4096.0 + 1.0).log10();

        let context = values.context();
        let value = values.value(index);
        let output = values.output(index);
        let color = gradient.color(output, dot_options.dark_mode);
        let color = format!("#{color:X}");

        let (label, tooltip) = templating.node(n, size, value, context);

        let classes = if let Some(classes) = &classes {
            &classes[index]
                .iter()
                .map(|i| format!("node{i}"))
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            ""
        };

        format!(
            r#"class = "{classes}" label = "{label}" tooltip = "{tooltip}" width = {width} fillcolor= "{color}""#,
        )
    };

    let edge_binding = |g: &StableGraph<NodeWeight, EdgeWeight>,
                        e: EdgeReference<'_, EdgeWeight>| {
        let source = g.node_weight(e.source()).unwrap();
        let target = g.node_weight(e.target()).unwrap();

        let (label, tooltip) = templating.edge(source, target, e.weight());

        let classes = if let Some(classes) = &classes {
            let i = if dot_options.highlight.unwrap() {
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
        &graph.inner,
        &[Config::EdgeNoLabel, Config::NodeNoLabel],
        &edge_binding,
        &node_binding,
    );

    format!("{dot:?}")
}
