use super::super::super::MAX_MATTER_LEVEL;
use super::BridgeLights;
use rs_matter::dm::clusters::decl::{level_control as cluster, on_off};
use rs_matter::dm::{Cluster, InvokeContext, ReadContext, WriteContext};
use rs_matter::error::{Error, ErrorCode};
use rs_matter::tlv::Nullable;
use rs_matter::with;

impl cluster::ClusterHandler for BridgeLights {
    const CLUSTER: Cluster<'static> = cluster::FULL_CLUSTER
        .with_features(cluster::Feature::LIGHTING.bits() | cluster::Feature::ON_OFF.bits())
        .with_attrs(with!(
            required;
            cluster::AttributeId::CurrentLevel
                | cluster::AttributeId::MinLevel
                | cluster::AttributeId::MaxLevel
                | cluster::AttributeId::Options
                | cluster::AttributeId::OnLevel
        ))
        .with_cmds(with!(
            cluster::CommandId::MoveToLevel
                | cluster::CommandId::Move
                | cluster::CommandId::Step
                | cluster::CommandId::Stop
                | cluster::CommandId::MoveToLevelWithOnOff
                | cluster::CommandId::MoveWithOnOff
                | cluster::CommandId::StepWithOnOff
                | cluster::CommandId::StopWithOnOff
        ));

    fn dataver(&self) -> u32 {
        self.level_dataver.get()
    }

    fn dataver_changed(&self) {
        self.level_dataver.changed();
    }

    fn current_level(&self, ctx: impl ReadContext) -> Result<Nullable<u8>, Error> {
        Ok(Nullable::some(self.endpoint(ctx.attr().endpoint_id)?.level))
    }

    fn min_level(&self, _ctx: impl ReadContext) -> Result<u8, Error> {
        Ok(1)
    }

    fn max_level(&self, _ctx: impl ReadContext) -> Result<u8, Error> {
        Ok(MAX_MATTER_LEVEL)
    }

    fn options(&self, ctx: impl ReadContext) -> Result<cluster::OptionsBitmap, Error> {
        cluster::OptionsBitmap::from_bits(self.endpoint(ctx.attr().endpoint_id)?.options)
            .ok_or_else(|| ErrorCode::Invalid.into())
    }

    fn on_level(&self, ctx: impl ReadContext) -> Result<Nullable<u8>, Error> {
        Ok(Nullable::new(
            self.endpoint(ctx.attr().endpoint_id)?.on_level,
        ))
    }

    fn set_options(
        &self,
        ctx: impl WriteContext,
        value: cluster::OptionsBitmap,
    ) -> Result<(), Error> {
        let mut endpoints = self.endpoints.write();
        let endpoint = endpoints
            .get_mut(&ctx.attr().endpoint_id)
            .ok_or(ErrorCode::EndpointNotFound)?;
        endpoint.options = value.bits();
        Ok(())
    }

    fn set_on_level(&self, ctx: impl WriteContext, value: Nullable<u8>) -> Result<(), Error> {
        let mut endpoints = self.endpoints.write();
        let endpoint = endpoints
            .get_mut(&ctx.attr().endpoint_id)
            .ok_or(ErrorCode::EndpointNotFound)?;
        endpoint.on_level = value.into_option();
        Ok(())
    }

    fn handle_move_to_level(
        &self,
        ctx: impl InvokeContext,
        request: cluster::MoveToLevelRequest<'_>,
    ) -> Result<(), Error> {
        self.apply_level_command(ctx, request.level()?)
    }

    fn handle_move(
        &self,
        ctx: impl InvokeContext,
        request: cluster::MoveRequest<'_>,
    ) -> Result<(), Error> {
        self.apply_level_command(ctx, move_target(request.move_mode()?))
    }

    fn handle_step(
        &self,
        ctx: impl InvokeContext,
        request: cluster::StepRequest<'_>,
    ) -> Result<(), Error> {
        self.apply_step_command(ctx, request.step_mode()?, request.step_size()?)
    }

    fn handle_stop(
        &self,
        _ctx: impl InvokeContext,
        _request: cluster::StopRequest<'_>,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn handle_move_to_level_with_on_off(
        &self,
        ctx: impl InvokeContext,
        request: cluster::MoveToLevelWithOnOffRequest<'_>,
    ) -> Result<(), Error> {
        self.apply_level_command(ctx, request.level()?)
    }

    fn handle_move_with_on_off(
        &self,
        ctx: impl InvokeContext,
        request: cluster::MoveWithOnOffRequest<'_>,
    ) -> Result<(), Error> {
        self.apply_level_command(ctx, move_target(request.move_mode()?))
    }

    fn handle_step_with_on_off(
        &self,
        ctx: impl InvokeContext,
        request: cluster::StepWithOnOffRequest<'_>,
    ) -> Result<(), Error> {
        self.apply_step_command(ctx, request.step_mode()?, request.step_size()?)
    }

    fn handle_stop_with_on_off(
        &self,
        _ctx: impl InvokeContext,
        _request: cluster::StopWithOnOffRequest<'_>,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn handle_move_to_closest_frequency(
        &self,
        _ctx: impl InvokeContext,
        _request: cluster::MoveToClosestFrequencyRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::CommandNotFound.into())
    }
}

impl BridgeLights {
    fn apply_level_command(&self, ctx: impl InvokeContext, level: u8) -> Result<(), Error> {
        let endpoint = ctx.cmd().endpoint_id;
        let on = self.set_level(endpoint, level)?;
        self.notify_level_command(&ctx, endpoint, on);
        Ok(())
    }

    fn apply_step_command(
        &self,
        ctx: impl InvokeContext,
        mode: cluster::StepModeEnum,
        step: u8,
    ) -> Result<(), Error> {
        let endpoint = ctx.cmd().endpoint_id;
        let target = self.step_level(endpoint, mode == cluster::StepModeEnum::Up, step)?;
        self.notify_level_command(&ctx, endpoint, target > 0);
        Ok(())
    }

    fn notify_level_command(&self, ctx: &impl InvokeContext, endpoint: u16, _on: bool) {
        ctx.notify_own_attr_changed(cluster::AttributeId::CurrentLevel as _);
        ctx.notify_attr_changed(
            endpoint,
            <Self as on_off::ClusterHandler>::CLUSTER.id,
            on_off::AttributeId::OnOff as _,
        );
    }
}

fn move_target(mode: cluster::MoveModeEnum) -> u8 {
    match mode {
        cluster::MoveModeEnum::Up => MAX_MATTER_LEVEL,
        cluster::MoveModeEnum::Down => 0,
    }
}
