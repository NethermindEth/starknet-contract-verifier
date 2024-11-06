use anyhow::{anyhow, Context, Result};
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_defs::db::DefsGroup;
use cairo_lang_defs::ids::{
    FileIndex, GenericTypeId, LanguageElementId, LocalVarId, ModuleFileId, ModuleId, NamedLanguageElementId, TopLevelLanguageElementId, UseId, UseLongId, VarId
};
use cairo_lang_filesystem::db::FilesGroup;
use cairo_lang_filesystem::ids::{CrateId, Directory, FileId, FileLongId};
use cairo_lang_semantic::diagnostic::{NotFoundItemType, SemanticDiagnostics};
use cairo_lang_semantic::expr::inference::InferenceId;
use cairo_lang_semantic::items::us::get_use_segments;
use cairo_lang_semantic::resolve::{ResolvedGenericItem, Resolver};
use cairo_lang_syntax::node::ast::{MaybeModuleBody, UsePath, UsePathLeaf};
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use cairo_lang_utils::Upcast;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

use crate::model::{CairoImport, CairoImportType, CairoModule, CairoSubmodules, ModulePath};
use cairo_lang_diagnostics::{DiagnosticsBuilder, ToOption};
use cairo_lang_semantic::items::functions::GenericFunctionId;

use std::path::PathBuf;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct FileData {
    pub id: FileId,
    pub name: String,
    pub path: PathBuf,
    pub index: usize,
}

impl FileData {
    pub fn new(id: FileId, name: String, path: PathBuf, index: usize) -> Self {
        Self {
            id,
            name,
            path,
            index,
        }
    }

    pub fn get_path(&self) -> PathBuf {
        self.path.clone()
    }

    pub fn get_relative_path(&self, root: &PathBuf) -> Result<PathBuf> {
        Ok(self
            .path
            .strip_prefix(root)
            .with_context(|| format!("Couldn't strip prefix {}", root.display()))?
            .to_path_buf())
    }
}

/// Extracts the imports of a child module and adds them to a hash map.
///
/// The function recursively processes the child module's top-level items. If a child module
/// contains a `use` statement, it's added to a hash map with the corresponding `UseId`. If it
/// contains another child module, the function is called recursively on that child module.
///
/// # Arguments
///
/// * `db` - A trait object that provides access to the database.
/// * `module_ast` - The syntax tree node for the child module to process.
/// * `module_file_id` - The ID of the file that contains the child module.
/// * `module_uses` - A mutable hash map that stores the `UseId` and syntax tree node for each
///   `use` statement found in the module and its children.
///
fn extract_child_module_imports(
    db: &dyn DefsGroup,
    module_ast: &ast::ItemModule,
    module_file_id: ModuleFileId,
    module_uses: &mut OrderedHashMap<UseId, UsePathLeaf>,
) {
    if let MaybeModuleBody::Some(module_body) = module_ast.body(db.upcast()) {
        module_body
            .items(db.upcast())
            .elements(db.upcast())
            .iter()
            .for_each(|el| match el {
                ast::ModuleItem::Use(item_use) => capture_imports(
                    db,
                    module_file_id,
                    module_uses,
                    item_use.use_path(db.upcast()),
                ),
                ast::ModuleItem::Module(item_module) => {
                    extract_child_module_imports(db, item_module, module_file_id, module_uses);
                }
                _ => {}
            });
    }
}

pub fn capture_modules_submodules(
    db: &RootDatabase,
    file_id: FileId,
    module_id: ModuleId,
) -> Result<Vec<CairoSubmodules>> {
    let mut submodules: Vec<CairoSubmodules> = vec![];
    // We check if there are any defined submodules in the given module.
    // If so recursively resolve them.
    let found_submodules = match db.module_submodules_ids(module_id) {
        Ok(submod) => submod,
        Err(_) => return Err(anyhow!("Fail to resolve module submodules")),
    };

    for submod in found_submodules.iter() {
        let submod_id = ModuleId::Submodule(*submod);
        // Check that the location of the submodule is of the same as the module's file.
        // This prevent over-resoluting the modules.
        if let Some(submod_file_data) = get_module_file(db, submod_id) {
            if submod_file_data.id == file_id {
                let root_mod_path = module_id.full_path(db);
                let current_mod_path = submod_id.full_path(db);
                let mod_name = match current_mod_path
                    .strip_prefix(format!("{}::", root_mod_path.as_str()).as_str())
                {
                    Some(name) => name.to_string(),
                    None => return Err(anyhow!("Fail to compute submod name from path")),
                };
                submodules.push(CairoSubmodules {
                    name: mod_name,
                    parent_path: ModulePath::new(root_mod_path),
                    path: ModulePath::new(current_mod_path),
                });
                let submod_submods = capture_modules_submodules(db, file_id, submod_id)?;
                submodules.extend(submod_submods);
            }
        }
    }

    Ok(submodules)
}

