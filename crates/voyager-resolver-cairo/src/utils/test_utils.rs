use cairo_lang_filesystem::ids::{CrateId, CrateLongId, Directory, FileLongId};
use std::collections::HashSet;
use std::path::PathBuf;

use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_filesystem::db::FilesGroup;
use cairo_lang_filesystem::db::{AsFilesGroupMut, FilesGroupEx};
use std::sync::Arc;

use crate::model::{CairoImport, CairoImportType, CairoModule, ModulePath};

/// Struct representing an import for tests - the path (module) of the item being imported and the implementation of the item.
pub struct TestImport {
    pub path: ModulePath,
    pub implementation: String,
}

/// Sets up test files for the given database and import to test
/// Creates the `src/lib.cairo` file, the `src/contract.cairo` file
/// and the `src/submod.cairo` file.
/// The `src/contract.cairo` file imports the given `test_import` path.
/// The `src/submod.cairo` file defines the given `test_import` implementation.
pub fn setup_test_files_with_imports(db: &mut RootDatabase, test_import: TestImport) -> CrateId {
    let crate_id = db.intern_crate(CrateLongId::Real("test".into()));
    let root = Directory::Real("src".into());
    db.set_crate_root(crate_id, Some(root));

    // Main module file
    set_file_content(db, "src/lib.cairo", "mod submod;\n mod contract;");

    // Contract module file
    set_file_content(
        db,
        "src/contract.cairo",
        &format!(
            "
            #[contract]
            mod ERC20 {{
                use {path};
            }}
            ",
            path = test_import.path
        ),
    );

    // Submod and subsubmod module files
    set_file_content(db, "src/submod.cairo", "mod subsubmod;");
    set_file_content(
        db,
        "src/submod/subsubmod.cairo",
        &format!(
            "
            {implementation}
            ",
            implementation = &test_import.implementation
        ),
    );
    crate_id
}

/// Sets the content of the file with the given path
pub fn set_file_content(db: &mut RootDatabase, path: &str, content: &str) {
    let file_id = db.intern_file(FileLongId::OnDisk(path.into()));
    db.as_files_group_mut()
        .override_file_content(file_id, Some(Arc::new(content.to_owned())));
}

pub fn setup_simple_modules() -> Vec<CairoModule> {
    let module_0 = CairoModule {
        dir: PathBuf::from("src"),
        main_file: PathBuf::from("src/lib.cairo"),
        path: ModulePath::new("test"),
        filepath: PathBuf::from("src/lib.cairo"),
        relative_filepath: PathBuf::from("lib.cairo"),
        imports: Default::default(),
        submodules: vec![],
    };

    let module_1 = CairoModule {
        dir: PathBuf::from("src/submod"),
        main_file: PathBuf::from("src/lib.cairo"),
        path: ModulePath::new("test::submod"),
        filepath: PathBuf::from("src/submod.cairo"),
        relative_filepath: PathBuf::from("submod.cairo"),
        imports: Default::default(),
        submodules: vec![],
    };

    let module_2 = CairoModule {
        dir: PathBuf::from("src/submod/subsubmod"),
        main_file: PathBuf::from("src/lib.cairo"),
        path: ModulePath::new("test::submod::subsubmod"),
        filepath: PathBuf::from("src/submod/subsubmod.cairo"),
        relative_filepath: PathBuf::from("subsubmod.cairo"),
        imports: Default::default(),
        submodules: vec![],
    };

    let module_3 = CairoModule {
        dir: PathBuf::from("src/submod/subsubmod"),
        main_file: PathBuf::from("src/lib.cairo"),
        path: ModulePath::new("test::contract"),
        filepath: PathBuf::from("src/contract.cairo"),
        relative_filepath: PathBuf::from("contract.cairo"),
        imports: HashSet::from([CairoImport {
            name: "foo".to_owned(),
            path: ModulePath::new("test::submod::subsubmod::foo"),
            resolved_path: ModulePath::new("test::submod::subsubmod::foo"),
            import_type: CairoImportType::Other,
        }]),
        submodules: vec![],
    };

    let modules = vec![module_0, module_1, module_2, module_3];
    modules
}
