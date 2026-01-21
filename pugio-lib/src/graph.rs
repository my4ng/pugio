use std::collections::{BTreeMap, HashMap, VecDeque};

use petgraph::{
    dot::{Config, Dot},
    graph::NodeIndex,
    prelude::StableGraph,
    stable_graph::EdgeReference,
    visit::{Bfs, Dfs, EdgeRef, Topo, Walker},
};

use crate::{
    cargo::{get_dep_graph, get_size_map},
    coloring::{Gradient, Values},
    template::Templating,
};

/// Represents a dependency directed acyclic graph (DAG) with size information, where each node
/// represents a crate, and each directed edge represents a binary relation of dependency of the
/// source node on the target node.
///
/// It also keeps the size information of each crate as parsed from `cargo-bloat` output in a
/// map, which can be accessed using the [`size`](Self::size) method for a given node index.
///
/// The node indices can be iterated using the [`node_indices`](Self::node_indices) method, though
/// there is **no** guarantee of the order of iteration. Use [`dfs`](Self::dfs), [`bfs`](Self::bfs),
/// or [`topo`](Self::topo) if the order of iteration is important, e.g. when a crate must be
/// traversed before its dependencies.
///
/// While [Graph] can be mutated, it is deliberately limited to only
/// [`change_root`](Self::change_root), [`remove_deep_deps`](Self::remove_deep_deps) and
/// [`remove_indices`](Self::remove_indices), as the graph structure should only be reduced and not
/// expanded. In addition, any remaining non-reachable node from the root after these operations
/// will also be removed.
///
/// # Examples
///
///
/// The following example demonstrates a common pattern that will fail to compile without `collect`
/// as otherwise it borrows from `graph` immutably:
///
/// ```
/// # use pugio_lib::graph::Graph;
/// fn remove_z(graph: &mut Graph) {
///     let iter = graph.node_indices().filter(|i| {
///        graph.node_weight(*i).short().starts_with('z')
///     }).collect::<Vec<_>>().into_iter();
///
///     graph.remove_indices(iter);
/// }
/// ```
///
/// Use ordered tranversals when required:
///
/// ```
/// # use pugio_lib::graph::Graph;
/// fn dep_counts(graph: &Graph) -> Vec<usize> {
///     let mut values = vec![0; graph.node_capacity()];
///
///     let nodes: Vec<usize> = graph.topo().collect();
///
///     for node in nodes.iter().rev() {
///         for target in graph.neighbors(*node, true) {
///             values[*node] += values[target] + 1;
///         }
///     }
///
///     values
/// }
/// ```

#[derive(Debug)]
pub struct Graph {
    inner: StableGraph<NodeWeight, EdgeWeight>,
    size_map: HashMap<String, usize>,
    std: Option<NodeIndex>,
    root: NodeIndex,
}

impl Graph {
    /// Create a new graph from the given `cargo-tree` and `cargo-bloat` outputs, with optional `std`
    /// standalone node.
    ///
    /// The `bin` parameter is needed for accurate size accounting if the binary name is different
    /// from its crate name.
    ///
    /// * `cargo_tree_output` should be the output of
    ///   `cargo tree --edges=no-build,no-proc-macro,no-dev,features --prefix=depth --color=never ...`
    /// * `cargo_bloat_output` should be the output of
    ///   `cargo bloat -n0 --message-format=json --crates ...`
    ///
    /// # Panics
    /// May panic if the cargo outputs are malformed.
    pub fn new(
        cargo_tree_output: &str,
        cargo_bloat_output: &str,
        std: bool,
        bin: Option<&str>,
    ) -> Self {
        let mut inner = get_dep_graph(cargo_tree_output);
        let mut size_map = get_size_map(cargo_bloat_output);
        if let Some(bin) = bin {
            let size = size_map.get(bin).copied().unwrap_or_default();
            let root_name = inner
                .node_weight(NodeIndex::new(0))
                .unwrap()
                .short()
                .to_string();
            *size_map.entry(root_name).or_default() += size;
        }
        let std = std.then(|| {
            let weight = NodeWeight {
                name: "std ".to_string(),
                short_end: 3,
                features: BTreeMap::new(),
            };
            inner.add_node(weight)
        });
        inner.shrink_to_fit();
        let mut graph = Graph {
            inner,
            size_map,
            std,
            root: NodeIndex::new(0),
        };
        graph.normalize_sizes();
        graph
    }

