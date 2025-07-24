# Dojo Toolchain Support Implementation Plan

## Overview

This document outlines the plan to add Dojo toolchain support to the Starknet contract verifier. The main difference between regular Scarb projects and Dojo projects is the build command:
- **Regular Scarb projects**: Use `scarb build` 
- **Dojo projects**: Use `sozo build`

## Current Architecture

The contract verifier currently:
1. Collects project metadata using Scarb's metadata command
2. Gathers source files and dependencies  
3. Sends project information to a remote API for verification
4. The remote API compiles the project using `scarb build`

## Required Changes

### 1. Project Type Detection & Selection

**Location**: `src/args.rs`
- Add a new enum `ProjectType` to distinguish between Scarb and Dojo projects
- Add CLI argument `--project-type` with options: `scarb`, `dojo`, `auto`
- When `auto` is selected (default), implement interactive prompt for user selection
- Add validation to ensure Dojo projects have proper dependencies

**Implementation**:
```rust
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ProjectType {
    /// Regular Scarb project (uses scarb build)
    Scarb,
    /// Dojo project (uses sozo build) 
    Dojo,
    /// Auto-detect project type with interactive prompt
    Auto,
}
```

**Enhanced CLI Integration**:
```rust
// Add to VerifyArgs struct around line 295
/// Project type for build tool selection
#[arg(
    long = "project-type",
    value_enum,
    default_value_t = ProjectType::Auto,
    help = "Specify the project type (scarb, dojo, or auto-detect)"
)]
pub project_type: ProjectType,
```

### 2. Enhanced Dojo Project Detection

**Location**: `src/args.rs` (new functions)
- Implement automatic Dojo project detection logic
- Check for Dojo-specific dependencies in Scarb.toml
- Validate project structure for Dojo compatibility

**Implementation**:
```rust
impl Project {
    /// Detect if this is a Dojo project by analyzing dependencies
    pub fn detect_project_type(&self) -> Result<ProjectType, ProjectError> {
        let metadata = self.metadata();
        
        // Check for dojo-core dependency in any package
        for package in &metadata.packages {
            if let Some(dependencies) = &package.dependencies {
                for dep in dependencies {
                    if dep.name == "dojo_core" || dep.name == "dojo-core" {
                        return Ok(ProjectType::Dojo);
                    }
                }
            }
        }
        
        // Check for dojo namespace imports in source files
        if self.has_dojo_imports()? {
            return Ok(ProjectType::Dojo);
        }
        
        // Default to Scarb if no Dojo indicators found
        Ok(ProjectType::Scarb)
    }
    
    /// Check if source files contain Dojo-specific imports
    fn has_dojo_imports(&self) -> Result<bool, ProjectError> {
        use std::fs;
        use walkdir::WalkDir;
        
        let root = self.root_dir();
        let src_dir = root.join("src");
        
        if !src_dir.exists() {
            return Ok(false);
        }
        
        for entry in WalkDir::new(src_dir).into_iter().filter_map(|e| e.ok()) {
            if entry.path().extension().and_then(|s| s.to_str()) == Some("cairo") {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if content.contains("use dojo::") || content.contains("dojo::")
                        || content.contains("#[dojo::") {
                        return Ok(true);
                    }
                }
            }
        }
        
        Ok(false)
    }
}
```

### 3. Interactive Project Type Selection

**Location**: `src/main.rs`
- Implement interactive prompt using `dialoguer` crate
- Display clear information about project types
- Default to Scarb for backward compatibility
- Only prompt when `--project-type=auto` is used and detection is ambiguous

