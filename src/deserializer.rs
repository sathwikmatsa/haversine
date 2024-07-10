use crate::{HaversineData, HaversineDataPoint};

/// Sample `HaversineData` JSON:
/// {
///  "pairs": [
///    {
///      "x0": 33.645001259581676,
///      "y0": -22.58786090058659,
///      "x1": -7.917869055261946,
///      "y1": 50.3982354259912
///    },
///    {
///      "x0": 177.74381301443074,
///      "y0": 67.14837062236548,
///      "x1": 176.66072571102146,
///      "y1": 62.52409931003097
///    }
///  ]
///}
use nom::{
    bytes::complete::take_while1,
    character::{
        complete::{char, multispace0},
        is_alphanumeric,
    },
    combinator::opt,
    error::ParseError,
    multi::many0,
    number::complete::double,
    sequence::{delimited, preceded, terminated, tuple},
    IResult, Parser,
};

fn ws<'a, F, O, E: ParseError<&'a [u8]>>(inner: F) -> impl Parser<&'a [u8], O, E>
where
    F: Parser<&'a [u8], O, E>,
{
    delimited(multispace0, inner, multispace0)
}

fn eat_char(c: char) -> impl Fn(&[u8]) -> IResult<&[u8], char> {
    move |i: &[u8]| ws(char(c)).parse(i)
}

fn key(i: &[u8]) -> IResult<&[u8], String> {
    let (rem, key) = ws(terminated(
        delimited(char('"'), take_while1(is_alphanumeric), char('"')),
        ws(eat_char(':')),
    ))
    .parse(i)?;
    Ok((rem, unsafe { String::from_utf8_unchecked(key.to_vec()) }))
}

fn coordinate() -> impl Fn(&[u8]) -> IResult<&[u8], (String, f64)> {
    move |i: &[u8]| {
        let (rem, (key, val)) = tuple((key, double)).parse(i)?;
        Ok((rem, (key, val)))
    }
}

fn haversine_datapoint(i: &[u8]) -> IResult<&[u8], HaversineDataPoint> {
    let (input, (_, c1, _, c2, _, c3, _, c4, _)) = tuple((
        eat_char('{'),
        coordinate(),
        eat_char(','),
        coordinate(),
        eat_char(','),
        coordinate(),
        eat_char(','),
        coordinate(),
        eat_char('}'),
    ))
    .parse(i)?;

    let mut x0 = None;
    let mut y0 = None;
    let mut x1 = None;
    let mut y1 = None;

    for (key, val) in [c1, c2, c3, c4] {
        match key.as_str() {
            "x0" => x0 = Some(val),
            "x1" => x1 = Some(val),
            "y0" => y0 = Some(val),
            "y1" => y1 = Some(val),
            _ => (),
        }
    }

    let validation_error = Err(nom::Err::Failure(nom::error::Error::new(
        input,
        nom::error::ErrorKind::Fail,
    )));

    if x0.is_none() || x1.is_none() || y0.is_none() || y1.is_none() {
        return validation_error;
    }

    Ok((
        input,
        HaversineDataPoint {
            x0: unsafe { x0.unwrap_unchecked() },
            x1: unsafe { x1.unwrap_unchecked() },
            y0: unsafe { y0.unwrap_unchecked() },
            y1: unsafe { y1.unwrap_unchecked() },
        },
    ))
}

fn haversine_datapoint_array(i: &[u8]) -> IResult<&[u8], Vec<HaversineDataPoint>> {
    let (rem, (_, mut any, last, _)) = tuple((
        eat_char('['),
        many0(terminated(haversine_datapoint, eat_char(','))),
        opt(haversine_datapoint),
        eat_char(']'),
    ))
    .parse(i)?;
    if last.is_some() {
        unsafe {
            any.push(last.unwrap_unchecked());
        }
    }
    Ok((rem, any))
}

fn haversine_data(i: &[u8]) -> IResult<&[u8], HaversineData> {
    let (rem, pairs) = delimited(
        eat_char('{'),
        preceded(key, haversine_datapoint_array),
        eat_char('}'),
    )
    .parse(i)?;
    Ok((rem, HaversineData { pairs }))
}

impl HaversineData {
    #[allow(clippy::pedantic)]
    #[allow(clippy::result_unit_err)]
    pub fn parse_from_json_slice(bytes: &[u8]) -> Result<HaversineData, ()> {
        haversine_data(bytes).map(|(_, data)| data).map_err(|_| ())
    }
}

#[cfg(test)]
#[allow(clippy::unreadable_literal)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_coordinate() {
        let coordinate_slice = br#""x0": 123.456"#;
        let out = coordinate()(coordinate_slice);
        assert!(out.is_ok());
        assert_eq!(out.unwrap().1, (String::from("x0"), 123.456f64));
    }

    #[test]
    fn deserialize_haversine_datapoint() {
        let datapoint_slice = br#"{
            "y0"   : 67.14837062236548,
            "x0": 177.74381301443074    ,
            "x1"   : 176.66072571102146   ,
            "y1": 62.52409931003097
        }"#;

        let out = haversine_datapoint(datapoint_slice);
        assert!(out.is_ok());
        assert_eq!(
            out.unwrap().1,
            HaversineDataPoint {
                x0: 177.74381301443074,
                y0: 67.14837062236548,
                x1: 176.66072571102146,
                y1: 62.52409931003097
            }
        );
    }

    #[test]
    fn deserialize_haversine_datapoint_array() {
        let slice = br#"[
            {
                "x0": 33.645001259581676,
                "y0": -22.58786090058659,
                "x1": -7.917869055261946,
                "y1": 50.3982354259912
            },
            {
                "x0": 177.74381301443074,
                "y0": 67.14837062236548,
                "x1": 176.66072571102146,
                "y1": 62.52409931003097
            }
        ]"#;
        let out = haversine_datapoint_array(slice);
        assert!(out.is_ok());
        assert_eq!(
            out.unwrap().1,
            vec![
                HaversineDataPoint {
                    x0: 33.645001259581676,
                    y0: -22.58786090058659,
                    x1: -7.917869055261946,
                    y1: 50.3982354259912
                },
                HaversineDataPoint {
                    x0: 177.74381301443074,
                    y0: 67.14837062236548,
                    x1: 176.66072571102146,
                    y1: 62.52409931003097
                }
            ]
        );
    }

    #[test]
    fn deserialize_haversine_data() {
        let slice = br#"{
            "pairs": [
                {
                    "x0": 33.645001259581676,
                    "y0": -22.58786090058659,
                    "x1": -7.917869055261946,
                    "y1": 50.3982354259912
                },
                {
                    "x0": 177.74381301443074,
                    "y0": 67.14837062236548,
                    "x1": 176.66072571102146,
                    "y1": 62.52409931003097
                }
            ]
        }"#;
        let out = haversine_data(slice);
        assert!(out.is_ok());
        assert_eq!(
            out.unwrap().1,
            HaversineData {
                pairs: vec![
                    HaversineDataPoint {
                        x0: 33.645001259581676,
                        y0: -22.58786090058659,
                        x1: -7.917869055261946,
                        y1: 50.3982354259912
                    },
                    HaversineDataPoint {
                        x0: 177.74381301443074,
                        y0: 67.14837062236548,
                        x1: 176.66072571102146,
                        y1: 62.52409931003097
                    }
                ]
            }
        );
    }
}
