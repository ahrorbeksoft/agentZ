//! A tab's tree of split panes, ported from herdr's `layout.rs` (`TileLayout`).
//!
//! The tree is shared state: the server keeps it, and clients render it. Focus, and the focus
//! history closing a pane uses, belong to each client. Pane ids come from the caller rather
//! than a global counter, since the server hands them out per workspace and saves them.
//! Geometry works in whole units (pixels in the app), as herdr's works in cells.

use std::cmp::Reverse;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PaneId(pub u64);

/// How a split divides its area: `Horizontal` puts its children side by side (split right),
/// `Vertical` stacks them (split down).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// A pane's position and focus after layout.
#[derive(Debug, Clone, PartialEq)]
pub struct PaneInfo {
    pub id: PaneId,
    pub rect: Rect,
    pub is_focused: bool,
}

/// A split's divider, for resizing with the mouse.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitBorder {
    /// Where the divider is: x for a horizontal split, y for a vertical one.
    pub pos: u16,
    pub direction: Direction,
    /// The first child's share.
    pub ratio: f32,
    /// The split's whole area.
    pub area: Rect,
    /// From the root to this split (false = first child, true = second).
    pub path: Vec<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Node {
    Pane(PaneId),
    Split {
        direction: Direction,
        ratio: f32,
        first: Box<Node>,
        second: Box<Node>,
    },
}

/// A tree of splits and the focused pane.
#[derive(Debug, Clone, PartialEq)]
pub struct TileLayout {
    root: Node,
    focus: PaneId,
    /// The pane focused before `focus`, which `close_focused` returns to. Only a real focus
    /// move writes it; tree edits go through the primitives that take a target (`split_pane`,
    /// `close_pane`, unfocused `insert_pane_near`), so they never disturb it.
    prev_focus: Option<PaneId>,
}

impl TileLayout {
    /// A layout with one pane.
    pub fn new(root_id: PaneId) -> Self {
        Self {
            root: Node::Pane(root_id),
            focus: root_id,
            prev_focus: None,
        }
    }

    /// A layout for a saved tree, focused on `focus` when it's there and on the first pane
    /// otherwise.
    pub fn from_saved(root: Node, focus: PaneId) -> Self {
        let mut ids = Vec::new();
        collect_ids(&root, &mut ids);
        let focus = if ids.contains(&focus) {
            focus
        } else {
            ids.first().copied().unwrap_or(focus)
        };
        Self {
            root,
            focus,
            prev_focus: None,
        }
    }

    /// Takes a newer tree, as the server sends it, keeping focus and its history where the
    /// panes still exist.
    pub fn set_root(&mut self, root: Node) {
        let mut ids = Vec::new();
        collect_ids(&root, &mut ids);
        if self.prev_focus.is_some_and(|prev| !ids.contains(&prev)) {
            self.prev_focus = None;
        }
        if !ids.contains(&self.focus) {
            let old_ids = self.pane_ids();
            let next = old_ids
                .iter()
                .position(|id| *id == self.focus)
                .and_then(|position| {
                    old_ids[position + 1..]
                        .iter()
                        .chain(old_ids[..position].iter().rev())
                        .find(|id| ids.contains(id))
                        .copied()
                });
            self.focus = match self.prev_focus.take() {
                Some(prev) => prev,
                None => next.or_else(|| ids.first().copied()).unwrap_or(self.focus),
            };
        }
        self.root = root;
    }

    /// Moves focus, remembering the pane being left.
    fn set_focus(&mut self, id: PaneId) {
        if id != self.focus {
            self.prev_focus = Some(self.focus);
            self.focus = id;
        }
    }

    pub fn focused(&self) -> PaneId {
        self.focus
    }

    pub fn pane_count(&self) -> usize {
        count_panes(&self.root)
    }

    /// Every pane's rect in `area`.
    pub fn panes(&self, area: Rect) -> Vec<PaneInfo> {
        let mut result = Vec::new();
        collect_panes(&self.root, area, self.focus, &mut result);
        result
    }

    /// Every split's divider in `area`.
    pub fn splits(&self, area: Rect) -> Vec<SplitBorder> {
        let mut result = Vec::new();
        collect_splits(&self.root, area, &mut Vec::new(), &mut result);
        result
    }

    /// Splits the focused pane and focuses the new one.
    pub fn split_focused(&mut self, direction: Direction, new_id: PaneId) -> bool {
        self.split_focused_with_ratio(direction, new_id, 0.5)
    }

    pub fn split_focused_with_ratio(
        &mut self,
        direction: Direction,
        new_id: PaneId,
        ratio: f32,
    ) -> bool {
        if !self.split_pane(self.focus, direction, new_id, ratio) {
            return false;
        }
        self.set_focus(new_id);
        true
    }

