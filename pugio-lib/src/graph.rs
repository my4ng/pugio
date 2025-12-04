use std::collections::{BTreeMap, HashMap, VecDeque};

use petgraph::{
    graph::NodeIndex,
    prelude::StableGraph,
    visit::{Bfs, Dfs, Topo, Walker},
};

use crate::cargo::{get_dep_graph, get_size_map};

#[derive(Debug, Default)]
pub struct Graph {
    pub(crate) inner: StableGraph<NodeWeight, EdgeWeight>,
    pub(crate) size_map: HashMap<String, usize>,
    std: Option<NodeIndex>,
    root: NodeIndex,
}

impl Graph {
    pub fn new(cargo_tree_output: &str, cargo_bloat_output: &str) -> Self {
        let mut graph = get_dep_graph(cargo_tree_output);
        graph.size_map = get_size_map(cargo_bloat_output);
        graph.normalize_sizes();
        graph
    }

    pub fn add_std(&mut self) {
        let weight = NodeWeight {
            name: "std ".to_string(),
            short_end: 3,
            features: BTreeMap::new(),
        };
        let index = self.inner.add_node(weight);
        self.std = Some(index);
    }

    pub fn std(&self) -> Option<usize> {
        self.std.map(|i| i.index())
    }

    pub fn root(&self) -> usize {
        self.root.index()
    }

    pub fn size(&self, index: usize) -> Option<usize> {
        let index = NodeIndex::new(index);
        let short_name = self.inner.node_weight(index).unwrap().short();
        self.size_map.get(short_name).copied()
    }

    pub fn node_count(&self) -> usize {
        self.inner.node_count()
    }

    pub fn node_weight(&self, index: usize) -> Option<&NodeWeight> {
        self.inner.node_weight(NodeIndex::new(index))
    }

    pub fn node_indices(&self) -> impl Iterator<Item = usize> {
        self.inner.node_indices().map(|i| i.index())
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

    pub(crate) fn node_classes(&self, is_dir_down: bool) -> Vec<Vec<usize>> {
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

    pub fn dfs(&self) -> impl Iterator<Item = usize> {
        Dfs::new(&self.inner, self.root)
            .iter(&self.inner)
            .map(|i| i.index())
    }

    pub fn bfs(&self) -> impl Iterator<Item = usize> {
        Bfs::new(&self.inner, self.root)
            .iter(&self.inner)
            .map(|i| i.index())
    }

    pub fn topo(&self) -> impl Iterator<Item = usize> {
        Topo::new(&self.inner).iter(&self.inner).map(|i| i.index())
    }

    pub fn children(&self, index: usize) -> impl Iterator<Item = usize> {
        let index = NodeIndex::new(index);
        self.inner.neighbors(index).map(|i| i.index())
    }

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

    pub fn remove_indices(&mut self, indices: impl Iterator<Item = usize>) {
        let inner = &mut self.inner;

        for index in indices {
            inner.remove_node(NodeIndex::new(index));
        }

        self.remove_unreachable();
    }

    pub fn change_root(&mut self, new_root_index: usize) {
        let index = NodeIndex::new(new_root_index);
        assert!(self.inner.contains_node(index));
        self.root = index;
        self.remove_unreachable();
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

    pub fn short(&self) -> &str {
        &self.name[..self.short_end]
    }

    pub fn extra(&self) -> &str {
        &self.name[self.short_end + 1..]
    }

    pub fn full(&self) -> &str {
        &self.name
    }

    pub fn features(&self) -> &BTreeMap<String, Vec<String>> {
        &self.features
    }
}

#[derive(Debug, Clone)]
pub struct EdgeWeight {
    pub(crate) features: BTreeMap<String, Vec<String>>,
}

impl EdgeWeight {
    pub fn features(&self) -> &BTreeMap<String, Vec<String>> {
        &self.features
    }
}
