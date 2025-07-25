use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use crate::gui::symbols;

use crate::agent::conversation::types::{Conversation, ConversationBranch, ConversationCheckpoint, BranchStatus};
use crate::agent::message::types::AgentMessage;

/// Visual conversation tree component for displaying conversation flow
pub struct ConversationTree {
    /// Tree configuration
    config: TreeConfig,
    
    /// Current conversation being displayed
    conversation_id: Option<Uuid>,
    
    /// Selected node in the tree
    selected_node: Option<NodeId>,
    
    /// Expanded nodes
    expanded_nodes: std::collections::HashSet<NodeId>,
    
    /// Node positions for layout
    node_positions: HashMap<NodeId, Position>,
    
    /// Tree layout cache
    layout_cache: Option<TreeLayout>,
}

/// Configuration for the conversation tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeConfig {
    /// Whether to show message content in nodes
    pub show_message_content: bool,
    
    /// Maximum characters to show in node labels
    pub max_label_length: usize,
    
    /// Whether to show timestamps
    pub show_timestamps: bool,
    
    /// Whether to show branch success scores
    pub show_success_scores: bool,
    
    /// Whether to show checkpoints
    pub show_checkpoints: bool,
    
    /// Node spacing configuration
    pub spacing: SpacingConfig,
    
    /// Visual style configuration
    pub style: StyleConfig,
    
    /// Animation settings
    pub animation: AnimationConfig,
}

/// Spacing configuration for tree layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacingConfig {
    /// Horizontal spacing between nodes
    pub horizontal_spacing: f32,
    
    /// Vertical spacing between levels
    pub vertical_spacing: f32,
    
    /// Spacing between branches
    pub branch_spacing: f32,
    
    /// Minimum node width
    pub min_node_width: f32,
    
    /// Minimum node height
    pub min_node_height: f32,
}

/// Visual style configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleConfig {
    /// Color scheme for different node types
    pub node_colors: NodeColorScheme,
    
    /// Line styles for connections
    pub connection_styles: ConnectionStyles,
    
    /// Font settings
    pub font: FontConfig,
    
    /// Border and shadow settings
    pub borders: BorderConfig,
}

/// Color scheme for different node types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeColorScheme {
    /// Color for main conversation messages
    pub main_message: String,
    
    /// Color for branch messages
    pub branch_message: String,
    
    /// Color for checkpoints
    pub checkpoint: String,
    
    /// Color for successful branches
    pub successful_branch: String,
    
    /// Color for failed branches
    pub failed_branch: String,
    
    /// Color for active branches
    pub active_branch: String,
    
    /// Color for selected nodes
    pub selected: String,
    
    /// Color for highlighted nodes
    pub highlighted: String,
}

/// Connection line styles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStyles {
    /// Main conversation flow line style
    pub main_flow: LineStyle,
    
    /// Branch connection line style
    pub branch_connection: LineStyle,
    
    /// Checkpoint connection line style
    pub checkpoint_connection: LineStyle,
    
    /// Merge connection line style
    pub merge_connection: LineStyle,
}

/// Line style configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineStyle {
    /// Line color
    pub color: String,
    
    /// Line width
    pub width: f32,
    
    /// Line pattern (solid, dashed, dotted)
    pub pattern: LinePattern,
    
    /// Arrow style for directional lines
    pub arrow: Option<ArrowStyle>,
}

/// Line pattern types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LinePattern {
    Solid,
    Dashed,
    Dotted,
    DashDot,
}

/// Arrow style configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrowStyle {
    /// Arrow size
    pub size: f32,
    
    /// Arrow color (if different from line)
    pub color: Option<String>,
    
    /// Arrow shape
    pub shape: ArrowShape,
}

/// Arrow shape types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ArrowShape {
    Triangle,
    Circle,
    Diamond,
    Square,
}

/// Font configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontConfig {
    /// Font family
    pub family: String,
    
    /// Font size for node labels
    pub size: f32,
    
    /// Font weight
    pub weight: FontWeight,
    
    /// Text color
    pub color: String,
}

/// Font weight options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FontWeight {
    Normal,
    Bold,
    Light,
}

/// Border configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorderConfig {
    /// Border width
    pub width: f32,
    
    /// Border color
    pub color: String,
    
    /// Border radius for rounded corners
    pub radius: f32,
    
    /// Shadow configuration
    pub shadow: Option<ShadowConfig>,
}

/// Shadow configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowConfig {
    /// Shadow color
    pub color: String,
    
    /// Shadow offset X
    pub offset_x: f32,
    
    /// Shadow offset Y
    pub offset_y: f32,
    
    /// Shadow blur radius
    pub blur: f32,
}

/// Animation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationConfig {
    /// Whether animations are enabled
    pub enabled: bool,
    
    /// Duration for node transitions (milliseconds)
    pub transition_duration: u32,
    
    /// Animation easing function
    pub easing: EasingFunction,
    
    /// Whether to animate layout changes
    pub animate_layout: bool,
}

