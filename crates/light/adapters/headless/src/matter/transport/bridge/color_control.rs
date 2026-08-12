use super::super::super::{MatterColorMode, MatterColorState, MatterColorWrite};
use super::BridgeLights;
use rs_matter::dm::clusters::decl::color_control as cluster;
use rs_matter::dm::{Cluster, InvokeContext, ReadContext, WriteContext};
use rs_matter::error::{Error, ErrorCode};
use rs_matter::tlv::Nullable;
use rs_matter::with;

const MIN_MIREDS: u16 = 153;
const MAX_MIREDS: u16 = 500;

macro_rules! unsupported_command {
    ($name:ident, $request:ty) => {
        fn $name(&self, _ctx: impl InvokeContext, _request: $request) -> Result<(), Error> {
            Err(ErrorCode::CommandNotFound.into())
        }
    };
}

impl cluster::ClusterHandler for BridgeLights {
    const CLUSTER: Cluster<'static> = cluster::FULL_CLUSTER
        .with_features(
            cluster::Feature::HUE_AND_SATURATION.bits()
                | cluster::Feature::COLOR_TEMPERATURE.bits(),
        )
        .with_attrs(with!(
            required;
            cluster::AttributeId::CurrentHue
                | cluster::AttributeId::CurrentSaturation
                | cluster::AttributeId::RemainingTime
                | cluster::AttributeId::ColorTemperatureMireds
                | cluster::AttributeId::ColorMode
                | cluster::AttributeId::Options
                | cluster::AttributeId::NumberOfPrimaries
                | cluster::AttributeId::EnhancedColorMode
                | cluster::AttributeId::ColorCapabilities
                | cluster::AttributeId::ColorTempPhysicalMinMireds
                | cluster::AttributeId::ColorTempPhysicalMaxMireds
                | cluster::AttributeId::CoupleColorTempToLevelMinMireds
        ))
        .with_cmds(with!(
            cluster::CommandId::MoveToHueAndSaturation | cluster::CommandId::MoveToColorTemperature
        ));

    fn dataver(&self) -> u32 {
        self.color_dataver.get()
    }

    fn dataver_changed(&self) {
        self.color_dataver.changed();
    }

    fn current_hue(&self, ctx: impl ReadContext) -> Result<u8, Error> {
        Ok(color(self, ctx.attr().endpoint_id)?.hue)
    }

    fn current_saturation(&self, ctx: impl ReadContext) -> Result<u8, Error> {
        Ok(color(self, ctx.attr().endpoint_id)?.saturation)
    }

    fn remaining_time(&self, _ctx: impl ReadContext) -> Result<u16, Error> {
        Ok(0)
    }

    fn color_temperature_mireds(&self, ctx: impl ReadContext) -> Result<u16, Error> {
        Ok(color(self, ctx.attr().endpoint_id)?.color_temperature_mireds)
    }

    fn color_mode(&self, ctx: impl ReadContext) -> Result<cluster::ColorModeEnum, Error> {
        Ok(match color(self, ctx.attr().endpoint_id)?.mode {
            MatterColorMode::HueSaturation => {
                cluster::ColorModeEnum::CurrentHueAndCurrentSaturation
            }
            MatterColorMode::ColorTemperature => cluster::ColorModeEnum::ColorTemperatureMireds,
        })
    }

    fn options(&self, _ctx: impl ReadContext) -> Result<cluster::OptionsBitmap, Error> {
        Ok(cluster::OptionsBitmap::EXECUTE_IF_OFF)
    }

    fn number_of_primaries(&self, _ctx: impl ReadContext) -> Result<Nullable<u8>, Error> {
        Ok(Nullable::none())
    }

    fn enhanced_color_mode(
        &self,
        ctx: impl ReadContext,
    ) -> Result<cluster::EnhancedColorModeEnum, Error> {
        Ok(match color(self, ctx.attr().endpoint_id)?.mode {
            MatterColorMode::HueSaturation => {
                cluster::EnhancedColorModeEnum::CurrentHueAndCurrentSaturation
            }
            MatterColorMode::ColorTemperature => {
                cluster::EnhancedColorModeEnum::ColorTemperatureMireds
            }
        })
    }

    fn color_capabilities(
        &self,
        _ctx: impl ReadContext,
    ) -> Result<cluster::ColorCapabilitiesBitmap, Error> {
        Ok(cluster::ColorCapabilitiesBitmap::from_bits_truncate(
            cluster::ColorCapabilitiesBitmap::HUE_SATURATION.bits()
                | cluster::ColorCapabilitiesBitmap::COLOR_TEMPERATURE.bits(),
        ))
    }

