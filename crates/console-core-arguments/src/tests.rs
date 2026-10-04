use super::*;
use console_core_words::Words;

const LIST: Flag = Flag { spelling: "--list", takes: Takes::None, about: "what there is" };
const STAGE: Flag = Flag { spelling: "--stage", takes: Takes::Value("STAGE"), about: "where it runs" };
const DRY_RUN: Flag = Flag { spelling: "--dry-run", takes: Takes::None, about: "what it would do" };
const YES: Flag = Flag { spelling: "--yes", takes: Takes::None, about: "do it" };
const SECONDS: Flag = Flag { spelling: "--seconds", takes: Takes::Value("SECONDS"), about: "how long" };

const CHECK: Command = Command {
    name: "console-check",
    about: "the feature checks",
    flags: &[LIST, STAGE, DRY_RUN, YES, SECONDS],
    operands: Operands::Any("CHECK"),
};

const PLAIN: Command = Command { name: "plain", about: "takes flags and nothing else", flags: &[YES], operands: Operands::None };

const TASKS: Command = Command { name: "cargo x", about: "the tasks", flags: &[], operands: Operands::Verbatim("ARGUMENT") };

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
enum Level {
    #[words(word = "up", about = "one step brighter")]
    Up,
    #[words(word = "down", about = "one step dimmer")]
    Down,
}

impl Subcommand for Level {
    fn variants() -> Result<impl Iterator<Item = Self>, Never> {
        Ok(Level::VARIANTS.iter().copied())
    }

    fn spelling(self) -> Result<&'static str, Never> {
        self.word()
    }

    fn about(self) -> Result<&'static str, Never> {
        Level::about(self)
    }
}

const LEVEL: Command = Command { name: "level", about: "a level", flags: &[YES], operands: Operands::None };

#[test]
fn a_flag_on_the_line_is_present_and_one_off_it_is_absent() -> Result<(), ValidationError> {
    let line = read(&CHECK, &["--list"])?;

    assert_eq!(line.presence(LIST), Ok(Presence::Present));
    assert_eq!(line.presence(YES), Ok(Presence::Absent));

    Ok(())
}

#[test]
fn a_value_is_the_word_after_its_flag_and_the_last_one_wins() -> Result<(), ValidationError> {
    let line = read(&CHECK, &["--stage", "here", "--stage", "device"])?;

    assert_eq!(line.value(STAGE), Ok(Some("device")));
    let Ok(values) = line.values(STAGE);

    assert_eq!(values, ["here", "device"]);
    assert_eq!(line.value(SECONDS), Ok(None));

    Ok(())
}

#[test]
fn the_reader_refuses_a_flag_the_command_does_not_declare() {
    let line = read(&CHECK, &["--stage", "device", "--dyr-run", "--yes"]);

    assert_eq!(line.map_err(|refusal| refusal.reason), Err(Reason::NoSuchFlag("--dyr-run".to_string())));
}

#[test]
fn the_reader_refuses_a_flag_with_its_value_missing() {
    let line = read(&CHECK, &["--list", "--stage"]);

    assert_eq!(line.map_err(|refusal| refusal.reason), Err(Reason::MissingValue("--stage")));
}

#[test]
fn either_spelling_asks_for_the_usage() {
    for asking in ["--help", "-h"] {
        let line = read(&CHECK, &["--list", asking]);

        assert_eq!(line.map_err(|refusal| refusal.reason), Err(Reason::HelpRequest), "{asking}");
    }
}

#[test]
fn the_operands_keep_their_order_around_the_flags() -> Result<(), ValidationError> {
    let line = read(&CHECK, &["brightness", "--yes", "volume"])?;

    let Ok(operands) = line.operands();

    assert_eq!(operands, ["brightness", "volume"]);
    assert_eq!(line.presence(YES), Ok(Presence::Present));

    Ok(())
}

#[test]
fn a_program_that_takes_no_operands_refuses_one() {
    let line = read(&PLAIN, &["--yes", "stray"]);

    assert_eq!(line.map_err(|refusal| refusal.reason), Err(Reason::ExtraArgument("stray".to_string())));
}

