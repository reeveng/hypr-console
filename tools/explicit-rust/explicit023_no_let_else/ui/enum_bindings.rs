// run-rustfix
// edition:2024

include!("auxiliary/let_else_enum_included.rs");

enum Record {
    Count(u16),
    Label { text: String },
    Empty,
}

fn count(record: Record) -> u16 {
    let Record::Count(value) = record else {
        return 0;
    };
    value
}

fn label(record: &Record) -> &str {
    let Record::Label { text } = record else {
        ::core::panic!("expected label");
    };
    text
}

fn expect_count(record: Record) {
    let Record::Count(..) = record else {
        panic!("expected count");
    };
}

fn expect_empty(record: Record) {
    let Record::Empty = record else {
        panic!("expected empty");
    };
}

fn included(record: Included) -> u16 {
    let Included::Count(value) = record else {
        return 0;
    };
    value
}

fn read_kind() -> Result<Included, ()> {
    Ok(Included::Count(9))
}

fn read_included() -> Result<u16, ()> {
    let Included::Count(value) = read_kind()? else {
        return Err(());
    };
    Ok(value)
}

fn main() {
    assert_eq!(count(Record::Count(5)), 5);
    assert_eq!(label(&Record::Label { text: "test".into() }), "test");
    expect_count(Record::Count(1));
    expect_empty(Record::Empty);
    assert_eq!(included(Included::Other), 0);
    assert_eq!(read_included(), Ok(9));
}
