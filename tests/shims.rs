//! The client shims, run unmodified against the binary.
//!
//! Replaces `scripts/compat/shim-differential.mjs`. If a shim needs an edit to
//! drive the binary, compatibility broke — the shims are the regression test,
//! not the subject.
//!
//! Each shim's output is compared with `tests/fixtures/shim-expectations.json`,
//! a committed recording made while the JavaScript engine still existed to
//! check it against. Re-record with:
//!
//! ```text
//! cargo test --test shims -- --ignored record
//! ```
//!
//! A shim whose toolchain is not installed is skipped and named in the summary,
//! never silently dropped. `TTID_REQUIRE` turns a missing toolchain into a
//! failure for the clients CI guarantees:
//!
//! ```text
//! TTID_REQUIRE=python,ruby,node,php,go,rust,java,csharp cargo test --test shims
//! ```
//!
//! Slow by nature: Kotlin and C# compile before they run.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

mod support;

use serde_json::Value;
use std::path::{Path, PathBuf};
use support::{available, normalize, run, run_with_env};

const EXPECTATIONS: &str = "tests/fixtures/shim-expectations.json";
const BINARY: &str = env!("CARGO_BIN_EXE_ttid");
const SHIMS: &str = "scripts/compat/shims";

/// How a client is built (if at all) and then invoked.
enum Build {
    /// Interpreted: run the driver straight from the repo.
    Direct(&'static [&'static str]),
    /// Compiled: stage the client and driver, build once, then run.
    Compiled,
}

struct Client {
    name: &'static str,
    /// The executable that must exist for this client to run at all.
    requires: &'static str,
    build: Build,
}

const CLIENTS: &[Client] = &[
    client(
        "python",
        "python3",
        Build::Direct(&["python3", "driver.py"]),
    ),
    client("ruby", "ruby", Build::Direct(&["ruby", "driver.rb"])),
    client("node", "node", Build::Direct(&["node", "driver.mjs"])),
    client("php", "php", Build::Direct(&["php", "driver.php"])),
    client(
        "dart",
        "dart",
        Build::Direct(&["dart", "run", "driver.dart"]),
    ),
    client("go", "go", Build::Compiled),
    client("rust", "rustc", Build::Compiled),
    client("java", "javac", Build::Compiled),
    client("kotlin", "kotlinc", Build::Compiled),
    client("swift", "swiftc", Build::Compiled),
    client("csharp", "dotnet", Build::Compiled),
];

