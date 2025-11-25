use colorgrad::BasisGradient;

#[cfg_attr(feature = "config", derive(serde_with::DeserializeFromStr))]
#[derive(Clone, Copy, strum::EnumString)]
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

#[cfg_attr(feature = "config", derive(serde_with::DeserializeFromStr))]
#[derive(Default, Clone)]
pub enum NodeColoringGradient {
    #[default]
    Reds,
    Oranges,
    Purples,
    Greens,
    Blues,
    Custom(BasisGradient),
}

impl std::str::FromStr for NodeColoringGradient {
    type Err = colorgrad::GradientBuilderError;

    fn from_str(s: &str) -> Result<NodeColoringGradient, Self::Err> {
        match s {
            "reds" => Ok(Self::Reds),
            "oranges" => Ok(Self::Oranges),
            "purples" => Ok(Self::Purples),
            "greens" => Ok(Self::Greens),
            "blues" => Ok(Self::Blues),
            _ => colorgrad::GradientBuilder::new()
                .css(s)
                .build()
                .map(Self::Custom),
        }
    }
}

impl From<NodeColoringGradient> for BasisGradient {
    fn from(value: NodeColoringGradient) -> Self {
        use colorgrad::preset::*;
        match value {
            NodeColoringGradient::Reds => reds(),
            NodeColoringGradient::Oranges => oranges(),
            NodeColoringGradient::Purples => purples(),
            NodeColoringGradient::Greens => greens(),
            NodeColoringGradient::Blues => blues(),
            NodeColoringGradient::Custom(gradient) => gradient,
        }
    }
}

pub struct NodeColoringValues {
    pub values: Vec<usize>,
    pub gamma: f32,
    pub max: usize,
    pub gradient: BasisGradient,
}
