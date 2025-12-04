use petgraph::visit::{Topo, Walker};
use serde::Deserialize;

use crate::graph::Graph;

pub use colorous::Color;

pub trait Gradient {
    type Input;

    fn color(&self, input: Self::Input, dark_mode: bool) -> Color;
}

pub trait Values {
    type Context;
    type Value;
    type Output;

    fn context(&self) -> Self::Context;
    fn value(&self, index: usize) -> Self::Value;
    fn output(&self, index: usize) -> Self::Output;
}

#[derive(Deserialize, Debug, Clone, Copy, strum::EnumString)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum NodeColoringScheme {
    CumSum,
    DepCount,
    RevDepCount,
}

impl From<NodeColoringScheme> for &'static str {
    fn from(value: NodeColoringScheme) -> Self {
        match value {
            NodeColoringScheme::CumSum => "cumulative sum",
            NodeColoringScheme::DepCount => "dependency count",
            NodeColoringScheme::RevDepCount => "reverse dependency count",
        }
    }
}

#[derive(Deserialize, Debug, Default, Clone, Copy, strum::EnumString)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum NodeColoringGradient {
    #[default]
    Reds,
    Oranges,
    Purples,
    Greens,
    Blues,
    BuPu,
    OrRd,
    PuRd,
    RdPu,
    Viridis,
    Cividis,
    Plasma,
}

impl From<NodeColoringGradient> for colorous::Gradient {
    fn from(value: NodeColoringGradient) -> Self {
        use colorous::*;
        match value {
            NodeColoringGradient::Reds => REDS,
            NodeColoringGradient::Oranges => ORANGES,
            NodeColoringGradient::Purples => PURPLES,
            NodeColoringGradient::Greens => GREENS,
            NodeColoringGradient::Blues => BLUES,
            NodeColoringGradient::BuPu => BLUE_PURPLE,
            NodeColoringGradient::OrRd => ORANGE_RED,
            NodeColoringGradient::PuRd => PURPLE_RED,
            NodeColoringGradient::RdPu => RED_PURPLE,
            NodeColoringGradient::Viridis => VIRIDIS,
            NodeColoringGradient::Cividis => CIVIDIS,
            NodeColoringGradient::Plasma => PLASMA,
        }
    }
}

impl Gradient for NodeColoringGradient {
    type Input = Option<f64>;

    fn color(&self, input: Self::Input, dark_mode: bool) -> Color {
        if let Some(input) = input {
            let mut color = colorous::Gradient::from(*self).eval_continuous(input.clamp(0.0, 1.0));

            if dark_mode {
                let mut hsl: colorsys::Hsl =
                    colorsys::Rgb::from(&(color.r, color.g, color.b)).into();
                hsl.set_lightness(100.0 - hsl.lightness());
                let (r, g, b) = colorsys::Rgb::from(hsl).into();
                color = Color { r, g, b };
            }
            color
        } else {
            #[allow(clippy::collapsible_else_if)]
            if dark_mode {
                Color {
                    r: 0x00,
                    g: 0x00,
                    b: 0x00,
                }
            } else {
                Color {
                    r: 0xff,
                    g: 0xff,
                    b: 0xff,
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct NodeColoringValues {
    values: Vec<usize>,
    gamma: f64,
    max: usize,
    scheme: NodeColoringScheme,
}

impl NodeColoringValues {
    pub fn cum_sums(graph: &Graph) -> Self {
        let map = &graph.size_map;
        let graph = &graph.inner;
        let mut values = vec![0; graph.capacity().0];

        for (index, size) in graph.node_indices().filter_map(|i| {
            let short_name = graph.node_weight(i).unwrap().short();
            map.get(short_name).copied().map(|s| (i.index(), s))
        }) {
            values[index] = size;
        }

        let nodes = Topo::new(&graph).iter(&graph).collect::<Vec<_>>();

        for node in nodes.iter().rev() {
            let sources: Vec<_> = graph
                .neighbors_directed(*node, petgraph::Direction::Incoming)
                .collect();
            for source in &sources {
                values[source.index()] += values[node.index()] / sources.len();
            }
        }

        let max = *values.iter().max().unwrap();

        Self {
            values,
            gamma: 0.25,
            max,
            scheme: NodeColoringScheme::CumSum,
        }
    }

    pub fn dep_counts(graph: &Graph) -> Self {
        let graph = &graph.inner;

        let mut values = vec![0; graph.capacity().0];

        let nodes = Topo::new(&graph).iter(&graph).collect::<Vec<_>>();

        for node in nodes.iter().rev() {
            for target in graph.neighbors(*node) {
                values[node.index()] += values[target.index()] + 1;
            }
        }

        let max = *values.iter().max().unwrap();

        Self {
            values,
            gamma: 0.25,
            max,
            scheme: NodeColoringScheme::DepCount,
        }
    }

    pub fn rev_dep_counts(graph: &Graph) -> Self {
        let graph = &graph.inner;

        let mut values = vec![0; graph.capacity().0];

        for node in Topo::new(&graph).iter(&graph) {
            for target in graph.neighbors(node) {
                values[target.index()] += 1;
            }
        }

        let max = *values.iter().max().unwrap();

        Self {
            values,
            gamma: 0.5,
            max,
            scheme: NodeColoringScheme::RevDepCount,
        }
    }

    pub fn set_gamma(&mut self, gamma: f64) {
        self.gamma = gamma.clamp(0.0, 1.0);
    }

    pub fn values(&self) -> &[usize] {
        &self.values
    }

    pub fn gamma(&self) -> f64 {
        self.gamma
    }

    pub fn max(&self) -> usize {
        self.max
    }

    pub fn scheme(&self) -> NodeColoringScheme {
        self.scheme
    }
}

impl Values for Option<NodeColoringValues> {
    type Context = Option<NodeColoringScheme>;
    type Value = Option<usize>;
    type Output = Option<f64>;

    fn context(&self) -> Self::Context {
        self.as_ref().map(|v| v.scheme)
    }

    fn value(&self, index: usize) -> Self::Value {
        self.as_ref().map(|v| v.values[index])
    }

    fn output(&self, index: usize) -> Self::Output {
        self.as_ref()
            .map(|v| (v.values[index] as f64 / v.max as f64).powf(v.gamma))
    }
}