/// Recursively capture all the use statements of a given file, inclusive of the root
/// module of the file and also the submodules of the file, while retaining the
/// context of the modules in which the use statements were at.
fn capture_file_modules_uses(
    db: &RootDatabase,
    file_id: FileId,
    module_id: ModuleId,
    module_uses: &mut OrderedHashMap<UseId, (UsePathLeaf, ModuleId)>,
) -> Result<()> {
    // First we capture the given module id imports.
    let found_module_uses = match db.module_uses(module_id) {
        Ok(mods) => mods,
        Err(_) => return Err(anyhow!("Fail to resolve modules uses")),
    };

    for (use_id, use_path_leaf) in found_module_uses.iter() {
        module_uses.insert(*use_id, (use_path_leaf.clone(), module_id));
    }

    // We then check if there are any defined submodules in the given module.
    // If so recursively resolve them.
    let found_submodules = match db.module_submodules_ids(module_id) {
        Ok(submod) => submod,
        Err(_) => return Err(anyhow!("Fail to resolve module submodules")),
    };

    for submod in found_submodules.iter() {
        let submod_id = ModuleId::Submodule(*submod);
        // Check that the location of the submodule is of the same as the module's file.
        // This prevent over-resoluting the modules.
        if let Some(submod_file_data) = get_module_file(db, submod_id) {
            if submod_file_data.id == file_id {
                capture_file_modules_uses(db, file_id, submod_id, module_uses)?;
            }
        }
    }

    Ok(())
}

/// Extracts the imports of a child module and adds them to a hash map.
///
/// The function recursively processes a use import. If a path containts only an
/// import, the use statmenent is added to the hash map with the corresponding `UseId`.
/// If a use contains more than one use path, it is recursively added.
///
/// # Arguments
///
/// * `db` - A trait object that provides access to the database.
/// * `module_file_id` - The ID of the file that contains the child module.
/// * `module_uses` - A mutable hash map that stores the `UseId` and syntax tree node for each
/// * `item_use` - The use statment
/// * `use_path` - The use path of the use statement
///
fn capture_imports(
    db: &dyn DefsGroup,
    module_file_id: ModuleFileId,
    module_uses: &mut OrderedHashMap<UseId, UsePathLeaf>,
    use_path: UsePath,
) {
    match use_path {
        UsePath::Leaf(use_path_leaf) => {
            let use_id = db.intern_use(UseLongId(module_file_id, use_path_leaf.stable_ptr()));
            module_uses.insert(use_id, use_path_leaf.clone());
        }
        UsePath::Single(use_path_single) => capture_imports(
            db,
            module_file_id,
            module_uses,
            use_path_single.use_path(db.upcast()),
        ),
        UsePath::Multi(use_path_multiple) => {
            use_path_multiple
                .use_paths(db.upcast())
                .elements(db.upcast())
                .into_iter()
                .for_each(|use_path_i| {
                    capture_imports(db, module_file_id, module_uses, use_path_i)
                });
        }
    };
}

