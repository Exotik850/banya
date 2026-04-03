use serde_json::Value as JsonValue;

use crate::builtin::registry::{
    NativeCapabilityRoute, NativeFunctionInfoBuilder, RoutedNativeFunction, native_capability,
    native_capability_route, native_function_info,
};

/// A logical AND function that evaluates multiple boolean conditions.
///
/// This is a compile-time native function that can evaluate complex
/// boolean logic without the overhead of WASM serialization.
///
/// # Example (JSON)
/// ```json
/// {
///   "plugin": "logical-and",
///   "capability": "evaluate",
///   "conditions": [true, true, false]
/// }
/// ```
pub struct LogicalAnd;

impl LogicalAnd {
    fn evaluate(&self, args: &JsonValue) -> Result<JsonValue, String> {
        let conditions = args
            .get("conditions")
            .and_then(|v| v.as_array())
            .ok_or("Missing or invalid 'conditions' argument (expected array)")?;

        let result = conditions.iter().all(|c| match c {
            JsonValue::Bool(b) => *b,
            JsonValue::Null => false,
            _ => true,
        });

        Ok(JsonValue::Bool(result))
    }
}

impl RoutedNativeFunction for LogicalAnd {
    fn info_builder(&self) -> NativeFunctionInfoBuilder {
        native_function_info("logical-and", "1.0.0")
            .description("Evaluates multiple conditions and returns true if ALL are true")
            .author("banya")
    }

    fn capability_routes(&self) -> Vec<NativeCapabilityRoute<Self>> {
        vec![native_capability_route(
            native_capability("evaluate")
                .description("Evaluate boolean AND logic")
                .input("conditions", "array<bool>")
                .output("bool"),
            Self::evaluate,
        )]
    }
}

/// A logical OR function that evaluates multiple boolean conditions.
///
/// # Example (JSON)
/// ```json
/// {
///   "plugin": "logical-or",
///   "capability": "evaluate",
///   "conditions": [false, true, false]
/// }
/// ```
pub struct LogicalOr;

impl LogicalOr {
    fn evaluate(&self, args: &JsonValue) -> Result<JsonValue, String> {
        let conditions = args
            .get("conditions")
            .and_then(|v| v.as_array())
            .ok_or("Missing or invalid 'conditions' argument (expected array)")?;

        let result = conditions.iter().any(|c| match c {
            JsonValue::Bool(b) => *b,
            JsonValue::Null => false,
            _ => true,
        });

        Ok(JsonValue::Bool(result))
    }
}

impl RoutedNativeFunction for LogicalOr {
    fn info_builder(&self) -> NativeFunctionInfoBuilder {
        native_function_info("logical-or", "1.0.0")
            .description("Evaluates multiple conditions and returns true if ANY are true")
            .author("banya")
    }

    fn capability_routes(&self) -> Vec<NativeCapabilityRoute<Self>> {
        vec![native_capability_route(
            native_capability("evaluate")
                .description("Evaluate boolean OR logic")
                .input("conditions", "array<bool>")
                .output("bool"),
            Self::evaluate,
        )]
    }
}

/// A logical NOT function that inverts a boolean condition.
///
/// # Example (JSON)
/// ```json
/// {
///   "plugin": "logical-not",
///   "capability": "evaluate",
///   "value": true
/// }
/// ```
pub struct LogicalNot;

impl LogicalNot {
    fn evaluate(&self, args: &JsonValue) -> Result<JsonValue, String> {
        let value = args
            .get("value")
            .and_then(serde_json::Value::as_bool)
            .ok_or("Missing or invalid 'value' argument (expected bool)")?;

        Ok(JsonValue::Bool(!value))
    }
}

impl RoutedNativeFunction for LogicalNot {
    fn info_builder(&self) -> NativeFunctionInfoBuilder {
        native_function_info("logical-not", "1.0.0")
            .description("Inverts a boolean condition")
            .author("banya")
    }

    fn capability_routes(&self) -> Vec<NativeCapabilityRoute<Self>> {
        vec![native_capability_route(
            native_capability("evaluate")
                .description("Evaluate boolean NOT logic")
                .input("value", "bool")
                .output("bool"),
            Self::evaluate,
        )]
    }
}

/// A comparison function that compares two values.
///
/// Supports operators: eq, ne, gt, lt, gte, lte
///
/// # Example (JSON)
/// ```json
/// {
///   "plugin": "compare",
///   "capability": "evaluate",
///   "operator": "gt",
///   "left": 42,
///   "right": 10
/// }
/// ```
pub struct Compare;