#[test]
fn two_dashes_end_the_flags_and_one_dash_is_no_flag() -> Result<(), ValidationError> {
    let line = read(&CHECK, &["-5", "--", "--yes"])?;

    let Ok(operands) = line.operands();

    assert_eq!(operands, ["-5", "--yes"]);
    assert_eq!(line.presence(YES), Ok(Presence::Absent));

    Ok(())
}

#[test]
fn a_verbatim_command_hands_on_what_follows_its_first_word_as_it_is() -> Result<(), ValidationError> {
    let line = read(&TASKS, &["rules", "--locked", "-p", "console-waiting", "--help"])?;

    let Ok(operands) = line.operands();

    assert_eq!(operands, ["rules", "--locked", "-p", "console-waiting", "--help"]);

    Ok(())
}

#[test]
fn the_enum_that_says_a_word_is_the_subcommand_the_reader_finds() -> Result<(), ValidationError> {
    let up = read_with::<Level, &str>(&LEVEL, &["up", "--yes"])?;
    let nothing = read_with::<Level, &str>(&LEVEL, &[])?;
    let sideways = read_with::<Level, &str>(&LEVEL, &["sideways"]);

    assert_eq!(up.subcommand(), Ok(Some(Level::Up)));
    assert_eq!(up.presence(YES), Ok(Presence::Present));
    assert_eq!(nothing.subcommand(), Ok(None));
    assert_eq!(sideways.map_err(|refusal| refusal.reason), Err(Reason::NoSuchSubcommand("sideways".to_string())));

    Ok(())
}

#[test]
fn a_subcommand_takes_no_operand_the_command_does_not_declare() {
    let line = read_with::<Level, &str>(&LEVEL, &["up", "down"]);

    assert_eq!(line.map_err(|refusal| refusal.reason), Err(Reason::ExtraArgument("down".to_string())));
}

#[test]
fn a_value_reads_as_the_type_it_asks_for_or_refuses_with_the_word_it_has() -> Result<(), ValidationError> {
    let line = read(&CHECK, &["--seconds", "2.5"])?;
    let soon = read(&CHECK, &["--seconds", "soon"])?;

    assert_eq!(line.parsed::<f64>(SECONDS), Ok(Some(2.5)));
    assert_eq!(line.parsed::<f64>(STAGE).map_err(|refusal| refusal.reason), Ok(None), "a flag not given is no value rather than a bad one");
    assert_eq!(
        soon.parsed::<f64>(SECONDS).map_err(|refusal| refusal.reason),
        Err(Reason::InvalidValue { of: "--seconds", value: "soon".to_string() })
    );

    Ok(())
}

#[test]
fn the_declaration_draws_the_usage() -> Result<(), Never> {
    let Ok(drawn) = usage::<NoSubcommand>(&CHECK);

    assert_eq!(
        drawn,
        "usage: console-check [--list] [--stage STAGE] [--dry-run] [--yes] [--seconds SECONDS] [CHECK...]\n\
         \n\
         the feature checks\n\
         \n\
         flags:\n  \
           --list             what there is\n  \
           --stage STAGE      where it runs\n  \
           --dry-run          what it would do\n  \
           --yes              do it\n  \
           --seconds SECONDS  how long\n  \
           --help             what this program takes, which is this"
    );

    Ok(())
}

#[test]
fn the_usage_lists_every_subcommand_with_what_it_does() -> Result<(), Never> {
    let Ok(drawn) = usage::<Level>(&LEVEL);

    assert!(drawn.starts_with("usage: level [SUBCOMMAND] [--yes]\n"), "{drawn}");
    assert!(drawn.contains("subcommands:\n  up      one step brighter\n  down    one step dimmer"), "{drawn}");

    Ok(())
}

