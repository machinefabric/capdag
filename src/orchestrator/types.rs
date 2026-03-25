//! Types for machine notation orchestration
//!
//! This module defines the error types and IR structures used by the orchestrator.

use crate::{Cap, InputStructure};
use std::collections::HashMap;
use thiserror::Error;

// =============================================================================
// Error Types
// =============================================================================

/// Errors that can occur during machine notation parsing and orchestration
#[derive(Debug, Error)]
pub enum ParseOrchestrationError {
    /// Machine notation parsing failed
    #[error("Machine notation parse failed: {0}")]
    MachineSyntaxParseFailed(String),

    /// Cap URN not found in registry
    #[error("Cap URN '{cap_urn}' not found in registry")]
    CapNotFound { cap_urn: String },

    /// Node media URN conflicts with existing assignment
    #[error(
        "Node '{node}' has conflicting media URNs: existing='{existing}', required_by_cap='{required_by_cap}'"
    )]
    NodeMediaConflict {
        node: String,
        existing: String,
        required_by_cap: String,
    },

    /// Graph contains a cycle (not a DAG)
    #[error("Graph is not a DAG, contains cycle involving nodes: {cycle_nodes:?}")]
    NotADag { cycle_nodes: Vec<String> },

    /// Graph contains unsupported or undecomposed control-flow nodes
    #[error("Invalid graph: {message}")]
    InvalidGraph { message: String },

    /// Cap URN parsing error
    #[error("Failed to parse Cap URN: {0}")]
    CapUrnParseError(String),

    /// Media URN parsing error
    #[error("Failed to parse Media URN: {0}")]
    MediaUrnParseError(String),

    /// Registry error
    #[error("Registry error: {0}")]
    RegistryError(String),

    /// Structure mismatch between connected nodes (record vs opaque)
    #[error(
        "Structure mismatch at node '{node}': source is {source_structure:?} but cap expects {expected_structure:?}"
    )]
    StructureMismatch {
        node: String,
        source_structure: InputStructure,
        expected_structure: InputStructure,
    },
}

// =============================================================================
// IR Structures
// =============================================================================

/// A resolved edge in the orchestration graph
#[derive(Debug, Clone)]
pub struct ResolvedEdge {
    /// Source node name
    pub from: String,
    /// Target node name
    pub to: String,
    /// Cap URN string
    pub cap_urn: String,
    /// Resolved cap definition
    pub cap: Cap,
    /// Input media URN from cap definition
    pub in_media: String,
    /// Output media URN from cap definition
    pub out_media: String,
}

/// A resolved orchestration graph
#[derive(Debug, Clone)]
pub struct ResolvedGraph {
    /// Map from node name to derived media URN
    pub nodes: HashMap<String, String>,
    /// Resolved edges with cap definitions
    pub edges: Vec<ResolvedEdge>,
    /// Original graph name (if any)
    pub graph_name: Option<String>,
}

impl ResolvedGraph {
    /// Generate Mermaid flowchart code from this resolved graph.
    pub fn to_mermaid(&self) -> String {
        use std::collections::HashSet;
        let mut out = String::new();
        out.push_str("graph LR\n");

        let mut targets: HashSet<&str> = HashSet::new();
        let mut sources: HashSet<&str> = HashSet::new();
        for edge in &self.edges {
            sources.insert(&edge.from);
            targets.insert(&edge.to);
        }

        for (name, media_urn) in &self.nodes {
            let is_input = sources.contains(name.as_str()) && !targets.contains(name.as_str());
            let is_output = targets.contains(name.as_str()) && !sources.contains(name.as_str());

            let esc_name = mermaid_escape(name);
            let esc_urn = mermaid_escape(media_urn);

            if is_input {
                out.push_str(&format!("    {}([\"{}<br/><small>{}</small>\"])\n", name, esc_name, esc_urn));
            } else if is_output {
                out.push_str(&format!("    {}(((\"{}<br/><small>{}</small>\")))\n", name, esc_name, esc_urn));
            } else {
                out.push_str(&format!("    {}[\"{}<br/><small>{}</small>\"]\n", name, esc_name, esc_urn));
            }
        }

        out.push('\n');

        let mut seen_edges: HashSet<(String, String, String)> = HashSet::new();
        for edge in &self.edges {
            let key = (edge.from.clone(), edge.to.clone(), edge.cap_urn.clone());
            if !seen_edges.insert(key) {
                continue;
            }
            let title = mermaid_escape(&edge.cap.title);
            let urn = mermaid_escape(&edge.cap_urn);
            out.push_str(&format!("    {} -->|\"{}<br/><small>{}</small>\"| {}\n", edge.from, title, urn, edge.to));
        }

        out
    }
}

fn mermaid_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "#quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// =============================================================================
// Cap Registry Trait
// =============================================================================

/// Trait for Cap registry abstraction
///
/// This allows dependency injection and testing without network access
#[async_trait::async_trait]
pub trait CapRegistryTrait: Send + Sync {
    /// Look up a cap by URN
    async fn lookup(&self, urn: &str) -> Result<Cap, ParseOrchestrationError>;
}

/// Implementation for capdag::CapRegistry
#[async_trait::async_trait]
impl CapRegistryTrait for crate::CapRegistry {
    async fn lookup(&self, urn: &str) -> Result<Cap, ParseOrchestrationError> {
        self.get_cap(urn)
            .await
            .map_err(|_e| ParseOrchestrationError::CapNotFound {
                cap_urn: urn.to_string(),
            })
    }
}
