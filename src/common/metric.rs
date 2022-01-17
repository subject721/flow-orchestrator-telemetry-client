use json;
use json::{JsonError, JsonValue};
use std::fmt;
use std::fmt::{Debug, Display, Formatter};
use std::slice::Iter;
use std::str::FromStr;
use std::string::String;

#[derive(Debug, PartialEq, Clone)]
pub enum MetricValue {
    Empty,
    Integer(i64),
    Number(f64),
    String(String),
}

#[derive(Debug, PartialEq, Clone)]
pub enum MetricRawUnit {
    None,
    Packets,
    Bits,
    Bytes,
    Seconds,
}

#[derive(Debug, PartialEq, Clone)]
pub enum OrderOfMagnitude {
    Nano,
    Micro,
    Milli,
    One,
    Kilo,
    Mega,
    Giga,
    Tera
}

#[derive(Debug, Clone)]
pub struct OoMParseError {

}

#[derive(Debug, PartialEq, Clone)]
pub struct MetricUnit {
    raw_unit_num : MetricRawUnit,
    raw_unit_den : MetricRawUnit,
    order_of_magnitude : OrderOfMagnitude
}

#[derive(Debug, PartialEq, Clone)]
pub struct Metric {
    label: String,
    unit : MetricUnit,
    value: MetricValue,
}

impl OrderOfMagnitude {
    pub fn get_exponent(&self) -> i32 {
        match self {
            OrderOfMagnitude::Nano => {-9}
            OrderOfMagnitude::Micro => {-6}
            OrderOfMagnitude::Milli => {-3}
            OrderOfMagnitude::One => {0}
            OrderOfMagnitude::Kilo => {3}
            OrderOfMagnitude::Mega => {6}
            OrderOfMagnitude::Giga => {9}
            OrderOfMagnitude::Tera => {12}
        }
    }

    pub fn get_factor(&self) -> f64 {
        10.0_f64.powi(self.get_exponent())
    }

    pub fn get_factor_rat(&self) -> (u64, u64) {
        let e = self.get_exponent();

        if e >= 0 {
            (10u64.pow(e as u32), 1)
        } else {
            (1, 10u64.pow(-e as u32))
        }
    }

    pub fn get_abbr(&self) -> &str {
        match self {
            OrderOfMagnitude::Nano => {"n"}
            OrderOfMagnitude::Micro => {"u"}
            OrderOfMagnitude::Milli => {"m"}
            OrderOfMagnitude::One => {""}
            OrderOfMagnitude::Kilo => {"k"}
            OrderOfMagnitude::Mega => {"M"}
            OrderOfMagnitude::Giga => {"G"}
            OrderOfMagnitude::Tera => {"T"}
        }
    }
}


impl Display for OoMParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "invalid order of magnitude")
    }
}

impl std::error::Error for OoMParseError {

}

impl TryFrom<&str> for OrderOfMagnitude {
    type Error = OoMParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "n" => Ok(OrderOfMagnitude::Nano),
            "u" => Ok(OrderOfMagnitude::Micro),
            "m" => Ok(OrderOfMagnitude::Milli),
            "k" => Ok(OrderOfMagnitude::Kilo),
            "M" => Ok(OrderOfMagnitude::Mega),
            "G" => Ok(OrderOfMagnitude::Giga),
            "T" => Ok(OrderOfMagnitude::Tera),
            _ => Err(Self::Error{})
        }
    }
}

impl MetricUnit {
    pub fn empty() -> MetricUnit {
        MetricUnit {
            raw_unit_num: MetricRawUnit::None,
            raw_unit_den: MetricRawUnit::None,
            order_of_magnitude: OrderOfMagnitude::One
        }
    }

    pub fn new(unit_num : MetricRawUnit, unit_den : MetricRawUnit, oom : OrderOfMagnitude) -> MetricUnit {
        MetricUnit {
            raw_unit_num: unit_num,
            raw_unit_den: unit_den,
            order_of_magnitude: oom
        }
    }

    pub fn get_raw_unit(&self) -> (&MetricRawUnit, &MetricRawUnit) {
        (&self.raw_unit_num, &self.raw_unit_den)
    }

    pub fn get_order_of_magnitude(&self) -> &OrderOfMagnitude {
        &self.order_of_magnitude
    }
}

impl fmt::Display for MetricRawUnit {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            MetricRawUnit::None => {fmt::Result::Ok(())}
            MetricRawUnit::Packets => {write!(f, "pkts")}
            MetricRawUnit::Bits => {write!(f, "bits")}
            MetricRawUnit::Bytes => {write!(f, "bytes")}
            MetricRawUnit::Seconds => {write!(f, "sec")}
        }
    }
}

impl fmt::Display for MetricUnit {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.raw_unit_den {
            MetricRawUnit::None => {
                write!(f, "{}{}", self.order_of_magnitude.get_abbr(), self.raw_unit_num.to_string())
            },
            _ => {
                write!(f, "{}{}/{}", self.order_of_magnitude.get_abbr(), self.raw_unit_num.to_string(), self.raw_unit_den.to_string())
            }
        }

    }
}

impl TryFrom<&json::JsonValue> for MetricUnit {
    type Error = json::JsonError;

