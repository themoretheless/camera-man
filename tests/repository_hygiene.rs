use std::path::Path;
use std::process::Command;

#[test]
fn tracked_tree_contains_no_legacy_or_private_artifacts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .current_dir(root)
        .output()
        .expect("git must be available for repository hygiene checks");
    if !output.status.success() {
        return;
    }

    let tracked = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8(path.to_vec()).expect("tracked paths must be UTF-8"))
        .collect::<Vec<_>>();
    let private_extensions = [
        ".key",
        ".p12",
        ".cer",
        ".csr",
        ".certSigningRequest",
        ".mobileprovision",
        ".provisionprofile",
    ];

    for path in tracked {
        assert!(
            !path.ends_with(".swift"),
            "legacy Swift source is tracked: {path}"
        );
        assert!(
            !path.ends_with(".DS_Store"),
            "Finder metadata is tracked: {path}"
        );
        assert!(
            !path.starts_with(".idea/"),
            "IDE metadata is tracked: {path}"
        );
        assert!(
            !path.starts_with("target/"),
            "build output is tracked: {path}"
        );
        assert!(
            !path.starts_with("signing/"),
            "signing artifact is tracked: {path}"
        );
        assert!(
            !private_extensions
                .iter()
                .any(|extension| path.ends_with(extension)),
            "private signing artifact is tracked: {path}"
        );
    }
}
