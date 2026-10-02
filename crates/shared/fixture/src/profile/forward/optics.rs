//! Focus is lens travel, not a measured focal distance. Zoom retains its beam/field convention.
use crate::{
    ChannelFunctionBehavior, CompiledPhysicalMapping, FixtureMode, OpeningConvention,
    PhysicalDataQuality, PhysicalUnit, ProfileError,
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FocusForwardValue {
    pub percent: f64,
    pub function_id: Uuid,
    pub quality: PhysicalDataQuality,
    /// True means native function travel, with no claim of a measured focus/softness curve.
    pub nominal: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoomForwardValue {
    pub degrees: f64,
    pub function_id: Uuid,
    pub convention: Option<OpeningConvention>,
    pub quality: PhysicalDataQuality,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OpticsForwardStatus {
    #[default]
    Unsupported,
    UnknownFunction,
    UnknownPhysicalMapping,
    Ambiguous,
    Resolved,
}
#[derive(Clone, Debug, PartialEq)]
pub struct OpticsForwardResult {
    pub head_id: Uuid,
    pub focus: Option<FocusForwardValue>,
    pub zoom: Option<ZoomForwardValue>,
    pub focus_status: OpticsForwardStatus,
    pub zoom_status: OpticsForwardStatus,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpticsForwardInputError {
    ChannelCount,
    RawOutOfRange,
    OutputLayout,
}
#[derive(Clone, Debug)]
pub struct CompiledOpticsForward {
    maxima: Box<[u32]>,
    heads: Box<[Head]>,
}
#[derive(Clone, Debug)]
struct Function {
    id: Uuid,
    from: u32,
    to: u32,
    continuous: bool,
    mapping: Option<CompiledPhysicalMapping>,
}
#[derive(Clone, Debug)]
struct Control {
    channel: usize,
    functions: Box<[Function]>,
}
#[derive(Clone, Debug)]
struct Head {
    id: Uuid,
    focus: Box<[Control]>,
    zoom: Box<[Control]>,
}

fn controls(
    mode: &FixtureMode,
    head: Uuid,
    attribute: &str,
) -> Result<Box<[Control]>, ProfileError> {
    let own = mode.channels.iter().any(|c| {
        c.head_id == head
            && c.functions
                .iter()
                .any(|f| f.attribute.0.as_ref() == attribute)
    });
    mode.channels
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            let owner = c.head_id == head
                || (!own
                    && mode
                        .heads
                        .iter()
                        .any(|h| h.id == c.head_id && h.master_shared));
            owner
                && c.functions
                    .iter()
                    .any(|f| f.attribute.0.as_ref() == attribute)
        })
        .map(|(channel, c)| {
            let functions = c
                .functions
                .iter()
                .filter(|f| f.attribute.0.as_ref() == attribute)
                .map(|f| {
                    let continuous =
                        matches!(f.behavior, ChannelFunctionBehavior::Continuous { .. });
                    // Legacy equal/unknown endpoints can still describe nominal focus travel. Only
                    // explicitly authored calibration failures are invalid configuration.
                    let mapping = match CompiledPhysicalMapping::compile(c, f) {
                        Ok(value) => value,
                        Err(error) if f.physical_mapping.is_some() => return Err(error),
                        Err(_) => None,
                    };
                    Ok(Function {
                        id: f.id,
                        from: f.dmx_from,
                        to: f.dmx_to,
                        continuous,
                        mapping,
                    })
                })
                .collect::<Result<Box<[_]>, ProfileError>>()?;
            Ok(Control { channel, functions })
        })
        .collect()
}
fn active<'a>(
    controls: &'a [Control],
    raw: &[u32],
) -> Result<(&'a Function, u32), OpticsForwardStatus> {
    if controls.is_empty() {
        return Err(OpticsForwardStatus::Unsupported);
    }
    if controls.len() != 1 {
        return Err(OpticsForwardStatus::Ambiguous);
    }
    let c = &controls[0];
    let value = raw[c.channel];
    c.functions
        .iter()
        .find(|f| (f.from..=f.to).contains(&value))
        .map(|f| (f, value))
        .ok_or(OpticsForwardStatus::UnknownFunction)
}
impl CompiledOpticsForward {
    pub fn compile(mode: &FixtureMode) -> Result<Self, ProfileError> {
        if mode.channels.len() > 4096 || mode.heads.len() > 4096 {
            return Err(ProfileError::Invalid(
                "Focus/Zoom forward capacity exceeded".into(),
            ));
        }
        super::validate_native_domains(mode)?;
        let heads = mode
            .heads
            .iter()
            .map(|h| {
                Ok(Head {
                    id: h.id,
                    focus: controls(mode, h.id, "focus")?,
                    zoom: controls(mode, h.id, "zoom")?,
                })
            })
            .collect::<Result<_, ProfileError>>()?;
        Ok(Self {
            maxima: mode
                .channels
                .iter()
                .map(|c| c.resolution.max_raw())
                .collect(),
            heads,
        })
    }
    /// Focus and zoom availability is independent, including in split/multi-head modes.
    pub fn inputs_available(&self, head: usize, focus: bool, available: &[bool]) -> bool {
        self.heads.get(head).is_some_and(|h| {
            let controls = if focus { &h.focus } else { &h.zoom };
            controls
                .iter()
                .all(|c| available.get(c.channel) == Some(&true))
        })
    }
    pub fn create_output(&self) -> Vec<OpticsForwardResult> {
        self.heads
            .iter()
            .map(|h| OpticsForwardResult {
                head_id: h.id,
                focus: None,
                zoom: None,
                focus_status: OpticsForwardStatus::Unsupported,
                zoom_status: OpticsForwardStatus::Unsupported,
            })
            .collect()
    }
    pub fn evaluate(
        &self,
        raw: &[u32],
        output: &mut [OpticsForwardResult],
    ) -> Result<(), OpticsForwardInputError> {
        if raw.len() != self.maxima.len() {
            return Err(OpticsForwardInputError::ChannelCount);
        }
        if raw.iter().zip(&self.maxima).any(|(v, max)| v > max) {
            return Err(OpticsForwardInputError::RawOutOfRange);
        }
        if output.len() != self.heads.len()
            || output
                .iter()
                .zip(&self.heads)
                .any(|(o, h)| o.head_id != h.id)
        {
            return Err(OpticsForwardInputError::OutputLayout);
        }
        for (head, out) in self.heads.iter().zip(output) {
            out.focus = None;
            out.zoom = None;
            out.focus_status = match active(&head.focus, raw) {
                Err(status) => status,
                Ok((f, _raw)) if !f.continuous || f.from >= f.to => {
                    OpticsForwardStatus::UnknownPhysicalMapping
                }
                Ok((f, raw)) => {
                    let authored = f.mapping.as_ref().and_then(|m| match m.unit {
                        PhysicalUnit::Percent => {
                            Some((m.physical_for_raw(raw).physical, m.quality))
                        }
                        PhysicalUnit::Normalized => {
                            Some((100. * m.physical_for_raw(raw).physical, m.quality))
                        }
                        _ => None,
                    });
                    let (percent, quality, nominal) = authored.map_or_else(
                        || {
                            (
                                100. * f64::from(raw - f.from) / f64::from(f.to - f.from),
                                PhysicalDataQuality::Estimated,
                                true,
                            )
                        },
                        |(v, q)| (v, q, false),
                    );
                    if !(0. ..=100.).contains(&percent) {
                        OpticsForwardStatus::UnknownPhysicalMapping
                    } else {
                        out.focus = Some(FocusForwardValue {
                            percent,
                            function_id: f.id,
                            quality,
                            nominal,
                        });
                        OpticsForwardStatus::Resolved
                    }
                }
            };
            out.zoom_status = match active(&head.zoom, raw) {
                Err(status) => status,
                Ok((f, raw)) => {
                    match f
                        .mapping
                        .as_ref()
                        .filter(|m| m.unit == PhysicalUnit::Degrees)
                    {
                        None => OpticsForwardStatus::UnknownPhysicalMapping,
                        Some(m) => {
                            let degrees = m.physical_for_raw(raw).physical;
                            if !(0. ..180.).contains(&degrees) || degrees == 0. {
                                OpticsForwardStatus::UnknownPhysicalMapping
                            } else {
                                out.zoom = Some(ZoomForwardValue {
                                    degrees,
                                    function_id: f.id,
                                    convention: m.opening_convention,
                                    quality: m.quality,
                                });
                                OpticsForwardStatus::Resolved
                            }
                        }
                    }
                }
            };
        }
        Ok(())
    }
}