/// This function extracts the modules for a given crate and returns them as a vector of `CairoModule`s.
///
/// # Arguments
///
/// * `db` - A reference to the root database to search for the crate and module data.
/// * `crate_id` - The `CrateId` of the crate for which to extract module data.
///
/// # Returns
///
/// * A `Vec<CairoModule>` representing the modules for the given crate, if they exist.
///
/// # Example
///
pub fn collect_crate_module_files(
    db: &RootDatabase,
    crate_id: CrateId,
) -> Result<Vec<CairoModule>> {
    let mut crate_modules = vec![];
    let defs_db: &dyn DefsGroup = db.upcast();
    let mut visited_files = HashMap::new();

    let crate_root_dir = match db
        .crate_config(crate_id)
        .expect(
            format!(
                "Failed to get crate root directory for crate ID {:?}",
                crate_id
            )
            .as_str(),
        )
        .root
    {
        Directory::Real(path) => path.display().to_string(),
        Directory::Virtual { .. } => {
            return Err(anyhow!("Virtual directories are not supported"));
        }
    };

    // Piece of code for debugging. Leaving this in for later usage.
    // for module_id in &*defs_db.crate_modules(crate_id) {
    //     println!("******");
    //     println!("module id {:?}", module_id);
    //     println!("module id full path {:?}", module_id.full_path(db));
    //     let module_file_data: Option<FileData> = get_module_file(db, *module_id);
    //     println!("module file {:?}", module_file_data.clone());
    // }

    for module_id in &*defs_db.crate_modules(crate_id) {
        let module_file = defs_db
            .module_main_file(ModuleId::CrateRoot(crate_id))
            .to_option()
            .with_context(|| format!("Expected module file for module {:?}", module_id))?;
        let main_file_path = match db.lookup_intern_file(module_file) {
            FileLongId::OnDisk(path) => Ok(path),
            FileLongId::Virtual(_) => {
                Err(anyhow!("Expected OnDisk file for module {:?}", module_id))
            },
            FileLongId::External(_) => {
                Err(anyhow!("Expected OnDisk file for module {:?}", module_id))
            }
        }?;
        let module_file_data = get_module_file(db, *module_id);

        // A module without a file means that it's a submodule inside a file module.
        if let Some(file) = module_file_data {
            // Skip files that have already been visited.
            // This happens when a file defines multiple modules.
            if visited_files.contains_key(&file) {
                continue;
            }
            visited_files.insert(file.clone(), true);

            let defs_group: &dyn DefsGroup = db.upcast();
            let module_dir: String = match defs_group
                .module_dir(*module_id)
                .to_option()
                .with_context(|| {
                    format!("Could not get module directory for module {:?}", module_id)
                })? {
                Directory::Real(path) => path.display().to_string(),
                Directory::Virtual { .. } => {
                    return Err(anyhow!("Virtual directories are not supported"));
                }
            };
            let module_imports = HashSet::from_iter(extract_file_imports(db, *module_id, &file)?);
            let module_submodules = capture_modules_submodules(db, file.id, *module_id)?;
            let cairo_module_data = CairoModule {
                dir: module_dir.into(),
                main_file: main_file_path,
                path: ModulePath::new(module_id.full_path(db.upcast())),
                filepath: file.path.clone(),
                relative_filepath: file.get_relative_path(&PathBuf::from(&crate_root_dir))?,
                imports: module_imports,
                submodules: module_submodules,
            };
            crate_modules.push(cairo_module_data);
        }
    }
    Ok(crate_modules)
}

/// This function extracts the file data for a given module and returns it as an `Option<FileData>`.
///
/// # Arguments
///
/// * `db` - A reference to the root database to search for the module and file data.
/// * `module_id` - The `ModuleId` of the module for which to extract file data.
///
/// # Returns
///
/// * An `Option<FileData>` representing the file data for the given module, if it exists. Returns `None` if the file is a virtual file.
pub fn get_module_file(db: &RootDatabase, module_id: ModuleId) -> Option<FileData> {
    // Get the module's files. Only gets OnDisk files, no virtual files.
    // If the module in question is not the root module of the file,
    // it returns the file that contains the module.
    let module_file_ids = db.module_files(module_id).to_option()?;
    let module_file =
        module_file_ids
            .iter()
            .find(|file_id| match db.lookup_intern_file(**file_id) {
                FileLongId::OnDisk(_) => true,
                FileLongId::Virtual(_) => false,
                FileLongId::External(_) => false,
            })?;
    match db.lookup_intern_file(*module_file) {
        FileLongId::OnDisk(path) => Some(FileData {
            id: *module_file,
            name: module_file.file_name(db),
            path,
            index: 0,
        }),
        FileLongId::Virtual(_) => None,
        FileLongId::External(_) => None,
    }
}