impl Compare {
    fn evaluate(&self, args: &JsonValue) -> Result<JsonValue, String> {
        let operator = args
            .get("operator")
            .and_then(|v| v.as_str())
            .ok_or("Missing or invalid 'operator' argument")?;

        let left = args.get("left").ok_or("Missing 'left' argument")?;
        let right = args.get("right").ok_or("Missing 'right' argument")?;

        let result = match operator {
            "eq" => left == right,
            "ne" => left != right,
            "gt" => compare_values(left, right).is_some_and(std::cmp::Ordering::is_gt),
            "lt" => compare_values(left, right).is_some_and(std::cmp::Ordering::is_lt),
            "gte" => compare_values(left, right).is_some_and(std::cmp::Ordering::is_ge),
            "lte" => compare_values(left, right).is_some_and(std::cmp::Ordering::is_le),
            _ => return Err(format!("Unknown operator: {operator}")),
        };

        Ok(JsonValue::Bool(result))
    }
}

impl RoutedNativeFunction for Compare {
    fn info_builder(&self) -> NativeFunctionInfoBuilder {
        native_function_info("compare", "1.0.0")
            .description("Compares two values using the specified operator")
            .author("banya")
    }

    fn capability_routes(&self) -> Vec<NativeCapabilityRoute<Self>> {
        vec![native_capability_route(
            native_capability("evaluate")
                .description("Compare two values")
                .inputs([("operator", "string"), ("left", "any"), ("right", "any")])
                .output("bool"),
            Self::evaluate,
        )]
    }
}

fn compare_values(left: &JsonValue, right: &JsonValue) -> Option<std::cmp::Ordering> {
    match (left, right) {
        (JsonValue::Number(l), JsonValue::Number(r)) => {
            let l = l.as_f64()?;
            let r = r.as_f64()?;
            l.partial_cmp(&r)
        }
        (JsonValue::String(l), JsonValue::String(r)) => Some(l.cmp(r)),
        (JsonValue::Bool(l), JsonValue::Bool(r)) => Some(l.cmp(r)),
        _ => None,
    }
}

/// A string manipulation function for compile-time string operations.
///
/// Supports operations: concat, upper, lower, trim, split, replace, contains, `starts_with`, `ends_with`
///
/// # Example (JSON)
/// ```json
/// {
///   "plugin": "string-ops",
///   "capability": "transform",
///   "operation": "concat",
///   "strings": ["Hello", " ", "World"]
/// }
/// ```
pub struct StringOps;

impl RoutedNativeFunction for StringOps {
    fn info_builder(&self) -> NativeFunctionInfoBuilder {
        native_function_info("string-ops", "1.0.0")
            .description("String manipulation operations")
            .author("banya")
    }

    fn capability_routes(&self) -> Vec<NativeCapabilityRoute<Self>> {
        vec![
            native_capability_route(
                native_capability("evaluate")
                    .description("Evaluate string conditions")
                    .inputs([
                        ("operation", "string"),
                        ("value", "string"),
                        ("pattern", "string"),
                    ])
                    .output("bool"),
                Self::handle_evaluate,
            ),
            native_capability_route(
                native_capability("transform")
                    .description("Transform strings")
                    .inputs([("operation", "string"), ("value", "string")])
                    .output("string"),
                Self::handle_transform,
            ),
        ]
    }
}
impl StringOps {
    fn handle_transform(&self, args: &JsonValue) -> Result<JsonValue, String> {
        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or("Missing or invalid 'operation' argument")?;
        self.transform(operation, args)
    }

    fn handle_evaluate(&self, args: &JsonValue) -> Result<JsonValue, String> {
        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or("Missing or invalid 'operation' argument")?;
        self.evaluate(operation, args)
    }

    fn transform(&self, operation: &str, args: &JsonValue) -> Result<JsonValue, String> {
        match operation {
            "concat" => {
                let strings = args
                    .get("strings")
                    .and_then(|v| v.as_array())
                    .ok_or("Missing or invalid 'strings' argument")?;
                let result: String = strings.iter().filter_map(|v| v.as_str()).collect();
                Ok(JsonValue::String(result))
            }
            "upper" => {
                let value = args
                    .get("value")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing or invalid 'value' argument")?;
                Ok(JsonValue::String(value.to_uppercase()))
            }
            "lower" => {
                let value = args
                    .get("value")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing or invalid 'value' argument")?;
                Ok(JsonValue::String(value.to_lowercase()))
            }
            "trim" => {
                let value = args
                    .get("value")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing or invalid 'value' argument")?;
                Ok(JsonValue::String(value.trim().to_string()))
            }
            "split" => {
                let value = args
                    .get("value")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing or invalid 'value' argument")?;
                let delimiter = args
                    .get("delimiter")
                    .and_then(|v| v.as_str())
                    .unwrap_or(",");
                let result: Vec<JsonValue> = value
                    .split(delimiter)
                    .map(|s| JsonValue::String(s.to_string()))
                    .collect();
                Ok(JsonValue::Array(result))
            }
            "replace" => {
                let value = args
                    .get("value")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing or invalid 'value' argument")?;
                let pattern = args
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing or invalid 'pattern' argument")?;
                let replacement = args
                    .get("replacement")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                Ok(JsonValue::String(value.replace(pattern, replacement)))
            }
            _ => Err(format!("Unknown transform operation: {operation}")),
        }
    }

