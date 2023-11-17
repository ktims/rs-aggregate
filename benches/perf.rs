use ipnet::Ipv4Net;
use json::JsonValue;
use plotters::backend::BitMapBackend;
use plotters::chart::ChartBuilder;
use plotters::coord::ranged1d::{IntoSegmentedCoord, SegmentValue};
use plotters::drawing::IntoDrawingArea;
use plotters::element::{EmptyElement, Text};
use plotters::series::{Histogram, PointSeries};
use plotters::style::full_palette::GREY;
use plotters::style::text_anchor::{HPos, Pos, VPos};
use plotters::style::{Color, IntoFont, RGBColor, ShapeStyle, BLACK, WHITE};
use rand::prelude::*;
use rand_chacha::ChaChaRng;
use std::ffi::OsStr;
use std::io::{Read, Write};

use std::process::Stdio;
use tempfile::NamedTempFile;

const BAR_COLOUR: RGBColor = RGBColor(66, 133, 244);

#[derive(Clone, Debug)]
struct TestDefinition {
    cmd: String,
    name: String, // including version
}

#[derive(Clone, Debug)]
struct TestResult {
    mean: f64,
    stddev: f64,
    median: f64,
    min: f64,
    max: f64,
}

impl From<JsonValue> for TestResult {
    fn from(value: JsonValue) -> Self {
        Self {
            mean: value["mean"].as_f64().unwrap(),
            stddev: value["stddev"].as_f64().unwrap(),
            median: value["median"].as_f64().unwrap(),
            min: value["min"].as_f64().unwrap(),
            max: value["max"].as_f64().unwrap(),
        }
    }
}

fn make_tests(input_path: &str) -> Vec<TestDefinition> {
    let our_version = format!("rs-aggregate {}", env!("CARGO_PKG_VERSION"));
    let our_path = env!("CARGO_BIN_EXE_rs-aggregate");

    let python_version_raw = std::process::Command::new("python3")
        .arg("--version")
        .stdout(Stdio::piped())
        .spawn()
        .expect("Unable to run python3")
        .wait_with_output()
        .expect("Couldn't get python3 output")
        .stdout;
    let python_version = String::from_utf8_lossy(&python_version_raw);

    let agg6_version_raw = std::process::Command::new("python3")
        .arg("-m")
        .arg("aggregate6")
        .arg("-V")
        .stdout(Stdio::piped())
        .spawn()
        .expect("Unable to run aggregate6")
        .wait_with_output()
        .expect("Couldn't get aggregate6 output")
        .stdout;
    let agg6_version = String::from_utf8_lossy(&agg6_version_raw);

    vec![
        TestDefinition {
            cmd: format!("{} {}", our_path, input_path),
            name: our_version.into(),
        },
        TestDefinition {
            cmd: format!("python3 -m aggregate6 {}", input_path),
            name: format!("{} ({})", agg6_version.trim(), python_version.trim()),
        },
    ]
}

fn make_v4_tests(input_path: &str) -> Vec<TestDefinition> {
    let mut all_tests = make_tests(input_path);

    let iprange_version_raw = std::process::Command::new("iprange")
        .arg("--version")
        .stdout(Stdio::piped())
        .spawn()
        .expect("Unable to run iprange")
        .wait_with_output()
        .expect("Couldn't get iprange output")
        .stdout;
    let iprange_version = String::from_utf8_lossy(&iprange_version_raw);

    all_tests.push(TestDefinition {
        cmd: format!("iprange --optimize {}", input_path),
        name: iprange_version.lines().nth(0).unwrap().into(),
    });

    all_tests
}

// We don't really care if aggregation will actually be possible, but we'll only
// generate prefixes with length 8->24 so some should be possible.
fn make_random_prefix(rng: &mut impl Rng) -> Ipv4Net {
    let prefix_len: u8 = rng.gen_range(8..25);
    let netaddr: u32 = rng.gen_range(0..(1 << prefix_len)) << 32 - prefix_len;

    Ipv4Net::new(netaddr.into(), prefix_len).unwrap()
}

// Generate 1024 random v4 addresses as a startup time test
fn make_startup_tests() -> (NamedTempFile, Vec<TestDefinition>) {
    let mut rng = ChaChaRng::seed_from_u64(0); // use a repeatable rng with custom seed
    let addresses = std::iter::repeat_with(|| make_random_prefix(&mut rng)).take(1024);

    let mut outfile = NamedTempFile::new().unwrap();
    let mut outfile_f = outfile.as_file();
    for addr in addresses {
        outfile_f.write_fmt(format_args!("{}\n", addr)).unwrap();
    }
    outfile.flush().unwrap();

    let outpath = outfile.path().as_os_str().to_string_lossy().to_string();

    // outfile needs to live on so destructor doesn't delete it before we run the benches
    (outfile, make_v4_tests(outpath.as_str()))
}