fn collect_submodule_declarations(
    db: &RootDatabase,
    module_id: &ModuleId,
    parent_path: &str,
) -> Vec<CairoImport> {
    let mut imports = Vec::new();
    let arc_module_declarations = db.module_submodules(*module_id).unwrap();

    let module_declarations = (*arc_module_declarations).clone();

    for (k, v) in module_declarations.iter() {
        let submodule_name = v.name(db).text(db);
        let submodule_path = format!("{}::{}", parent_path, submodule_name);
        match v.body(db) {
            MaybeModuleBody::None(_) => {
                imports.push(CairoImport {
                    name: submodule_name.to_string(),
                    path: ModulePath::new(submodule_path.clone()),
                    resolved_path: ModulePath::new(submodule_path),
                    import_type: CairoImportType::Module,
                });
            }

            MaybeModuleBody::Some(_body) => {
                let module_id = ModuleId::Submodule(*k);
                let mut declared_modules =
                    collect_submodule_declarations(db, &module_id, &submodule_path);
                imports.append(&mut declared_modules);
            }
        }
    }
    // TODO: maybe we need to make this unique via a hashmap?
    imports
}

/// This function extracts the imports from a file and returns them as a `Vec` of `CairoImport`s.
///
/// # Arguments
///
/// * `db` - A reference to the root database to search for the file and resolve imports.
/// * `module_id` - The `ModuleId` of the module containing the file for which to extract imports.
/// * `file_data` - A reference to a `FileData` struct representing the file for which to extract imports.
///
/// # Returns
///
/// * A `Vec` of `CairoImport`s representing the imports in the given file.
pub fn extract_file_imports(
    db: &RootDatabase,
    module_id: ModuleId,
    file_data: &FileData,
) -> Result<Vec<CairoImport>> {
    let mut imports: Vec<CairoImport> = vec![];
    let mut module_uses: OrderedHashMap<UseId, (UsePathLeaf, ModuleId)> = OrderedHashMap::default();

    // Attempt to capture all modules uses for this module recursively
    match capture_file_modules_uses(db, file_data.id, module_id, &mut module_uses) {
        Ok(_) => (),
        Err(err) => return Err(err),
    };

    let file_module_name = module_id.full_path(db);
    let submodules_declarations = collect_submodule_declarations(db, &module_id, &file_module_name);
    imports.extend(submodules_declarations);

    // Resolve the module's imports
    // the resolver depends on the current module file id
    let mut diagnostics = DiagnosticsBuilder::default();

    for (use_id, (use_path, use_mod_id)) in module_uses.iter() {
        // Use Path needs to break down into segments
        let mut segments = vec![];
        get_use_segments(db, &ast::UsePath::Leaf(use_path.clone()), &mut segments).unwrap();

        let import_path = segments
            .clone()
            .into_iter()
            .map(|x| x.as_syntax_node().get_text(db).trim().to_string())
            .join("::");

        // The resolver must resolve using the correct module that is importing it.
        let mut resolver = Resolver::new(
            db,
            ModuleFileId(*use_mod_id, FileIndex(0)),
            InferenceId::NoContext,
        );

        let resolved_item_maybe =
            resolver.resolve_generic_path(&mut diagnostics, segments, NotFoundItemType::Identifier);

        // If the import is resolved, get the full path
        if let Ok(resolved_item) = resolved_item_maybe {
            let full_path = get_full_path(db.upcast(), &resolved_item);
            let import_type = match resolved_item {
                ResolvedGenericItem::Module(_) => CairoImportType::Module,
                _ => CairoImportType::Other,
            };

            // Turns out there are generated code that are considered uses even
            // when there's no 'use' keyword used. These usually start with dunder (__),
            // or double underscore. We avoid those.
            // TODO: Since there is a chance where people do use dunder for their module imports
            // this will not work foolproof. Fix that!
            // We also don't want anything starting with core (which are builtins).
            if !import_path.starts_with("__") && !full_path.starts_with("core") {
                imports.push(CairoImport {
                    name: use_id.name(db).to_string(),
                    path: ModulePath::new(import_path.clone()),
                    resolved_path: ModulePath::new(full_path),
                    import_type,
                });
            }
        } else {
            return Err(anyhow!(
                "IMPORT NOT RESOLVED: {}",
                use_path.as_syntax_node().get_text(db.upcast())
            ));
        }
    }

    Ok(imports)
}

