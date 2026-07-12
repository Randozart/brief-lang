use crate::interpreter::{RuntimeError, Value};

pub fn len_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::len_impl(args)
}
pub fn concat_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::concat_impl(args)
}
pub fn trim_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::trim_impl(args)
}
pub fn contains_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::contains_impl(args)
}
pub fn to_lower_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::to_lower_impl(args)
}
pub fn to_upper_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::to_upper_impl(args)
}
pub fn replace_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::replace_impl(args)
}
pub fn chars_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::chars_impl(args)
}
pub fn starts_with_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::starts_with_impl(args)
}
pub fn ends_with_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::ends_with_impl(args)
}
pub fn from_str_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::from_str_impl(args)
}
pub fn to_string_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::to_string_impl(args)
}
pub fn string_trim_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::string_trim_impl(args)
}
pub fn string_to_lower_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::string_to_lower_impl(args)
}
pub fn string_contains_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::string_contains_impl(args)
}
pub fn string_starts_with_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::string_starts_with_impl(args)
}
pub fn string_split_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::string_split_impl(args)
}
pub fn substring_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::substring_impl(args)
}
pub fn int_to_string_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::int_to_string_impl(args)
}

