//! Futile's display list as menus keep it: nodes live while their objects hold them,
//! containers draw their children in the order they were added (`FContainer.AddChild`
//! appends), and an object changes its sprites in place every frame.

use rw_render::StageBatch;

use crate::atlas::AtlasManager;
use crate::geometry::Matrix;
use crate::label::Label;
use crate::stage::{NodeTransform, Sprite};

/// A node of a [`StageTree`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(usize);

#[derive(Debug, Clone)]
enum TreeItem {
    Container(NodeTransform),
    Sprite(Sprite),
    Label(Label),
}

#[derive(Debug, Clone)]
struct TreeNode {
    item: TreeItem,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
}

/// The stage and everything added to it.
#[derive(Debug, Clone)]
pub struct StageTree {
    nodes: Vec<TreeNode>,
}

impl Default for StageTree {
    fn default() -> Self {
        Self::new()
    }
}

impl StageTree {
    /// `Futile.stage`.
    pub const ROOT: NodeId = NodeId(0);

    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: vec![TreeNode {
                item: TreeItem::Container(NodeTransform::default()),
                parent: None,
                children: Vec::new(),
            }],
        }
    }

    fn add(&mut self, parent: NodeId, item: TreeItem) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(TreeNode {
            item,
            parent: Some(parent),
            children: Vec::new(),
        });
        self.nodes[parent.0].children.push(id);
        id
    }

    /// `new FContainer()` added to `parent`.
    pub fn add_container(&mut self, parent: NodeId) -> NodeId {
        self.add(parent, TreeItem::Container(NodeTransform::default()))
    }

    /// A new container not yet on the stage (`new FContainer()`).
    pub fn new_container(&mut self) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(TreeNode {
            item: TreeItem::Container(NodeTransform::default()),
            parent: None,
            children: Vec::new(),
        });
        id
    }

    pub fn add_sprite(&mut self, parent: NodeId, sprite: Sprite) -> NodeId {
        self.add(parent, TreeItem::Sprite(sprite))
    }

    pub fn add_label(&mut self, parent: NodeId, label: Label) -> NodeId {
        self.add(parent, TreeItem::Label(label))
    }

    /// `AddChild` of a node that is not in a container, or moves it to the top of
    /// `parent`.
    pub fn add_child(&mut self, parent: NodeId, child: NodeId) {
        self.remove_from_container(child);
        self.nodes[parent.0].children.push(child);
        self.nodes[child.0].parent = Some(parent);
    }

    /// `RemoveFromContainer`: the node and its children are no longer drawn.
    pub fn remove_from_container(&mut self, id: NodeId) {
        if let Some(parent) = self.nodes[id.0].parent.take() {
            self.nodes[parent.0].children.retain(|c| *c != id);
        }
    }

    /// The sprite of a node made by [`add_sprite`](Self::add_sprite).
    ///
    /// # Panics
    /// When the node is not a sprite.
    pub fn sprite_mut(&mut self, id: NodeId) -> &mut Sprite {
        match &mut self.nodes[id.0].item {
            TreeItem::Sprite(s) => s,
            _ => panic!("stage node {id:?} is not a sprite"),
        }
    }

    /// # Panics
    /// When the node is not a sprite.
    #[must_use]
    pub fn sprite(&self, id: NodeId) -> &Sprite {
        match &self.nodes[id.0].item {
            TreeItem::Sprite(s) => s,
            _ => panic!("stage node {id:?} is not a sprite"),
        }
    }

    /// # Panics
    /// When the node is not a label.
    pub fn label_mut(&mut self, id: NodeId) -> &mut Label {
        match &mut self.nodes[id.0].item {
            TreeItem::Label(l) => l,
            _ => panic!("stage node {id:?} is not a label"),
        }
    }

    /// # Panics
    /// When the node is not a label.
    #[must_use]
    pub fn label(&self, id: NodeId) -> &Label {
        match &self.nodes[id.0].item {
            TreeItem::Label(l) => l,
            _ => panic!("stage node {id:?} is not a label"),
        }
    }

    /// The transform of any node.
    pub fn transform_mut(&mut self, id: NodeId) -> &mut NodeTransform {
        match &mut self.nodes[id.0].item {
            TreeItem::Container(t) => t,
            TreeItem::Sprite(s) => &mut s.transform,
            TreeItem::Label(l) => &mut l.transform,
        }
    }

    /// Moves a node to `(x, y)`.
    pub fn set_xy(&mut self, id: NodeId, x: f32, y: f32) {
        let tr = self.transform_mut(id);
        tr.x = x;
        tr.y = y;
    }

    /// Appends the stage's quads, in draw order, to `batch`.
    pub fn push_quads(&self, atlases: &AtlasManager, batch: &mut StageBatch) {
        self.push_node(Self::ROOT, atlases, batch, &Matrix::IDENTITY, 1.0);
    }

    fn push_node(
        &self,
        id: NodeId,
        atlases: &AtlasManager,
        batch: &mut StageBatch,
        parent: &Matrix,
        parent_alpha: f32,
    ) {
        let node = &self.nodes[id.0];
        let transform = match &node.item {
            TreeItem::Container(t) => t,
            TreeItem::Sprite(s) => &s.transform,
            TreeItem::Label(l) => &l.transform,
        };
        if !transform.visible {
            return;
        }
        let (matrix, alpha) = transform.concat(parent, parent_alpha);
        match &node.item {
            TreeItem::Sprite(s) => s.push_quad(atlases, batch, &matrix, alpha),
            TreeItem::Label(l) => l.push_quads(batch, &matrix, alpha),
            TreeItem::Container(_) => {
                for &child in &node.children {
                    self.push_node(child, atlases, batch, &matrix, alpha);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn children_draw_in_the_order_added_and_removed_nodes_vanish() {
        let mut t = StageTree::new();
        let c = t.add_container(StageTree::ROOT);
        let a = t.add_sprite(c, Sprite::new("a"));
        let later = t.add_sprite(StageTree::ROOT, Sprite::new("later"));
        let b = t.add_sprite(c, Sprite::new("b"));
        assert_eq!(t.nodes[c.0].children, [a, b]);
        assert_eq!(t.nodes[0].children, [c, later]);
        t.remove_from_container(a);
        assert_eq!(t.nodes[c.0].children, [b]);
        t.add_child(c, a);
        assert_eq!(t.nodes[c.0].children, [b, a]);
    }
}
