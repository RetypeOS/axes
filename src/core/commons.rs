// Helper function to wrap a string in quotes and escape internal quotes.
pub fn wrap_value(value: &str) -> String {
    // Escape any existing double quotes and then wrap the whole string in double quotes.
    format!("\"{}\"", value.replace('"', "\\\""))
}
