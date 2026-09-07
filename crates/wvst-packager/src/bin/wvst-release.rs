use std::path::Path;
use wvst_packager::release;

fn main() {
    if let Err(error) = run(std::env::args().skip(1).collect()) {
        eprintln!("wvst-release: {error}");
        std::process::exit(2);
    }
}
fn run(args: Vec<String>) -> Result<(), String> {
    let root = Path::new(".");
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["check"] => print_version(release::check(root, None)?),
        ["check", "--tag", tag] => print_version(release::check(root, Some(tag))?),
        ["sync"] => print_version(release::sync(root)?),
        ["prepare", version, "--date", date] => {
            print_version(release::prepare(root, version, date)?)
        }
        ["notes", version] => println!("{}", release::notes(root, version)?),
        ["bundle", target, binaries, out, commit] => println!(
            "{}",
            release::bundle(root, target, Path::new(binaries), Path::new(out), commit)?.display()
        ),
        ["checksums", directory, commit] => release::checksums(root, Path::new(directory), commit)?,
        ["notary-credentials"] => release::store_notary_credentials()?,
        ["verify-macos-signatures", binaries] => {
            release::verify_macos_signatures(Path::new(binaries))?
        }
        ["bundle-macos", target, binaries, out, commit] => println!(
            "{}",
            release::bundle_macos(root, target, Path::new(binaries), Path::new(out), commit)?
                .display()
        ),
        ["targets"] => println!(
            "{}",
            serde_json::to_string(release::release_targets()).map_err(|e| e.to_string())?
        ),
        [] | ["--help"] | ["-h"] => println!(
            "Run from repository root:\n  wvst-release check [--tag vVERSION]\n  wvst-release sync\n  wvst-release prepare VERSION --date YYYY-MM-DD\n  wvst-release notes VERSION\n  wvst-release bundle TARGET BINARY_DIR OUTPUT_DIR COMMIT_SHA\n  wvst-release bundle-macos TARGET BINARY_DIR OUTPUT_DIR COMMIT_SHA\n  wvst-release notary-credentials\n  wvst-release verify-macos-signatures BINARY_DIR\n  wvst-release checksums ARTIFACT_DIR COMMIT_SHA\n  wvst-release targets"
        ),
        _ => return Err("invalid arguments; use --help".into()),
    }
    Ok(())
}
fn print_version(version: semver::Version) {
    println!(
        "{}",
        serde_json::json!({"version":version.to_string(),"tag":format!("v{version}"),"prerelease":!version.pre.is_empty()})
    );
}
