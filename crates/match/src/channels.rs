use footy_core::MetricId;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Stable, source-neutral football process primitives. Metrics do not affect goals directly: they
/// load into one or more process channels, channels can cause downstream channels, and only terminal
/// channel state is allowed to alter scoring/defensive intensity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TeamProcessChannel {
    Possession,
    BallRetention,
    Progression,
    WidthCreation,
    CentralAccess,
    TransitionAttack,
    ChanceCreation,
    BoxEntries,
    ShotVolume,
    ShotQuality,
    Finishing,
    CounterPress,
    Pressing,
    TransitionDefense,
    DefensiveSuppression,
    AerialControl,
    SetPieceAttack,
    SetPieceDefense,
    Goalkeeping,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricChannelLoading {
    pub metric: MetricId,
    pub channel: TeamProcessChannel,
    /// Contribution of one reference-normalized player metric unit to this channel.
    pub weight: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelEdge {
    pub from: TeamProcessChannel,
    pub to: TeamProcessChannel,
    /// Linear causal propagation coefficient from the upstream channel.
    pub weight: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GoalChannelLoading {
    pub channel: TeamProcessChannel,
    /// Positive raises own scoring log-intensity; negative lowers it.
    pub attack_log_weight: f64,
    /// Positive improves defence and therefore suppresses opponent scoring log-intensity.
    pub defense_log_weight: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CausalChannelGraph {
    pub metric_loadings: Vec<MetricChannelLoading>,
    pub edges: Vec<ChannelEdge>,
    pub goal_loadings: Vec<GoalChannelLoading>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChannelEvaluation {
    pub values: Vec<(TeamProcessChannel, f64)>,
    pub attack_log_adjustment: f64,
    pub defense_log_adjustment: f64,
}

impl CausalChannelGraph {
    pub fn validate(&self) -> bool {
        if self.metric_loadings.iter().any(|value| !value.weight.is_finite())
            || self.edges.iter().any(|value| !value.weight.is_finite() || value.from == value.to)
            || self.goal_loadings.iter().any(|value| !value.attack_log_weight.is_finite() || !value.defense_log_weight.is_finite())
        {
            return false;
        }
        self.topological_order().is_some()
    }

    /// Evaluate reference-normalized metric-rate deviations such as `realized/reference - 1`.
    /// The graph must be acyclic, making the causal order explicit rather than relying on feature
    /// ordering or repeated fixed-point updates.
    pub fn evaluate(&self, metric_deviations: &[(MetricId, f64)]) -> Option<ChannelEvaluation> {
        let order = self.topological_order()?;
        let mut values = BTreeMap::<TeamProcessChannel, f64>::new();
        // Callers provide one deviation per active player/metric. Those are cross-sectional
        // observations of the same team process, not independent team-level shocks to sum. A raw
        // sum makes channel magnitude scale with XI size (and with how many players happen to have
        // a model), which can turn modest player-rate differences into extreme goal intensities.
        // Aggregate repeated metric ids to their active-player mean before applying channel
        // loadings. A metric supplied once is unchanged, preserving the source-neutral API.
        let mut metric_means = BTreeMap::<MetricId, (f64, u32)>::new();
        for (metric, deviation) in metric_deviations {
            if !deviation.is_finite() { return None; }
            let entry = metric_means.entry(*metric).or_default();
            entry.0 += *deviation;
            entry.1 = entry.1.saturating_add(1);
        }
        for (metric, (sum, count)) in metric_means {
            let deviation = sum / f64::from(count.max(1));
            for loading in self.metric_loadings.iter().filter(|value| value.metric == metric) {
                *values.entry(loading.channel).or_default() += loading.weight * deviation;
            }
        }
        for channel in order {
            let source = values.get(&channel).copied().unwrap_or(0.0);
            if source == 0.0 { continue; }
            for edge in self.edges.iter().filter(|edge| edge.from == channel) {
                *values.entry(edge.to).or_default() += edge.weight * source;
            }
        }
        let mut attack = 0.0;
        let mut defense = 0.0;
        for loading in &self.goal_loadings {
            let value = values.get(&loading.channel).copied().unwrap_or(0.0);
            attack += loading.attack_log_weight * value;
            defense += loading.defense_log_weight * value;
        }
        Some(ChannelEvaluation { values: values.into_iter().collect(), attack_log_adjustment: attack, defense_log_adjustment: defense })
    }

    pub fn topological_order(&self) -> Option<Vec<TeamProcessChannel>> {
        let mut nodes = BTreeSet::new();
        for value in &self.metric_loadings { nodes.insert(value.channel); }
        for edge in &self.edges { nodes.insert(edge.from); nodes.insert(edge.to); }
        for value in &self.goal_loadings { nodes.insert(value.channel); }
        let mut indegree: BTreeMap<TeamProcessChannel, usize> = nodes.iter().map(|node| (*node, 0)).collect();
        let mut adjacency = BTreeMap::<TeamProcessChannel, Vec<TeamProcessChannel>>::new();
        for edge in &self.edges {
            adjacency.entry(edge.from).or_default().push(edge.to);
            *indegree.entry(edge.to).or_default() += 1;
        }
        let mut queue: VecDeque<TeamProcessChannel> = indegree.iter().filter(|(_, degree)| **degree == 0).map(|(node, _)| *node).collect();
        let mut order = Vec::with_capacity(nodes.len());
        while let Some(node) = queue.pop_front() {
            order.push(node);
            if let Some(children) = adjacency.get(&node) {
                for child in children {
                    let degree = indegree.get_mut(child)?;
                    *degree = degree.saturating_sub(1);
                    if *degree == 0 { queue.push_back(*child); }
                }
            }
        }
        (order.len() == nodes.len()).then_some(order)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_metric_reaches_goals_only_through_channels() {
        let metric = MetricId(1);
        let graph = CausalChannelGraph {
            metric_loadings: vec![MetricChannelLoading { metric, channel: TeamProcessChannel::Progression, weight: 1.0 }],
            edges: vec![
                ChannelEdge { from: TeamProcessChannel::Progression, to: TeamProcessChannel::ChanceCreation, weight: 0.5 },
                ChannelEdge { from: TeamProcessChannel::ChanceCreation, to: TeamProcessChannel::ShotQuality, weight: 0.4 },
            ],
            goal_loadings: vec![GoalChannelLoading { channel: TeamProcessChannel::ShotQuality, attack_log_weight: 0.2, defense_log_weight: 0.0 }],
        };
        let result = graph.evaluate(&[(metric, 1.0)]).unwrap();
        assert!((result.attack_log_adjustment - 0.04).abs() < 1e-9);
    }

    #[test]
    fn graph_rejects_cycles() {
        let graph = CausalChannelGraph {
            metric_loadings: Vec::new(),
            edges: vec![
                ChannelEdge { from: TeamProcessChannel::Progression, to: TeamProcessChannel::ChanceCreation, weight: 1.0 },
                ChannelEdge { from: TeamProcessChannel::ChanceCreation, to: TeamProcessChannel::Progression, weight: 1.0 },
            ],
            goal_loadings: Vec::new(),
        };
        assert!(!graph.validate());
    }

    #[test]
    fn repeated_player_metric_signals_are_averaged_not_summed() {
        let metric = MetricId(1);
        let graph = CausalChannelGraph {
            metric_loadings: vec![MetricChannelLoading {
                metric,
                channel: TeamProcessChannel::ShotQuality,
                weight: 1.0,
            }],
            edges: Vec::new(),
            goal_loadings: vec![GoalChannelLoading {
                channel: TeamProcessChannel::ShotQuality,
                attack_log_weight: 0.2,
                defense_log_weight: 0.0,
            }],
        };
        let one = graph.evaluate(&[(metric, 0.5)]).unwrap();
        let eleven = graph.evaluate(&vec![(metric, 0.5); 11]).unwrap();
        assert!((one.attack_log_adjustment - 0.1).abs() < 1e-12);
        assert!((eleven.attack_log_adjustment - one.attack_log_adjustment).abs() < 1e-12);
    }
}