/// Returns the full path of a resolved generic item in the compiler database.
///
/// # Arguments
///
/// * `db` - A reference to the root database to search for the resolved generic item.
/// * `resolved_item` - The resolved generic item for which to retrieve the full path.
///
/// # Returns
///
/// * A `String` representing the full path of the resolved generic item if successful.
///
fn get_full_path(db: &RootDatabase, resolved_item: &ResolvedGenericItem) -> String {
    match resolved_item {
        ResolvedGenericItem::Trait(trait_id) => trait_id.full_path(db),
        ResolvedGenericItem::GenericConstant(const_id) => const_id.full_path(db),
        ResolvedGenericItem::Module(module_id) => module_id.full_path(db),
        ResolvedGenericItem::GenericFunction(generic_func_id) => {
            match generic_func_id {
                GenericFunctionId::Free(id) => id.full_path(db),
                GenericFunctionId::Extern(id) => id.full_path(db),
                GenericFunctionId::Impl(id) => {
                    //TODO figure out whether trait_id or impl_id is required here
                    id.function.full_path(db)
                }
                GenericFunctionId::Trait(concrete_trait_generic_function_id) => concrete_trait_generic_function_id.trait_function(db).full_path(db),
            }
        }
        ResolvedGenericItem::TraitFunction(trait_func_id) => trait_func_id.full_path(db),
        ResolvedGenericItem::GenericType(generic_type_id) => generic_type_id.full_path(db),
        ResolvedGenericItem::GenericTypeAlias(generic_type_alias) => {
            generic_type_alias.full_path(db)
        }
        ResolvedGenericItem::Variant(variant) => variant.enum_id.full_path(db),
        ResolvedGenericItem::Impl(impl_id) => impl_id.full_path(db),
        ResolvedGenericItem::GenericImplAlias(impl_alias) => impl_alias.full_path(db),
        ResolvedGenericItem::Variable(var_id) => {
            match var_id {
                VarId::Param(param_id) => param_id.full_path(db),
                VarId::Local(local_var_id) => local_var_id.module_file_id(db).0.full_path(db),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::{set_file_content, setup_test_files_with_imports, TestImport};
    use cairo_lang_defs::db::DefsGroup;
    use cairo_lang_filesystem::db::{CrateConfiguration, FilesGroup, FilesGroupEx};
    use cairo_lang_filesystem::ids::{CrateLongId, Directory};
    use cairo_lang_semantic::plugin::PluginSuite;
    use cairo_lang_starknet::plugin::StarkNetPlugin;

    fn setup_default_environment(
        path: &str,
        implementation: &str,
        _import_type: CairoImportType,
    ) -> Result<(RootDatabase, ModuleId, FileData, CrateId)> {
        let db = &mut RootDatabase::builder()
            .with_plugin_suite(
                PluginSuite::default()
                    .add_plugin::<StarkNetPlugin>()
                    .to_owned(),
            )
            .build()?;

        let test_import = TestImport {
            path: ModulePath::new(path),
            implementation: implementation.to_owned(),
        };
        let _crate_id = setup_test_files_with_imports(db, test_import);
        let path: PathBuf = "src/contract.cairo".into();
        let module_id = db.file_modules(FileId::new(db, path.clone())).unwrap()[0];
        let file_id = db.intern_file(FileLongId::OnDisk(path.clone()));
        let file_data = FileData {
            id: file_id,
            name: file_id.file_name(db),
            path,
            index: 0,
        };

        Ok((db.snapshot(), module_id, file_data, _crate_id))
    }

    macro_rules! assert_import_properties {
        ($import:expr, $name:expr, $resolved_path:expr, $import_type:expr) => {
            assert_eq!($import.name, $name);
            assert_eq!($import.resolved_path, ModulePath::new($resolved_path));
            assert_eq!($import.import_type, $import_type);
        };
    }

    #[test]
    fn test_extract_import_module() -> Result<(), Box<dyn std::error::Error>> {
        let (db, module_id, file_data, _) = setup_default_environment(
            "test::submod::subsubmod::mod_a",
            "mod mod_a{}",
            CairoImportType::Module,
        )?;

        let import = extract_file_imports(&db, module_id, &file_data)?[0].clone();
        assert_import_properties!(
            import,
            "mod_a",
            "test::submod::subsubmod::mod_a",
            CairoImportType::Module
        );

        Ok(())
    }

    #[test]
    fn test_extract_declared_module() -> Result<(), Box<dyn std::error::Error>> {
        let db = &mut RootDatabase::builder()
            .with_plugin_suite(
                PluginSuite::default()
                    .add_plugin::<StarkNetPlugin>()
                    .to_owned(),
            )
            .build()?;

        let crate_id = db.intern_crate(CrateLongId::Real("test".into()));
        let root = Directory::Real("src".into());
        db.set_crate_config(crate_id, Some(CrateConfiguration::default_for_root(root)));
        set_file_content(db, "src/lib.cairo", "mod submod;");
        set_file_content(db, "src/submod.cairo", "fn foo{}");
        let path: PathBuf = "src/lib.cairo".into();
        let module_id = db.file_modules(FileId::new(db, path.clone())).unwrap()[0];
        let file_id = db.intern_file(FileLongId::OnDisk(path.clone()));
        let file_data = FileData {
            id: file_id,
            name: file_id.file_name(db),
            path,
            index: 0,
        };
        let import = extract_file_imports(db, module_id, &file_data)?[0].clone();
        assert_import_properties!(import, "submod", "test::submod", CairoImportType::Module);

        Ok(())
    }

    #[test]
    fn test_extract_declared_module_nested() -> Result<()> {
        let db = &mut RootDatabase::builder()
            .with_plugin_suite(
                PluginSuite::default()
                    .add_plugin::<StarkNetPlugin>()
                    .to_owned(),
            )
            .build()?;

        let crate_id = db.intern_crate(CrateLongId::Real("test".into()));
        let root = Directory::Real("src".into());
        db.set_crate_config(crate_id, Some(CrateConfiguration::default_for_root(root)));
        set_file_content(
            db,
            "src/lib.cairo",
            "use test::submod::subsubmod::foo;\n \
        mod submod {mod subsubmod;}\n",
        );
        set_file_content(db, "src/submod/subsubmod.cairo", "fn foo{}");
        let path: PathBuf = "src/lib.cairo".into();
        let module_id = db.file_modules(FileId::new(db, path.clone())).unwrap()[0];
        let file_id = db.intern_file(FileLongId::OnDisk(path.clone()));
        let file_data = FileData {
            id: file_id,
            name: file_id.file_name(db),
            path,
            index: 0,
        };
        let imports = extract_file_imports(db, module_id, &file_data)?;
        assert_import_properties!(
            imports[0],
            "subsubmod",
            "test::submod::subsubmod",
            CairoImportType::Module
        );

        assert_import_properties!(
            imports[1],
            "foo",
            "test::submod::subsubmod::foo",
            CairoImportType::Other
        );

        Ok(())
    }

    #[test]
    fn test_extract_import_struct() -> Result<()> {
        let (db, module_id, file_data, _) = setup_default_environment(
            "test::submod::subsubmod::MyStruct",
            "struct MyStruct{}",
            CairoImportType::Other,
        )?;

        let import = extract_file_imports(&db, module_id, &file_data)?[0].clone();
        assert_import_properties!(
            import,
            "MyStruct",
            "test::submod::subsubmod::MyStruct",
            CairoImportType::Other
        );

        Ok(())
    }

    #[test]
    fn test_extract_import_trait() -> Result<()> {
        let (db, module_id, file_data, _) = setup_default_environment(
            "test::submod::subsubmod::MyTrait",
            "trait MyTrait{}",
            CairoImportType::Other,
        )?;

        let import = extract_file_imports(&db, module_id, &file_data)?[0].clone();
        assert_import_properties!(
            import,
            "MyTrait",
            "test::submod::subsubmod::MyTrait",
            CairoImportType::Other
        );

        Ok(())
    }

    #[test]
    fn test_extract_import_constant() -> Result<()> {
        let (db, module_id, file_data, _) = setup_default_environment(
            "test::submod::subsubmod::MY_CONST",
            "const MY_CONST = 10;",
            CairoImportType::Other,
        )?;

        let import = extract_file_imports(&db, module_id, &file_data)?[0].clone();
        assert_import_properties!(
            import,
            "MY_CONST",
            "test::submod::subsubmod::MY_CONST",
            CairoImportType::Other
        );
        Ok(())
    }

    #[test]
    fn test_extract_import_generic() -> Result<()> {
        let (db, module_id, file_data, _) = setup_default_environment(
            "test::submod::subsubmod::foo_generic",
            "fn foo_generic<T>(value:T){}",
            CairoImportType::Other,
        )?;

        let import = extract_file_imports(&db, module_id, &file_data)?[0].clone();
        assert_import_properties!(
            import,
            "foo_generic",
            "test::submod::subsubmod::foo_generic",
            CairoImportType::Other
        );
        Ok(())
    }
    //TODO add more tests for the rest of import types

    #[test]
    fn test_extract_crate_modules() {
        let db = &mut RootDatabase::builder()
            .with_plugin_suite(
                PluginSuite::default()
                    .add_plugin::<StarkNetPlugin>()
                    .to_owned(),
            )
            .build()
            .unwrap();

        let crate_id = setup_test_files_with_imports(
            db,
            TestImport {
                path: ModulePath::new("test::submod::subsubmod::foo"),
                implementation: "fn foo(){}".to_owned(),
            },
        );
        let actual_modules = collect_crate_module_files(db, crate_id).unwrap();

        // Only resolves modules that are files. Modules inside of files are not extracted, but their imports are
        // added to the file imports.
        let expected_modules = vec![
            CairoModule {
                dir: PathBuf::from("src"),
                main_file: PathBuf::from("src/lib.cairo"),
                path: ModulePath::new("test"),
                filepath: PathBuf::from("src/lib.cairo"),
                relative_filepath: PathBuf::from("src/lib.cairo"),
                imports: HashSet::from([
                    CairoImport {
                        name: "submod".to_owned(),
                        path: ModulePath::new("test::submod"),
                        resolved_path: ModulePath::new("test::submod"),
                        import_type: CairoImportType::Module,
                    },
                    CairoImport {
                        name: "contract".to_owned(),
                        path: ModulePath::new("test::contract"),
                        resolved_path: ModulePath::new("test::contract"),
                        import_type: CairoImportType::Module,
                    },
                ]),
                submodules: vec![],
            },
            CairoModule {
                dir: PathBuf::from("src/submod"),
                main_file: PathBuf::from("src/lib.cairo"),
                path: ModulePath::new("test::submod"),
                filepath: PathBuf::from("src/submod.cairo"),
                relative_filepath: PathBuf::from("src/lib.cairo"),
                imports: HashSet::from([CairoImport {
                    name: "subsubmod".to_owned(),
                    path: ModulePath::new("test::submod::subsubmod"),
                    resolved_path: ModulePath::new("test::submod::subsubmod"),
                    import_type: CairoImportType::Module,
                }]),
                submodules: vec![],
            },
            CairoModule {
                dir: PathBuf::from("src/submod/subsubmod"),
                main_file: PathBuf::from("src/lib.cairo"),
                path: ModulePath::new("test::submod::subsubmod"),
                filepath: PathBuf::from("src/submod/subsubmod.cairo"),
                relative_filepath: PathBuf::from("src/lib.cairo"),
                imports: Default::default(),
                submodules: vec![],
            },
            CairoModule {
                dir: PathBuf::from("src/contract"),
                main_file: PathBuf::from("src/lib.cairo"),
                path: ModulePath::new("test::contract"),
                filepath: PathBuf::from("src/contract.cairo"),
                relative_filepath: PathBuf::from("src/lib.cairo"),
                imports: HashSet::from([CairoImport {
                    name: "foo".to_owned(),
                    path: ModulePath::new("test::submod::subsubmod::foo"),
                    resolved_path: ModulePath::new("test::submod::subsubmod::foo"),
                    import_type: CairoImportType::Other,
                }]),
                submodules: vec![],
            },
        ];

        assert_eq!(
            expected_modules.len(),
            actual_modules.len(),
            "Expected {} CairoModules but got {}",
            expected_modules.len(),
            actual_modules.len()
        );

        for (expected, actual) in expected_modules.iter().zip(actual_modules.iter()) {
            assert_eq!(expected.dir, actual.dir);
            assert_eq!(expected.main_file, actual.main_file);
            assert_eq!(expected.path, actual.path);
            assert_eq!(expected.filepath, actual.filepath);
            assert_eq!(expected.imports, actual.imports);
        }
    }

    #[test]
    fn test_get_module_file() {
        let db = &mut RootDatabase::builder()
            .with_plugin_suite(
                PluginSuite::default()
                    .add_plugin::<StarkNetPlugin>()
                    .to_owned(),
            )
            .build()
            .unwrap();

        let crate_id = setup_test_files_with_imports(
            db,
            TestImport {
                path: ModulePath::new("test::submod::subsubmod::foo"),
                implementation: "fn foo(){}".to_owned(),
            },
        );
        let crate_modules = db.crate_modules(crate_id);
        let module_id = crate_modules[1];
        let file_data = get_module_file(db, module_id).unwrap();
        assert_eq!(file_data.name, "submod.cairo");
        assert_eq!(file_data.index, 0);
        assert_eq!(file_data.path.to_str().unwrap(), "src/submod.cairo");
    }

    #[test]
    fn test_module_submodules_should_extract_single_level_submodules() {
        let db = &mut RootDatabase::builder()
            .with_plugin_suite(
                PluginSuite::default()
                    .add_plugin::<StarkNetPlugin>()
                    .to_owned(),
            )
            .build()
            .unwrap();

        let crate_id = setup_test_files_with_imports(
            db,
            TestImport {
                path: ModulePath::new("test::submod::subsubmod::inlinemod::foo"),
                implementation: "
                mod inlinemod {
                    fn foo(){}
                }
                "
                .to_owned(),
            },
        );
        let crate_modules = db.crate_modules(crate_id);
        // The second modules found will be the subsubmod.cairo file.
        let module_id = crate_modules[2];
        let file_data = get_module_file(db, module_id).unwrap();

        let submodules = capture_modules_submodules(db, file_data.id, module_id).unwrap();
        assert_eq!(submodules.len(), 1);
        assert_eq!(
            submodules[0],
            CairoSubmodules {
                name: "inlinemod".to_string(),
                path: ModulePath::new("test::submod::subsubmod::inlinemod"),
                parent_path: ModulePath::new("test::submod::subsubmod")
            }
        )
    }

    #[test]
    fn test_module_submodules_should_extract_multi_level_submodules() {
        let db = &mut RootDatabase::builder()
            .with_plugin_suite(
                PluginSuite::default()
                    .add_plugin::<StarkNetPlugin>()
                    .to_owned(),
            )
            .build()
            .unwrap();

        let crate_id = setup_test_files_with_imports(
            db,
            TestImport {
                path: ModulePath::new("test::submod::subsubmod::inlinemod::inline2mod::foo"),
                implementation: "
                mod inlinemod {
                    mod inline2mod {
                        fn foo(){}
                    }
                    mod inline3mod {
                        fn bar(){}
                    }
                }
                "
                .to_owned(),
            },
        );
        let crate_modules = db.crate_modules(crate_id);
        // The second modules found will be the subsubmod.cairo file.
        let module_id = crate_modules[2];
        let file_data = get_module_file(db, module_id).unwrap();

        let submodules = capture_modules_submodules(db, file_data.id, module_id).unwrap();
        assert_eq!(submodules.len(), 3);
        assert_eq!(
            submodules[1],
            CairoSubmodules {
                name: "inline2mod".to_string(),
                path: ModulePath::new("test::submod::subsubmod::inlinemod::inline2mod"),
                parent_path: ModulePath::new("test::submod::subsubmod::inlinemod")
            }
        );
        assert_eq!(
            submodules[2],
            CairoSubmodules {
                name: "inline3mod".to_string(),
                path: ModulePath::new("test::submod::subsubmod::inlinemod::inline3mod"),
                parent_path: ModulePath::new("test::submod::subsubmod::inlinemod")
            }
        );
    }
}