#[test]
fn a_refusal_names_the_word_it_refuses_and_the_program() {
    let line = read(&CHECK, &["--dry"]);

    assert_eq!(line.map_err(|refusal| refusal.to_string()), Err("console-check: --dry is not a flag console-check takes".to_string()));
}

#[test]
fn a_program_that_needs_a_subcommand_refuses_a_line_without_one() -> Result<(), ValidationError> {
    let told = read_with::<Level, &str>(&LEVEL, &["down"])?;
    let untold = read_with::<Level, &str>(&LEVEL, &["--yes"])?;

    assert_eq!(told.require_subcommand(), Ok(Level::Down));
    assert_eq!(untold.require_subcommand().map_err(|refusal| refusal.reason), Err(Reason::MissingSubcommand));

    Ok(())
}

#[test]
fn exactly_hands_back_the_operands_it_names_and_refuses_any_other_count() -> Result<(), ValidationError> {
    let both = read(&CHECK, &["nl_NL.UTF-8", "UTF-8"])?;
    let one = read(&CHECK, &["nl_NL.UTF-8"])?;
    let three = read(&CHECK, &["nl_NL.UTF-8", "UTF-8", "stray"])?;

    assert_eq!(both.exactly(["NAME", "CHARSET"]).map(|[name, charset]| (name.as_str(), charset.as_str())), Ok(("nl_NL.UTF-8", "UTF-8")));
    assert_eq!(one.exactly(["NAME", "CHARSET"]).map_err(|refusal| refusal.reason), Err(Reason::MissingOperands(vec!["CHARSET"])));
    assert_eq!(three.exactly(["NAME", "CHARSET"]).map_err(|refusal| refusal.reason), Err(Reason::ExtraArgument("stray".to_string())));

    Ok(())
}

#[test]
fn the_refusal_names_every_missing_operand() -> Result<(), ValidationError> {
    let none = read(&CHECK, &["--yes"])?;

    assert_eq!(
        none.exactly(["NAME", "CHARSET"]).map_err(|refusal| refusal.to_string()),
        Err("console-check: console-check needs NAME and CHARSET".to_string())
    );

    Ok(())
}

const SETTINGS: Command = Command { name: "settings-panel", about: "the settings", flags: &[], operands: Operands::Optional("TAB") };

#[test]
fn an_optional_operand_is_one_or_none_and_the_reader_refuses_a_second() -> Result<(), ValidationError> {
    let nothing: [&str; 0] = [];
    let none = read(&SETTINGS, &nothing)?;
    let sound = read(&SETTINGS, &["Sound"])?;
    let both = read(&SETTINGS, &["Sound", "Battery"]);
    let Ok(drawn) = usage::<NoSubcommand>(&SETTINGS);
    let Ok(no_tab) = none.operands();
    let Ok(one_tab) = sound.operands();

    assert!(no_tab.is_empty(), "{no_tab:?}");
    assert_eq!(one_tab, ["Sound"]);
    assert_eq!(both.map_err(|refusal| refusal.reason), Err(Reason::ExtraArgument("Battery".to_string())));
    assert!(drawn.starts_with("usage: settings-panel [TAB]\n"), "{drawn}");

    Ok(())
}

#[test]
fn one_of_hands_back_the_one_flag_present_and_refuses_none_or_two() -> Result<(), ValidationError> {
    let one = read(&CHECK, &["--yes"])?;
    let none = read(&CHECK, &["brightness"])?;
    let two = read(&CHECK, &["--list", "--yes"])?;

    assert_eq!(one.one_of(&[LIST, YES]), Ok(YES));
    assert_eq!(none.one_of(&[LIST, YES]).map_err(|refusal| refusal.reason), Err(Reason::MissingFlag(vec!["--list", "--yes"])));
    assert_eq!(two.one_of(&[LIST, YES]).map_err(|refusal| refusal.reason), Err(Reason::ExtraArgument("--yes".to_string())));

    Ok(())
}

const UNZIP: Command = Command { name: "files-unzip", about: "unpack one archive", flags: &[YES], operands: Operands::Named(&["ARCHIVE", "FOLDER"]) };

