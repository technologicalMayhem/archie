use std::collections::HashSet;

pub type Package = String;

#[derive(Clone)]
pub enum Message {
    AddPackages(HashSet<Package>),
    AddDependencies(HashSet<Package>),
    RemovePackages(HashSet<Package>),
    BuildPackage(Package),
    BuildSuccess(Package),
    BuildFailure(Package),
    ArtifactsUploaded {
        package: Package,
        files: Vec<String>,
        build_time: i64,
    },
}
