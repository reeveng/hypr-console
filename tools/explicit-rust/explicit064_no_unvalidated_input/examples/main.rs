#[allow(dead_code)]
#[derive(serde::Deserialize)]
struct TakenAsItComes {
    //~v WARN EXPLICIT064_NO_UNVALIDATED_INPUT
    name: String,
}

#[allow(dead_code)]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Asked {
    //~v WARN EXPLICIT064_NO_UNVALIDATED_INPUT
    name: String,
}

#[allow(dead_code)]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Bounded {
    name: [u8; 32],
}

#[allow(dead_code)]
struct Kept {
    name: String,
}

#[allow(dead_code)]
#[derive(serde::Serialize)]
struct Written {
    name: String,
}

fn main() {}