    /// Splits `target` into it and `new_id`, without moving focus. False when `target` isn't
    /// in the layout or `new_id` already is.
    pub fn split_pane(
        &mut self,
        target: PaneId,
        direction: Direction,
        new_id: PaneId,
        ratio: f32,
    ) -> bool {
        self.insert_pane_near(target, new_id, direction, ratio, false)
    }

    /// Puts `moved`, which isn't in the layout, next to `target`. Focus and its history stay
    /// unless `focus` is set.
    pub fn insert_pane_near(
        &mut self,
        target: PaneId,
        moved: PaneId,
        direction: Direction,
        ratio: f32,
        focus: bool,
    ) -> bool {
        if target == moved {
            return false;
        }
        if self.pane_ids().contains(&moved) {
            return false;
        }
        let Some(node) = find_pane_mut(&mut self.root, target) else {
            return false;
        };
        *node = split_node(target, direction, moved, valid_split_ratio(ratio));
        if focus {
            self.set_focus(moved);
        }
        true
    }

    /// Closes the focused pane, returning focus to the pane it came from when that one is
    /// still open. False for the last pane.
    pub fn close_focused(&mut self) -> bool {
        if self.pane_count() <= 1 {
            return false;
        }
        let target = self.focus;
        let ids = self.pane_ids();
        let Some(position) = ids.iter().position(|id| *id == target) else {
            return false;
        };
        let ordered = if position + 1 < ids.len() {
            ids[position + 1]
        } else {
            ids[position - 1]
        };
        let new_focus = match self.prev_focus {
            Some(prev) if prev != target && ids.contains(&prev) => prev,
            _ => ordered,
        };
        // With two panes or more, removing one always leaves a tree.
        let old = std::mem::replace(&mut self.root, Node::Pane(target));
        let Some(new_root) = remove_pane(old, target) else {
            return false;
        };
        self.root = new_root;
        self.focus = new_focus;
        self.prev_focus = None;
        true
    }

    /// Closes any pane. Focus and its history stay unless it's the focused one.
    pub fn close_pane(&mut self, id: PaneId) -> bool {
        if self.focus == id {
            return self.close_focused();
        }
        if self.pane_count() <= 1 || !self.pane_ids().contains(&id) {
            return false;
        }
        let old = std::mem::replace(&mut self.root, Node::Pane(id));
        let Some(new_root) = remove_pane(old, id) else {
            return false;
        };
        self.root = new_root;
        if self.prev_focus == Some(id) {
            self.prev_focus = None;
        }
        true
    }

    pub fn focus_pane(&mut self, id: PaneId) {
        if self.pane_ids().contains(&id) {
            self.set_focus(id);
        }
    }

    /// Swaps two panes, keeping the splits. True only when both exist and differ.
    pub fn swap_panes(&mut self, first: PaneId, second: PaneId) -> bool {
        if first == second {
            return false;
        }
        let ids = self.pane_ids();
        if !ids.contains(&first) || !ids.contains(&second) {
            return false;
        }
        swap_pane_ids(&mut self.root, first, second);
        true
    }

    /// Sets the ratio of the split at `path`.
    pub fn set_ratio_at(&mut self, path: &[bool], ratio: f32) -> bool {
        set_ratio_at(&mut self.root, path, ratio.clamp(0.1, 0.9))
    }

    pub fn ratio_at(&self, path: &[bool]) -> Option<f32> {
        get_ratio_at(&self.root, path)
    }

    /// Moves the nearest split in `nav`'s direction for the focused pane. A positive `delta`
    /// grows it.
    pub fn resize_focused(&mut self, nav: NavDirection, delta: f32, area: Rect) {
        let panes = self.panes(area);
        let Some(focused) = panes.iter().find(|pane| pane.is_focused) else {
            return;
        };
        let focused_rect = focused.rect;
        let splits = self.splits(area);

        let target_direction = match nav {
            NavDirection::Left | NavDirection::Right => Direction::Horizontal,
            NavDirection::Up | NavDirection::Down => Direction::Vertical,
        };
        let grows = matches!(nav, NavDirection::Right | NavDirection::Down);

        let best =
            nearest_resize_split(&splits, target_direction, focused_rect, nav).or_else(|| {
                nearest_resize_split(&splits, target_direction, focused_rect, opposite(nav))
            });

        if let Some(split) = best {
            let path = split.path.clone();
            let current_ratio = get_ratio_at(&self.root, &path).unwrap_or(0.5);
            let adjustment = if grows { delta } else { -delta };
            self.set_ratio_at(&path, current_ratio + adjustment);
        }
    }

    /// Like [`Self::resize_focused`] for any pane. True when a ratio changed.
    pub fn resize_pane(
        &mut self,
        pane_id: PaneId,
        nav: NavDirection,
        delta: f32,
        area: Rect,
    ) -> bool {
        if !self.pane_ids().contains(&pane_id) {
            return false;
        }
        let before = split_ratios(&self.root);
        let previous_focus = self.focus;
        self.focus = pane_id;
        self.resize_focused(nav, delta, area);
        self.focus = previous_focus;
        split_ratios(&self.root) != before
    }

