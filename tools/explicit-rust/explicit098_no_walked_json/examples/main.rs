use serde_json::Value;

fn guessed(said: &str) -> Option<i64> {
    //~v WARN EXPLICIT098_NO_WALKED_JSON
    let reply: Value = match serde_json::from_str(said) {
        Ok(reply) => reply,
        Err(_) => return None,
    };

    reply.get("windows").and_then(Value::as_i64)
}

fn guessed_from_bytes(said: &[u8]) -> Result<Value, serde_json::Error> {
    //~v WARN EXPLICIT098_NO_WALKED_JSON
    serde_json::from_slice::<Value>(said)
}

fn known(said: &str) -> Result<Vec<u32>, serde_json::Error> {
    serde_json::from_str::<Vec<u32>>(said)
}

fn main() {
    let _ = guessed("{}");
    let _ = guessed_from_bytes(b"{}");
    let _ = known("[1]");
}
