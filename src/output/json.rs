use crate::engine::types::SpeedTestResult;

pub fn print_json(result: &SpeedTestResult) {
    println!("{}", serde_json::to_string(result).unwrap());
}

pub fn print_json_pretty(result: &SpeedTestResult) {
    println!("{}", serde_json::to_string_pretty(result).unwrap());
}
