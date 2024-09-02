use anyhow::{Context, Result};
use cairo_lang_filesystem::ids::Directory;
use std::collections::HashSet;
use std::fmt::{Debug, Display};
use std::path::PathBuf;

#[derive(Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ModulePath(pub(crate) String);

impl ModulePath {
    pub fn new<T>(path: T) -> Self
    where
        T: ToString,
    {
        ModulePath(path.to_string())
    }

    pub fn get_path(&self) -> &String {
        &self.0
    }

    pub fn get_parent_path(&self) -> ModulePath {
        let path = self.0.clone();
        let mut parent_path = path.split("::").collect::<Vec<&str>>();
        parent_path.pop();
        let res = parent_path.join("::");
        ModulePath(res)
    }

    pub fn get_crate(&self) -> String {
        let path = self.0.clone();
        let crate_name = path.split("::").collect::<Vec<&str>>()[0];
        crate_name.to_string()
    }

    pub fn get_modules(&self) -> Vec<&str> {
        self.0.split("::").collect::<Vec<&str>>()
    }
}

impl Display for ModulePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Debug for ModulePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct CairoCrate {
    pub root_dir: Directory,
    pub main_file: PathBuf,
    pub modules: Vec<CairoModule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CairoModule {
    pub dir: PathBuf,
    pub main_file: PathBuf,
    pub path: ModulePath,
    pub filepath: PathBuf,
    pub relative_filepath: PathBuf,
    pub submodules: Vec<CairoSubmodules>,
    pub imports: HashSet<CairoImport>,
}

impl Display for CairoModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CairoModule {{ path: {:?} }}", self.path,)
    }
}

impl CairoModule {
    pub(crate) fn get_root_dir(&self) -> Result<PathBuf> {
        let module_root = self.main_file.clone();
        Ok(module_root
            .parent()
            .with_context(|| format!("Failed to get parent of {:?}", module_root))?
            .parent()
            .with_context(|| format!("Failed to get grandparent of {:?}", module_root))?
            .to_path_buf())
    }

    /// Attempt to check if a ModulePath given will resolve into a CairoModule or
    /// a CairoSubmodule.
    pub fn is_module_path_resolved(&self, mod_path: ModulePath) -> bool {
        if mod_path == self.path {
            return true;
        }
        if self.submodules.iter().any(|submod| submod.path == mod_path) {
            return true;
        }
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CairoSubmodules {
    pub name: String,
    pub parent_path: ModulePath,
    pub path: ModulePath,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CairoImport {
    pub name: String,
    pub path: ModulePath,
    pub resolved_path: ModulePath,
    pub import_type: CairoImportType,
}

impl CairoImport {
    pub fn get_import_module(&self) -> ModulePath {
        match self.import_type {
            CairoImportType::Module => ModulePath::new(self.resolved_path.clone()),
            CairoImportType::Other => self.resolved_path.get_parent_path(),
        }
    }

    pub fn is_remapped(&self) -> bool {
        self.path != ModulePath::new(self.resolved_path.clone())
    }
    pub fn is_super_import(&self) -> bool {
        self.path.0.starts_with("super")
    }

    pub fn resolved_parent_module(&self) -> ModulePath {
        self.resolved_path.get_parent_path()
    }

    pub fn unresolved_parent_module(&self) -> ModulePath {
        self.path.get_parent_path()
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum CairoImportType {
    Module,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CairoAttachmentModule {
    pub path: ModulePath,
    pub children: HashSet<ModulePath>,
    pub imports: HashSet<ModulePath>,
}

impl CairoAttachmentModule {
    pub fn new(path: ModulePath) -> Self {
        CairoAttachmentModule {
            path,
            children: HashSet::new(),
            imports: HashSet::new(),
        }
    }

    pub fn add_child(&mut self, child: ModulePath) {
        self.children.insert(child);
    }

    pub fn add_import(&mut self, import: ModulePath) {
        self.imports.insert(import);
    }
}
