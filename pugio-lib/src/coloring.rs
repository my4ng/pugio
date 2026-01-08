use serde::Deserialize;

use crate::graph::Graph;

/// RGB Color.
pub use colorous::Color;

/// Trait for providing coloring based on input values.
pub trait Gradient {
    /// The type of input used for determining color.
    type Input;

    /// Get the color corresponding to the given input, given the dark mode and inverse settings.
    fn color(&self, input: Self::Input, dark_mode: bool, inverse: bool) -> Color;
}

/// Trait for providing values for coloring nodes.
pub trait Values {
    /// The type of context shared across all nodes.
    type Context;
    /// The type of value for each node.
    type Value;
    /// The type of output used for coloring.
    type Output;

    /// Get the shared context.
    fn context(&self) -> Self::Context;
    /// Get the value for the node at the given index.
    ///
    /// # Panics
    /// May panic if the node index does not exist.
    fn value(&self, index: usize) -> Self::Value;
    /// Get the output for the node at the given index.
    ///
    /// # Panics
    /// May panic if the node index does not exist.
    fn output(&self, index: usize) -> Self::Output;
}

/// Scheme for coloring nodes.
#[derive(Deserialize, Debug, Clone, Copy, strum::EnumString)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum NodeColoringScheme {
    /// Cumulative sum
    ///
    /// The cumulative size of a node and all its dependencies.
    CumSum,
    /// Dependency count
    ///
    /// The number of transitive dependencies a node has.
    DepCount,
    /// Reverse dependency count
    ///
    /// The number of paths from the root to a node.
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

/// Gradient for coloring nodes.
///
/// This implements the [`Gradient`] trait to provide colors based on input values of type `Option<f64>`.
#[derive(Deserialize, Debug, Default, Clone, Copy, strum::EnumString)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum NodeColoringGradient {
    /// Reds
    #[default]
    Reds,
    /// Oranges
    Oranges,
    /// Purples
    Purples,
    /// Greens
    Greens,
    /// Blues
    Blues,
    /// Blue-Purple
    BuPu,
    /// Orange-Red
    OrRd,
    /// Purple-Red
    PuRd,
    /// Red-Purple
    RdPu,
    /// Viridis
    Viridis,
    /// Cividis
    Cividis,
    /// Plasma
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

    fn color(&self, input: Self::Input, dark_mode: bool, inverse: bool) -> Color {
        if let Some(input) = input {
            let input = input.clamp(0.0, 1.0);
            let input = if inverse { 1.0 - input } else { input };
            let mut color = colorous::Gradient::from(*self).eval_continuous(input);

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

/// Values for coloring nodes.
///
/// This implements the [`Values`] trait to provide coloring values and outputs based on the selected
/// [`NodeColoringScheme`].
#[derive(Debug)]
pub struct NodeColoringValues {
    indices: Vec<usize>,
    values: Vec<usize>,
    gamma: f64,
    max: usize,
    scheme: NodeColoringScheme,
}

impl NodeColoringValues {
    /// Create new coloring values for the given graph and scheme.
    pub fn new(graph: &Graph, scheme: NodeColoringScheme) -> Self {
        match scheme {
            NodeColoringScheme::CumSum => Self::cum_sums(graph),
            NodeColoringScheme::DepCount => Self::dep_counts(graph),
            NodeColoringScheme::RevDepCount => Self::rev_dep_counts(graph),
        }
    }

    /// Create cumulative sum coloring values for the given graph.
    fn cum_sums(graph: &Graph) -> Self {
        let mut values = vec![0; graph.node_capacity()];

        for (index, size) in graph
            .node_indices()
            .filter_map(|i| graph.size(i).map(|s| (i, s)))
        {
            values[index] = size;
        }

        let nodes: Vec<usize> = graph.topo().collect();

        for node in nodes.iter().rev() {
            let sources: Vec<_> = graph.neighbors(*node, false).collect();
            for source in sources.iter() {
                values[*source] += values[*node] / sources.len();
            }
        }

        let max = *values.iter().max().unwrap();
        let indices = graph.node_indices().collect();

        Self {
            indices,
            values,
            gamma: 0.25,
            max,
            scheme: NodeColoringScheme::CumSum,
        }
    }

    /// Create dependency count coloring values for the given graph.
    fn dep_counts(graph: &Graph) -> Self {
        let mut values = vec![0; graph.node_capacity()];

        let nodes: Vec<usize> = graph.topo().collect();

        for node in nodes.iter().rev() {
            for target in graph.neighbors(*node, true) {
                values[*node] += values[target] + 1;
            }
        }

        let max = *values.iter().max().unwrap();
        let indices = graph.node_indices().collect();

        Self {
            indices,
            values,
            gamma: 0.25,
            max,
            scheme: NodeColoringScheme::DepCount,
        }
    }

    /// Create reverse dependency count coloring values for the given graph.
    fn rev_dep_counts(graph: &Graph) -> Self {
        let mut values = vec![0; graph.node_capacity()];

        for node in graph.topo() {
            for target in graph.neighbors(node, true) {
                values[target] += 1;
            }
        }

        let max = *values.iter().max().unwrap();
        let indices = graph.node_indices().collect();

        Self {
            indices,
            values,
            gamma: 0.5,
            max,
            scheme: NodeColoringScheme::RevDepCount,
        }
    }

    /// Get an iterator over the node indices of the graph as it was in [`new`](Self::new) and their
    /// corresponding values.
    pub fn indices_values(&self) -> impl Iterator<Item = (usize, usize)> {
        self.indices.iter().copied().map(|i| (i, self.values[i]))
    }

    /// Set the gamma value, clamped between 0.0 and 1.0.
    pub fn set_gamma(&mut self, gamma: f64) {
        self.gamma = gamma.clamp(0.0, 1.0);
    }

    /// Get the gamma value.
    pub fn gamma(&self) -> f64 {
        self.gamma
    }

    /// Get the maximum value.
    pub fn max(&self) -> usize {
        self.max
    }

    /// Get the coloring scheme.
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