**Implementation**:
```rust
use dialoguer::Select;

fn determine_project_type(args: &VerifyArgs) -> Result<ProjectType, CliError> {
    match args.project_type {
        ProjectType::Scarb => Ok(ProjectType::Scarb),
        ProjectType::Dojo => {
            // Validate that this is actually a Dojo project
            validate_dojo_project(&args.path)?;
            Ok(ProjectType::Dojo)
        },
        ProjectType::Auto => {
            // Try automatic detection first
            match args.path.detect_project_type()? {
                ProjectType::Dojo => {
                    info!("Detected Dojo project automatically");
                    Ok(ProjectType::Dojo)
                },
                ProjectType::Scarb => {
                    info!("Detected Scarb project automatically");
                    Ok(ProjectType::Scarb)
                },
                ProjectType::Auto => {
                    // Fallback to interactive prompt
                    let options = vec![
                        "Regular Scarb project (uses scarb build)",
                        "Dojo project (uses sozo build)",
                    ];
                    
                    let selection = Select::new()
                        .with_prompt("What type of project are you verifying?")
                        .items(&options)
                        .default(0)
                        .interact()?;
                    
                    match selection {
                        0 => Ok(ProjectType::Scarb),
                        1 => {
                            validate_dojo_project(&args.path)?;
                            Ok(ProjectType::Dojo)
                        },
                        _ => unreachable!(),
                    }
                }
            }
        }
    }
}

fn validate_dojo_project(project: &Project) -> Result<(), CliError> {
    // Check if sozo is available (optional warning)
    if let Err(_) = std::process::Command::new("sozo").arg("--version").output() {
        warn!("sozo command not found. Dojo project verification will be handled remotely.");
    }
    
    // Validate project has Dojo dependencies
    if project.detect_project_type()? != ProjectType::Dojo {
        return Err(CliError::InvalidProjectType {
            specified: "dojo".to_string(),
            detected: "scarb".to_string(),
            suggestions: vec![
                "Add dojo-core dependency to Scarb.toml".to_string(),
                "Use --project-type=scarb for regular Scarb projects".to_string(),
            ],
        });
    }
    
    Ok(())
}
```

### 4. Update Project Metadata

**Location**: `src/api/models.rs`
- Extend `ProjectMetadataInfo` struct to include build tool information
- Add `build_tool` field to specify which build command to use

**Implementation**:
```rust
#[derive(Debug, Clone)]
pub struct ProjectMetadataInfo {
    pub cairo_version: semver::Version,
    pub scarb_version: semver::Version,
    pub project_dir_path: String,
    pub contract_file: String,
    pub package_name: String,
    pub build_tool: String, // "scarb" or "sozo"
}

impl ProjectMetadataInfo {
    pub fn new(
        cairo_version: semver::Version,
        scarb_version: semver::Version,
        project_dir_path: String,
        contract_file: String,
        package_name: String,
        project_type: ProjectType,
    ) -> Self {
        Self {
            cairo_version,
            scarb_version,
            project_dir_path,
            contract_file,
            package_name,
            build_tool: match project_type {
                ProjectType::Dojo => "sozo".to_string(),
                _ => "scarb".to_string(),
            },
        }
    }
}
```

### 5. Update API Client

**Location**: `src/api/client.rs`
- Modify `verify_class` method to include build tool information in the API request
- Add `build_tool` field to the multipart form data sent to the remote API

**Implementation**:
```rust
pub fn verify_class(
    &self,
    class_hash: &ClassHash,
    license: Option<String>, 
    name: &str,
    project_metadata: ProjectMetadataInfo,
    files: &[FileInfo],
) -> Result<String, ApiClientError> {
    let mut body = multipart::Form::new()
        .percent_encode_noop()
        .text(
            "compiler_version",
            project_metadata.cairo_version.to_string(),
        )
        .text("scarb_version", project_metadata.scarb_version.to_string())
        .text("package_name", project_metadata.package_name)
        .text("name", name.to_string())
        .text("contract_file", project_metadata.contract_file.clone())
        .text("contract-name", project_metadata.contract_file)
        .text("project_dir_path", project_metadata.project_dir_path)
        .text("build_tool", project_metadata.build_tool); // Add this line

    // ... rest of implementation remains the same
}
```

### 6. Enhanced Error Handling

**Location**: `src/main.rs`
- Add new error types for Dojo-specific issues
- Provide context-aware error messages

