use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, DrawingArea, GestureClick, Label, Orientation, ScrolledWindow, Separator,
};

const NODE_W: f64 = 148.0;
const NODE_H: f64 = 34.0;
const H_GAP: f64 = 24.0;
const V_GAP: f64 = 64.0;
const MARGIN: f64 = 24.0;

struct GraphNode {
    path: PathBuf,
    label: String,
    x: f64,
    y: f64,
}

type OpenCb = Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>;

#[derive(Clone)]
pub struct DepGraph {
    widget: GtkBox,
    drawing_area: DrawingArea,
    project_root: Rc<PathBuf>,
    nodes: Rc<RefCell<Vec<GraphNode>>>,
    edges: Rc<RefCell<Vec<(usize, usize)>>>,
    hovered: Rc<RefCell<Option<usize>>>,
    on_open: OpenCb,
}

impl DepGraph {
    pub fn new(project_root: PathBuf) -> Self {
        let widget = GtkBox::new(Orientation::Vertical, 0);

        let header = GtkBox::new(Orientation::Horizontal, 0);
        header.set_margin_start(10);
        header.set_margin_end(10);
        header.set_margin_top(6);
        header.set_margin_bottom(6);
        let title = Label::new(Some("File graph"));
        title.set_xalign(0.0);
        title.set_hexpand(true);
        title.add_css_class("heading");
        header.append(&title);

        widget.append(&Separator::new(Orientation::Horizontal));
        widget.append(&header);
        widget.append(&Separator::new(Orientation::Horizontal));

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_hexpand(true);

        let drawing_area = DrawingArea::new();
        drawing_area.set_hexpand(true);
        drawing_area.set_vexpand(true);
        drawing_area.set_focusable(true);
        drawing_area.set_cursor_from_name(Some("default"));
        scroll.set_child(Some(&drawing_area));
        widget.append(&scroll);

        let nodes: Rc<RefCell<Vec<GraphNode>>> = Rc::new(RefCell::new(Vec::new()));
        let edges: Rc<RefCell<Vec<(usize, usize)>>> = Rc::new(RefCell::new(Vec::new()));
        let hovered: Rc<RefCell<Option<usize>>> = Rc::new(RefCell::new(None));
        let on_open: OpenCb = Rc::new(RefCell::new(None));

        // Draw function
        {
            let nodes_d = nodes.clone();
            let edges_d = edges.clone();
            let hovered_d = hovered.clone();
            drawing_area.set_draw_func(move |_area, ctx, _w, _h| {
                let nodes = nodes_d.borrow();
                let edges = edges_d.borrow();
                let hover = *hovered_d.borrow();

                ctx.set_source_rgb(0.12, 0.12, 0.12);
                ctx.paint().ok();

                if nodes.is_empty() {
                    ctx.set_source_rgb(0.5, 0.5, 0.5);
                    ctx.select_font_face(
                        "sans-serif",
                        gtk4::cairo::FontSlant::Normal,
                        gtk4::cairo::FontWeight::Normal,
                    );
                    ctx.set_font_size(13.0);
                    ctx.move_to(MARGIN, MARGIN + 16.0);
                    ctx.show_text("No linked files found. Open a .typ file.")
                        .ok();
                    return;
                }

                // Edges
                ctx.set_source_rgba(0.5, 0.5, 0.5, 0.7);
                ctx.set_line_width(1.5);
                for &(from, to) in edges.iter() {
                    let nf = &nodes[from];
                    let nt = &nodes[to];
                    let x1 = nf.x + NODE_W / 2.0;
                    let y1 = nf.y + NODE_H;
                    let x2 = nt.x + NODE_W / 2.0;
                    let y2 = nt.y;
                    // Bezier curve for nicer look
                    let cy = (y1 + y2) / 2.0;
                    ctx.move_to(x1, y1);
                    ctx.curve_to(x1, cy, x2, cy, x2, y2);
                    ctx.stroke().ok();
                }

                // Nodes
                ctx.select_font_face(
                    "sans-serif",
                    gtk4::cairo::FontSlant::Normal,
                    gtk4::cairo::FontWeight::Normal,
                );
                ctx.set_font_size(12.0);

                for (i, node) in nodes.iter().enumerate() {
                    let is_root = i == 0;
                    let is_hov = hover == Some(i);

                    let (r, g, b) = if is_hov {
                        (0.35, 0.60, 0.95)
                    } else if is_root {
                        (0.25, 0.70, 0.45)
                    } else {
                        (0.22, 0.22, 0.28)
                    };

                    // Shadow
                    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.25);
                    rounded_rect(ctx, node.x + 2.0, node.y + 3.0, NODE_W, NODE_H, 6.0);
                    ctx.fill().ok();

                    // Fill
                    ctx.set_source_rgb(r, g, b);
                    rounded_rect(ctx, node.x, node.y, NODE_W, NODE_H, 6.0);
                    ctx.fill().ok();

                    // Border
                    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.15);
                    ctx.set_line_width(1.0);
                    rounded_rect(ctx, node.x, node.y, NODE_W, NODE_H, 6.0);
                    ctx.stroke().ok();

                    // Label
                    ctx.set_source_rgb(1.0, 1.0, 1.0);
                    if let Ok(ext) = ctx.text_extents(&node.label) {
                        let tx = (node.x + NODE_W / 2.0 - ext.width() / 2.0).max(node.x + 4.0);
                        let ty = node.y + NODE_H / 2.0 - ext.y_bearing() - ext.height() / 2.0;
                        ctx.move_to(tx, ty);
                        ctx.show_text(&node.label).ok();
                    }
                }
            });
        }

        let g = Self {
            widget,
            drawing_area,
            project_root: Rc::new(project_root),
            nodes,
            edges,
            hovered,
            on_open,
        };

        // Click handler — extract path before dropping borrow to avoid RefCell conflict
        // when the callback triggers dep_graph.refresh() which needs borrow_mut.
        {
            let nodes_c = g.nodes.clone();
            let on_open_c = g.on_open.clone();
            let da_c = g.drawing_area.clone();
            let click = GestureClick::new();
            click.connect_pressed(move |_, _, x, y| {
                let clicked_path = {
                    let nodes = nodes_c.borrow();
                    nodes
                        .iter()
                        .find(|n| x >= n.x && x <= n.x + NODE_W && y >= n.y && y <= n.y + NODE_H)
                        .map(|n| n.path.clone())
                }; // borrow released here
                if let Some(path) = clicked_path {
                    if let Some(f) = on_open_c.borrow().as_ref() {
                        f(path);
                    }
                }
                da_c.queue_draw();
            });
            g.drawing_area.add_controller(click);
        }

        // Motion controller for hover
        {
            let nodes_m = g.nodes.clone();
            let hover_m = g.hovered.clone();
            let da_m = g.drawing_area.clone();
            let motion = gtk4::EventControllerMotion::new();
            motion.connect_motion(move |_, x, y| {
                let nodes = nodes_m.borrow();
                let mut found = None;
                for (i, node) in nodes.iter().enumerate() {
                    if x >= node.x && x <= node.x + NODE_W && y >= node.y && y <= node.y + NODE_H {
                        found = Some(i);
                        break;
                    }
                }
                if *hover_m.borrow() != found {
                    *hover_m.borrow_mut() = found;
                    da_m.queue_draw();
                }
            });
            motion.connect_leave(move |_| {});
            g.drawing_area.add_controller(motion);
        }

        g
    }

    pub fn set_on_open(&self, f: impl Fn(PathBuf) + 'static) {
        *self.on_open.borrow_mut() = Some(Box::new(f));
    }

    pub fn widget(&self) -> &GtkBox {
        &self.widget
    }

    pub fn refresh(&self, root_file: Option<&PathBuf>) {
        let root = if let Some(r) = root_file {
            r.clone()
        } else {
            let candidate = self.project_root.join("main.typ");
            if candidate.exists() {
                candidate
            } else {
                let files = crate::project::collect_typ_files(&self.project_root);
                match files.into_iter().next() {
                    Some(f) => f,
                    None => return,
                }
            }
        };

        let deps = build_dep_map(&root, &self.project_root);
        let (nodes, edges) = layout_graph(&root, &deps);

        // Update DrawingArea content size
        let max_x = nodes
            .iter()
            .map(|n| n.x + NODE_W + MARGIN)
            .fold(0.0f64, f64::max);
        let max_y = nodes
            .iter()
            .map(|n| n.y + NODE_H + MARGIN)
            .fold(0.0f64, f64::max);

        self.drawing_area.set_content_width(max_x.ceil() as i32);
        self.drawing_area.set_content_height(max_y.ceil() as i32);

        *self.nodes.borrow_mut() = nodes;
        *self.edges.borrow_mut() = edges;
        *self.hovered.borrow_mut() = None;
        self.drawing_area.queue_draw();
    }
}

