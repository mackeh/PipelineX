use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a single step within a CI job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepInfo {
    pub name: String,
    pub uses: Option<String>,
    pub run: Option<String>,
    pub estimated_duration_secs: Option<f64>,
}

/// Represents a cache configuration detected or recommended.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub path: String,
    pub key_pattern: String,
    pub restore_keys: Vec<String>,
}

/// Matrix strategy for a job.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MatrixStrategy {
    pub variables: HashMap<String, Vec<String>>,
    pub total_combinations: usize,
}

/// A node in the Pipeline DAG representing a single job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobNode {
    pub id: String,
    pub name: String,
    pub steps: Vec<StepInfo>,
    pub needs: Vec<String>,
    pub runs_on: String,
    pub estimated_duration_secs: f64,
    pub caches: Vec<CacheConfig>,
    pub matrix: Option<MatrixStrategy>,
    pub condition: Option<String>,
    pub env: HashMap<String, String>,
    pub paths_filter: Option<Vec<String>>,
    pub paths_ignore: Option<Vec<String>>,
}

impl JobNode {
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            steps: Vec::new(),
            needs: Vec::new(),
            runs_on: "ubuntu-latest".to_string(),
            estimated_duration_secs: 0.0,
            caches: Vec::new(),
            matrix: None,
            condition: None,
            env: HashMap::new(),
            paths_filter: None,
            paths_ignore: None,
        }
    }
}

/// Edge types in the Pipeline DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DagEdge {
    /// Hard dependency — job B cannot start until job A completes.
    Dependency,
    /// Artifact dependency — job B needs artifacts from job A.
    Artifact,
}

/// Trigger event for the workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTrigger {
    pub event: String,
    pub branches: Option<Vec<String>>,
    pub paths: Option<Vec<String>>,
    pub paths_ignore: Option<Vec<String>>,
}

/// The unified Pipeline DAG — the core data structure of PipelineX.
#[derive(Debug, Clone)]
pub struct PipelineDag {
    pub name: String,
    pub source_file: String,
    pub provider: String,
    pub triggers: Vec<WorkflowTrigger>,
    pub graph: DiGraph<JobNode, DagEdge>,
    pub node_map: HashMap<String, NodeIndex>,
    pub env: HashMap<String, String>,
}

impl PipelineDag {
    pub fn new(name: String, source_file: String, provider: String) -> Self {
        Self {
            name,
            source_file,
            provider,
            triggers: Vec::new(),
            graph: DiGraph::new(),
            node_map: HashMap::new(),
            env: HashMap::new(),
        }
    }

    /// Add a job node to the DAG, returning its index.
    pub fn add_job(&mut self, job: JobNode) -> NodeIndex {
        let id = job.id.clone();
        let idx = self.graph.add_node(job);
        self.node_map.insert(id, idx);
        idx
    }

    /// Add a dependency edge between two jobs.
    pub fn add_dependency(&mut self, from_id: &str, to_id: &str) -> anyhow::Result<()> {
        let from_idx = self
            .node_map
            .get(from_id)
            .ok_or_else(|| anyhow::anyhow!("Job '{}' not found in DAG", from_id))?;
        let to_idx = self
            .node_map
            .get(to_id)
            .ok_or_else(|| anyhow::anyhow!("Job '{}' not found in DAG", to_id))?;
        self.graph.add_edge(*from_idx, *to_idx, DagEdge::Dependency);
        Ok(())
    }

    /// Get all root jobs (jobs with no dependencies).
    pub fn root_jobs(&self) -> Vec<NodeIndex> {
        self.graph
            .node_indices()
            .filter(|&idx| {
                self.graph
                    .neighbors_directed(idx, Direction::Incoming)
                    .count()
                    == 0
            })
            .collect()
    }

    /// Get all leaf jobs (jobs that nothing depends on).
    pub fn leaf_jobs(&self) -> Vec<NodeIndex> {
        self.graph
            .node_indices()
            .filter(|&idx| {
                self.graph
                    .neighbors_directed(idx, Direction::Outgoing)
                    .count()
                    == 0
            })
            .collect()
    }

    /// Get the total number of jobs.
    pub fn job_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Get total step count across all jobs.
    pub fn step_count(&self) -> usize {
        self.graph.node_weights().map(|j| j.steps.len()).sum()
    }

    /// Get a job node by its ID.
    pub fn get_job(&self, id: &str) -> Option<&JobNode> {
        self.node_map.get(id).map(|idx| &self.graph[*idx])
    }

    /// Get all job IDs.
    pub fn job_ids(&self) -> Vec<String> {
        self.graph.node_weights().map(|j| j.id.clone()).collect()
    }

    /// Compute the maximum parallelism (max number of jobs that can run concurrently).
    pub fn max_parallelism(&self) -> usize {
        // BFS level-based approach: jobs at the same depth can run in parallel
        let mut levels: HashMap<NodeIndex, usize> = HashMap::new();
        let roots = self.root_jobs();

        for root in &roots {
            self.compute_levels(*root, 0, &mut levels);
        }

        if levels.is_empty() {
            return 0;
        }

        let max_level = *levels.values().max().unwrap_or(&0);
        let mut level_counts = vec![0usize; max_level + 1];
        for level in levels.values() {
            level_counts[*level] += 1;
        }
        level_counts.into_iter().max().unwrap_or(0)
    }

    fn compute_levels(
        &self,
        node: NodeIndex,
        level: usize,
        levels: &mut HashMap<NodeIndex, usize>,
    ) {
        let current = levels.entry(node).or_insert(0);
        if level > *current {
            *current = level;
        }
        let level = *levels.get(&node).unwrap();
        let neighbors: Vec<_> = self
            .graph
            .neighbors_directed(node, Direction::Outgoing)
            .collect();
        for neighbor in neighbors {
            self.compute_levels(neighbor, level + 1, levels);
        }
    }
}