#[test]
fn named_operands_are_all_needed_and_the_usage_draws_them_bare() -> Result<(), ValidationError> {
    let both = read(&UNZIP, &["a.zip", "--yes", "here"])?;
    let one = read(&UNZIP, &["a.zip"]);
    let three = read(&UNZIP, &["a.zip", "here", "there"]);
    let asking = read(&UNZIP, &["--help"]);
    let Ok(drawn) = usage::<NoSubcommand>(&UNZIP);

    assert_eq!(both.exactly(["ARCHIVE", "FOLDER"]).map(|[archive, folder]| (archive.as_str(), folder.as_str())), Ok(("a.zip", "here")));
    assert_eq!(one.map_err(|refusal| refusal.reason), Err(Reason::MissingOperands(vec!["FOLDER"])));
    assert_eq!(three.map_err(|refusal| refusal.reason), Err(Reason::ExtraArgument("there".to_string())));
    assert_eq!(asking.map_err(|refusal| refusal.reason), Err(Reason::HelpRequest), "asking for the usage is not a missing operand");
    assert!(drawn.starts_with("usage: files-unzip [--yes] ARCHIVE FOLDER\n"), "{drawn}");

    Ok(())
}

const INTRODUCE: Command = Command { name: "console-bluetooth", about: "introduce a device", flags: &[], operands: Operands::Named(&["ADDRESS"]) };

#[test]
fn a_line_with_no_subcommand_is_missing_the_subcommand_before_its_operands() -> Result<(), ValidationError> {
    let nothing: [&str; 0] = [];
    let untold = read_with::<Level, &str>(&INTRODUCE, &nothing)?;
    let bare = read_with::<Level, &str>(&INTRODUCE, &["up"]);

    assert_eq!(untold.require_subcommand().map_err(|refusal| refusal.reason), Err(Reason::MissingSubcommand));
    assert_eq!(bare.map_err(|refusal| refusal.reason), Err(Reason::MissingOperands(vec!["ADDRESS"])));

    Ok(())
}

#[test]
fn run_main_runs_what_the_words_ask_and_ends_on_a_refusal_or_a_fault_without_it() {
    let decide = |words: &[String]| read(&PLAIN, words).map(|line| line.presence(YES));
    let said = |words: &[&str]| -> Vec<String> { words.iter().map(|word| (*word).to_string()).collect() };
    let answered = |yes: Result<Presence, Never>| {
        let Ok(yes) = yes;

        Ok::<ExitCode, Never>(match yes {
            Presence::Present => ExitCode::from(7),
            Presence::Absent => ExitCode::from(3),
        })
    };

    let Ok(asked) = run_main(&PLAIN, &said(&["--yes"]), decide, answered);
    let Ok(plain) = run_main(&PLAIN, &said(&[]), decide, answered);
    let Ok(refused) = run_main(&PLAIN, &said(&["--no"]), decide, |_never_run| Ok::<ExitCode, Never>(ExitCode::SUCCESS));
    let Ok(helped) = run_main(&PLAIN, &said(&["--help"]), decide, |_never_run| Ok::<ExitCode, Never>(ExitCode::from(7)));
    let Ok(failed) = run_main(&PLAIN, &said(&[]), decide, |_absent| Err::<(), &str>("the disk is full"));
    let Ok(finished) = run_main(&PLAIN, &said(&[]), decide, |_absent| Ok::<(), Never>(()));

    assert_eq!(asked, ExitCode::from(7), "what the program answers is the exit code");
    assert_eq!(plain, ExitCode::from(3), "the program runs on what the words asked");
    assert_eq!(refused, ExitCode::from(MISUSED), "a word nobody declared ends it before it runs");
    assert_eq!(helped, ExitCode::from(HELPED), "a request for the usage is answered, not run");
    assert_eq!(failed, ExitCode::FAILURE, "a fault is a failure");
    assert_eq!(finished, ExitCode::SUCCESS, "a program with nothing to answer finishes as a success");
}
