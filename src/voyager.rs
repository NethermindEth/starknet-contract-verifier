use camino::Utf8PathBuf;
use scarb_metadata::Metadata;

// Use this instead of metadata.runtime_manifest, because of:
// https://docs.rs/scarb-metadata/latest/scarb_metadata/struct.Metadata.html#compatibility
// > With very old Scarb versions (<0.5.0), this field may end up being
// > empty path upon deserializing from scarb metadata call. In this
// >  case, fall back to WorkspaceMetadata.manifest field value.
// but I've actually got this in scarb 0.5.1, so...
#[must_use]
pub fn manifest_path(metadata: &Metadata) -> &Utf8PathBuf {
    if metadata.runtime_manifest == Utf8PathBuf::new() {
        &metadata.workspace.manifest_path
    } else {
        &metadata.runtime_manifest
    }
}
