use assert_cmd::Command;
use glob::glob;
use predicates::prelude::*; // Used for writing assertions
use rstest::*;
use std::{error::Error, fs::File, io::Read, path::Path};

// Really should normalize the data (lex sort) before comparison
#[rstest]
#[case("test-data/dfz_combined", "")] // Basic aggregation test
#[case("test-data/max_pfxlen", "-m 20")] // Filter on prefix length
#[case("test-data/max_pfxlen_split", "-m 20,32")] // Filter on prefix length (split v4/v6)
#[case("test-data/v4_only", "-4")] // Filter v4 only
#[case("test-data/v6_only", "-6")] // Filter v4 only
fn dfz_test(#[case] path: &str, #[case] args: &str) -> Result<(), Box<dyn Error>> {
    let mut cmd = Command::cargo_bin("rs-aggregate")?;
    let in_path = Path::new(path).join("input");
    let expect_path = Path::new(path).join("expected");
    let mut expect_file = File::open(expect_path)?;
    let mut expect_data: Vec<u8> =
        Vec::with_capacity(expect_file.metadata()?.len().try_into().unwrap());
    expect_file.read_to_end(&mut expect_data)?;

    let assert = cmd
        .arg(in_path)
        .args(args.split_whitespace())
        .timeout(std::time::Duration::from_secs(30))
        .assert();

    assert
        .success()
        .stdout(predicate::eq(expect_data))
        .stderr(predicate::str::is_empty());

    Ok(())
}

#[rstest]
#[case("2001:db8::23ab:f007/64", "2001:db8::/64")]
#[case("198.51.100.123/24", "198.51.100.0/24")]
fn truncate_test(#[case] input: &str, #[case] expect: &str) -> Result<(), Box<dyn Error>> {
    let mut cmd = Command::cargo_bin("rs-aggregate")?;

    let assert = cmd.write_stdin(input).assert();
    assert
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::eq(format!(
            "ERROR: '{}' is not a valid IP network, ignoring.\n",
            input
        )));

    let assert = cmd.arg("-t").write_stdin(input).assert();
    assert
        .success()
        .stdout(predicate::eq(format!("{}\n", expect)))
        .stderr(predicate::str::is_empty());

    Ok(())
}

#[rstest]
#[case("test-data/multi_input", "")]
fn multi_input_test(#[case] path: &str, #[case] args: &str) -> Result<(), Box<dyn Error>> {
    let mut cmd = Command::cargo_bin("rs-aggregate")?;

    let inputs = glob((path.to_owned() + "/input*").as_str())?;

    let expect_path = Path::new(path).join("expected");
    let mut expect_file = File::open(expect_path)?;
    let mut expect_data: Vec<u8> =
        Vec::with_capacity(expect_file.metadata()?.len().try_into().unwrap());
    expect_file.read_to_end(&mut expect_data)?;

    let assert = cmd
        .args(args.split_whitespace())
        .args(inputs.map(|x| x.unwrap()))
        .timeout(std::time::Duration::from_secs(30))
        .assert();

    assert
        .success()
        .stdout(predicate::eq(expect_data))
        .stderr(predicate::str::is_empty());

    Ok(())
}
