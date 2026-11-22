//! Type-aware numeric edits prepared from one fingerprinted compiler snapshot.

#[path = "../edits.rs"]
pub mod edits;
#[path = "../numerics.rs"]
pub mod numerics;

use edits::{Outcome, Plan};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::PathBuf,
};

#[derive(Default)]
struct Args {
    root: PathBuf,
    packages: Vec<String>,
    facts: Vec<PathBuf>,
    passes: BTreeSet<String>,
    arithmetic: bool,
    generated: bool,
    apply: bool,
    diff: Option<PathBuf>,
    number_crate: String,
}

fn arguments() -> Outcome<Option<Args>> {
    let mut result = Args {
        root: env::current_dir()?,
        number_crate: "console_core_number_conversion".into(),
        ..Args::default()
    };
    result.passes.insert("casts".into());
    let mut args = env::args().skip(1).peekable();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--root" => result.root = args.next().ok_or("--root needs a directory")?.into(),
            "--package" => result
                .packages
                .push(args.next().ok_or("--package needs a name")?),
            "--facts" => {
                let initial = result.facts.len();
                while args.peek().is_some_and(|arg| !arg.starts_with("--")) {
                    result
                        .facts
                        .push(args.next().ok_or("missing fact path")?.into());
                }
                if result.facts.len() == initial {
                    return Err("--facts needs at least one JSON array".into());
                }
            }
            "--passes" => {
                result.passes = args
                    .next()
                    .ok_or("--passes needs casts,arithmetic")?
                    .split(',')
                    .map(String::from)
                    .collect()
            }
            "--arithmetic" => {
                if args.next().as_deref() != Some("wrapping") {
                    return Err(
                        "only explicitly selected --arithmetic wrapping is supported".into(),
                    );
                }
                result.arithmetic = true;
            }
            "--number-crate" => {
                result.number_crate = args.next().ok_or("--number-crate needs an identifier")?
            }
            "--include-generated" => result.generated = true,
            "--apply" => result.apply = true,
            "--diff" => result.diff = Some(args.next().ok_or("--diff needs a path")?.into()),
            "--help" | "-h" => {
                println!(
                    "numeric --package NAME [--package NAME] --facts FILE... [--root DIR] [--include-generated] [--passes casts,arithmetic] [--arithmetic wrapping] [--number-crate ALIAS] [--diff FILE] [--apply]\n\nDefault: preview casts only. Arithmetic requires explicit --arithmetic wrapping. Facts must include current source fingerprints and expression owner metadata. Const, foreign and generated contexts are excluded. Division, remainder and unsupported domain casts remain review items. Apply verifies the snapshot again and backs up originals under TMPDIR/builds/explicit-numeric."
                );
                return Ok(None);
            }
            _ => return Err(format!("unknown argument: {argument}").into()),
        }
    }
    if result.packages.is_empty() || result.facts.is_empty() {
        return Err("--package and --facts are required".into());
    }
    if result
        .passes
        .iter()
        .any(|pass| pass != "casts" && pass != "arithmetic")
    {
        return Err("--passes accepts casts,arithmetic only".into());
    }
    if result.passes.contains("arithmetic") && !result.arithmetic {
        return Err("arithmetic edits require --arithmetic wrapping".into());
    }
    if result.arithmetic {
        result.passes.insert("arithmetic".into());
    }
    let mut chars = result.number_crate.chars();
    if !chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        || chars.any(|c| !c.is_ascii_alphanumeric() && c != '_')
    {
        return Err("--number-crate must be a Rust identifier".into());
    }
    Ok(Some(result))
}

fn prepare(args: &Args, records: &[Value]) -> Outcome<(Plan, BTreeMap<PathBuf, Vec<u8>>)> {
    let root = args.root.canonicalize()?;
    let mut packages = vec![];
    for name in &args.packages {
        if name.contains(['/', '\\']) || name == "." || name == ".." {
            return Err("package must be a directory name".into());
        }
        let first = root.join("crates").join(name);
        let package = if first.is_dir() {
            first
        } else {
            root.join("forks").join(name)
        }
        .canonicalize()?;
        if !package.starts_with(&root) {
            return Err("package outside repository".into());
        }
        packages.push(package);
    }
    let mut paths = BTreeSet::new();
    for record in records {
        if let Some(path) = record["span"]["file"].as_str() {
            let path = root.join(path).canonicalize()?;
            if numerics::owned(&path, &packages, args.generated) {
                paths.insert(path);
            }
        }
    }
    let mut plan = Plan::new(&root, paths)?;
    plan.validate_sources(records)?;
    numerics::numeric(
        &mut plan,
        records,
        &numerics::Options {
            packages,
            generated: args.generated,
            casts: args.passes.contains("casts"),
            arithmetic: args.arithmetic,
            number_crate: args.number_crate.clone(),
        },
    )?;
    let outputs = plan.outputs()?;
    for (path, bytes) in &outputs {
        let source = edits::Source::from_bytes(bytes.clone())?;
        syn::parse_file(&source.text).map_err(|error| {
            format!(
                "prepared output is not valid Rust syntax in {}: {error}",
                path.display()
            )
        })?;
    }
    Ok((plan, outputs))
}