    /// Get the index of the `std` standalone node, if it exists.
    pub fn std(&self) -> Option<usize> {
        self.std.map(|i| i.index())
    }

    /// Get the index of the root node.
    pub fn root(&self) -> usize {
        self.root.index()
    }

    /// Get the number of nodes currently in the graph.
    ///
    /// Node indices may be greater than this value if nodes have been removed.
    /// Use [`node_capacity`](Self::node_capacity) instead for allocation.
    pub fn node_count(&self) -> usize {
        self.inner.node_count()
    }

    /// Get the capacity of nodes in the graph.
    ///
    /// This is the upper bound of node indices.
    pub fn node_capacity(&self) -> usize {
        self.inner.capacity().0
    }

    /// Get an iterator over the node indices of the graph.
    pub fn node_indices(&self) -> impl Iterator<Item = usize> {
        self.inner.node_indices().map(|i| i.index())
    }

    /// Get the weight of the node at the given index.
    ///
    /// # Panics
    /// Panics if the node does not exist in the graph.
    pub fn node_weight(&self, index: usize) -> &NodeWeight {
        self.inner.node_weight(NodeIndex::new(index)).unwrap()
    }

    /// Get the weight of the edge at the given index.
    ///
    /// # Panics
    /// Panics if the source or target node, or the directed edge between them, does not exist in the graph.
    ///
    /// ```
    /// # use pugio_lib::graph::{EdgeWeight, Graph};
    /// fn edge_iter(graph: &Graph) -> impl Iterator<Item = &EdgeWeight> {
    ///     graph.node_indices().flat_map(move |i| {
    ///         graph.neighbors(i, true).map(move |j| graph.edge_weight(i, j))
    ///     })
    /// }
    /// ```
    pub fn edge_weight(&self, source: usize, target: usize) -> &EdgeWeight {
        self.inner
            .edge_weight(
                self.inner
                    .find_edge(NodeIndex::new(source), NodeIndex::new(target))
                    .unwrap(),
            )
            .unwrap()
    }

    /// Get the size of the node at the given index.
    ///
    /// Returns `None` if its name is not in the size map.
    ///
    /// # Panics
    /// Panics if the node does not exist in the graph.
    pub fn size(&self, index: usize) -> Option<usize> {
        let short_name = self
            .inner
            .node_weight(NodeIndex::new(index))
            .unwrap()
            .short();
        self.size_map.get(short_name).copied()
    }

    fn normalize_sizes(&mut self) {
        let inner = &self.inner;

        let mut counts = HashMap::with_capacity(inner.node_count());
        for node in inner.node_weights() {
            *counts.entry(node.short()).or_default() += 1;
        }

        for (name, size) in self.size_map.iter_mut() {
            let count = counts.get(name.as_str()).copied().unwrap_or(1);
            *size /= count;
        }
    }

    fn node_classes(&self, is_dir_down: bool) -> Vec<Vec<usize>> {
        let graph = &self.inner;

        let mut classes = vec![Vec::new(); graph.capacity().0];
        let nodes = Topo::new(&graph).iter(&graph).collect::<Vec<_>>();

        if is_dir_down {
            for node in nodes.iter() {
                classes[node.index()].push(node.index());
                for target in graph.neighbors(*node) {
                    // ASSERT: graph is known to be DAG, hence no reflexive edge
                    let [source, target] = classes
                        .get_disjoint_mut([node.index(), target.index()])
                        .unwrap();
                    target.extend_from_slice(source);
                }
            }
        } else {
            for node in nodes.iter().rev() {
                classes[node.index()].push(node.index());
                for target in graph.neighbors(*node) {
                    // ASSERT: graph is known to be DAG, hence no reflexive edge
                    let [source, target] = classes
                        .get_disjoint_mut([node.index(), target.index()])
                        .unwrap();
                    source.extend_from_slice(target);
                }
            }
        }

        classes
    }

    /// Get an iterator over the node indices of the graph in depth-first search order from the root.
    pub fn dfs(&self) -> impl Iterator<Item = usize> {
        Dfs::new(&self.inner, self.root)
            .iter(&self.inner)
            .map(|i| i.index())
    }

    /// Get an iterator over the node indices of the graph in breadth-first search order from the root.
    pub fn bfs(&self) -> impl Iterator<Item = usize> {
        Bfs::new(&self.inner, self.root)
            .iter(&self.inner)
            .map(|i| i.index())
    }

