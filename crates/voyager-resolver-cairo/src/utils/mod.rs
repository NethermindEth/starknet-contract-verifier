use anyhow::{anyhow, Result};
use camino::Utf8Path;

use crate::compiler::scarb_utils::read_additional_scarb_manifest_metadata;
use crate::model::{CairoAttachmentModule, CairoImport, CairoModule, ModulePath};

use scarb::core::Workspace;
use std::collections::HashMap;

use std::fs;
use std::fs::{copy, File};
use std::io::{BufWriter, Write};
use std::ops::Add;
use std::path::Path;
use std::process::Command;

#[cfg(any(feature = "testing", test))]
pub mod test_utils;

/// Finds the modules of the crate that attach other modules to the module tree.
/// # Arguments
///
/// * `required_modules` - A `Vec` of `String`s containing the paths of the required modules for the project.
/// # Returns
///
/// A `HashMap` where the keys are the parent modules and the values are `HashSet`s containing the names of the child modules.
///
/// # Example
/// Let `test::submod::contract` be a required module for our compilation.
/// This function will return a HashMap with entries `test -> [submod], submod -> [contract]`
pub fn generate_attachment_module_data(
    required_modules: &Vec<ModulePath>,
    remapped_imports: Vec<CairoImport>,
) -> HashMap<ModulePath, CairoAttachmentModule> {
    let mut declaration_modules: HashMap<ModulePath, CairoAttachmentModule> = HashMap::new();

    for required_module in required_modules {
        let required_parts: Vec<&str> = required_module.get_modules();
        let mut parent_module = String::new();

        for i in 0..required_parts.len() - 1 {
            if i > 0 {
                parent_module.push_str("::");
            }
            parent_module.push_str(required_parts[i]);
            let parent_module = ModulePath::new(parent_module.clone());

            declaration_modules
                .entry(parent_module.clone())
                .or_insert(CairoAttachmentModule::new(parent_module.clone()))
                .add_child(ModulePath::new(required_parts[i + 1].to_string()));
        }
    }

    remapped_imports.iter().for_each(|i| {
        let parent_module = i.unresolved_parent_module();
        declaration_modules
            .entry(parent_module.clone())
            .or_insert(CairoAttachmentModule::new(parent_module))
            .add_import(i.resolved_path.clone());
    });

    declaration_modules
}

pub fn get_import_remaps(modules_to_verify: Vec<&CairoModule>) -> Vec<CairoImport> {
    let all_imports = modules_to_verify
        .iter()
        .map(|m| m.imports.clone())
        .collect::<Vec<_>>();
    let imports_path_not_matching_resolved_path = all_imports
        .iter()
        .flat_map(|i| i.iter())
        .filter(|i| i.is_remapped())
        .cloned()
        .collect::<Vec<_>>();
    imports_path_not_matching_resolved_path
}

/// Generate .cairo files for each attachment module in the `attachment_modules` HashMap, writing `mod` declaration statements
/// for each submodule of the parent.
///
/// # Arguments
///
/// * `declaration_modules` - A `HashMap<String, HashSet<String>>` containing the parent modules and the forward declarations of child modules.
/// * `target_dir` - The directory in which to generate the .cairo files.
pub fn create_attachment_files(
    attachment_modules: &HashMap<ModulePath, CairoAttachmentModule>,
    target_dir: &Utf8Path,
) -> Result<()> {
    for (parent_module, attachment_module) in attachment_modules {
        let child_modules = &attachment_module.children;
        let mut filename = String::new();
        let path_split = parent_module.0.split("::");
        let crate_name = parent_module.get_crate();
        if !target_dir.exists() {
            return Err(anyhow!("failed to create attachment files"))
        }
        let source_path = target_dir.join(crate_name).join("src");
        filename = match path_split.clone().count() {
            1 => "lib.cairo".to_string(),
            _ => {
                for part in path_split {
                    filename = part.to_string().add(".cairo")
                }
                filename
            }
        };

        let dest_path = Path::new(&source_path).join(filename);
        // Create the parent directories for the destination file if they don't exist
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = File::create(dest_path)?;
        let mut writer = BufWriter::new(file);

        for child_module in child_modules {
            writeln!(writer, "mod {};", child_module)?;
        }

        for import in &attachment_module.imports {
            writeln!(writer, "use {};", import)?;
        }
    }

    Ok(())
}