/// Animation easing functions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EasingFunction {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Bounce,
}

impl Default for TreeConfig {
    fn default() -> Self {
        Self {
            show_message_content: true,
            max_label_length: 100,
            show_timestamps: true,
            show_success_scores: true,
            show_checkpoints: true,
            spacing: SpacingConfig {
                horizontal_spacing: 150.0,
                vertical_spacing: 80.0,
                branch_spacing: 60.0,
                min_node_width: 120.0,
                min_node_height: 40.0,
            },
            style: StyleConfig {
                node_colors: NodeColorScheme {
                    main_message: "#4A90E2".to_string(),
                    branch_message: "#7ED321".to_string(),
                    checkpoint: "#F5A623".to_string(),
                    successful_branch: "#50E3C2".to_string(),
                    failed_branch: "#D0021B".to_string(),
                    active_branch: "#9013FE".to_string(),
                    selected: "#FF6B35".to_string(),
                    highlighted: "#FFE066".to_string(),
                },
                connection_styles: ConnectionStyles {
                    main_flow: LineStyle {
                        color: "#4A90E2".to_string(),
                        width: 2.0,
                        pattern: LinePattern::Solid,
                        arrow: Some(ArrowStyle {
                            size: 8.0,
                            color: None,
                            shape: ArrowShape::Triangle,
                        }),
                    },
                    branch_connection: LineStyle {
                        color: "#7ED321".to_string(),
                        width: 1.5,
                        pattern: LinePattern::Dashed,
                        arrow: Some(ArrowStyle {
                            size: 6.0,
                            color: None,
                            shape: ArrowShape::Triangle,
                        }),
                    },
                    checkpoint_connection: LineStyle {
                        color: "#F5A623".to_string(),
                        width: 1.0,
                        pattern: LinePattern::Dotted,
                        arrow: None,
                    },
                    merge_connection: LineStyle {
                        color: "#9013FE".to_string(),
                        width: 2.0,
                        pattern: LinePattern::DashDot,
                        arrow: Some(ArrowStyle {
                            size: 8.0,
                            color: None,
                            shape: ArrowShape::Diamond,
                        }),
                    },
                },
                font: FontConfig {
                    family: "Inter".to_string(),
                    size: 12.0,
                    weight: FontWeight::Normal,
                    color: "#333333".to_string(),
                },
                borders: BorderConfig {
                    width: 1.0,
                    color: "#CCCCCC".to_string(),
                    radius: 4.0,
                    shadow: Some(ShadowConfig {
                        color: "#00000020".to_string(),
                        offset_x: 2.0,
                        offset_y: 2.0,
                        blur: 4.0,
                    }),
                },
            },
            animation: AnimationConfig {
                enabled: true,
                transition_duration: 300,
                easing: EasingFunction::EaseInOut,
                animate_layout: true,
            },
        }
    }
}

/// Node identifier in the tree
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeId {
    /// Main conversation message
    Message(Uuid),
    
    /// Branch message
    BranchMessage(Uuid, Uuid), // (branch_id, message_id)
    
    /// Checkpoint
    Checkpoint(Uuid),
    
    /// Branch root
    BranchRoot(Uuid),
}

/// Position in 2D space
#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

/// Tree layout information
#[derive(Debug, Clone)]
pub struct TreeLayout {
    /// All nodes in the tree
    pub nodes: Vec<TreeNode>,
    
    /// Connections between nodes
    pub connections: Vec<TreeConnection>,
    
    /// Tree bounds
    pub bounds: Bounds,
    
    /// Layout generation timestamp
    pub generated_at: DateTime<Utc>,
}

/// A node in the conversation tree
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// Node identifier
    pub id: NodeId,
    
    /// Node position
    pub position: Position,
    
    /// Node dimensions
    pub size: Size,
    
    /// Node display information
    pub display: NodeDisplay,
    
    /// Node metadata
    pub metadata: NodeMetadata,
    
    /// Whether the node is selected
    pub selected: bool,
    
    /// Whether the node is highlighted
    pub highlighted: bool,
    
    /// Whether the node is expanded (for nodes with children)
    pub expanded: bool,
}

/// Node size
#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

/// Node display information
#[derive(Debug, Clone)]
pub struct NodeDisplay {
    /// Primary label text
    pub label: String,
    
    /// Secondary text (timestamp, etc.)
    pub subtitle: Option<String>,
    
    /// Node type for styling
    pub node_type: NodeType,
    
    /// Visual indicators
    pub indicators: Vec<NodeIndicator>,
    
    /// Custom styling overrides
    pub style_overrides: Option<NodeStyleOverrides>,
}

/// Types of nodes in the tree
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeType {
    UserMessage,
    AssistantMessage,
    SystemMessage,
    BranchPoint,
    Checkpoint,
    MergePoint,
}

