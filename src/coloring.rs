use colorous::Gradient;

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
#[derive(Default, Clone, strum::EnumString)]
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

impl From<NodeColoringGradient> for Gradient {
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

pub struct NodeColoringValues {
    pub values: Vec<usize>,
    pub gamma: f64,
    pub max: usize,
    pub gradient: Gradient,
}