fn main() -> Outcome<()> {
    let Some(args) = arguments()? else {
        return Ok(());
    };
    let mut records = vec![];
    for path in &args.facts {
        records.extend(serde_json::from_slice::<Vec<Value>>(&fs::read(path)?)?);
    }
    let (plan, outputs) = prepare(&args, &records)?;
    if let Some(path) = &args.diff {
        fs::write(path, plan.unified_diff(&outputs)?)?;
    }
    let backup = if args.apply && !outputs.is_empty() {
        let temporary = env::var_os("TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/var/tmp"));
        Some(plan.write(&outputs, &temporary.join("builds/explicit-numeric"))?)
    } else {
        None
    };
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"changes":plan.stats,"review":plan.review,"files":outputs.len(),"backup":backup})
        )?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_numeric_edits_prepare_valid_source_from_one_snapshot() -> Outcome<()> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let temporary = env::var_os("TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| "/var/tmp".into());
        let root = temporary.join(format!("numeric-test-{nonce}"));
        let path = root.join("crates/example/src/lib.rs");
        fs::create_dir_all(path.parent().ok_or("missing parent")?)?;
        let text = "fn demo(a: i16, b: i16) -> u8 { (a + b) as u8 }\n";
        fs::write(&path, text)?;
        let location = |lo, hi| json!({"file":path,"lo":lo,"hi":hi});
        let plus = text.find("a + b").ok_or("missing addition")?;
        let cast_start = plus - 1;
        let cast_end = cast_start + "(a + b) as u8".len();
        let records = vec![
            json!({"kind":"source","span":location(0,text.len()),"fnv1a64":edits::fingerprint(text.as_bytes()),"bytes":text.len()}),
            json!({"kind":"fn","id":"demo","span":location(0,text.len()),"const":false,"foreign_abi":false}),
            json!({"kind":"expr","node":"binary","span":location(plus,plus+5),"owner_kind":"Fn","owner_span":location(0,text.len()),"generated_owner":false,"integral":true,"op":"Add","type":"i16","left_type":"i16","right_type":"i16","left":location(plus,plus+1),"right":location(plus+4,plus+5)}),
            json!({"kind":"expr","node":"cast","span":location(cast_start,cast_end),"owner_kind":"Fn","owner_span":location(0,text.len()),"generated_owner":false,"from_type":"i16","type":"u8","value":location(cast_start,plus+6)}),
        ];
        let args = Args {
            root,
            packages: vec!["example".into()],
            passes: ["casts".into(), "arithmetic".into()].into(),
            arithmetic: true,
            number_crate: "console_core_number_conversion".into(),
            ..Args::default()
        };
        let (plan, output) = prepare(&args, &records)?;
        let rewritten = std::str::from_utf8(&output[&path])?;
        assert!(rewritten.contains("wrapped::<_, u8>((i16::wrapping_add(a, b)))"));
        assert_eq!(plan.stats.get("integer_bits"), Some(&1));
        assert_eq!(plan.stats.get("integer_policy"), Some(&1));
        assert!(plan.unified_diff(&output)?.contains("+fn demo"));
        let mut opaque_operand = records.clone();
        opaque_operand[3]["value"] = Value::Null;
        let (plan, output) = prepare(&args, &opaque_operand)?;
        assert_eq!(
            plan.review.get("generated_operand_without_source"),
            Some(&1)
        );
        assert!(std::str::from_utf8(&output[&path])?.contains("as u8"));
        Ok(())
    }

    #[test]
    fn primitive_compound_assignment_keeps_amount_before_place() {
        fn place<'a>(value: &'a mut i32, events: &mut Vec<&'static str>) -> &'a mut i32 {
            events.push("place");
            value
        }
        fn amount(events: &mut Vec<&'static str>) -> i32 {
            events.push("amount");
            2
        }
        let (mut original, mut rewritten) = (3, 3);
        let (mut before, mut after) = (vec![], vec![]);
        *place(&mut original, &mut before) += amount(&mut before);
        let numeric_amount = amount(&mut after);
        let numeric_target = &mut (*place(&mut rewritten, &mut after));
        *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
        assert_eq!(original, rewritten);
        assert_eq!(before, after);
        assert_eq!(after, ["amount", "place"]);
    }
}
