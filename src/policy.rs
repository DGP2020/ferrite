use crate::models::{Attempt, FunctionUnit, Stage, Tier};

/// Decides which Nemotron tier handles a given call.
///
/// Build this interface on Day 1 so Track B (cascade + optional predictive
/// router) is additive rather than a fork.
pub trait ModelPolicy: Send + Sync {
    fn select(&self, unit: &FunctionUnit, stage: Stage, history: &[Attempt]) -> Tier;
}

impl<P: ?Sized + ModelPolicy> ModelPolicy for Box<P> {
    fn select(&self, unit: &FunctionUnit, stage: Stage, history: &[Attempt]) -> Tier {
        (**self).select(unit, stage, history)
    }
}

/// Track A: static tier per stage. Repair escalates after two failed attempts.
pub struct StaticPolicy {
    pub plan_tier: Tier,
    pub translate_tier: Tier,
    pub seed_tier: Tier,
    pub repair_tier: Tier,
    pub explain_tier: Tier,
    pub repair_escalate_after: u32,
}

impl Default for StaticPolicy {
    fn default() -> Self {
        Self {
            plan_tier: Tier::Ultra,
            translate_tier: Tier::Super,
            seed_tier: Tier::Nano,
            repair_tier: Tier::Super,
            explain_tier: Tier::Ultra,
            repair_escalate_after: 2,
        }
    }
}

impl ModelPolicy for StaticPolicy {
    fn select(&self, _unit: &FunctionUnit, stage: Stage, history: &[Attempt]) -> Tier {
        let failed_attempts = history
            .iter()
            .filter(|a| a.stage == stage && !a.compile_result.ok)
            .count() as u32;

        match stage {
            Stage::Plan => self.plan_tier,
            Stage::Translate => self.translate_tier,
            Stage::Seed => self.seed_tier,
            Stage::Repair => {
                if failed_attempts >= self.repair_escalate_after {
                    Tier::Ultra
                } else {
                    self.repair_tier
                }
            }
            Stage::Explain => self.explain_tier,
        }
    }
}

/// Track B v1: start cheap, escalate on verification failure.
/// Every verifier outcome is a free label for routing.
pub struct CascadePolicy {
    pub min_tier: Tier,
    /// After this many total failed attempts on a function, escalate one tier.
    pub escalate_after: u32,
}

impl CascadePolicy {
    pub fn next_tier(current: Tier) -> Tier {
        match current {
            Tier::Nano => Tier::Super,
            Tier::Super => Tier::Ultra,
            Tier::Ultra => Tier::Ultra,
        }
    }
}

impl Default for CascadePolicy {
    fn default() -> Self {
        Self {
            min_tier: Tier::Nano,
            escalate_after: 1,
        }
    }
}

impl ModelPolicy for CascadePolicy {
    fn select(&self, _unit: &FunctionUnit, stage: Stage, history: &[Attempt]) -> Tier {
        // Only translation/repair stages use the cascade; others fall back to
        // the cheapest plausible tier.
        if stage != Stage::Translate && stage != Stage::Repair {
            return self.min_tier;
        }

        let failed: u32 = history
            .iter()
            .filter(|a| {
                a.stage == stage
                    && (!a.compile_result.ok
                        || a.verdict
                            .as_ref()
                            .is_some_and(|v| v.kind != crate::models::VerdictKind::Equivalent))
            })
            .count() as u32;

        let starter = if stage == Stage::Repair {
            history.last().map(|a| a.tier).unwrap_or(self.min_tier)
        } else {
            self.min_tier
        };

        if failed >= self.escalate_after {
            Self::next_tier(starter)
        } else {
            starter
        }
    }
}