// ── Graph building ────────────────────────────────────────────────────────────

fn build_dep_map(root: &Path, project_root: &Path) -> HashMap<PathBuf, Vec<PathBuf>> {
    let mut map: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    let mut queue: VecDeque<PathBuf> = VecDeque::new();
    let mut visited: HashSet<PathBuf> = HashSet::new();

    let canonical_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    queue.push_back(canonical_root.clone());

    while let Some(path) = queue.pop_front() {
        if visited.contains(&path) {
            continue;
        }
        visited.insert(path.clone());

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let base = path.parent().unwrap_or(project_root);
        let imports = crate::project::parse_typ_imports(&content, base);

        for child in &imports {
            if !queue.contains(child) && !visited.contains(child) {
                queue.push_back(child.clone());
            }
        }
        map.insert(path, imports);
    }
    map
}

fn layout_graph(
    root: &Path,
    deps: &HashMap<PathBuf, Vec<PathBuf>>,
) -> (Vec<GraphNode>, Vec<(usize, usize)>) {
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<(usize, usize)> = Vec::new();
    let mut path_to_idx: HashMap<PathBuf, usize> = HashMap::new();
    let mut levels: Vec<Vec<usize>> = Vec::new();
    let mut visited: HashSet<PathBuf> = HashSet::new();
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();

    queue.push_back((root.to_path_buf(), 0));

    while let Some((path, depth)) = queue.pop_front() {
        if visited.contains(&path) {
            continue;
        }
        visited.insert(path.clone());

        let label = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();

        let idx = nodes.len();
        path_to_idx.insert(path.clone(), idx);
        nodes.push(GraphNode {
            path: path.clone(),
            label,
            x: 0.0,
            y: 0.0,
        });

        while levels.len() <= depth {
            levels.push(Vec::new());
        }
        levels[depth].push(idx);

        if let Some(children) = deps.get(&path) {
            for child in children {
                if !visited.contains(child) {
                    queue.push_back((child.clone(), depth + 1));
                }
            }
        }
    }

    // Calculate positions
    let max_w = levels
        .iter()
        .map(|lvl| lvl.len() as f64 * (NODE_W + H_GAP) - H_GAP)
        .fold(0.0f64, f64::max);

    for (depth, level_nodes) in levels.iter().enumerate() {
        let count = level_nodes.len() as f64;
        let total_w = count * NODE_W + (count - 1.0) * H_GAP;
        let start_x = MARGIN + (max_w - total_w) / 2.0;
        let y = MARGIN + depth as f64 * (NODE_H + V_GAP);

        for (i, &node_idx) in level_nodes.iter().enumerate() {
            nodes[node_idx].x = start_x + i as f64 * (NODE_W + H_GAP);
            nodes[node_idx].y = y;
        }
    }

    // Build edges
    for (path, children) in deps {
        if let Some(&parent_idx) = path_to_idx.get(path) {
            for child in children {
                if let Some(&child_idx) = path_to_idx.get(child) {
                    edges.push((parent_idx, child_idx));
                }
            }
        }
    }

    (nodes, edges)
}

fn rounded_rect(ctx: &gtk4::cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    use std::f64::consts::PI;
    ctx.new_sub_path();
    ctx.arc(x + w - r, y + r, r, -PI / 2.0, 0.0);
    ctx.arc(x + w - r, y + h - r, r, 0.0, PI / 2.0);
    ctx.arc(x + r, y + h - r, r, PI / 2.0, PI);
    ctx.arc(x + r, y + r, r, PI, 3.0 * PI / 2.0);
    ctx.close_path();
}