    pub fn pane_ids(&self) -> Vec<PaneId> {
        let mut ids = Vec::new();
        collect_ids(&self.root, &mut ids);
        ids
    }

    pub fn root(&self) -> &Node {
        &self.root
    }

    pub fn into_root(self) -> Node {
        self.root
    }
}

/// The nearest pane in `direction` from `focused`.
pub fn find_in_direction(
    focused: &PaneInfo,
    direction: NavDirection,
    panes: &[PaneInfo],
) -> Option<PaneId> {
    let focused_rect = focused.rect;

    panes
        .iter()
        .enumerate()
        .filter(|(_, pane)| pane.id != focused.id)
        .filter(|(_, pane)| {
            let rect = pane.rect;
            let fr = focused_rect;
            match direction {
                NavDirection::Left => {
                    rect.x + rect.width <= fr.x
                        && ranges_overlap(rect.y, rect.height, fr.y, fr.height)
                }
                NavDirection::Right => {
                    rect.x >= fr.x + fr.width
                        && ranges_overlap(rect.y, rect.height, fr.y, fr.height)
                }
                NavDirection::Up => {
                    rect.y + rect.height <= fr.y
                        && ranges_overlap(rect.x, rect.width, fr.x, fr.width)
                }
                NavDirection::Down => {
                    rect.y >= fr.y + fr.height && ranges_overlap(rect.x, rect.width, fr.x, fr.width)
                }
            }
        })
        .min_by_key(|(index, pane)| {
            let rect = pane.rect;
            let fr = focused_rect;
            let edge_distance = match direction {
                NavDirection::Left => fr.x.saturating_sub(rect.x + rect.width),
                NavDirection::Right => rect.x.saturating_sub(fr.x + fr.width),
                NavDirection::Up => fr.y.saturating_sub(rect.y + rect.height),
                NavDirection::Down => rect.y.saturating_sub(fr.y + fr.height),
            };
            let (overlap, center_distance) = match direction {
                NavDirection::Left | NavDirection::Right => (
                    range_overlap_amount(rect.y, rect.height, fr.y, fr.height),
                    range_center_distance(rect.y, rect.height, fr.y, fr.height),
                ),
                NavDirection::Up | NavDirection::Down => (
                    range_overlap_amount(rect.x, rect.width, fr.x, fr.width),
                    range_center_distance(rect.x, rect.width, fr.x, fr.width),
                ),
            };
            (edge_distance, Reverse(overlap), center_distance, *index)
        })
        .map(|(_, pane)| pane.id)
}

fn ranges_overlap(a_start: u16, a_len: u16, b_start: u16, b_len: u16) -> bool {
    a_start < b_start + b_len && a_start + a_len > b_start
}

fn split_on_requested_edge(split: &SplitBorder, focused: Rect, nav: NavDirection) -> bool {
    split_edge_distance(split, focused, nav) <= 1
}

fn split_area_overlaps_focused_pane(split: &SplitBorder, focused: Rect, nav: NavDirection) -> bool {
    match nav {
        NavDirection::Left | NavDirection::Right => {
            ranges_overlap(split.area.y, split.area.height, focused.y, focused.height)
        }
        NavDirection::Up | NavDirection::Down => {
            ranges_overlap(split.area.x, split.area.width, focused.x, focused.width)
        }
    }
}

fn nearest_resize_split(
    splits: &[SplitBorder],
    target_direction: Direction,
    focused: Rect,
    nav: NavDirection,
) -> Option<&SplitBorder> {
    splits
        .iter()
        .filter(|split| split.direction == target_direction)
        .filter(|split| split_area_overlaps_focused_pane(split, focused, nav))
        .filter(|split| split_on_requested_edge(split, focused, nav))
        .min_by_key(|split| split_edge_distance(split, focused, nav))
}

fn opposite(nav: NavDirection) -> NavDirection {
    match nav {
        NavDirection::Left => NavDirection::Right,
        NavDirection::Right => NavDirection::Left,
        NavDirection::Up => NavDirection::Down,
        NavDirection::Down => NavDirection::Up,
    }
}

fn split_edge_distance(split: &SplitBorder, focused: Rect, nav: NavDirection) -> u32 {
    let pos = i32::from(split.pos);
    match nav {
        NavDirection::Left => (pos - i32::from(focused.x)).unsigned_abs(),
        NavDirection::Right => (pos - i32::from(focused.x + focused.width)).unsigned_abs(),
        NavDirection::Up => (pos - i32::from(focused.y)).unsigned_abs(),
        NavDirection::Down => (pos - i32::from(focused.y + focused.height)).unsigned_abs(),
    }
}

fn range_overlap_amount(a_start: u16, a_len: u16, b_start: u16, b_len: u16) -> u16 {
    let a_end = a_start.saturating_add(a_len);
    let b_end = b_start.saturating_add(b_len);
    a_end.min(b_end).saturating_sub(a_start.max(b_start))
}

