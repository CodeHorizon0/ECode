use std::cmp::Ordering;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rfd::FileDialog;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Directory,
    File,
}

pub struct ExplorerNode {
    pub path: PathBuf,
    pub name: String,
    pub kind: NodeKind,
    pub expanded: bool,
    pub children_loaded: bool,
    pub children: Vec<ExplorerNode>,
}

impl ExplorerNode {
    fn directory(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| path.to_str().unwrap_or("Project"))
            .to_string();

        Self {
            path,
            name,
            kind: NodeKind::Directory,
            expanded: false,
            children_loaded: false,
            children: Vec::new(),
        }
    }

    fn file(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("file")
            .to_string();

        Self {
            path,
            name,
            kind: NodeKind::File,
            expanded: false,
            children_loaded: true,
            children: Vec::new(),
        }
    }
}

pub struct VisibleNode {
    pub path: PathBuf,
    pub name: String,
    pub kind: NodeKind,
    pub depth: usize,
    pub expanded: bool,
}

pub struct Workspace {
    root: Option<ExplorerNode>,
    selected: Option<PathBuf>,
}

impl Workspace {
    pub fn new() -> Self {
        Self {
            root: None,
            selected: None,
        }
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_ref().map(|node| node.path.as_path())
    }

    pub fn selected(&self) -> Option<&Path> {
        self.selected.as_deref()
    }

    pub fn is_open(&self) -> bool {
        self.root.is_some()
    }

    pub fn open_dialog(&mut self) -> io::Result<Option<PathBuf>> {
        let Some(path) = FileDialog::new()
            .set_title("Open Project Folder")
            .pick_folder()
        else {
            return Ok(None);
        };

        self.set_root(path.clone())?;
        Ok(Some(path))
    }

    pub fn set_root(&mut self, path: PathBuf) -> io::Result<()> {
        let metadata = fs::metadata(&path)?;
        if !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Selected path is not a directory",
            ));
        }

        let mut root = ExplorerNode::directory(path);
        load_children(&mut root)?;
        root.expanded = true;

        self.root = Some(root);
        self.selected = None;
        Ok(())
    }

    pub fn refresh(&mut self) -> io::Result<()> {
        let Some(root_path) = self.root().map(Path::to_path_buf) else {
            return Ok(());
        };

        self.set_root(root_path)
    }

    pub fn visible_nodes(&self) -> Vec<VisibleNode> {
        let mut result = Vec::new();

        let Some(root) = self.root.as_ref() else {
            return result;
        };

        append_visible(root, 0, true, &mut result);
        result
    }

    pub fn toggle_directory(&mut self, path: &Path) -> io::Result<()> {
        let Some(root) = self.root.as_mut() else {
            return Ok(());
        };

        let Some(node) = find_node_mut(root, path) else {
            return Ok(());
        };

        if node.kind != NodeKind::Directory {
            return Ok(());
        }

        if !node.children_loaded {
            load_children(node)?;
        }

        node.expanded = !node.expanded;
        Ok(())
    }

    pub fn select(&mut self, path: PathBuf) {
        self.selected = Some(path);
    }

    pub fn create_file(&mut self, parent: &Path, name: &str) -> io::Result<PathBuf> {
        validate_name(name)?;

        let path = parent.join(name);
        if path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "A file or directory with this name already exists",
            ));
        }

        fs::File::create(&path)?;
        self.refresh_parent(parent)?;
        self.selected = Some(path.clone());
        Ok(path)
    }

    pub fn create_directory(&mut self, parent: &Path, name: &str) -> io::Result<PathBuf> {
        validate_name(name)?;

        let path = parent.join(name);
        if path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "A file or directory with this name already exists",
            ));
        }

        fs::create_dir(&path)?;
        self.refresh_parent(parent)?;
        self.selected = Some(path.clone());
        Ok(path)
    }

    pub fn rename_selected(&mut self, name: &str) -> io::Result<Option<PathBuf>> {
        let Some(selected) = self.selected.clone() else {
            return Ok(None);
        };

        validate_name(name)?;

        let Some(parent) = selected.parent() else {
            return Ok(None);
        };

        let target = parent.join(name);
        if target == selected {
            return Ok(Some(selected));
        }

        if target.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "A file or directory with this name already exists",
            ));
        }

        fs::rename(&selected, &target)?;
        self.refresh_parent(parent)?;
        self.selected = Some(target.clone());
        Ok(Some(target))
    }

    pub fn delete_selected(&mut self) -> io::Result<Option<PathBuf>> {
        let Some(selected) = self.selected.clone() else {
            return Ok(None);
        };

        if Some(selected.as_path()) == self.root() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The project root cannot be deleted from the explorer",
            ));
        }

        let metadata = fs::metadata(&selected)?;

        if metadata.is_dir() {
            fs::remove_dir_all(&selected)?;
        } else {
            fs::remove_file(&selected)?;
        }

        let parent = selected.parent().map(Path::to_path_buf);

        if let Some(parent) = parent.as_deref() {
            self.refresh_parent(parent)?;
        }

        self.selected = None;
        Ok(Some(selected))
    }

    pub fn parent_for_new_item(&self) -> Option<PathBuf> {
        let Some(selected) = self.selected.as_ref() else {
            return self.root().map(Path::to_path_buf);
        };

        match fs::metadata(selected) {
            Ok(metadata) if metadata.is_dir() => Some(selected.clone()),
            Ok(_) | Err(_) => selected.parent().map(Path::to_path_buf),
        }
    }

    fn refresh_parent(&mut self, parent: &Path) -> io::Result<()> {
        let found = if let Some(root) = self.root.as_mut() {
            if let Some(node) = find_node_mut(root, parent) {
                if node.kind == NodeKind::Directory {
                    load_children(node)?;
                    node.expanded = true;
                }
                true
            } else {
                false
            }
        } else {
            false
        };

        if found {
            Ok(())
        } else {
            self.refresh()
        }
    }
}