fn hyperfine_harness<S>(cmd: S) -> Result<TestResult, Box<dyn std::error::Error>>
where
    S: AsRef<OsStr>,
{
    let resultfile = NamedTempFile::new().expect("Unable to create tempfile");

    let mut process = std::process::Command::new("hyperfine")
        .arg("--export-json")
        .arg(resultfile.path())
        .arg("--min-runs")
        .arg("10")
        .arg("-N")
        .arg("--")
        .arg(&cmd)
        .stdout(Stdio::null())
        .spawn()
        .expect("unable to run command");
    let _rc = process.wait().expect("unable to wait on process");

    let mut raw_result_buf = Vec::new();
    resultfile
        .as_file()
        .read_to_end(&mut raw_result_buf)
        .expect("Can't read results");
    resultfile.close().unwrap();

    let hf_result = json::parse(&String::from_utf8_lossy(&raw_result_buf)).expect(
        format!(
            "Can't parse hyperfine json results from command `{}`",
            cmd.as_ref().to_string_lossy()
        )
        .as_str(),
    );

    let final_result = &hf_result["results"][0];

    Ok((final_result.clone()).into())
}

fn plot_results(
    results: &Vec<(TestDefinition, TestResult)>,
    caption: &str,
    outfile: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Second result is our baseline
    let norm_numerator = results[1].1.mean;
    let max_result = norm_numerator / results.iter().map(|x| x.1.mean).reduce(f64::min).unwrap();

    let drawing = BitMapBackend::new(outfile, (640, 480)).into_drawing_area();
    drawing.fill(&WHITE)?;
    let mut chart = ChartBuilder::on(&drawing)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .caption(caption, ("Roboto", 24).into_font())
        .build_cartesian_2d((0..results.len() - 1).into_segmented(), 0.0..max_result)?;

    chart
        .configure_mesh()
        .y_desc("Speedup vs aggregate6")
        .y_labels(5)
        .y_label_formatter(&|x| std::fmt::format(format_args!("{:.0}", *x)))
        .light_line_style(WHITE)
        .bold_line_style(GREY)
        .disable_x_mesh()
        .x_label_style(("Roboto", 18).into_font())
        .x_label_formatter(&|x| match x {
            SegmentValue::Exact(val) => results[*val].0.name.clone(),
            SegmentValue::CenterOf(val) => results[*val].0.name.clone(),
            SegmentValue::Last => String::new(),
        })
        .draw()?;

    chart.draw_series(
        Histogram::vertical(&chart)
            .style(BAR_COLOUR.filled())
            .margin(10)
            .data(
                results
                    .iter()
                    .enumerate()
                    .map(|(x, y)| (x, norm_numerator / y.1.mean)),
            ),
    )?;

    chart.draw_series(PointSeries::of_element(
        results
            .iter()
            .enumerate()
            .map(|(x, y)| (SegmentValue::CenterOf(x), norm_numerator / y.1.mean)),
        5,
        ShapeStyle::from(&BLACK).filled(),
        &|coord, _size, _style| {
            let (target_y, target_colour) = if coord.1 < 25.0 {
                (-25, BAR_COLOUR)
            } else {
                (25, WHITE)
            };
            EmptyElement::at(coord.clone())
                + Text::new(
                    format!("{:.1} x", coord.1),
                    (0, target_y),
                    ("Roboto", 18)
                        .into_font()
                        .color(&target_colour)
                        .pos(Pos::new(HPos::Center, VPos::Center)),
                )
        },
    ))?;

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_and_plot(
        make_tests("test-data/dfz_combined/input"),
        "doc/perfcomp_all.png",
        "IPv4 & IPv6 Full DFZ",
    )?;
    run_and_plot(
        make_v4_tests("test-data/dfz_v4/input"),
        "doc/perfcomp_v4.png",
        "IPv4 Full DFZ",
    )?;

    // Need to hold on to tmpfile so it doesn't get deleted before we can bench
    let (_tmpfile, tests) = make_startup_tests();
    run_and_plot(
        tests,
        "doc/perfcomp_startup.png",
        "1024 Random IPv4 Prefixes",
    )?;

    Ok(())
}

fn run_and_plot(
    tests: Vec<TestDefinition>,
    filename: &str,
    caption: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut results: Vec<(TestDefinition, TestResult)> = Vec::new();
    for test in tests {
        println!("Running bench: {:?}", test);
        results.push((test.clone(), hyperfine_harness(&test.cmd)?));
    }
    plot_results(&results, caption, filename)?;
    Ok(())
}