    fn color_temp_physical_min_mireds(&self, _ctx: impl ReadContext) -> Result<u16, Error> {
        Ok(MIN_MIREDS)
    }

    fn color_temp_physical_max_mireds(&self, _ctx: impl ReadContext) -> Result<u16, Error> {
        Ok(MAX_MIREDS)
    }

    fn couple_color_temp_to_level_min_mireds(&self, _ctx: impl ReadContext) -> Result<u16, Error> {
        Ok(MIN_MIREDS)
    }

    fn set_options(
        &self,
        _ctx: impl WriteContext,
        _value: cluster::OptionsBitmap,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn handle_move_to_hue_and_saturation(
        &self,
        ctx: impl InvokeContext,
        request: cluster::MoveToHueAndSaturationRequest<'_>,
    ) -> Result<(), Error> {
        let endpoint_id = ctx.cmd().endpoint_id;
        self.set_color(
            endpoint_id,
            MatterColorWrite::HueSaturation {
                hue: request.hue()?,
                saturation: request.saturation()?,
            },
        )?;
        notify_color(&ctx, endpoint_id);
        Ok(())
    }

    fn handle_move_to_color_temperature(
        &self,
        ctx: impl InvokeContext,
        request: cluster::MoveToColorTemperatureRequest<'_>,
    ) -> Result<(), Error> {
        let endpoint_id = ctx.cmd().endpoint_id;
        self.set_color(
            endpoint_id,
            MatterColorWrite::ColorTemperature {
                mireds: request
                    .color_temperature_mireds()?
                    .clamp(MIN_MIREDS, MAX_MIREDS),
            },
        )?;
        notify_color(&ctx, endpoint_id);
        Ok(())
    }

    unsupported_command!(handle_move_to_hue, cluster::MoveToHueRequest<'_>);
    unsupported_command!(handle_move_hue, cluster::MoveHueRequest<'_>);
    unsupported_command!(handle_step_hue, cluster::StepHueRequest<'_>);
    unsupported_command!(
        handle_move_to_saturation,
        cluster::MoveToSaturationRequest<'_>
    );
    unsupported_command!(handle_move_saturation, cluster::MoveSaturationRequest<'_>);
    unsupported_command!(handle_step_saturation, cluster::StepSaturationRequest<'_>);
    unsupported_command!(handle_move_to_color, cluster::MoveToColorRequest<'_>);
    unsupported_command!(handle_move_color, cluster::MoveColorRequest<'_>);
    unsupported_command!(handle_step_color, cluster::StepColorRequest<'_>);
    unsupported_command!(
        handle_enhanced_move_to_hue,
        cluster::EnhancedMoveToHueRequest<'_>
    );
    unsupported_command!(
        handle_enhanced_move_hue,
        cluster::EnhancedMoveHueRequest<'_>
    );
    unsupported_command!(
        handle_enhanced_step_hue,
        cluster::EnhancedStepHueRequest<'_>
    );
    unsupported_command!(
        handle_enhanced_move_to_hue_and_saturation,
        cluster::EnhancedMoveToHueAndSaturationRequest<'_>
    );
    unsupported_command!(handle_color_loop_set, cluster::ColorLoopSetRequest<'_>);
    unsupported_command!(handle_stop_move_step, cluster::StopMoveStepRequest<'_>);
    unsupported_command!(
        handle_move_color_temperature,
        cluster::MoveColorTemperatureRequest<'_>
    );
    unsupported_command!(
        handle_step_color_temperature,
        cluster::StepColorTemperatureRequest<'_>
    );
}

fn color(lights: &BridgeLights, endpoint_id: u16) -> Result<MatterColorState, Error> {
    lights
        .endpoint(endpoint_id)?
        .color
        .ok_or_else(|| ErrorCode::Failure.into())
}

fn notify_color(ctx: &impl InvokeContext, endpoint_id: u16) {
    for attribute in [
        cluster::AttributeId::CurrentHue,
        cluster::AttributeId::CurrentSaturation,
        cluster::AttributeId::ColorTemperatureMireds,
        cluster::AttributeId::ColorMode,
        cluster::AttributeId::EnhancedColorMode,
    ] {
        ctx.notify_attr_changed(
            endpoint_id,
            <BridgeLights as cluster::ClusterHandler>::CLUSTER.id,
            attribute as _,
        );
    }
}