fn range_center_distance(a_start: u16, a_len: u16, b_start: u16, b_len: u16) -> u16 {
    let a_center = a_start.saturating_mul(2).saturating_add(a_len);
    let b_center = b_start.saturating_mul(2).saturating_add(b_len);
    a_center.abs_diff(b_center)
}

fn count_panes(node: &Node) -> usize {
    match node {
        Node::Pane(_) => 1,
        Node::Split { first, second, .. } => count_panes(first) + count_panes(second),
    }
}

fn collect_panes(node: &Node, area: Rect, focus: PaneId, result: &mut Vec<PaneInfo>) {
    match node {
        Node::Pane(id) => result.push(PaneInfo {
            id: *id,
            rect: area,
            is_focused: *id == focus,
        }),
        Node::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let (a, b) = split_rect(area, *direction, *ratio);
            collect_panes(first, a, focus, result);
            collect_panes(second, b, focus, result);
        }
    }
}

fn collect_splits(node: &Node, area: Rect, path: &mut Vec<bool>, result: &mut Vec<SplitBorder>) {
    if let Node::Split {
        direction,
        ratio,
        first,
        second,
    } = node
    {
        let (a, b) = split_rect(area, *direction, *ratio);
        let pos = match direction {
            Direction::Horizontal => a.x + a.width,
            Direction::Vertical => a.y + a.height,
        };
        result.push(SplitBorder {
            pos,
            direction: *direction,
            ratio: *ratio,
            area,
            path: path.clone(),
        });
        path.push(false);
        collect_splits(first, a, path, result);
        path.pop();
        path.push(true);
        collect_splits(second, b, path, result);
        path.pop();
    }
}

fn collect_ids(node: &Node, ids: &mut Vec<PaneId>) {
    match node {
        Node::Pane(id) => ids.push(*id),
        Node::Split { first, second, .. } => {
            collect_ids(first, ids);
            collect_ids(second, ids);
        }
    }
}

fn split_ratios(node: &Node) -> Vec<(Vec<bool>, f32)> {
    fn collect(node: &Node, path: &mut Vec<bool>, out: &mut Vec<(Vec<bool>, f32)>) {
        if let Node::Split {
            ratio,
            first,
            second,
            ..
        } = node
        {
            out.push((path.clone(), *ratio));
            path.push(false);
            collect(first, path, out);
            path.pop();
            path.push(true);
            collect(second, path, out);
            path.pop();
        }
    }

    let mut out = Vec::new();
    collect(node, &mut Vec::new(), &mut out);
    out
}

fn swap_pane_ids(node: &mut Node, first: PaneId, second: PaneId) {
    match node {
        Node::Pane(id) if *id == first => *id = second,
        Node::Pane(id) if *id == second => *id = first,
        Node::Pane(_) => {}
        Node::Split {
            first: first_child,
            second: second_child,
            ..
        } => {
            swap_pane_ids(first_child, first, second);
            swap_pane_ids(second_child, first, second);
        }
    }
}

fn find_pane_mut(node: &mut Node, target: PaneId) -> Option<&mut Node> {
    match node {
        Node::Pane(id) if *id == target => Some(node),
        Node::Pane(_) => None,
        Node::Split { first, second, .. } => {
            find_pane_mut(first, target).or_else(|| find_pane_mut(second, target))
        }
    }
}

fn split_node(target: PaneId, direction: Direction, new_id: PaneId, ratio: f32) -> Node {
    Node::Split {
        direction,
        ratio,
        first: Box::new(Node::Pane(target)),
        second: Box::new(Node::Pane(new_id)),
    }
}

fn valid_split_ratio(ratio: f32) -> f32 {
    if ratio.is_finite() {
        ratio.clamp(0.1, 0.9)
    } else {
        0.5
    }
}

fn remove_pane(node: Node, target: PaneId) -> Option<Node> {
    match node {
        Node::Pane(id) if id == target => None,
        Node::Pane(_) => Some(node),
        Node::Split {
            direction,
            ratio,
            first,
            second,
        } => match (remove_pane(*first, target), remove_pane(*second, target)) {
            (None, Some(second)) => Some(second),
            (Some(first), None) => Some(first),
            (Some(first), Some(second)) => Some(Node::Split {
                direction,
                ratio,
                first: Box::new(first),
                second: Box::new(second),
            }),
            (None, None) => None,
        },
    }
}

fn set_ratio_at(node: &mut Node, path: &[bool], new_ratio: f32) -> bool {
    let Node::Split {
        ratio,
        first,
        second,
        ..
    } = node
    else {
        return false;
    };
    match path.split_first() {
        None => {
            *ratio = new_ratio;
            true
        }
        Some((true, rest)) => set_ratio_at(second, rest, new_ratio),
        Some((false, rest)) => set_ratio_at(first, rest, new_ratio),
    }
}