const fn client(name: &'static str, requires: &'static str, build: Build) -> Client {
    Client {
        name,
        requires,
        build,
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ttid-shim-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("creates a scratch directory");
    dir
}

fn copy(from: &str, to: &Path) {
    std::fs::copy(from, to).unwrap_or_else(|error| panic!("copy {from}: {error}"));
}

/// Run a build step, returning its combined output on failure.
fn build_step(argv: &[&str], cwd: &Path) -> Result<(), String> {
    let cwd = cwd.to_string_lossy().into_owned();
    let output = run(argv[0], &argv[1..], None, Some(&cwd));
    if output.code == 0 {
        return Ok(());
    }
    Err(format!(
        "{} exited {}\n{}{}",
        argv.join(" "),
        output.code,
        output.stdout,
        output.stderr
    ))
}

/// Lay out and build a compiled client; returns the command that runs it.
fn prepare(name: &str) -> Result<(Vec<String>, PathBuf), String> {
    let dir = scratch(name);
    let at = |file: &str| dir.join(file);
    let path = |p: &PathBuf| p.to_string_lossy().into_owned();

    match name {
        "go" => {
            std::fs::create_dir_all(dir.join("ttid")).unwrap();
            copy("clients/go/ttid.go", &dir.join("ttid/ttid.go"));
            copy(&format!("{SHIMS}/driver.go"), &at("main.go"));
            std::fs::write(at("go.mod"), "module ttidshim\n\ngo 1.21\n").unwrap();
            build_step(&["go", "build", "-o", "driver", "."], &dir)?;
            Ok((vec![path(&at("driver"))], dir))
        }
        "rust" => {
            copy("clients/rust/ttid.rs", &at("ttid.rs"));
            copy(&format!("{SHIMS}/main.rs"), &at("main.rs"));
            build_step(
                &[
                    "rustc",
                    "-O",
                    "--edition",
                    "2021",
                    "main.rs",
                    "-o",
                    "driver",
                ],
                &dir,
            )?;
            Ok((vec![path(&at("driver"))], dir))
        }
        "java" => {
            copy("clients/java/Ttid.java", &at("Ttid.java"));
            copy(&format!("{SHIMS}/Driver.java"), &at("Driver.java"));
            build_step(&["javac", "-d", "out", "Ttid.java", "Driver.java"], &dir)?;
            Ok((
                vec!["java".into(), "-cp".into(), "out".into(), "Driver".into()],
                dir,
            ))
        }
        "kotlin" => {
            copy("clients/kotlin/Ttid.kt", &at("Ttid.kt"));
            copy(&format!("{SHIMS}/driver.kt"), &at("driver.kt"));
            build_step(
                &[
                    "kotlinc",
                    "Ttid.kt",
                    "driver.kt",
                    "-include-runtime",
                    "-d",
                    "driver.jar",
                ],
                &dir,
            )?;
            Ok((vec!["java".into(), "-jar".into(), "driver.jar".into()], dir))
        }
        "swift" => {
            copy("clients/swift/Ttid.swift", &at("Ttid.swift"));
            copy(&format!("{SHIMS}/main.swift"), &at("main.swift"));
            build_step(
                &["swiftc", "-O", "Ttid.swift", "main.swift", "-o", "driver"],
                &dir,
            )?;
            Ok((vec![path(&at("driver"))], dir))
        }
        "csharp" => {
            copy("clients/csharp/Ttid.cs", &at("Ttid.cs"));
            copy(&format!("{SHIMS}/Program.cs"), &at("Program.cs"));
            std::fs::write(
                at("driver.csproj"),
                "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    \
                 <OutputType>Exe</OutputType>\n    <TargetFramework>net8.0</TargetFramework>\n    \
                 <Nullable>enable</Nullable>\n    <ImplicitUsings>disable</ImplicitUsings>\n    \
                 <AssemblyName>driver</AssemblyName>\n    \
                 <RootNamespace>TtidDriver</RootNamespace>\n  </PropertyGroup>\n</Project>\n",
            )
            .unwrap();
            build_step(
                &[
                    "dotnet", "build", "-c", "Release", "-o", "out", "--nologo", "-v", "q",
                ],
                &dir,
            )?;
            Ok((vec!["dotnet".into(), path(&at("out/driver.dll"))], dir))
        }
        other => Err(format!("no build defined for {other}")),
    }
}

/// Run one client's driver and return its normalized output.
fn drive(client: &Client) -> Result<String, String> {
    let repo = std::env::current_dir().unwrap();
    let (argv, cwd) = match client.build {
        Build::Direct(argv) => {
            let mut owned: Vec<String> = argv.iter().map(|a| (*a).to_owned()).collect();
            // The driver path is relative to the repo root.
            let last = owned.len() - 1;
            owned[last] = format!("{SHIMS}/{}", owned[last]);
            (owned, repo.clone())
        }
        Build::Compiled => prepare(client.name)?,
    };

    let borrowed: Vec<&str> = argv.iter().map(String::as_str).collect();
    let cwd = cwd.to_string_lossy().into_owned();
    let output = run_with_env(
        borrowed[0],
        &borrowed[1..],
        None,
        Some(&cwd),
        &[("TTID_BIN", BINARY)],
    );

    if output.code != 0 {
        return Err(format!(
            "the shim failed against the binary (exit {})\n{}",
            output.code,
            output
                .stderr
                .lines()
                .rev()
                .take(8)
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    Ok(normalize(&output.stdout))
}

fn selected() -> Vec<&'static Client> {
    let only = std::env::var("TTID_ONLY").ok();
    CLIENTS
        .iter()
        .filter(|client| {
            only.as_ref()
                .is_none_or(|list| list.split(',').any(|name| name == client.name))
        })
        .collect()
}

fn required() -> Vec<String> {
    std::env::var("TTID_REQUIRE")
        .map(|list| list.split(',').map(str::to_owned).collect())
        .unwrap_or_default()
}

#[test]
fn every_client_shim_drives_the_binary_unmodified() {
    let expected: Value =
        serde_json::from_str(&std::fs::read_to_string(EXPECTATIONS).expect("the recording exists"))
            .expect("the recording is valid JSON");

    let require = required();
    let known: Vec<&str> = CLIENTS.iter().map(|c| c.name).collect();
    for name in &require {
        assert!(
            known.contains(&name.as_str()),
            "TTID_REQUIRE names an unknown client: {name}. Known: {}",
            known.join(", ")
        );
    }

    let mut failures = Vec::new();
    let mut skipped = Vec::new();
    let mut ran = 0;

    for client in selected() {
        if !available(client.requires) {
            if require.iter().any(|n| n == client.name) {
                failures.push(format!(
                    "{}: required, but `{}` is not installed",
                    client.name, client.requires
                ));
            } else {
                skipped.push(format!("{} (no {})", client.name, client.requires));
            }
            continue;
        }

        match drive(client) {
            Err(message) => failures.push(format!("{}: {message}", client.name)),
            Ok(actual) => {
                ran += 1;
                match expected[client.name].as_str() {
                    None => failures.push(format!(
                        "{}: no recorded expectation — add one with `--ignored record`",
                        client.name
                    )),
                    Some(want) if want != actual => failures.push(format!(
                        "{}: output does not match the recording\n--- expected ---\n{want}\n--- actual ---\n{actual}",
                        client.name
                    )),
                    Some(_) => println!(
                        "  {:8} {} operations as recorded",
                        client.name,
                        actual.trim().lines().count()
                    ),
                }
            }
        }
    }

    if !skipped.is_empty() {
        // Never let reduced coverage read as full coverage.
        println!("  skipped {}: {}", skipped.len(), skipped.join(", "));
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
    assert!(ran > 0, "no client shims ran at all");
    println!("  {ran} of {} client shims verified", CLIENTS.len());
}

/// Re-record every client that runs on this machine. Ignored by default so a
/// normal run cannot rewrite what it is meant to be checked against.
#[test]
#[ignore = "rewrites tests/fixtures/shim-expectations.json"]
fn record() {
    let mut recorded: serde_json::Map<String, Value> = serde_json::from_str(
        &std::fs::read_to_string(EXPECTATIONS).unwrap_or_else(|_| "{}".to_owned()),
    )
    .unwrap_or_default();

    for client in selected() {
        if !available(client.requires) {
            println!(
                "  {:8} skipped: {} not installed",
                client.name, client.requires
            );
            continue;
        }
        match drive(client) {
            Ok(output) => {
                recorded.insert(client.name.to_owned(), Value::String(output));
                println!("  {:8} recorded", client.name);
            }
            Err(message) => panic!("{}: {message}", client.name),
        }
    }

    let rendered = format!(
        "{}\n",
        serde_json::to_string_pretty(&Value::Object(recorded)).unwrap()
    );
    std::fs::write(EXPECTATIONS, rendered).expect("writes the recording");
}
