//! Reading a script's return value into a scan.
//!
//! Everything here is defensive on purpose. The value being read was produced by source text that
//! shipped inside a fixture package, and the only acceptable outcomes are a usable scan or a
//! stated reason there is not one. A malformed point must not panic, must not be silently dropped
//! into a figure with a hole in it, and must not reach the renderer as a NaN — a single NaN
//! deflection becomes a vertex at infinity and takes the whole picture with it.

use crate::{MAX_POINTS, normalise};
use rquickjs::{Array, Ctx, Object, Value};
use viz_scene::{LaserScan, ScanPoint};

pub(crate) fn read_output(ctx: &Ctx<'_>, output: &Value<'_>) -> Result<LaserScan, String> {
    // A script with nothing to draw may say so by returning nothing at all.
    if output.is_null() || output.is_undefined() {
        return Ok(LaserScan::default());
    }
    // The convenient short form: return the points directly rather than an object wrapping them.
    let (points_value, points_per_second) = if output.is_array() {
        (output.clone(), None)
    } else {
        let object = output
            .as_object()
            .ok_or_else(|| "scan must return an object or an array of points".to_string())?;
        let points: Value = object
            .get("points")
            .map_err(|_| "scan result has no `points`".to_string())?;
        (points, read_rate(object))
    };

    let array = points_value
        .as_array()
        .ok_or_else(|| "scan result `points` is not an array".to_string())?;
    if array.len() > MAX_POINTS {
        return Err(format!(
            "scan returned {} points, more than the {MAX_POINTS} a scan may contain",
            array.len()
        ));
    }

    let mut points = Vec::with_capacity(array.len());
    for (index, entry) in array.iter::<Value>().enumerate() {
        let entry =
            entry.map_err(|error| format!("scan point {index} could not be read: {error}"))?;
        points.push(read_point(ctx, &entry, index)?);
    }
    normalise(&mut points);
    Ok(LaserScan {
        points,
        // The engine does not read the fixture's slots back out; the decoder owns them.
        slots: Vec::new(),
        // Zero means "the fixture's own rated speed"; only an explicit, sane figure overrides it.
        points_per_second: points_per_second.unwrap_or(0.0),
        fault: None,
    })
}

fn read_rate(object: &Object<'_>) -> Option<f32> {
    let rate: f64 = object
        .get("pointsPerSecond")
        .or_else(|_| object.get("points_per_second"))
        .ok()?;
    let rate = rate as f32;
    (rate.is_finite() && rate > 0.0).then_some(rate)
}

fn read_point(ctx: &Ctx<'_>, entry: &Value<'_>, index: usize) -> Result<ScanPoint, String> {
    if let Some(array) = entry.as_array() {
        return read_compact_point(array, index);
    }
    let object = entry
        .as_object()
        .ok_or_else(|| format!("scan point {index} is neither an object nor an array"))?;
    let _ = ctx;
    Ok(ScanPoint {
        x: number(object, "x", index)?,
        y: number(object, "y", index)?,
        colour: [
            optional_number(object, "r"),
            optional_number(object, "g"),
            optional_number(object, "b"),
        ],
        dwell: share(optional_number(object, "amount")),
    })
}

/// `[x, y, r, g, b, amount]` — worth having once a figure runs to hundreds of points, where the
/// object form spends more time allocating property maps than the scan does computing.
fn read_compact_point(array: &Array<'_>, index: usize) -> Result<ScanPoint, String> {
    if array.len() < 2 {
        return Err(format!(
            "scan point {index} needs at least an x and a y deflection"
        ));
    }
    let at = |slot: usize| -> f32 {
        array
            .get::<f64>(slot)
            .ok()
            .map(|value| value as f32)
            .filter(|value| value.is_finite())
            .unwrap_or(0.0)
    };
    let x = at(0);
    let y = at(1);
    if !x.is_finite() || !y.is_finite() {
        return Err(format!(
            "scan point {index} has a deflection that is not a number"
        ));
    }
    Ok(ScanPoint {
        x,
        y,
        colour: [at(2), at(3), at(4)],
        dwell: share(at(5)),
    })
}

fn number(object: &Object<'_>, field: &str, index: usize) -> Result<f32, String> {
    let value: f64 = object
        .get(field)
        .map_err(|_| format!("scan point {index} has no `{field}`"))?;
    let value = value as f32;
    if !value.is_finite() {
        return Err(format!(
            "scan point {index} has a `{field}` that is not a number"
        ));
    }
    Ok(value)
}

/// A missing colour channel is off, not an error: a script drawing in red has no reason to write
/// `g: 0, b: 0` on every point of a figure.
fn optional_number(object: &Object<'_>, field: &str) -> f32 {
    object
        .get::<_, f64>(field)
        .ok()
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
}

/// The script speaks in percent of the scan; everything downstream works in fractions.
fn share(amount: f32) -> f32 {
    if !amount.is_finite() || amount <= 0.0 {
        return 0.0;
    }
    amount / 100.0
}
