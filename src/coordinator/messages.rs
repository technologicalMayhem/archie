pub type Package = String;

#[derive(Clone)]
pub enum Message {
    AddPackages(Vec<Package>),
    RemovePackages(Vec<Package>),
    BuildPackage(Package),
    BuildSuccess(Package),
    BuildFailure(Package),
    ArtifactsUploaded {
        package: Package,
        files: Vec<String>,
        build_time: i64,
    },
}