fn append_visible(
    node: &ExplorerNode,
    depth: usize,
    include_root: bool,
    result: &mut Vec<VisibleNode>,
) {
    if include_root {
        result.push(VisibleNode {
            path: node.path.clone(),
            name: node.name.clone(),
            kind: node.kind,
            depth,
            expanded: node.expanded,
        });
    }

    if node.kind != NodeKind::Directory || !node.expanded {
        return;
    }

    let child_depth = if include_root { depth + 1 } else { depth };

    for child in &node.children {
        append_visible(child, child_depth, true, result);
    }
}

fn find_node_mut<'a>(node: &'a mut ExplorerNode, path: &Path) -> Option<&'a mut ExplorerNode> {
    if node.path == path {
        return Some(node);
    }

    for child in &mut node.children {
        if let Some(found) = find_node_mut(child, path) {
            return Some(found);
        }
    }

    None
}

fn load_children(node: &mut ExplorerNode) -> io::Result<()> {
    if node.kind != NodeKind::Directory {
        return Ok(());
    }

    let mut children = Vec::new();

    for entry in fs::read_dir(&node.path)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;

        if metadata.is_dir() {
            children.push(ExplorerNode::directory(path));
        } else if metadata.is_file() {
            children.push(ExplorerNode::file(path));
        }
    }

    children.sort_by(|left, right| {
        let kind_order = match (left.kind, right.kind) {
            (NodeKind::Directory, NodeKind::File) => Ordering::Less,
            (NodeKind::File, NodeKind::Directory) => Ordering::Greater,
            _ => Ordering::Equal,
        };

        kind_order.then_with(|| {
            left.name
                .to_ascii_lowercase()
                .cmp(&right.name.to_ascii_lowercase())
        })
    });

    node.children = children;
    node.children_loaded = true;
    Ok(())
}

fn validate_name(name: &str) -> io::Result<()> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Name cannot be empty",
        ));
    }

    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Name cannot contain path separators",
        ));
    }

    Ok(())
}