/// Visual indicators for nodes
#[derive(Debug, Clone)]
pub struct NodeIndicator {
    /// Indicator type
    pub indicator_type: IndicatorType,
    
    /// Display text or icon
    pub display: String,
    
    /// Position relative to node
    pub position: IndicatorPosition,
    
    /// Tooltip text
    pub tooltip: Option<String>,
}

/// Types of node indicators
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndicatorType {
    Success,
    Warning,
    Error,
    Info,
    Branch,
    Checkpoint,
    Merge,
    Tool,
    Code,
    File,
}

/// Position of indicators relative to nodes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndicatorPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

/// Style overrides for individual nodes
#[derive(Debug, Clone)]
pub struct NodeStyleOverrides {
    /// Background color override
    pub background_color: Option<String>,
    
    /// Border color override
    pub border_color: Option<String>,
    
    /// Text color override
    pub text_color: Option<String>,
    
    /// Font weight override
    pub font_weight: Option<FontWeight>,
}

/// Node metadata
#[derive(Debug, Clone)]
pub struct NodeMetadata {
    /// When this node was created
    pub created_at: DateTime<Utc>,
    
    /// Node depth in the tree
    pub depth: usize,
    
    /// Number of child nodes
    pub child_count: usize,
    
    /// Success score (if applicable)
    pub success_score: Option<f32>,
    
    /// Associated message or checkpoint data
    pub data: NodeData,
}

/// Data associated with tree nodes
#[derive(Debug, Clone)]
pub enum NodeData {
    Message(AgentMessage),
    Branch(ConversationBranch),
    Checkpoint(ConversationCheckpoint),
}

/// Connection between tree nodes
#[derive(Debug, Clone)]
pub struct TreeConnection {
    /// Source node
    pub from: NodeId,
    
    /// Target node
    pub to: NodeId,
    
    /// Connection type
    pub connection_type: ConnectionType,
    
    /// Connection path points
    pub path: Vec<Position>,
    
    /// Connection style
    pub style: LineStyle,
    
    /// Connection metadata
    pub metadata: ConnectionMetadata,
}

/// Types of connections between nodes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionType {
    /// Sequential message flow
    Sequential,
    
    /// Branch divergence
    Branch,
    
    /// Branch merge
    Merge,
    
    /// Checkpoint reference
    Checkpoint,
    
    /// Jump/restoration
    Jump,
}

/// Connection metadata
#[derive(Debug, Clone)]
pub struct ConnectionMetadata {
    /// Connection strength or weight
    pub weight: f32,
    
    /// Whether this connection is active
    pub active: bool,
    
    /// Transition probability (for branch connections)
    pub probability: Option<f32>,
}

/// Tree bounds
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

/// Actions that can be triggered from the conversation tree
#[derive(Debug, Clone)]
pub enum TreeAction {
    /// Select a node
    SelectNode(NodeId),
    
    /// Jump to a message
    JumpToMessage(Uuid),
    
    /// Create a branch from a message
    CreateBranch(Uuid),
    
    /// Create a checkpoint at a message
    CreateCheckpoint(Uuid),
    
    /// Restore from a checkpoint
    RestoreCheckpoint(Uuid),
    
    /// Delete a checkpoint
    DeleteCheckpoint(Uuid),
    
    /// Show checkpoint details
    ShowCheckpointDetails(Uuid),
    
    /// Expand/collapse a node
    ToggleExpansion(NodeId),
}

impl ConversationTree {
    /// Create a new conversation tree
    pub fn new(config: TreeConfig) -> Self {
        Self {
            config,
            conversation_id: None,
            selected_node: None,
            expanded_nodes: std::collections::HashSet::new(),
            node_positions: HashMap::new(),
            layout_cache: None,
        }
    }
    
    /// Create tree with default configuration
    pub fn with_default_config() -> Self {
        Self::new(TreeConfig::default())
    }
    
    /// Load a conversation into the tree
    pub fn load_conversation(&mut self, conversation: &Conversation) -> Result<()> {
        self.conversation_id = Some(conversation.id);
        self.layout_cache = None; // Invalidate cache
        self.generate_layout(conversation)?;
        Ok(())
    }
    
