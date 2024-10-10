use std::{collections::HashMap, sync::Arc};

use anyhow::anyhow;
use serde::{Deserialize, Serialize};

use crate::{statement::Statement, traits::DbStatementArgInterface};

pub struct ExecutionStatement<A: DbStatementArgInterface> {
    stmt: Arc<Statement<A, ()>>,
    fallback: Option<Arc<ExecutionStatement<A>>>,
}

pub struct ExecutionPlanBuilder<A: DbStatementArgInterface> {
    plan: ExecutionPlan<A>,
}

impl<A: DbStatementArgInterface> Default for ExecutionPlanBuilder<A> {
    fn default() -> Self {
        Self {
            plan: ExecutionPlan {
                run: Default::default(),
                post_run: Default::default(),
            },
        }
    }
}

impl<A: DbStatementArgInterface> ExecutionPlanBuilder<A> {
    /// Add a concurrently executed statement.
    ///
    /// Returns the builder and the index of the branch.
    ///
    /// `fallback` runs in the concurrent branch as fallback.
    /// For fallbacks that should run for failed branches after
    /// the execution of all branches use
    /// [`Self::post_execution_fallback`] with the returned
    /// branch index instead.
    pub fn add_concurrent(
        mut self,
        stmt: Arc<Statement<A, ()>>,
        fallback: Option<Arc<Statement<A, ()>>>,
    ) -> (Self, usize) {
        let branch_index = self.plan.run.len();
        self.plan.run.push(ExecutionStatement {
            stmt,
            fallback: fallback.map(|f| {
                Arc::new(ExecutionStatement {
                    stmt: f,
                    fallback: None,
                })
            }),
        });
        (self, branch_index)
    }

    pub fn post_execution_fallback(
        mut self,
        branch_index: usize,
        stmt: Arc<Statement<A, ()>>,
    ) -> Self {
        self.plan.post_run.insert(
            branch_index,
            ExecutionStatement {
                stmt,
                fallback: None,
            },
        );
        self
    }

    pub fn build(self) -> Arc<ExecutionPlan<A>> {
        Arc::new(self.plan)
    }
}

/// The result on how the query was successfully or partially
/// sucessfully executed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ExecutionBranchResult {
    /// The query was executed successfully.
    Success(u64),
    /// The main query failed, but the fallback
    /// was executed successfully.
    FallbackUsed(u64),
    /// The main query (and fallback if given) failed,
    /// but the fallback system, that triggers
    /// after all branches executed, executed
    /// successfully.
    PostRunFallbackUsed(u64),
}

/// A helper to contruct how multiple statements
/// should be executed, with fallbacks etc.
pub struct ExecutionPlan<A: DbStatementArgInterface> {
    run: Vec<ExecutionStatement<A>>,
    post_run: HashMap<usize, ExecutionStatement<A>>,
}

impl<A: DbStatementArgInterface + Clone> ExecutionPlan<A> {
    pub fn builder() -> ExecutionPlanBuilder<A> {
        Default::default()
    }

    pub async fn execute(&self, args: A) -> Vec<anyhow::Result<ExecutionBranchResult>> {
        let runs = self
            .run
            .iter()
            .map(|r| async {
                let res = r.stmt.execute(args.clone()).await;
                if let (Err(main_err), Some(stmt)) = (&res, &r.fallback) {
                    let res = stmt.stmt.execute(args.clone()).await;
                    match res {
                        Ok(affected_rows) => {
                            return Ok(ExecutionBranchResult::FallbackUsed(affected_rows));
                        }
                        Err(err) => {
                            return Err(anyhow!(
                                "main query failed: {main_err}. fallback query failed: {err}"
                            ));
                        }
                    }
                }
                res.map(ExecutionBranchResult::Success)
            })
            .collect::<Vec<_>>();

        let mut res = futures::future::join_all(runs).await;

        let err_runs = res
            .iter()
            .enumerate()
            .filter_map(|(branch_index, r)| {
                if r.is_err() {
                    self.post_run
                        .get(&branch_index)
                        .map(|stmt| (branch_index, stmt.stmt.execute(args.clone())))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let (indices, futures): (Vec<_>, Vec<_>) = err_runs.into_iter().unzip();

        let fallback_res = futures::future::join_all(futures).await;

        for (i, fallback_res) in fallback_res.into_iter().enumerate() {
            match fallback_res {
                Ok(affected_rows) => {
                    res[indices[i]] = Ok(ExecutionBranchResult::PostRunFallbackUsed(affected_rows));
                }
                Err(err) => {
                    if let Err(main_err) = &res[indices[i]] {
                        res[indices[i]] =
                            Err(anyhow!("{main_err}. fallback run query failed: {err}"));
                    }
                }
            }
        }

        res
    }
}
