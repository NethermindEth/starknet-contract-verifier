use anyhow::{Context, Result};
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_defs::ids::ModuleId;
use cairo_lang_filesystem::db::FilesGroup;
use cairo_lang_filesystem::ids::{CrateId, CrateLongId, FileLongId};
use camino::Utf8PathBuf;
use std::collections::HashMap;
use std::path::PathBuf;

use cairo_lang_defs::db::DefsGroup;
use cairo_lang_diagnostics::ToOption;
use cairo_lang_utils::Upcast;
use petgraph::Graph;

use crate::compiler::queries::collect_crate_module_files;
use crate::model::{CairoAttachmentModule, CairoCrate, CairoModule, ModulePath};

use crate::utils::{
    copy_required_files, create_attachment_files, generate_attachment_module_data,
    get_import_remaps, run_scarb_build,
};

use crate::compiler::scarb_utils::{
    generate_scarb_updated_files, get_contracts_to_verify, read_scarb_metadata,
    update_crate_roots_from_metadata,
};
use crate::graph::{create_graph, get_required_module_for_contracts, EdgeWeight};
// use crate::graph::display_graphviz;
use scarb::compiler::{CairoCompilationUnit, CompilationUnitAttributes, Compiler};
use scarb::core::{TargetKind, Workspace};
use scarb::flock::Filesystem;

pub struct VoyagerGenerator;

pub mod queries;
pub mod scarb_utils;

impl Compiler for VoyagerGenerator {
    fn target_kind(&self) -> TargetKind {
        TargetKind::STARKNET_CONTRACT
    }

