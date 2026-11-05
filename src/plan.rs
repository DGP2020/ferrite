//! Bottom-up translation order with ownership and type-mapping hints.
//!
//! Owned by Ultra in the plan stage. Skeleton for now.

use crate::models::{FunctionUnit, UnitStatus};
use std::collections::{HashMap, HashSet};

pub struct Planner;

impl Planner {
    /// Sort units so callees come before callers (bottom-up topological sort).
    pub fn order(units: Vec<FunctionUnit>) -> Vec<FunctionUnit> {
        let mut by_id: HashMap<String, FunctionUnit> = HashMap::new();
        let mut id_by_name: HashMap<String, String> = HashMap::new();
        let mut ordered_ids = Vec::with_capacity(units.len());
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();

        let mut input_order = Vec::with_capacity(units.len());
        for u in units {
            input_order.push(u.id.clone());
            id_by_name.insert(u.name.clone(), u.id.clone());
            id_by_name.insert(u.id.clone(), u.id.clone());
            by_id.insert(u.id.clone(), u);
        }

        fn dfs(
            id: &str,
            by_id: &HashMap<String, FunctionUnit>,
            id_by_name: &HashMap<String, String>,
            visited: &mut HashSet<String>,
            visiting: &mut HashSet<String>,
            ordered: &mut Vec<String>,
        ) {
            if visited.contains(id) {
                return;
            }
            if visiting.contains(id) {
                return;
            }

            visiting.insert(id.to_string());

            if let Some(unit) = by_id.get(id) {
                for callee in &unit.callees {
                    let target_id = id_by_name.get(callee).map(|s| s.as_str()).unwrap_or(callee);
                    if by_id.contains_key(target_id) {
                        dfs(target_id, by_id, id_by_name, visited, visiting, ordered);
                    }
                }
            }

            visiting.remove(id);
            visited.insert(id.to_string());
            ordered.push(id.to_string());
        }

        for id in &input_order {
            dfs(
                id,
                &by_id,
                &id_by_name,
                &mut visited,
                &mut visiting,
                &mut ordered_ids,
            );
        }

        ordered_ids
            .into_iter()
            .filter_map(|id| by_id.remove(&id))
            .map(|u| FunctionUnit {
                status: UnitStatus::Planned,
                ..u
            })
            .collect()
    }
}