    /// Generate tree layout from conversation
    fn generate_layout(&mut self, conversation: &Conversation) -> Result<TreeLayout> {
        let mut nodes = Vec::new();
        let mut connections = Vec::new();
        let mut current_y = 0.0;
        let mut current_x = 0.0;
        
        // Generate main conversation flow
        let mut prev_node_id = None;
        for (i, message) in conversation.messages.iter().enumerate() {
            let node_id = NodeId::Message(message.id);
            let position = Position {
                x: current_x,
                y: current_y,
            };
            
            let node = self.create_message_node(message, position, i)?;
            nodes.push(node);
            
            // Create connection from previous message
            if let Some(prev_id) = prev_node_id {
                connections.push(TreeConnection {
                    from: prev_id,
                    to: node_id.clone(),
                    connection_type: ConnectionType::Sequential,
                    path: vec![
                        Position { x: current_x - self.config.spacing.horizontal_spacing, y: current_y },
                        position,
                    ],
                    style: self.config.style.connection_styles.main_flow.clone(),
                    metadata: ConnectionMetadata {
                        weight: 1.0,
                        active: true,
                        probability: None,
                    },
                });
            }
            
            prev_node_id = Some(node_id);
            current_x += self.config.spacing.horizontal_spacing;
        }
        
        // Generate branches
        for branch in &conversation.branches {
            let branch_nodes = self.generate_branch_layout(branch, &mut current_y)?;
            nodes.extend(branch_nodes.0);
            connections.extend(branch_nodes.1);
        }
        
        // Generate checkpoints
        for checkpoint in &conversation.checkpoints {
            let checkpoint_node = self.create_checkpoint_node(checkpoint, current_y)?;
            nodes.push(checkpoint_node);
            current_y += self.config.spacing.vertical_spacing;
        }
        
        // Calculate bounds
        let bounds = self.calculate_bounds(&nodes);
        
        let layout = TreeLayout {
            nodes,
            connections,
            bounds,
            generated_at: Utc::now(),
        };
        
        self.layout_cache = Some(layout.clone());
        Ok(layout)
    }
    
    /// Generate layout for a conversation branch
    fn generate_branch_layout(
        &self,
        branch: &ConversationBranch,
        current_y: &mut f32,
    ) -> Result<(Vec<TreeNode>, Vec<TreeConnection>)> {
        let mut nodes = Vec::new();
        let mut connections = Vec::new();
        
        *current_y += self.config.spacing.branch_spacing;
        let branch_start_y = *current_y;
        let mut branch_x = 0.0;
        
        // Create branch root node
        let branch_root_id = NodeId::BranchRoot(branch.id);
        let branch_root_position = Position {
            x: branch_x,
            y: branch_start_y,
        };
        
        let branch_root_node = self.create_branch_root_node(branch, branch_root_position)?;
        nodes.push(branch_root_node);
        
        // Create branch messages
        let mut prev_node_id = Some(branch_root_id);
        for message in &branch.messages {
            branch_x += self.config.spacing.horizontal_spacing;
            let node_id = NodeId::BranchMessage(branch.id, message.id);
            let position = Position {
                x: branch_x,
                y: branch_start_y,
            };
            
            let node = self.create_branch_message_node(message, position, branch)?;
            nodes.push(node);
            
            // Create connection from previous node
            if let Some(prev_id) = prev_node_id {
                connections.push(TreeConnection {
                    from: prev_id,
                    to: node_id.clone(),
                    connection_type: ConnectionType::Sequential,
                    path: vec![
                        Position { x: branch_x - self.config.spacing.horizontal_spacing, y: branch_start_y },
                        position,
                    ],
                    style: self.config.style.connection_styles.branch_connection.clone(),
                    metadata: ConnectionMetadata {
                        weight: 0.8,
                        active: branch.status == BranchStatus::Active,
                        probability: None,
                    },
                });
            }
            
            prev_node_id = Some(node_id);
        }
        
        *current_y += self.config.spacing.vertical_spacing;
        
        Ok((nodes, connections))
    }
    
    /// Create a message node
    fn create_message_node(
        &self,
        message: &AgentMessage,
        position: Position,
        index: usize,
    ) -> Result<TreeNode> {
        let label = if self.config.show_message_content {
            let content = &message.content;
            if content.len() > self.config.max_label_length {
                format!("{}...", &content[..self.config.max_label_length - 3])
            } else {
                content.clone()
            }
        } else {
            format!("Message {}", index + 1)
        };
        
        let subtitle = if self.config.show_timestamps {
            Some(message.timestamp.format("%H:%M:%S").to_string())
        } else {
            None
        };
        
        let node_type = match message.role {
            crate::llm::client::Role::User => NodeType::UserMessage,
            crate::llm::client::Role::Assistant => NodeType::AssistantMessage,
            crate::llm::client::Role::System => NodeType::SystemMessage,
            crate::llm::client::Role::Function => NodeType::SystemMessage,
        };
        
        let mut indicators = Vec::new();
        
        // Add tool call indicators
        if !message.tool_calls.is_empty() {
            indicators.push(NodeIndicator {
                indicator_type: IndicatorType::Tool,
                display: "🔧".to_string(),
                position: IndicatorPosition::TopRight,
                tooltip: Some(format!("{} tool calls", message.tool_calls.len())),
            });
        }
        
        let size = Size {
            width: self.config.spacing.min_node_width,
            height: self.config.spacing.min_node_height,
        };
        
        Ok(TreeNode {
            id: NodeId::Message(message.id),
            position,
            size,
            display: NodeDisplay {
                label,
                subtitle,
                node_type,
                indicators,
                style_overrides: None,
            },
            metadata: NodeMetadata {
                created_at: message.timestamp,
                depth: index,
                child_count: 0,
                success_score: None,
                data: NodeData::Message(message.clone()),
            },
            selected: self.selected_node == Some(NodeId::Message(message.id)),
            highlighted: false,
            expanded: true,
        })
    }
    