    fn try_from(value: &JsonValue) -> Result<Self, Self::Error> {
        match value.as_str() {
            Some(value) => {
                let mut offset = 0;

                while offset < 2 {
                    let first = &value[..offset];
                    let shifted_str = &value[offset..];

                    let oom : Option<OrderOfMagnitude> = OrderOfMagnitude::try_from(first).ok();

                    match shifted_str {
                        "pkts" => {return Ok(MetricUnit{ raw_unit_num: MetricRawUnit::Packets, raw_unit_den: MetricRawUnit::None, order_of_magnitude: oom.unwrap_or(OrderOfMagnitude::One) })},
                        "bits" => {return Ok(MetricUnit{ raw_unit_num: MetricRawUnit::Bits, raw_unit_den: MetricRawUnit::None, order_of_magnitude: oom.unwrap_or(OrderOfMagnitude::One) })},
                        "bytes" => {return Ok(MetricUnit{ raw_unit_num: MetricRawUnit::Bytes, raw_unit_den: MetricRawUnit::None, order_of_magnitude: oom.unwrap_or(OrderOfMagnitude::One) })},
                        "sec" => {return Ok(MetricUnit{ raw_unit_num: MetricRawUnit::Seconds, raw_unit_den: MetricRawUnit::None, order_of_magnitude: oom.unwrap_or(OrderOfMagnitude::One) })},
                        _ => ()
                    }

                    offset += 1
                }
            },
            None => ()
        }

        Ok(MetricUnit{ raw_unit_num: MetricRawUnit::None, raw_unit_den: MetricRawUnit::None, order_of_magnitude: OrderOfMagnitude::One })
    }
}

impl fmt::Display for MetricValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetricValue::Empty => fmt::Result::Ok(()),
            MetricValue::Integer(value) => {
                write!(f, "{}", value)
            }
            MetricValue::Number(value) => {
                write!(f, "{}", value)
            }
            MetricValue::String(value) => {
                write!(f, "\"{}\"", value)
            }
        }
    }
}
impl From<&MetricValue> for f64 {
    fn from(metric_value: &MetricValue) -> Self {
        match metric_value {
            MetricValue::Empty => 0.0f64,
            MetricValue::Integer(value) => *value as f64,
            MetricValue::Number(value) => *value,
            MetricValue::String(_) => 0.0f64
        }
    }
}


// impl Into<f64> for MetricValue {
//
//     fn into(&self) -> f64 {
//         match self {
//             MetricValue::Empty => 0.0f64,
//             MetricValue::Integer(value) => f64::from_i64(*value).unwrap(),
//             MetricValue::Number(value) => *value,
//             MetricValue::String(value) => 0.0f64
//         }
//     }
// }

impl TryFrom<&json::JsonValue> for MetricValue {
    type Error = json::Error;

    fn try_from(value: &JsonValue) -> Result<Self, Self::Error> {
        match value {
            JsonValue::Object(o) => {
                let type_field = o["type"].as_str();
                let value_field = &o["value"];

                return if let Some(type_str) = type_field {
                    match type_str {
                        "empty" => Ok(MetricValue::Empty),
                        "string" => Ok(MetricValue::String(
                            value_field
                                .as_str()
                                .ok_or(JsonError::WrongType(
                                    "could not convert value to string".to_string(),
                                ))?
                                .to_string(),
                        )),
                        "integer" => Ok(MetricValue::Integer(value_field.as_i64().ok_or(
                            JsonError::WrongType("could not convert value to string".to_string()),
                        )?)),
                        "number" => Ok(MetricValue::Number(value_field.as_f64().ok_or(
                            JsonError::WrongType("could not convert value to string".to_string()),
                        )?)),
                        _ => Err(JsonError::WrongType("unknown type".to_string())),
                    }
                } else {
                    Err(JsonError::WrongType(
                        "type field is missing or has wrong type".to_string(),
                    ))
                };
            }
            _ => Err(Self::Error::WrongType("invalid json type".to_string())),
        }
    }
}

impl fmt::Display for Metric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} {}", &self.label, &self.value, self.unit.to_string())
    }
}

impl TryFrom<&json::JsonValue> for Metric {
    type Error = json::JsonError;

    fn try_from(value: &JsonValue) -> Result<Self, Self::Error> {
        match value {
            json::JsonValue::Object(obj) => {
                let label_field = obj["label"].as_str();
                let metric_value_obj = &obj["value"];
                let unit_field = &obj["unit"];

                return if let Some(label_str) = label_field {
                    Ok(Metric {
                        label: label_str.to_string(),
                        unit: MetricUnit::try_from(unit_field)?,
                        value: MetricValue::try_from(metric_value_obj)?,
                    })
                } else {
                    Err(Self::Error::WrongType("invalid json type".to_string()))
                };
            }
            _ => Err(Self::Error::WrongType("invalid json type".to_string())),
        }
    }
}

impl Metric {
    pub fn new(label : String, unit : MetricUnit, value : MetricValue) -> Metric {
        Metric {
            label,
            unit,
            value
        }
    }

    pub fn get_label(&self) -> &str {
        &self.label
    }

    pub fn get_unit(&self) -> &MetricUnit {
        &self.unit
    }

    pub fn get_value(&self) -> &MetricValue {
        &self.value
    }
}



#[test]
fn metric_value_creation_test01() {
    let mobj = json::parse(r#"{
                "type": "integer",
                "value": 123456}"#).unwrap();

    let value = MetricValue::try_from(&mobj).unwrap();

    assert_eq!(value, MetricValue::Integer(123456));


}