**Implementation**:
```rust
#[derive(Debug, Error)]
pub enum CliError {
    // ... existing error types ...
    
    #[error("[E025] Invalid project type specified\n\nSpecified: {specified}\nDetected: {detected}\n\nSuggestions:\n{}", suggestions.join("\n  • "))]
    InvalidProjectType {
        specified: String,
        detected: String,
        suggestions: Vec<String>,
    },
    
    #[error("[E026] Dojo project validation failed\n\nSuggestions:\n  • Ensure dojo-core is listed in dependencies\n  • Check that Scarb.toml is properly configured for Dojo\n  • Verify project structure follows Dojo conventions\n  • Run 'sozo build' to test project compilation")]
    DojoValidationFailed,
    
    #[error("[E027] Interactive prompt failed\n\nSuggestions:\n  • Use --project-type=scarb or --project-type=dojo to skip prompt\n  • Ensure terminal supports interactive input\n  • Check that stdin is available")]
    InteractivePromptFailed(#[from] dialoguer::Error),
}

impl CliError {
    pub const fn error_code(&self) -> &'static str {
        match self {
            // ... existing error codes ...
            Self::InvalidProjectType { .. } => "E025",
            Self::DojoValidationFailed => "E026",
            Self::InteractivePromptFailed(_) => "E027",
        }
    }
}
```

### 7. Update Main Verification Logic

**Location**: `src/main.rs`
- Modify `execute_verification` function to determine project type
- Pass build tool information through the verification pipeline
- Ensure proper error handling for Dojo-specific issues

**Implementation**:
```rust
fn execute_verification(
    public: &ApiClient,
    args: &VerifyArgs,
    file_infos: Vec<FileInfo>,
    package_meta: PackageMetadata,
    contract_file: String,
    project_dir_path: String,
    license_info: &license::LicenseInfo,
) -> Result<String, CliError> {
    // Determine project type
    let project_type = determine_project_type(args)?;
    
    // Log the selected build tool
    match project_type {
        ProjectType::Dojo => info!("Using sozo build for Dojo project"),
        ProjectType::Scarb => info!("Using scarb build for Scarb project"),
        ProjectType::Auto => unreachable!("Auto should be resolved by now"),
    }
    
    // Create project metadata with build tool information
    let project_metadata = ProjectMetadataInfo::new(
        package_meta.manifest_metadata.cairo_version.clone(),
        package_meta.manifest_metadata.scarb_version.clone(),
        project_dir_path,
        contract_file,
        package_meta.name.clone(),
        project_type,
    );
    
    // ... rest of implementation remains the same
}
```

## Dependencies

### Required New Dependencies

Add to `Cargo.toml`:
```toml
[dependencies]
# ... existing dependencies ...
dialoguer = "0.11.0"  # For interactive prompts
```

### Optional Dependencies

Consider adding for enhanced functionality:
```toml
[dependencies]
# ... existing dependencies ...
which = "6.0.0"  # For checking sozo availability
```

## Implementation Steps

### Phase 1: Core Infrastructure (Priority: High)
1. **Add dependencies to Cargo.toml**
   - Add `dialoguer = "0.11.0"`
   - Add `which = "6.0.0"` (optional)

2. **Add ProjectType enum and CLI argument** (`src/args.rs`)
   - Define enum with Scarb, Dojo, Auto options
   - Add `--project-type` argument to `VerifyArgs`
   - Add validation functions and detection logic

3. **Update ProjectMetadataInfo struct** (`src/api/models.rs`)
   - Add `build_tool` field
   - Update constructor to accept ProjectType
   - Add helper methods for build tool determination

4. **Modify API client** (`src/api/client.rs`)
   - Update `verify_class` method to send build tool info
   - Ensure backward compatibility with existing API

### Phase 2: Detection & Validation (Priority: High)
5. **Implement project type detection** (`src/args.rs`)
   - Add dependency analysis logic
   - Add source file import detection
   - Add validation functions

6. **Add error handling** (`src/main.rs`)
   - Define new error types for Dojo scenarios
   - Add context-aware error messages
   - Implement proper error propagation

### Phase 3: Interactive Selection (Priority: Medium)
7. **Add interactive prompt logic** (`src/main.rs`)
   - Implement project type selection UI
   - Add user-friendly prompts with clear descriptions
   - Handle auto-detection and fallback logic

8. **Update verification pipeline** (`src/main.rs`)
   - Modify `execute_verification` to handle project type
   - Update metadata creation to include build tool
   - Ensure proper error propagation

### Phase 4: Testing & Polish (Priority: Low)
9. **Add comprehensive testing**
   - Unit tests for project type detection
   - Integration tests with sample Dojo projects
   - Error handling validation tests