/// Copies the required Cairo modules' files to the target directory.
///
/// # Arguments
///
/// * `required_modules` - A vector containing references to the CairoModule instances that need to be copied.
/// * `target_dir` - A reference to the target directory where the files should be copied.
/// * `ws` - A reference to the Workspace.
///
/// # Errors
///
/// Returns an error if the function encounters an error while copying files or creating directories.
/// TODO: add comprehensive test for this.
pub fn copy_required_files(
    required_modules: &Vec<&CairoModule>,
    target_dir: &Utf8Path,
    ws: &Workspace,
) -> Result<()> {
    let root_path = ws.root();
    let mut root_parts = root_path.components().peekable();
    if !target_dir.exists() {
        return Err(anyhow!("unable to resolve target dir"));
    }

    // Skip the first component if it is the Windows or Unix root
    if let Some(component) = root_parts.peek() {
        if component.as_os_str().is_empty() {
            root_parts.next();
        }
    }

    // Copy each required module's .cairo file to the target directory
    for module in required_modules {
        let crate_name = module.path.get_crate();
        let filepath = Path::new(&module.filepath);
        let mut filepath_parts = filepath.components().peekable();

        // Skip the first component if it is the project root
        if let Some(component) = filepath_parts.peek() {
            if component.as_os_str() == root_path {
                filepath_parts.next();
            }
        }

        // Construct the destination path for the .cairo & readme & license file
        let root_dir = module.get_root_dir()?;
        let filepath_relative = filepath.strip_prefix(root_dir.clone())?;
        let dest_path = Path::new(target_dir)
            .join(crate_name.clone())
            .join(filepath_relative);

        let manifest_path = root_dir.clone().join("Scarb.toml");
        let scarb_toml_content = fs::read_to_string(&manifest_path)?;
        let additional_metadata =
            read_additional_scarb_manifest_metadata(scarb_toml_content.as_str())?;

        // Create the parent directories for the destination file if they don't exist
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Copy the .cairo file to the destination path
        let source_path = Path::new(&module.filepath);
        copy(source_path, &dest_path)?;

        // Attempt to copy the readme and license files to the target directory
        let root_dir_clone = root_dir.clone();
        let base_source_root_path = Path::new(&root_dir_clone);
        let base_dest_root_path = Path::new(target_dir).join(crate_name);

        let readme_source_path = base_source_root_path.join(&additional_metadata.readme);
        let readme_dest_path = base_dest_root_path.join(&additional_metadata.readme);
        let license_file_source_path =
            base_source_root_path.join(&additional_metadata.license_file);
        let license_file_dest_path = base_dest_root_path.join(&additional_metadata.license_file);

        // Only copy if there is a readme or license file
        if !additional_metadata.readme.is_empty() && readme_source_path.exists() {
            copy(readme_source_path, readme_dest_path)?;
        }

        if !additional_metadata.license_file.is_empty() && license_file_source_path.exists() {
            copy(license_file_source_path, license_file_dest_path)?;
        }
    }

    Ok(())
}

pub fn run_scarb_build(path: &str) -> Result<()> {
    let output = Command::new("scarb")
        .arg("build")
        .current_dir(path)
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        panic!("{}", String::from_utf8_lossy(&output.stdout));
    }
}

pub fn run_starknet_compile(path: &str) -> Result<()> {
    let output = Command::new("starknet-compile").arg(path).output()?;

    if output.status.success() {
        Ok(())
    } else {
        panic!("{}", String::from_utf8_lossy(&output.stderr));
    }
}

#[cfg(test)]
mod tests {
    use crate::model::ModulePath;
    use crate::utils::generate_attachment_module_data;

    #[test]
    fn test_find_attachment_modules() {
        let required_modules = vec![
            ModulePath::new("test::module::child"),
            ModulePath::new("test::module::child2"),
            ModulePath::new("test::module2::child"),
            ModulePath::new("test::module2::child2"),
            ModulePath::new("test::module2::child3"),
            ModulePath::new("test::module3"),
        ];

        let declaration_modules = generate_attachment_module_data(&required_modules, vec![]);

        assert_eq!(declaration_modules.len(), 3);
        assert_eq!(
            declaration_modules
                .get(&ModulePath::new("test"))
                .unwrap()
                .children
                .len(),
            3
        );
        assert_eq!(
            declaration_modules
                .get(&ModulePath::new("test::module"))
                .unwrap()
                .children
                .len(),
            2
        );
        assert_eq!(
            declaration_modules
                .get(&ModulePath::new("test::module2"))
                .unwrap()
                .children
                .len(),
            3
        );
    }
}