fn get_ratio_at(node: &Node, path: &[bool]) -> Option<f32> {
    let Node::Split {
        ratio,
        first,
        second,
        ..
    } = node
    else {
        return None;
    };
    match path.split_first() {
        None => Some(*ratio),
        Some((true, rest)) => get_ratio_at(second, rest),
        Some((false, rest)) => get_ratio_at(first, rest),
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the ratio is clamped to 0.1..0.9, so the product fits"
)]
fn split_rect(area: Rect, direction: Direction, ratio: f32) -> (Rect, Rect) {
    match direction {
        Direction::Horizontal => {
            let first_width = (f32::from(area.width) * ratio).round() as u16;
            let second_width = area.width.saturating_sub(first_width);
            (
                Rect::new(area.x, area.y, first_width, area.height),
                Rect::new(area.x + first_width, area.y, second_width, area.height),
            )
        }
        Direction::Vertical => {
            let first_height = (f32::from(area.height) * ratio).round() as u16;
            let second_height = area.height.saturating_sub(first_height);
            (
                Rect::new(area.x, area.y, area.width, first_height),
                Rect::new(area.x, area.y + first_height, area.width, second_height),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(id: u64) -> PaneId {
        PaneId(id)
    }

    /// A layout with one pane, and an id source for new ones.
    fn fresh() -> (TileLayout, PaneId, impl FnMut() -> PaneId) {
        let mut next = 100;
        let root = pane(next);
        let allocate = move || {
            next += 1;
            pane(next)
        };
        (TileLayout::new(root), root, allocate)
    }

    fn split_focused(layout: &mut TileLayout, direction: Direction, id: PaneId) -> PaneId {
        assert!(layout.split_focused(direction, id));
        id
    }

    fn sample_layout() -> TileLayout {
        TileLayout::from_saved(
            Node::Split {
                direction: Direction::Horizontal,
                ratio: 0.3,
                first: Box::new(Node::Pane(pane(1))),
                second: Box::new(Node::Split {
                    direction: Direction::Vertical,
                    ratio: 0.6,
                    first: Box::new(Node::Pane(pane(2))),
                    second: Box::new(Node::Split {
                        direction: Direction::Horizontal,
                        ratio: 0.4,
                        first: Box::new(Node::Pane(pane(3))),
                        second: Box::new(Node::Pane(pane(4))),
                    }),
                }),
            },
            pane(2),
        )
    }

    fn pane_rects(layout: &TileLayout) -> Vec<(PaneId, Rect)> {
        layout
            .panes(Rect::new(0, 0, 100, 40))
            .into_iter()
            .map(|info| (info.id, info.rect))
            .collect()
    }

    fn pane_rect(layout: &TileLayout, pane_id: PaneId) -> Rect {
        pane_rects(layout)
            .into_iter()
            .find_map(|(id, rect)| (id == pane_id).then_some(rect))
            .expect("pane should exist")
    }

    fn split_snapshot(layout: &TileLayout) -> Vec<(Direction, f32)> {
        fn collect(node: &Node, out: &mut Vec<(Direction, f32)>) {
            if let Node::Split {
                direction,
                ratio,
                first,
                second,
            } = node
            {
                out.push((*direction, *ratio));
                collect(first, out);
                collect(second, out);
            }
        }

        let mut out = Vec::new();
        collect(layout.root(), &mut out);
        out
    }

    #[test]
    fn split_paths_preserve_preorder_geometry_and_resize_targets() {
        let ids = [pane(1), pane(2), pane(3), pane(4)];
        let mut layout = TileLayout::from_saved(
            Node::Split {
                direction: Direction::Horizontal,
                ratio: 0.5,
                first: Box::new(split_node(ids[0], Direction::Vertical, ids[1], 0.5)),
                second: Box::new(split_node(ids[2], Direction::Vertical, ids[3], 0.25)),
            },
            ids[0],
        );
        let area = Rect::new(3, 7, 120, 80);
        let splits = layout.splits(area);
        assert_eq!(splits.len(), 3);
        for (split, (path, direction, pos, rect)) in splits.iter().zip([
            (vec![], Direction::Horizontal, 63, area),
            (
                vec![false],
                Direction::Vertical,
                47,
                Rect::new(3, 7, 60, 80),
            ),
            (
                vec![true],
                Direction::Vertical,
                27,
                Rect::new(63, 7, 60, 80),
            ),
        ]) {
            assert_eq!(
                (split.path.clone(), split.direction, split.pos, split.area),
                (path, direction, pos, rect)
            );
        }
        assert!(layout.set_ratio_at(&splits[2].path, 0.5));
        let resized = layout.splits(area);
        assert_eq!(resized[1].pos, splits[1].pos);
        assert_eq!(resized[2].pos, 47);
    }

    #[test]
    fn rejected_splits_and_insertions_preserve_layout_and_focus_history() {
        let (mut layout, root, mut allocate) = fresh();
        let second = split_focused(&mut layout, Direction::Horizontal, allocate());
        let absent = allocate();
        let before = pane_rects(&layout);
        let splits = split_snapshot(&layout);
        assert!(!layout.split_pane(absent, Direction::Vertical, allocate(), 0.5));
        assert!(!layout.split_pane(root, Direction::Vertical, second, 0.5));
        for (target, moved) in [(absent, allocate()), (root, second), (root, root)] {
            assert!(!layout.insert_pane_near(target, moved, Direction::Vertical, 0.3, true));
        }
        assert_eq!(pane_rects(&layout), before);
        assert_eq!(split_snapshot(&layout), splits);
        assert_eq!(layout.focused(), second);
        assert_eq!(layout.prev_focus, Some(root));
        assert!(layout.close_focused());
        assert_eq!(layout.focused(), root);
    }

    #[test]
    fn splitting_a_deep_leaf_preserves_other_branches_and_focus() {
        let (mut layout, root, mut allocate) = fresh();
        let right = allocate();
        assert!(layout.split_pane(root, Direction::Horizontal, right, 0.6));
        let bottom_left = allocate();
        assert!(layout.split_pane(root, Direction::Vertical, bottom_left, 0.4));
        let right_rect = pane_rect(&layout, right);
        let focus = layout.focused();
        let new = allocate();
        assert!(layout.split_pane(bottom_left, Direction::Horizontal, new, f32::NAN));
        assert_eq!(layout.pane_ids(), [root, bottom_left, new, right]);
        assert_eq!(pane_rect(&layout, right), right_rect);
        assert_eq!(layout.focused(), focus);
        assert_eq!(
            split_snapshot(&layout),
            [
                (Direction::Horizontal, 0.6),
                (Direction::Vertical, 0.4),
                (Direction::Horizontal, 0.5)
            ]
        );
    }

    #[test]
    fn swap_panes_exchanges_leaf_ids_without_changing_cells() {
        let mut layout = sample_layout();
        let before_rects = pane_rects(&layout);
        let before_splits = split_snapshot(&layout);

        assert!(layout.swap_panes(pane(2), pane(4)));

        assert_eq!(layout.pane_count(), 4);
        assert_eq!(split_snapshot(&layout), before_splits);
        assert_eq!(layout.focused(), pane(2));

        let after_rects = pane_rects(&layout);
        assert_eq!(after_rects[0], before_rects[0]);
        assert_eq!(after_rects[1], (pane(4), before_rects[1].1));
        assert_eq!(after_rects[2], before_rects[2]);
        assert_eq!(after_rects[3], (pane(2), before_rects[3].1));
    }

    #[test]
    fn swap_panes_is_noop_for_same_or_missing_pane() {
        let mut layout = sample_layout();
        let before_rects = pane_rects(&layout);
        let before_splits = split_snapshot(&layout);
        let before_focus = layout.focused();

        assert!(!layout.swap_panes(pane(2), pane(2)));
        assert!(!layout.swap_panes(pane(2), pane(99)));
        assert!(!layout.swap_panes(pane(99), pane(2)));

        assert_eq!(pane_rects(&layout), before_rects);
        assert_eq!(split_snapshot(&layout), before_splits);
        assert_eq!(layout.focused(), before_focus);
    }

    #[test]
    fn insert_existing_pane_near_target_preserves_existing_ids_and_focuses_moved_pane() {
        let (mut layout, root, _) = fresh();
        let moved = pane(99);

        assert!(layout.insert_pane_near(root, moved, Direction::Horizontal, 0.25, true));

        assert_eq!(layout.pane_count(), 2);
        assert_eq!(layout.pane_ids(), vec![root, moved]);
        assert_eq!(layout.focused(), moved);
        assert_eq!(split_snapshot(&layout), vec![(Direction::Horizontal, 0.25)]);
        assert_eq!(pane_rect(&layout, root), Rect::new(0, 0, 25, 40));
        assert_eq!(pane_rect(&layout, moved), Rect::new(25, 0, 75, 40));
    }

    #[test]
    fn split_focused_with_ratio_sets_new_split_ratio() {
        let (mut layout, root, mut allocate) = fresh();
        layout.focus_pane(root);

        assert!(layout.split_focused_with_ratio(Direction::Horizontal, allocate(), 0.333));

        let splits = split_snapshot(&layout);
        assert_eq!(splits.len(), 1);
        assert_eq!(splits[0].0, Direction::Horizontal);
        assert!((splits[0].1 - 0.333).abs() < f32::EPSILON);
    }

    #[test]
    fn resize_pane_preserves_focus_and_reports_change() {
        let mut layout = sample_layout();
        let original_focus = layout.focused();

        assert!(layout.resize_pane(pane(1), NavDirection::Right, 0.05, Rect::new(0, 0, 100, 40)));

        assert_eq!(layout.focused(), original_focus);
        let split = split_snapshot(&layout)[0];
        assert_eq!(split.0, Direction::Horizontal);
        assert!((split.1 - 0.35).abs() < f32::EPSILON);
    }

    #[test]
    fn resize_second_child_toward_split_decreases_ratio() {
        let (mut layout, root, mut allocate) = fresh();
        let right = split_focused(&mut layout, Direction::Horizontal, allocate());
        layout.focus_pane(root);

        assert!(layout.resize_pane(right, NavDirection::Left, 0.05, Rect::new(0, 0, 100, 40)));

        let split = split_snapshot(&layout)[0];
        assert_eq!(split.0, Direction::Horizontal);
        assert!((split.1 - 0.45).abs() < f32::EPSILON);
        assert_eq!(layout.focused(), root);
    }

    #[test]
    fn resize_outer_edges_shrink_focused_pane() {
        let area = Rect::new(0, 0, 100, 40);
        for (direction, nav, resize_second, expected) in [
            (Direction::Horizontal, NavDirection::Left, false, 0.45),
            (Direction::Horizontal, NavDirection::Right, true, 0.55),
            (Direction::Vertical, NavDirection::Up, false, 0.45),
            (Direction::Vertical, NavDirection::Down, true, 0.55),
        ] {
            let (mut layout, first, mut allocate) = fresh();
            let second = split_focused(&mut layout, direction, allocate());
            let target = if resize_second { second } else { first };

            assert!(layout.resize_pane(target, nav, 0.05, area));
            let split = split_snapshot(&layout)[0];
            assert_eq!(split.0, direction);
            assert!((split.1 - expected).abs() < f32::EPSILON, "{nav:?}");
        }
    }

    #[test]
    fn resize_outer_edge_falls_back_to_ancestor_splits() {
        for (outer, inner, nav) in [
            (
                Direction::Horizontal,
                Direction::Vertical,
                NavDirection::Left,
            ),
            (Direction::Vertical, Direction::Horizontal, NavDirection::Up),
        ] {
            let mut layout = TileLayout::from_saved(
                Node::Split {
                    direction: outer,
                    ratio: 0.6,
                    first: Box::new(split_node(pane(1), inner, pane(2), 0.5)),
                    second: Box::new(Node::Pane(pane(3))),
                },
                pane(1),
            );
            let before = pane_rect(&layout, pane(1));

            assert!(layout.resize_pane(pane(1), nav, 0.05, Rect::new(0, 0, 100, 40)));

            let after = pane_rect(&layout, pane(1));
            match outer {
                Direction::Horizontal => {
                    assert_eq!(after.height, before.height);
                    assert!(after.width < before.width);
                }
                Direction::Vertical => {
                    assert_eq!(after.width, before.width);
                    assert!(after.height < before.height);
                }
            }
            let splits = split_snapshot(&layout);
            assert_eq!(splits[0].0, outer);
            assert!((splits[0].1 - 0.55).abs() < f32::EPSILON);
            assert_eq!(splits[1], (inner, 0.5));
        }
    }

    #[test]
    fn resize_uses_split_in_same_branch_when_borders_share_coordinate() {
        let mut layout = TileLayout::from_saved(
            Node::Split {
                direction: Direction::Vertical,
                ratio: 0.5,
                first: Box::new(split_node(pane(1), Direction::Horizontal, pane(2), 0.5)),
                second: Box::new(split_node(pane(3), Direction::Horizontal, pane(4), 0.5)),
            },
            pane(3),
        );

        assert!(layout.resize_pane(pane(3), NavDirection::Right, 0.05, Rect::new(0, 0, 100, 40)));

        let splits = split_snapshot(&layout);
        assert_eq!(splits[0], (Direction::Vertical, 0.5));
        assert_eq!(splits[1], (Direction::Horizontal, 0.5));
        assert_eq!(splits[2].0, Direction::Horizontal);
        assert!((splits[2].1 - 0.55).abs() < f32::EPSILON);
    }

    #[test]
    fn find_in_direction_tiebreaks_by_larger_overlap_before_layout_order() {
        let info = |id, rect| PaneInfo {
            id: pane(id),
            rect,
            is_focused: id == 1,
        };
        let focused = info(1, Rect::new(10, 10, 10, 10));
        let panes = vec![
            focused.clone(),
            info(2, Rect::new(0, 10, 10, 2)),
            info(3, Rect::new(0, 10, 10, 8)),
        ];

        assert_eq!(
            find_in_direction(&focused, NavDirection::Left, &panes),
            Some(pane(3))
        );
    }

    #[test]
    fn close_focused_returns_to_the_pane_focus_came_from() {
        let mut layout = sample_layout();
        layout.focus_pane(pane(4));

        assert!(layout.close_focused());

        assert_eq!(layout.focused(), pane(2));
    }

    #[test]
    fn close_focused_returns_to_the_pane_that_opened_a_split() {
        let (mut layout, first, mut allocate) = fresh();
        let second = split_focused(&mut layout, Direction::Horizontal, allocate());
        let third = split_focused(&mut layout, Direction::Vertical, allocate());
        assert_eq!(layout.pane_ids().len(), 3);

        layout.focus_pane(first);
        let opened = split_focused(&mut layout, Direction::Horizontal, allocate());
        assert_eq!(layout.focused(), opened);

        assert!(layout.close_focused());

        assert_eq!(layout.focused(), first);
        assert!(layout.pane_ids().contains(&second));
        assert!(layout.pane_ids().contains(&third));
    }

    #[test]
    fn closing_a_background_pane_keeps_the_focused_pane_history() {
        let mut layout = sample_layout();
        layout.focus_pane(pane(4));

        assert!(layout.close_pane(pane(1)));
        assert_eq!(layout.focused(), pane(4));

        assert!(layout.close_focused());
        assert_eq!(layout.focused(), pane(2));
    }

    #[test]
    fn closing_the_remembered_pane_drops_the_focus_history() {
        let mut layout = sample_layout();
        layout.focus_pane(pane(4));

        assert!(layout.close_pane(pane(2)));

        assert!(layout.close_focused());
        assert_eq!(layout.focused(), pane(3));
    }

    #[test]
    fn close_focused_uses_tree_order_without_focus_history() {
        let mut layout = sample_layout();

        assert!(layout.close_focused());

        assert_eq!(layout.focused(), pane(3));
    }

    #[test]
    fn close_focused_does_not_reuse_history_after_it_is_consumed() {
        let mut layout = sample_layout();
        layout.focus_pane(pane(4));

        assert!(layout.close_focused());
        assert_eq!(layout.focused(), pane(2));

        assert!(layout.close_focused());
        assert_eq!(layout.focused(), pane(3));
    }

    #[test]
    fn resize_does_not_disturb_the_close_focus_target() {
        let mut layout = sample_layout();
        layout.focus_pane(pane(4));
        layout.resize_pane(pane(1), NavDirection::Right, 0.05, Rect::new(0, 0, 100, 40));

        assert!(layout.close_focused());

        assert_eq!(layout.focused(), pane(2));
    }

    #[test]
    fn split_pane_leaves_focus_and_history_untouched() {
        let mut layout = sample_layout();
        layout.focus_pane(pane(4));

        assert!(layout.split_pane(pane(1), Direction::Horizontal, pane(9), 0.5));

        assert!(layout.pane_ids().contains(&pane(9)));
        assert_eq!(layout.focused(), pane(4));
        assert!(layout.close_focused());
        assert_eq!(layout.focused(), pane(2));
    }

    #[test]
    fn split_pane_missing_target_changes_nothing() {
        let mut layout = sample_layout();
        let ids = layout.pane_ids();

        assert!(!layout.split_pane(pane(99), Direction::Horizontal, pane(9), 0.5));

        assert_eq!(layout.pane_ids(), ids);
    }

    #[test]
    fn insert_pane_near_unfocused_keeps_focus_and_history() {
        let mut layout = sample_layout();
        layout.focus_pane(pane(4));

        assert!(layout.insert_pane_near(pane(1), pane(9), Direction::Horizontal, 0.5, false));

        assert_eq!(layout.focused(), pane(4));
        assert!(layout.close_focused());
        assert_eq!(layout.focused(), pane(2));
    }

    #[test]
    fn failed_split_rollback_preserves_focus_history() {
        let mut layout = sample_layout();
        layout.focus_pane(pane(4));

        assert!(layout.split_pane(layout.focused(), Direction::Horizontal, pane(9), 0.5));
        assert!(layout.close_pane(pane(9)));

        assert_eq!(layout.focused(), pane(4));
        assert!(layout.close_focused());
        assert_eq!(layout.focused(), pane(2));
    }

    #[test]
    fn a_newer_tree_keeps_focus_and_its_history_where_they_still_exist() {
        let mut layout = sample_layout();
        layout.focus_pane(pane(4));

        let mut server = sample_layout();
        assert!(server.close_pane(pane(1)));
        layout.set_root(server.root().clone());
        assert_eq!(layout.focused(), pane(4));
        assert_eq!(layout.prev_focus, Some(pane(2)));

        // Another client closed the focused pane: focus goes back where it came from.
        assert!(server.close_pane(pane(4)));
        layout.set_root(server.root().clone());
        assert_eq!(layout.focused(), pane(2));
        assert_eq!(layout.prev_focus, None);

        assert!(server.close_pane(pane(2)));
        layout.set_root(server.root().clone());
        assert_eq!(layout.focused(), pane(3));
    }

    #[test]
    fn trees_round_trip_through_json() {
        let layout = sample_layout();
        let json = serde_json::to_string(layout.root()).expect("encodes");
        let root: Node = serde_json::from_str(&json).expect("decodes");
        assert_eq!(&root, layout.root());
        assert_eq!(
            TileLayout::from_saved(root, pane(7)).focused(),
            pane(1),
            "a missing focus falls back to the first pane"
        );
    }
}