10. **Update documentation and help text**
    - Update CLI help messages
    - Add examples for Dojo projects
    - Update error messages to be context-aware

## Technical Considerations

### Dependencies
- `dialoguer 0.11.0`: For interactive prompts with wide terminal support
- `which 6.0.0`: For checking sozo availability (optional)
- Minimal impact on existing dependencies
- All new dependencies are optional for core functionality

### Backward Compatibility
- Default behavior remains unchanged (Scarb projects)
- Existing CLI usage continues to work without modification
- API gracefully handles missing build_tool field
- New `--project-type` argument defaults to `auto` (backward compatible)

### Error Handling
- Specific error codes (E025-E027) for Dojo-related issues
- Context-aware error messages with actionable suggestions
- Graceful degradation when sozo is not installed
- Clear validation messages for project type mismatches

### Performance Considerations
- Project type detection is cached (single detection per run)
- File scanning is optimized (early exit when Dojo indicators found)
- Interactive prompts only shown when necessary
- Minimal overhead for Scarb projects

### Security Considerations
- No execution of local sozo commands for verification
- All build processes happen remotely on the verification server
- Project type detection only reads files, never executes them
- Input validation for all project type selections

### Remote API Requirements
The remote verification API will need to:
- Accept the new `build_tool` parameter in multipart form data
- Use `sozo build` instead of `scarb build` for Dojo projects  
- Handle Dojo-specific compilation requirements and dependencies
- Maintain backward compatibility for requests without `build_tool` field
- Provide clear error messages for Dojo-specific compilation failures

## User Experience Flow

### Auto-detection (Default)
1. User runs verification command without `--project-type`
2. Tool analyzes project structure (dependencies + source imports)
3. If Dojo project detected → proceed with sozo build
4. If Scarb project detected → proceed with scarb build
5. If ambiguous → show interactive prompt with project type options

### Explicit Selection
1. User specifies `--project-type=dojo` or `--project-type=scarb`
2. Tool validates selection against project structure
3. If mismatch → show error with correction suggestions
4. If valid → proceed with verification using specified build tool

### Error Scenarios
- **Missing sozo installation** → Warning message (not blocking)
- **Wrong project type selected** → Clear error with auto-detection results
- **Dojo project without proper dependencies** → Validation error with setup guidance
- **Interactive prompt failure** → Fallback instructions for manual selection

## Testing Strategy

### Unit Tests
- Project type detection logic with various dependency configurations
- Error handling for invalid project structures
- CLI argument parsing and validation

### Integration Tests
- End-to-end verification flow with sample Dojo projects
- API client integration with build tool parameter
- Error scenarios and recovery paths

### Manual Testing
- Interactive prompt behavior in different terminal environments
- Project type detection with real Dojo projects
- Backward compatibility with existing Scarb projects

## Success Criteria

1. ✅ **Backward Compatibility**: Regular Scarb projects continue to work unchanged
2. ✅ **Dojo Support**: Dojo projects can be verified using `sozo build`
3. ✅ **Smart Detection**: Automatic project type detection works reliably
4. ✅ **User Experience**: Interactive prompt guides users to correct selection
5. ✅ **Error Handling**: Clear error messages for configuration issues
6. ✅ **API Compatibility**: Backward compatible API that doesn't break existing integrations
7. ✅ **Documentation**: Clear documentation explains both project types and usage

## Risk Mitigation

### Technical Risks
- **API compatibility**: Implement graceful fallback for missing build_tool field
- **Detection accuracy**: Provide manual override options for edge cases
- **Performance**: Optimize file scanning and cache detection results

### User Experience Risks
- **Confusion**: Clear documentation and help text for new options
- **Migration**: Gradual rollout with opt-in Dojo support initially
- **Support**: Comprehensive error messages with actionable suggestions

## Next Steps

1. **Phase 1 Implementation**: Begin with core infrastructure changes
2. **Testing**: Create comprehensive test suite with sample projects
3. **Documentation**: Update CLI help, README, and examples
4. **API Coordination**: Work with remote API team for backend changes
5. **Gradual Rollout**: Deploy as opt-in feature initially, then make default

---

*This enhanced plan ensures a smooth transition that maintains backward compatibility while adding powerful new functionality for Dojo developers with improved detection, validation, and user experience.*