    /// Get an iterator over the node indices of the graph in topological order.
    pub fn topo(&self) -> impl Iterator<Item = usize> {
        Topo::new(&self.inner).iter(&self.inner).map(|i| i.index())
    }

    /// Get an iterator over the neighboring node indices of the given node index.
    ///
    /// If `outgoing` is `true`, get the outgoing neighbors (i.e., dependencies),
    /// otherwise get the incoming neighbors (i.e., dependents).
    pub fn neighbors(&self, index: usize, outgoing: bool) -> impl Iterator<Item = usize> {
        let index = NodeIndex::new(index);
        let direction = if outgoing {
            petgraph::Direction::Outgoing
        } else {
            petgraph::Direction::Incoming
        };
        self.inner
            .neighbors_directed(index, direction)
            .map(|i| i.index())
    }

    /// Remove all nodes that are deeper than `max_depth` from the root, and any nodes that
    /// are subsequently not reachable from the root.
    pub fn remove_deep_deps(&mut self, max_depth: usize) {
        let inner = &mut self.inner;

        // TODO: use petgraph#868 once merged
        let mut queue = VecDeque::from([(self.root, 0)]);
        let mut has_visited = vec![false; inner.capacity().0];
        has_visited[self.root.index()] = true;

        while let Some((node, depth)) = queue.pop_front()
            && depth < max_depth
        {
            for target in inner.neighbors(node) {
                if !has_visited[target.index()] {
                    queue.push_back((target, depth + 1));
                    has_visited[target.index()] = true;
                }
            }
        }

        remove_not_visited(inner, &has_visited, self.std);
    }

    fn remove_unreachable(&mut self) {
        let inner = &self.inner;
        let mut has_visited = vec![false; inner.capacity().0];
        for node_index in Dfs::new(inner, self.root).iter(inner) {
            has_visited[node_index.index()] = true;
        }

        remove_not_visited(&mut self.inner, &has_visited, self.std);
    }

    /// Remove the nodes at the given indices, and any nodes that are subsequently not reachable
    /// from the root.
    pub fn remove_indices(&mut self, indices: impl Iterator<Item = usize>) {
        let inner = &mut self.inner;

        for index in indices {
            inner.remove_node(NodeIndex::new(index));
        }

        self.remove_unreachable();
    }

    /// Change the root node to the given index, and remove any nodes that are not reachable from
    /// the new root.
    pub fn change_root(&mut self, new_root_index: usize) {
        let index = NodeIndex::new(new_root_index);
        assert!(self.inner.contains_node(index));
        self.root = index;
        self.remove_unreachable();
    }

    /// Output the graph in DOT format with the given options, templating, coloring values, and
    /// gradient.
    ///
    /// The `values` parameter should have been created from this graph, possibly before any
    /// node removals.
    ///
    /// ```
    /// # use pugio_lib::graph::Graph;
    /// use pugio_lib::template::{Template, Templating};
    /// use pugio_lib::coloring::{Gradient, Values, NodeColoringScheme, NodeColoringGradient, NodeColoringValues};
    ///
    /// fn output(graph: &Graph) -> String {
    ///     let template_options = Default::default();
    ///     let template = Template::new(&template_options).unwrap();
    ///     let values = Some(NodeColoringValues::new(graph, NodeColoringScheme::CumSum));
    ///     let gradient = NodeColoringGradient::Viridis;
    ///
    ///     graph.output_dot(&Default::default(), &template, &values, &gradient)
    /// }
    ///
    /// ```
    pub fn output_dot<C, V, T, R, S, G>(
        &self,
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
            .map(|is_dir_down| self.node_classes(is_dir_down));

        let node_binding = |_, (i, _): (NodeIndex, _)| {
            let index = i.index();
            let size = self.size(index).unwrap_or_default();
            let width = (size as f32 / 4096.0 + 1.0).log10();

            let context = values.context();
            let value = values.value(index);
            let output = values.output(index);
            let color = gradient.color(output, dot_options.dark_mode, dot_options.inverse_gradient);
            let color = format!("#{color:X}");

            let (label, tooltip) = templating.node(self, index, value, context);

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

        let edge_binding = |_, e: EdgeReference<'_, EdgeWeight>| {
            let (label, tooltip) = templating.edge(self, e.source().index(), e.target().index());

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
            &self.inner,
            &[Config::EdgeNoLabel, Config::NodeNoLabel],
            &edge_binding,
            &node_binding,
        );

        format!("{dot:?}")
    }
}

