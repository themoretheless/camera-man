use std::collections::HashSet;
use std::fs;
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

/// `scripts/verify-kani.sh` names each harness it runs, so a proof added to the
/// tree would otherwise sit unproved and read as verified.
#[test]
fn every_kani_harness_runs_in_the_proof_gate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate = fs::read_to_string(root.join("scripts/verify-kani.sh"))
        .expect("proof gate must be readable");

    let output = Command::new("git")
        .args(["ls-files", "-z", "--", "src"])
        .current_dir(root)
        .output()
        .expect("git must be available to enumerate sources");
    if !output.status.success() {
        return;
    }

    let mut harnesses = Vec::new();
    for relative_path in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| path.ends_with(b".rs"))
        .map(|path| String::from_utf8(path.to_vec()).expect("tracked paths must be UTF-8"))
    {
        let source = fs::read_to_string(root.join(&relative_path)).unwrap();
        for attribute in source.match_indices("#[kani::proof]") {
            let declared = source[attribute.0 + attribute.1.len()..]
                .split_once("fn ")
                .and_then(|(_, rest)| {
                    rest.find(|character: char| !(character.is_alphanumeric() || character == '_'))
                        .map(|end| rest[..end].to_owned())
                });
            harnesses.push((
                relative_path.clone(),
                declared.expect("a proof attribute needs a fn"),
            ));
        }
    }

    assert!(
        harnesses.len() >= 5,
        "only {} proof attributes found; the enumeration stopped seeing sources",
        harnesses.len()
    );
    let distinct = harnesses
        .iter()
        .map(|(_, name)| name)
        .collect::<HashSet<_>>();
    assert_eq!(
        distinct.len(),
        harnesses.len(),
        "harness names must be unique for the gate to name them"
    );
    for (relative_path, name) in &harnesses {
        assert!(
            gate.contains(&format!("--harness {name}")),
            "harness {relative_path}::{name} is never run by scripts/verify-kani.sh"
        );
    }
}