    /// This function does not actually compile the code. Rather, it extracts
    /// information about the project's crates, modules, and dependencies to perform various tasks related
    /// to verifying and copying the Cairo code. These tasks include:
    ///
    /// - Building the project configuration, overriding it with the local corelib.
    /// - Collecting the main crate IDs from the compilation unit and the compiler database, after updating it with Scarb's metadata.
    /// - Extracting a vector of `CairoCrate` structs, which contain the crate root directory, main file, and modules for each crate in the project.
    /// - Creating a graph where nodes are Cairo modules, and edges are the dependencies between modules.
    /// - Reading the Scarb manifest file to get the list of contracts to verify from the [tool.voyager] section.
    /// - Finding all module dependencies for the contracts to verify using the graph.
    /// - Finding all "attachment" modules for the required modules. Attachment modules are modules that attach other modules to the module tree.
    /// - Creating the attachment files for these modules.
    /// - Getting the Cairo Modules corresponding to the required modules paths and copying these modules in the target directory.
    /// - Generating the Scarb manifest files for the output directory, updating the dependencies to include the required modules as local dependencies.
    ///
    /// # Arguments
    ///
    /// * `unit` - The `CompilationUnit` containing the Cairo code to compile.
    /// * `ws` - The `Workspace` containing the project.
    ///
    /// # Errors
    ///
    /// This function returns an error if any of the following occur:
    ///
    /// - The target has parameters.
    /// - There is a problem getting the corelib path.
    /// - There is a problem building the project configuration.
    /// - There is a problem reading the Scarb metadata from the manifest file.
    /// - There is a problem extracting the modules from a crate.
    /// - There is a problem creating the graph.
    /// - There is a problem getting the contracts to verify.
    /// - There is a problem creating the attachment files.
    /// - There is a problem copying the required modules to the target directory.
    /// - There is a problem generating the Scarb manifest files for the output directory.
    fn compile(
        &self,
        unit: CairoCompilationUnit,
        db: &mut RootDatabase,
        ws: &Workspace<'_>,
    ) -> Result<()> {
        // TODO: Do we still need this check?!
        // Get the properties of the target to ensure it has no parameters
        // As our custom compiler target is starknet-contracts by default.
        // let props = unit.target.kind.downcast::<ExternalTargetKind>();
        //ensure!(
        //    props.params.is_empty(),
        //    "target `{}` does not accept any parameters",
        //    props.kind_name
        //);

        //let mut db = RootDatabase::builder()
        //    .with_project_config(config.clone())
        //    .with_starknet()
        //    .build()?;

        // We can use scarb metadata to update crate root info with external dependencies,
        // This updates the compiler database with the crate roots of the external dependencies.
        // This enables resolving external dependencies paths.
        let manifest_path: PathBuf = unit.main_component().package.manifest_path().into();
        let metadata = read_scarb_metadata(&manifest_path)
            .expect("Failed to obtain scarb metadata from manifest file.");
        update_crate_roots_from_metadata(db, metadata.clone());

        // We need all crate ids different than `core`
        let project_crate_ids = unit
            .components
            .iter()
            .filter(|component| component.cairo_package_name() != "core")
            .map(|component| db.intern_crate(CrateLongId::Real(component.cairo_package_name())))
            .collect();

        // Get a vector of CairoCrate, which contain the crate root directory, main file and modules for each crate in the project.
        let project_crates = self.get_project_crates(db, project_crate_ids)?;

        // Collect all modules from all crates in the project.
        let project_modules = project_crates
            .iter()
            .flat_map(|c| c.modules.iter().cloned())
            .collect::<Vec<CairoModule>>();

        // Creates a graph where nodes are cairo modules, and edges are the dependencies between modules.
        let graph = create_graph(&project_modules);
        // println!("Graph is:\n");
        // display_graphviz(&graph);

        // Read Scarb manifest file to get the list of contracts to verify from the [tool.voyager] section.
        // This returns the relative file paths of the contracts to verify.
        let contracts_to_verify = get_contracts_to_verify(&unit.main_component().package)?;

        // Collect the CairoModule corresponding to the file paths of the contracts.
        let modules_to_verify = project_modules
            .iter()
            .filter(|m| contracts_to_verify.contains(&m.relative_filepath))
            .collect::<Vec<_>>();

        let (required_modules_paths, attachment_modules_data) =
            self.get_reduced_project(&graph, modules_to_verify)?;

        let target_dir = Utf8PathBuf::from(
            manifest_path
                .parent()
                .unwrap()
                .join("voyager-verify")
                .to_str()
                .unwrap(),
        );

        // Clean directory
        std::fs::remove_dir_all(target_dir.clone()).unwrap_or_else(|e| {
            if e.kind() != std::io::ErrorKind::NotFound {
                panic!(
                    "Error removing target directory \"{}\" caused by:\n{}",
                    target_dir, e
                )
            }
        });

        // treat target dir as a Filesystem
        let target_dir = Filesystem::new(target_dir);

        create_attachment_files(&attachment_modules_data, &target_dir)
            .with_context(|| "Failed to create attachment files")?;

        // Get the Cairo Modules corresponding to the required modules paths.
        let required_modules = project_modules
            .iter()
            .filter(|m| required_modules_paths.contains(&m.path))
            .collect::<Vec<_>>();
        // Copy these modules in the target directory.
        // Copy readme files and license files over too
        copy_required_files(&required_modules, &target_dir, ws)?;

        // Generate each of the scarb manifest files for the output directory.
        // The dependencies are updated to include the required modules as local dependencies.
        generate_scarb_updated_files(metadata, &target_dir, required_modules)?;

        let package_name = unit.main_component().package.id.name.to_string();
        let generated_crate_dir = target_dir.path_existent().unwrap().join(package_name);
        //Locally run scarb build to make sure that everything compiles correctly before sending the files to voyager.
        run_scarb_build(generated_crate_dir.as_str())?;

        Ok(())
    }
}