fn remove_not_visited(
    graph: &mut StableGraph<NodeWeight, EdgeWeight>,
    has_visited: &[bool],
    std_index: Option<NodeIndex>,
) {
    for index in has_visited.iter().enumerate().filter_map(|(i, b)| {
        let index = NodeIndex::new(i);
        if !b && Some(index) != std_index {
            Some(index)
        } else {
            None
        }
    }) {
        graph.remove_node(index);
    }
}

/// The weight of a node in the dependency graph, representing a crate.
///
/// The crate name already has the hyphen `-` replaced with `_` as used in code.
#[derive(Clone)]
pub struct NodeWeight {
    name: String,
    short_end: usize,
    pub(crate) features: BTreeMap<String, Vec<String>>,
}

impl std::fmt::Debug for NodeWeight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeWeight")
            .field("name", &self.name)
            .field("features", &self.features)
            .finish()
    }
}

impl NodeWeight {
    pub(crate) fn new(
        name: String,
        short_end: usize,
        features: BTreeMap<String, Vec<String>>,
    ) -> Self {
        Self {
            name,
            short_end,
            features,
        }
    }

    /// Short name of the crate.
    ///
    /// For example, if the full name is `pugio_lib v1.0.0`, this returns `pugio_lib`.
    pub fn short(&self) -> &str {
        &self.name[..self.short_end]
    }

    /// Extra information of the crate, including version and potentially path.
    ///
    /// For example, if the full name is `pugio_lib v1.0.0`, this returns `v1.0.0`.
    pub fn extra(&self) -> &str {
        &self.name[self.short_end + 1..]
    }

    /// Full name of the crate.
    ///
    /// For example, `pugio_lib v1.0.0`.
    pub fn full(&self) -> &str {
        &self.name
    }

    /// Get the enabled features of the crate.
    ///
    /// This returns a map from a feature to features that it directly enable.
    ///
    /// For example, if a crate has features:
    /// - `default = ["a", "b"]`
    /// - `a = ["c"]`
    /// - `b = []`
    /// - `c = []`
    ///
    /// This returns a map: `{("default": ["a", "b"]), ("a": ["c"]), ("b": []), ("c": [])}`
    ///
    /// This does not include enabled optional dependencies, e.g. `feature = ["dep:crate"]`.
    /// Dependency features, e.g. `feature = ["crate/feature"]` are represented in [`EdgeWeight`]
    /// instead.
    // TODO: report `cargo tree` unable to output feature = ["crate/feature"], which should result
    // in pugio-lib feature "feature"
    //    |- pugio-lib ...
    //    |- crate feature "feature"
    //       |- crate ...
    pub fn features(&self) -> &BTreeMap<String, Vec<String>> {
        &self.features
    }
}

/// The weight of a directed edge in the dependency graph, representing a binary relation of
/// dependency of the source node on the target node.
#[derive(Debug, Clone)]
pub struct EdgeWeight {
    pub(crate) features: BTreeMap<String, Vec<String>>,
}

impl EdgeWeight {
    /// Get the features that are enabled by the dependency.
    ///
    /// This returns a map from a feature of the source crate to features of the target crate that
    /// it enables.
    ///
    /// For example, if the dependent crate has features `a = ["crate/b", "crate/c"]`, enabling
    /// features "b" and "c" of the dependency "crate", this returns a map: `{("a": ["b", "c"]}`.
    pub fn features(&self) -> &BTreeMap<String, Vec<String>> {
        &self.features
    }
}

/// Options for outputting the graph in DOT format.
#[derive(Debug, Default)]
pub struct DotOptions {
    /// If `Some(true)`, highlight nodes in downward direction (dependencies) from the root.
    ///
    /// If `Some(false)`, highlight nodes in upward direction (reverse dependencies) to the root.
    ///
    /// If `None`, do not highlight any nodes.
    pub highlight: Option<bool>,
    /// Name of the binary, if different from the crate name.
    pub bin: Option<String>,
    /// If `true`, invert the gradient for coloring.
    pub inverse_gradient: bool,
    /// If `true`, use dark mode for coloring.
    pub dark_mode: bool,
}