    fn evaluate(&self, operation: &str, args: &JsonValue) -> Result<JsonValue, String> {
        let value = args
            .get("value")
            .and_then(|v| v.as_str())
            .ok_or("Missing or invalid 'value' argument")?;

        match operation {
            "contains" => {
                let pattern = args
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing or invalid 'pattern' argument")?;
                Ok(JsonValue::Bool(value.contains(pattern)))
            }
            "starts_with" => {
                let pattern = args
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing or invalid 'pattern' argument")?;
                Ok(JsonValue::Bool(value.starts_with(pattern)))
            }
            "ends_with" => {
                let pattern = args
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing or invalid 'pattern' argument")?;
                Ok(JsonValue::Bool(value.ends_with(pattern)))
            }
            "is_empty" => Ok(JsonValue::Bool(value.is_empty())),
            _ => Err(format!("Unknown evaluate operation: {operation}")),
        }
    }
}

/// A math operations function for compile-time calculations.
///
/// Supports operations: add, sub, mul, div, mod, pow, abs, min, max, clamp
///
/// # Example (JSON)
/// ```json
/// {
///   "plugin": "math",
///   "capability": "calculate",
///   "operation": "add",
///   "values": [1, 2, 3, 4, 5]
/// }
/// ```
pub struct Math;

impl Math {
    fn calculate(&self, args: &JsonValue) -> Result<JsonValue, String> {
        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or("Missing or invalid 'operation' argument")?;

        let values = args
            .get("values")
            .and_then(|v| v.as_array())
            .ok_or("Missing or invalid 'values' argument (expected array)")?;

        let nums: Vec<f64> = values.iter().filter_map(serde_json::Value::as_f64).collect();

        if nums.is_empty() {
            return Err("No valid numeric values provided".to_string());
        }

        let result = match operation {
            "add" => nums.iter().sum::<f64>(),
            "sub" => nums.iter().skip(1).fold(nums[0], |acc, &x| acc - x),
            "mul" => nums.iter().product::<f64>(),
            "div" => {
                if nums.len() < 2 {
                    return Err("Division requires at least 2 values".to_string());
                }
                let mut result = nums[0];
                for &n in &nums[1..] {
                    if n == 0.0 {
                        return Err("Division by zero".to_string());
                    }
                    result /= n;
                }
                result
            }
            "mod" => {
                if nums.len() != 2 {
                    return Err("Modulo requires exactly 2 values".to_string());
                }
                if nums[1] == 0.0 {
                    return Err("Modulo by zero".to_string());
                }
                nums[0] % nums[1]
            }
            "pow" => {
                if nums.len() != 2 {
                    return Err("Power requires exactly 2 values".to_string());
                }
                nums[0].powf(nums[1])
            }
            "abs" => {
                if nums.len() != 1 {
                    return Err("Abs requires exactly 1 value".to_string());
                }
                nums[0].abs()
            }
            "min" => nums.iter().copied().fold(f64::INFINITY, f64::min),
            "max" => nums.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            "clamp" => {
                if nums.len() != 3 {
                    return Err("Clamp requires exactly 3 values (value, min, max)".to_string());
                }
                nums[0].clamp(nums[1], nums[2])
            }
            _ => return Err(format!("Unknown math operation: {operation}")),
        };

        // Return as integer if it's a whole number
        if result.fract() == 0.0 && result.abs() < i64::MAX as f64 {
            Ok(JsonValue::Number(serde_json::Number::from(result as i64)))
        } else {
            serde_json::Number::from_f64(result)
                .map(JsonValue::Number)
                .ok_or_else(|| "Result is not a valid JSON number".to_string())
        }
    }
}

impl RoutedNativeFunction for Math {
    fn info_builder(&self) -> NativeFunctionInfoBuilder {
        native_function_info("math", "1.0.0")
            .description("Mathematical operations")
            .author("banya")
    }

    fn capability_routes(&self) -> Vec<NativeCapabilityRoute<Self>> {
        vec![native_capability_route(
                native_capability("calculate")
                    .description("Perform mathematical calculations")
                    .inputs([("operation", "string"), ("values", "array<number>")])
                    .output("number"),
                Self::calculate,
            )]
    }
}
