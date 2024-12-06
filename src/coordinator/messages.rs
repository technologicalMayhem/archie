pub type Package = String;

#[derive(Clone)]
pub enum Message {
    AddPackages(Vec<Package>),
    RemovePackages(Vec<Package>),
    AcceptedWork {
        package: Package,
        worker: String,
    },
    BuildPackage(Package),
    BuildSuccess(Package),
    BuildFailure {
        worker: String,
    },
    ArtifactsUploaded {
        package: Package,
        files: Vec<String>,
        build_time: i64,
    },
}