    /// Create a branch root node
    fn create_branch_root_node(
        &self,
        branch: &ConversationBranch,
        position: Position,
    ) -> Result<TreeNode> {
        let label = branch.title.clone();
        let subtitle = if self.config.show_success_scores {
            branch.success_score.map(|score| format!("Success: {:.1}%", score * 100.0))
        } else {
            None
        };
        
        let mut indicators = Vec::new();
        
        // Add status indicator
        match branch.status {
            BranchStatus::Successful => {
                indicators.push(NodeIndicator {
                    indicator_type: IndicatorType::Success,
                    display: "✓".to_string(),
                    position: IndicatorPosition::TopRight,
                    tooltip: Some("Successful branch".to_string()),
                });
            }
            BranchStatus::Failed => {
                indicators.push(NodeIndicator {
                    indicator_type: IndicatorType::Error,
                    display: symbols::get_error_symbol().to_string(),
                    position: IndicatorPosition::TopRight,
                    tooltip: Some("Failed branch".to_string()),
                });
            }
            BranchStatus::Active => {
                indicators.push(NodeIndicator {
                    indicator_type: IndicatorType::Info,
                    display: "⚡".to_string(),
                    position: IndicatorPosition::TopRight,
                    tooltip: Some("Active branch".to_string()),
                });
            }
            _ => {}
        }
        
        let size = Size {
            width: self.config.spacing.min_node_width * 1.2,
            height: self.config.spacing.min_node_height,
        };
        
        Ok(TreeNode {
            id: NodeId::BranchRoot(branch.id),
            position,
            size,
            display: NodeDisplay {
                label,
                subtitle,
                node_type: NodeType::BranchPoint,
                indicators,
                style_overrides: None,
            },
            metadata: NodeMetadata {
                created_at: branch.created_at,
                depth: 0,
                child_count: branch.messages.len(),
                success_score: branch.success_score,
                data: NodeData::Branch(branch.clone()),
            },
            selected: self.selected_node == Some(NodeId::BranchRoot(branch.id)),
            highlighted: false,
            expanded: self.expanded_nodes.contains(&NodeId::BranchRoot(branch.id)),
        })
    }
    
    /// Create a branch message node
    fn create_branch_message_node(
        &self,
        message: &AgentMessage,
        position: Position,
        branch: &ConversationBranch,
    ) -> Result<TreeNode> {
        let label = if self.config.show_message_content {
            let content = &message.content;
            if content.len() > self.config.max_label_length {
                format!("{}...", &content[..self.config.max_label_length - 3])
            } else {
                content.clone()
            }
        } else {
            "Branch Message".to_string()
        };
        
        let subtitle = if self.config.show_timestamps {
            Some(message.timestamp.format("%H:%M:%S").to_string())
        } else {
            None
        };
        
        let node_type = match message.role {
            crate::llm::client::Role::User => NodeType::UserMessage,
            crate::llm::client::Role::Assistant => NodeType::AssistantMessage,
            crate::llm::client::Role::System => NodeType::SystemMessage,
            crate::llm::client::Role::Function => NodeType::SystemMessage,
        };
        
        let size = Size {
            width: self.config.spacing.min_node_width * 0.9,
            height: self.config.spacing.min_node_height * 0.9,
        };
        
        Ok(TreeNode {
            id: NodeId::BranchMessage(branch.id, message.id),
            position,
            size,
            display: NodeDisplay {
                label,
                subtitle,
                node_type,
                indicators: Vec::new(),
                style_overrides: None,
            },
            metadata: NodeMetadata {
                created_at: message.timestamp,
                depth: 1,
                child_count: 0,
                success_score: None,
                data: NodeData::Message(message.clone()),
            },
            selected: self.selected_node == Some(NodeId::BranchMessage(branch.id, message.id)),
            highlighted: false,
            expanded: true,
        })
    }
    