impl VoyagerGenerator {
    /// Gets a vector of `CairoCrate` structs, given a database and a list of crate IDs
    ///
    /// # Arguments
    /// * `db` - a mutable reference to the root database
    /// * `project_crate_ids` - a vector of `CrateId` values representing the IDs of the crates to be retrieved
    ///
    /// # Returns
    /// `Result<Vec<CairoCrate>>` - a vector of `CairoCrate` structs.
    /// Returns `Ok` with the vector if the operation is successful, otherwise returns `Err`.
    fn get_project_crates(
        &self,
        db: &mut RootDatabase,
        project_crate_ids: Vec<CrateId>,
    ) -> Result<Vec<CairoCrate>> {
        let project_crates = project_crate_ids
            .iter()
            .map(|crate_id| -> Result<CairoCrate> {
                let crate_id = *crate_id;

                // Get the root directory and main file (lib.cairo) for the crate.
                // The main file is expected to be an OnDisk file.
                let defs_db = db.upcast();
                let crate_root_dir = defs_db
                    .crate_config(crate_id)
                    .expect(
                        format!(
                            "Failed to get crate root directory for crate ID {:?}",
                            crate_id
                        )
                        .as_str(),
                    )
                    .root;

                let main_file = defs_db
                    .module_main_file(ModuleId::CrateRoot(crate_id))
                    .to_option()
                    .with_context(|| {
                        format!("Failed to get main file for crate ID {:?}", crate_id)
                    })?;
                let main_file_path = match db.lookup_intern_file(main_file) {
                    FileLongId::OnDisk(path) => path,
                    FileLongId::Virtual(_) => panic!("Expected OnDisk file."),
                };

                // Extract a vector of modules for the crate.
                // We only collect "file" modules, which are related to a Cairo file.
                // Internal modules are not collected here.
                let crate_modules = collect_crate_module_files(db, crate_id)?;
                Ok(CairoCrate {
                    root_dir: crate_root_dir,
                    main_file: main_file_path,
                    modules: crate_modules,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(project_crates)
    }

    /// Given the module graph and a list of module contracts to verify,
    /// returns the path of the modules required for compilation, and the data for the attachment modules.
    /// Attachment modules are modules that declare submodules, attaching them to the module tree.
    /// These modules don't have content that is required by the contracts to verify,
    /// but they are required for compilation as they structure the module tree.
    ///
    /// # Arguments
    /// * `graph` - a reference to the graph containing the module dependencies
    /// * `modules_to_verify` - a vector of references to `CairoModule` structs that need to be verified
    ///
    /// # Returns
    /// `Result<(Vec<ModulePath>, HashMap<ModulePath, CairoAttachmentModule>)>` - a tuple containing a vector of module paths and a hash map of module paths to `CairoAttachmentModule` structs.
    /// Returns `Ok` with the tuple if the operation is successful, otherwise returns `Err`.
    pub fn get_reduced_project(
        &self,
        graph: &Graph<ModulePath, EdgeWeight>,
        modules_to_verify: Vec<&CairoModule>,
    ) -> Result<(Vec<ModulePath>, HashMap<ModulePath, CairoAttachmentModule>)> {
        // Using the graph, find all module dependencies for the contracts to verify.
        // Modules forward declarations are not included in the dependencies.
        let required_modules_paths = get_required_module_for_contracts(graph, &modules_to_verify)?;

        // Find all "attachment" modules for the required modules. Attachment modules are modules that forward declare submodules,
        // attaching them to the module tree.
        // We then create the attachment files for these modules.
        let imports_path_not_matching_resolved_path = get_import_remaps(modules_to_verify);

        let attachment_modules_data = generate_attachment_module_data(
            &required_modules_paths,
            imports_path_not_matching_resolved_path,
        );

        let unrequired_attachment_modules: HashMap<ModulePath, CairoAttachmentModule> =
            attachment_modules_data
                .iter()
                .filter(|(k, _)| !required_modules_paths.contains(k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
        Ok((required_modules_paths, unrequired_attachment_modules))
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::queries::collect_crate_module_files;
    use crate::compiler::VoyagerGenerator;
    use crate::graph::create_graph;
    use crate::model::{CairoAttachmentModule, ModulePath};
    use crate::utils::test_utils::set_file_content;
    use cairo_lang_compiler::db::RootDatabase;
    use cairo_lang_filesystem::db::{CrateConfiguration, FilesGroup, FilesGroupEx};
    use cairo_lang_filesystem::ids::{CrateLongId, Directory};
    use cairo_lang_semantic::plugin::PluginSuite;
    use cairo_lang_starknet::plugin::StarkNetPlugin;
    use std::collections::HashSet;
    use std::path::PathBuf;

    #[test]
    fn test_reduced_project_no_remap() {
        let db = &mut RootDatabase::builder()
            .with_plugin_suite(
                PluginSuite::default()
                    .add_plugin::<StarkNetPlugin>()
                    .to_owned(),
            )
            .build()
            .unwrap();

        let crate_id = db.intern_crate(CrateLongId::Real("test".into()));
        let root = Directory::Real("src".into());
        db.set_crate_config(crate_id, Some(CrateConfiguration::default_for_root(root)));

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
                path = ModulePath::new("test::submod::subsubmod::foo")
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
                implementation = "fn foo(){}".to_owned(),
            ),
        );

        let modules = collect_crate_module_files(db, crate_id).unwrap();
        let graph = create_graph(&modules);
        let contracts_to_verify = vec![PathBuf::from("contract.cairo")];
        // Map the relative file paths to the module paths inside the crate.
        let modules_to_verify = modules
            .iter()
            .filter(|m| contracts_to_verify.contains(&m.relative_filepath))
            .collect::<Vec<_>>();

        let voyager_compiler = VoyagerGenerator {};
        let (required_modules_paths, _) = voyager_compiler
            .get_reduced_project(&graph, modules_to_verify)
            .unwrap();
        assert_eq!(required_modules_paths.len(), 2);
        assert_eq!(required_modules_paths[0], ModulePath::new("test::contract"));
        assert_eq!(
            required_modules_paths[1],
            ModulePath::new("test::submod::subsubmod")
        )
    }

    #[test]
    fn test_reduced_project_with_remap() {
        let db = &mut RootDatabase::builder()
            .with_plugin_suite(
                PluginSuite::default()
                    .add_plugin::<StarkNetPlugin>()
                    .to_owned(),
            )
            .build()
            .unwrap();

        let crate_id = db.intern_crate(CrateLongId::Real("test".into()));

        let root = Directory::Real("src".into());
        db.set_crate_config(crate_id, Some(CrateConfiguration::default_for_root(root)));

        // Main module file
        set_file_content(
            db,
            "src/lib.cairo",
            "mod submod;\n mod contract\n; \
    use submod::subsubmod::foo;",
        );

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
                path = ModulePath::new("test::foo")
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
                implementation = "fn foo(){}".to_owned(),
            ),
        );

        let modules = collect_crate_module_files(db, crate_id).unwrap();
        let graph = create_graph(&modules);
        let contracts_to_verify = vec![PathBuf::from("contract.cairo")];
        // Map the relative file paths to the module paths inside the crate.
        let modules_to_verify = modules
            .iter()
            .filter(|m| contracts_to_verify.contains(&m.relative_filepath))
            .collect::<Vec<_>>();

        let voyager_compiler = VoyagerGenerator {};
        let (required_modules_paths, attachment_modules_data) = voyager_compiler
            .get_reduced_project(&graph, modules_to_verify)
            .unwrap();

        // contract.cairo depends on test::submod::submod(::foo)
        assert_eq!(required_modules_paths.len(), 2);
        assert_eq!(required_modules_paths[0], ModulePath::new("test::contract"));
        assert_eq!(
            required_modules_paths[1],
            ModulePath::new("test::submod::subsubmod")
        );

        // lib.cairo imports test::submod::subsubmod::foo and makes it available in the root module
        assert_eq!(attachment_modules_data.len(), 2);
        assert_eq!(
            attachment_modules_data
                .get(&ModulePath::new("test"))
                .unwrap(),
            &CairoAttachmentModule {
                path: ModulePath::new("test"),
                children: HashSet::from([ModulePath::new("submod"), ModulePath::new("contract")]),
                imports: HashSet::from([ModulePath::new("test::submod::subsubmod::foo")]),
            }
        );

        assert_eq!(
            attachment_modules_data
                .get(&ModulePath::new("test::submod"))
                .unwrap(),
            &CairoAttachmentModule {
                path: ModulePath::new("test::submod"),
                children: HashSet::from([ModulePath::new("subsubmod")]),
                imports: HashSet::new(),
            }
        );
    }

    #[test]
    fn test_reduced_project_import_from_attachment() {
        let db = &mut RootDatabase::builder()
            .with_plugin_suite(
                PluginSuite::default()
                    .add_plugin::<StarkNetPlugin>()
                    .to_owned(),
            )
            .build()
            .unwrap();

        let crate_id = db.intern_crate(CrateLongId::Real("test".into()));
        let root = Directory::Real("src".into());
        db.set_crate_config(crate_id, Some(CrateConfiguration::default_for_root(root)));

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
                path = ModulePath::new("test::submod::foo")
            ),
        );

        // Submod and subsubmod module files
        set_file_content(db, "src/submod.cairo", "mod subsubmod;\n fn foo(){}");
        set_file_content(db, "src/submod/subsubmod.cairo", "");

        let modules = collect_crate_module_files(db, crate_id).unwrap();
        let graph = create_graph(&modules);
        let contracts_to_verify = vec![PathBuf::from("contract.cairo")];
        // Map the relative file paths to the module paths inside the crate.
        let modules_to_verify = modules
            .iter()
            .filter(|m| contracts_to_verify.contains(&m.relative_filepath))
            .collect::<Vec<_>>();

        let voyager_compiler = VoyagerGenerator {};
        let (required_modules_paths, _attachment_modules_data) = voyager_compiler
            .get_reduced_project(&graph, modules_to_verify)
            .unwrap();

        // Here, subsubmod is required because we import a content from its parent "submod".
        // Therefore, the whole `submod` file is required.
        assert_eq!(required_modules_paths.len(), 3);
        assert_eq!(required_modules_paths[0], ModulePath::new("test::contract"));
        assert_eq!(required_modules_paths[1], ModulePath::new("test::submod"));
        assert_eq!(
            required_modules_paths[2],
            ModulePath::new("test::submod::subsubmod")
        );
    }
}
