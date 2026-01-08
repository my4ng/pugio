/*!
`pugio-lib` is a library to generate, filter, and output dependency graphs of Rust projects,
for dependency visualisation such as bloat or security auditing. It is used as the backend of
the [`pugio`](https://crates.io/crates/pugio) CLI tool.

It provides traits such as [`coloring::Gradient`], [`coloring::Values`], and
[`template::Templating`] for full customisation of the generated graphs beyond only binary size
analysis.

# Example
Here is a simple example of how to use `pugio-lib` to generate a dependency graph in DOT format,
using custom defined templating, values and gradient implementations.

```no_run
use pugio_lib::graph::Graph;
use pugio_lib::template::Templating;
use pugio_lib::coloring::{Color, Gradient, Values};

// Output of `cargo tree --edges=no-build,no-proc-macro,no-dev,features --prefix=depth --color=never ...`
let cargo_tree_output = "...";
// Output of `cargo bloat -n0 --message-format=json --crates ...`
let cargo_bloat_output = "...";

let mut graph = Graph::new(cargo_tree_output, cargo_bloat_output, false, None);

// Remove dependencies more than 3 levels deep.
graph.remove_deep_deps(3);

// Remove all dependencies that are path or git specified.
let iter = graph.node_indices().filter(|i| {
    graph.node_weight(*i).extra().ends_with(")")
}).collect::<Vec<_>>().into_iter();

graph.remove_indices(iter);

// Custom Gradient implementation.
struct CustomGradient;

impl Gradient for CustomGradient {
    type Input = usize;

    // Ignore `dark_mode` and `inverse` for simplicity.
    fn color(&self, input: Self::Input, dark_mode: bool, inverse: bool) -> Color {
        if input > 4096 {
            Color {
                r: 255,
                g: 0,
                b: 0,
            }
        } else {
            Color {
                r: 0,
                g: 255,
                b: 0,
            }
        }
    }
}

// Custom Templating implementation.
struct CustomTemplate;

impl Templating for CustomTemplate {
    type Context = bool;
    type Value = &'static str;

    fn node(
        &self,
        graph: &Graph,
        index: usize,
        value: Self::Value,
        context: Self::Context,
    ) -> (String, String) {
        let tooltip = if context {
            value.to_string()
        } else {
            "".to_string()
        };
        (graph.node_weight(index).short().to_string(), tooltip)
    }

    fn edge(
        &self,
        _graph: &Graph,
        _source: usize,
        _target: usize,
    ) -> (String, String) {
        ("".to_string(), "".to_string())
    }
}

// Custom Values implementation.
struct CustomValues(Vec<usize>);

impl CustomValues {
    fn new(graph: &Graph) -> Self {
        let mut values = vec![0; graph.node_capacity()];
        for index in graph.node_indices() {
            let size = graph.size(index).unwrap_or_default();
            values[index] = size;
        }
        Self(values)
    }
}

impl Values for CustomValues {
    type Context = bool;
    type Value = &'static str;
    type Output = usize;

    fn context(&self) -> Self::Context {
        true
    }

    fn value(&self, index: usize) -> Self::Value {
        if self.0[index] > 4096 {
            "large"
        } else {
            "small"
        }
    }

    fn output(&self, index: usize) -> Self::Output {
        self.0[index]
    }
}

let custom_values = CustomValues::new(&graph);

// Output the graph in DOT format.
let dot_output = graph.output_dot(&Default::default(), &CustomTemplate, &custom_values, &CustomGradient);
```
*/

mod cargo;
pub mod coloring;
pub mod error;
pub mod graph;
pub mod template;