    /// Create a checkpoint node
    fn create_checkpoint_node(
        &self,
        checkpoint: &ConversationCheckpoint,
        y_position: f32,
    ) -> Result<TreeNode> {
        let position = Position {
            x: 0.0,
            y: y_position,
        };
        
        let label = checkpoint.title.clone();
        let subtitle = if self.config.show_timestamps {
            Some(checkpoint.created_at.format("%H:%M:%S").to_string())
        } else {
            None
        };
        
        let mut indicators = Vec::new();
        
        if checkpoint.auto_generated {
            indicators.push(NodeIndicator {
                indicator_type: IndicatorType::Info,
                display: "🤖".to_string(),
                position: IndicatorPosition::TopLeft,
                tooltip: Some("Auto-generated checkpoint".to_string()),
            });
        }
        
        let size = Size {
            width: self.config.spacing.min_node_width * 0.8,
            height: self.config.spacing.min_node_height * 0.8,
        };
        
        Ok(TreeNode {
            id: NodeId::Checkpoint(checkpoint.id),
            position,
            size,
            display: NodeDisplay {
                label,
                subtitle,
                node_type: NodeType::Checkpoint,
                indicators,
                style_overrides: None,
            },
            metadata: NodeMetadata {
                created_at: checkpoint.created_at,
                depth: 0,
                child_count: 0,
                success_score: None,
                data: NodeData::Checkpoint(checkpoint.clone()),
            },
            selected: self.selected_node == Some(NodeId::Checkpoint(checkpoint.id)),
            highlighted: false,
            expanded: true,
        })
    }
    
    /// Calculate bounds of the tree
    fn calculate_bounds(&self, nodes: &[TreeNode]) -> Bounds {
        if nodes.is_empty() {
            return Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 0.0,
                max_y: 0.0,
            };
        }
        
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        
        for node in nodes {
            min_x = min_x.min(node.position.x);
            min_y = min_y.min(node.position.y);
            max_x = max_x.max(node.position.x + node.size.width);
            max_y = max_y.max(node.position.y + node.size.height);
        }
        
        Bounds { min_x, min_y, max_x, max_y }
    }
    
    /// Get current tree layout
    pub fn get_layout(&self) -> Option<&TreeLayout> {
        self.layout_cache.as_ref()
    }
    
    /// Select a node in the tree
    pub fn select_node(&mut self, node_id: Option<NodeId>) {
        self.selected_node = node_id;
        
        // Update layout cache to reflect selection
        if let Some(ref mut layout) = self.layout_cache {
            for node in &mut layout.nodes {
                node.selected = self.selected_node == Some(node.id.clone());
            }
        }
    }
    
    /// Toggle node expansion
    pub fn toggle_node_expansion(&mut self, node_id: NodeId) {
        if self.expanded_nodes.contains(&node_id) {
            self.expanded_nodes.remove(&node_id);
        } else {
            self.expanded_nodes.insert(node_id.clone());
        }
        
        // Update layout cache
        if let Some(ref mut layout) = self.layout_cache {
            for node in &mut layout.nodes {
                if node.id == node_id {
                    node.expanded = self.expanded_nodes.contains(&node_id);
                    break;
                }
            }
        }
    }
    
    /// Highlight nodes matching a condition
    pub fn highlight_nodes<F>(&mut self, condition: F)
    where
        F: Fn(&TreeNode) -> bool,
    {
        if let Some(ref mut layout) = self.layout_cache {
            for node in &mut layout.nodes {
                node.highlighted = condition(node);
            }
        }
    }
    
    /// Clear all highlights
    pub fn clear_highlights(&mut self) {
        if let Some(ref mut layout) = self.layout_cache {
            for node in &mut layout.nodes {
                node.highlighted = false;
            }
        }
    }
    
    /// Get node at position
    pub fn get_node_at_position(&self, position: Position) -> Option<&TreeNode> {
        if let Some(ref layout) = self.layout_cache {
            for node in &layout.nodes {
                if position.x >= node.position.x
                    && position.x <= node.position.x + node.size.width
                    && position.y >= node.position.y
                    && position.y <= node.position.y + node.size.height
                {
                    return Some(node);
                }
            }
        }
        None
    }
    
    /// Update tree configuration
    pub fn update_config(&mut self, config: TreeConfig) {
        self.config = config;
        self.layout_cache = None; // Invalidate cache
    }
    
    /// Get current configuration
    pub fn get_config(&self) -> &TreeConfig {
        &self.config
    }
    
    /// Render the conversation tree with UI interactions
    pub fn render(&mut self, ui: &mut egui::Ui, theme: &crate::gui::theme::AppTheme) -> Result<Option<TreeAction>> {
        use egui::{Pos2, Rect, Vec2};
        
        let mut action = None;
        
        if let Some(ref layout) = self.layout_cache {
            // Draw connections first (behind nodes)
            for connection in &layout.connections {
                self.draw_connection(ui.painter(), connection, theme);
            }
            
            // Draw nodes and handle interactions
            for node in &layout.nodes {
                let node_rect = Rect::from_min_size(
                    Pos2::new(node.position.x, node.position.y),
                    Vec2::new(node.size.width, node.size.height),
                );
                
                // Check for mouse interactions
                let response = ui.allocate_rect(node_rect, egui::Sense::click());
                
                // Handle left click
                if response.clicked() {
                    action = Some(TreeAction::SelectNode(node.id.clone()));
                }
                
                // Handle right click for context menu
                if response.secondary_clicked() {
                    action = self.show_context_menu(ui, &node.id);
                }
                
                // Draw the node
                self.draw_node(ui.painter(), node, theme, response.hovered());
            }
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("No conversation loaded");
            });
        }
        
        Ok(action)
    }
    
    /// Show context menu for a node
    fn show_context_menu(&self, ui: &mut egui::Ui, node_id: &NodeId) -> Option<TreeAction> {
        let mut action = None;
        
        ui.menu_button("⋮", |ui| {
            match node_id {
                NodeId::Message(message_id) => {
                    if ui.button("📍 Create Checkpoint").clicked() {
                        action = Some(TreeAction::CreateCheckpoint(*message_id));
                        ui.close_menu();
                    }
                    if ui.button("🌳 Create Branch").clicked() {
                        action = Some(TreeAction::CreateBranch(*message_id));
                        ui.close_menu();
                    }
                    if ui.button("🔗 Jump to Message").clicked() {
                        action = Some(TreeAction::JumpToMessage(*message_id));
                        ui.close_menu();
                    }
                },
                NodeId::Checkpoint(checkpoint_id) => {
                    if ui.button("🔄 Restore Checkpoint").clicked() {
                        action = Some(TreeAction::RestoreCheckpoint(*checkpoint_id));
                        ui.close_menu();
                    }
                    if ui.button("ℹ️ Show Details").clicked() {
                        action = Some(TreeAction::ShowCheckpointDetails(*checkpoint_id));
                        ui.close_menu();
                    }
                    if ui.button("🗑️ Delete Checkpoint").clicked() {
                        action = Some(TreeAction::DeleteCheckpoint(*checkpoint_id));
                        ui.close_menu();
                    }
                },
                NodeId::BranchMessage(_branch_id, message_id) => {
                    if ui.button("📍 Create Checkpoint").clicked() {
                        action = Some(TreeAction::CreateCheckpoint(*message_id));
                        ui.close_menu();
                    }
                    if ui.button("🔗 Jump to Message").clicked() {
                        action = Some(TreeAction::JumpToMessage(*message_id));
                        ui.close_menu();
                    }
                },
                NodeId::BranchRoot(_branch_id) => {
                    if ui.button("🌳 Expand/Collapse").clicked() {
                        action = Some(TreeAction::ToggleExpansion(node_id.clone()));
                        ui.close_menu();
                    }
                },
            }
        });
        
        action
    }
    
    /// Draw a connection between nodes
    fn draw_connection(&self, painter: &egui::Painter, connection: &TreeConnection, theme: &crate::gui::theme::AppTheme) {
        use egui::{Color32, Pos2, Stroke};
        
        let color = match connection.connection_type {
            ConnectionType::Sequential => theme.accent_color(),
            ConnectionType::Branch => theme.success_color(),
            ConnectionType::Checkpoint => Color32::from_rgb(245, 166, 35), // Orange for checkpoints
            ConnectionType::Merge => theme.warning_color(),
            ConnectionType::Jump => theme.error_color(),
        };
        
        let stroke = Stroke::new(connection.style.width, color);
        
        if connection.path.len() >= 2 {
            for i in 0..connection.path.len() - 1 {
                let from = Pos2::new(connection.path[i].x, connection.path[i].y);
                let to = Pos2::new(connection.path[i + 1].x, connection.path[i + 1].y);
                painter.line_segment([from, to], stroke);
            }
        }
    }
    
    /// Draw a node
    fn draw_node(&self, painter: &egui::Painter, node: &TreeNode, _theme: &crate::gui::theme::AppTheme, hovered: bool) {
        use egui::{Color32, FontId, Pos2, Rect, CornerRadius, Stroke, Vec2};
        
        let rect = Rect::from_min_size(
            Pos2::new(node.position.x, node.position.y),
            Vec2::new(node.size.width, node.size.height),
        );
        
        // Determine colors based on node type and state
        let (bg_color, border_color) = match node.display.node_type {
            NodeType::UserMessage => (Color32::from_rgb(70, 130, 180), Color32::from_rgb(100, 149, 237)),
            NodeType::AssistantMessage => (Color32::from_rgb(60, 179, 113), Color32::from_rgb(144, 238, 144)),
            NodeType::SystemMessage => (Color32::from_rgb(128, 128, 128), Color32::from_rgb(169, 169, 169)),
            NodeType::BranchPoint => (Color32::from_rgb(255, 165, 0), Color32::from_rgb(255, 140, 0)),
            NodeType::Checkpoint => (Color32::from_rgb(245, 166, 35), Color32::from_rgb(255, 215, 0)),
            NodeType::MergePoint => (Color32::from_rgb(147, 112, 219), Color32::from_rgb(186, 85, 211)),
        };
        
        let final_bg_color = if node.selected {
            Color32::from_rgb(255, 107, 53) // Orange for selected
        } else if hovered {
            bg_color.gamma_multiply(1.2)
        } else {
            bg_color
        };
        
        let final_border_color = if node.highlighted {
            Color32::YELLOW
        } else {
            border_color
        };
        
        // Draw node background
        painter.rect_filled(rect, CornerRadius::same(4), final_bg_color);
        
        // Draw node border
        painter.rect_stroke(rect, CornerRadius::same(4), Stroke::new(2.0, final_border_color), egui::StrokeKind::Outside);
        
        // Draw node label
        let text_color = Color32::WHITE;
        let font_id = FontId::proportional(12.0);
        
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &node.display.label,
            font_id,
            text_color,
        );
        
        // Draw indicators
        for (_i, indicator) in node.display.indicators.iter().enumerate() {
            let indicator_pos = match indicator.position {
                IndicatorPosition::TopLeft => rect.min + Vec2::new(2.0, 2.0),
                IndicatorPosition::TopRight => rect.min + Vec2::new(rect.width() - 12.0, 2.0),
                IndicatorPosition::BottomLeft => rect.min + Vec2::new(2.0, rect.height() - 12.0),
                IndicatorPosition::BottomRight => rect.min + Vec2::new(rect.width() - 12.0, rect.height() - 12.0),
                IndicatorPosition::Center => rect.center(),
            };
            
            painter.text(
                indicator_pos,
                egui::Align2::LEFT_TOP,
                &indicator.display,
                FontId::proportional(10.0),
                Color32::WHITE,
            );
        }
        
        // Draw subtitle if present
        if let Some(ref subtitle) = node.display.subtitle {
            let subtitle_pos = rect.min + Vec2::new(4.0, rect.height() - 14.0);
            painter.text(
                subtitle_pos,
                egui::Align2::LEFT_TOP,
                subtitle,
                FontId::proportional(8.0),
                Color32::LIGHT_GRAY,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::conversation::types::Conversation;
    use crate::agent::message::types::AgentMessage;

    #[test]
    fn test_tree_creation() {
        let tree = ConversationTree::with_default_config();
        assert!(tree.conversation_id.is_none());
        assert!(tree.selected_node.is_none());
        assert!(tree.layout_cache.is_none());
    }
    
    #[test]
    fn test_node_selection() {
        let mut tree = ConversationTree::with_default_config();
        let node_id = NodeId::Message(Uuid::new_v4());
        
        tree.select_node(Some(node_id.clone()));
        assert_eq!(tree.selected_node, Some(node_id));
        
        tree.select_node(None);
        assert!(tree.selected_node.is_none());
    }
    
    #[test]
    fn test_node_expansion() {
        let mut tree = ConversationTree::with_default_config();
        let node_id = NodeId::BranchRoot(Uuid::new_v4());
        
        tree.toggle_node_expansion(node_id.clone());
        assert!(tree.expanded_nodes.contains(&node_id));
        
        tree.toggle_node_expansion(node_id.clone());
        assert!(!tree.expanded_nodes.contains(&node_id));
    }
    
    #[tokio::test]
    async fn test_conversation_loading() {
        let mut tree = ConversationTree::with_default_config();
        let mut conversation = Conversation::new("Test Conversation".to_string(), None);
        
        // Add some messages
        conversation.add_message(AgentMessage::user("Hello"));
        conversation.add_message(AgentMessage::assistant("Hi there!"));
        
        let result = tree.load_conversation(&conversation);
        assert!(result.is_ok());
        assert_eq!(tree.conversation_id, Some(conversation.id));
        assert!(tree.layout_cache.is_some());
    }
    
    #[test]
    fn test_bounds_calculation() {
        let tree = ConversationTree::with_default_config();
        
        let nodes = vec![
            TreeNode {
                id: NodeId::Message(Uuid::new_v4()),
                position: Position { x: 10.0, y: 20.0 },
                size: Size { width: 100.0, height: 50.0 },
                display: NodeDisplay {
                    label: "Test".to_string(),
                    subtitle: None,
                    node_type: NodeType::UserMessage,
                    indicators: Vec::new(),
                    style_overrides: None,
                },
                metadata: NodeMetadata {
                    created_at: Utc::now(),
                    depth: 0,
                    child_count: 0,
                    success_score: None,
                    data: NodeData::Message(AgentMessage::user("test")),
                },
                selected: false,
                highlighted: false,
                expanded: true,
            },
        ];
        
        let bounds = tree.calculate_bounds(&nodes);
        assert_eq!(bounds.min_x, 10.0);
        assert_eq!(bounds.min_y, 20.0);
        assert_eq!(bounds.max_x, 110.0);
        assert_eq!(bounds.max_y, 70.0);
    }
} 