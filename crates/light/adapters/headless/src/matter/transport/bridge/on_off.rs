use super::BridgeLights;
use rs_matter::dm::clusters::decl::on_off as cluster;
use rs_matter::dm::{Cluster, InvokeContext, ReadContext};
use rs_matter::error::{Error, ErrorCode};
use rs_matter::with;

impl cluster::ClusterHandler for BridgeLights {
    const CLUSTER: Cluster<'static> = cluster::FULL_CLUSTER
        .with_features(cluster::Feature::LIGHTING.bits())
        .with_attrs(with!(required; cluster::AttributeId::OnOff))
        .with_cmds(with!(
            cluster::CommandId::Off | cluster::CommandId::On | cluster::CommandId::Toggle
        ));

    fn dataver(&self) -> u32 {
        self.on_off_dataver.get()
    }

    fn dataver_changed(&self) {
        self.on_off_dataver.changed();
    }

    fn on_off(&self, ctx: impl ReadContext) -> Result<bool, Error> {
        Ok(self.endpoint(ctx.attr().endpoint_id)?.on)
    }

    fn handle_off(&self, ctx: impl InvokeContext) -> Result<(), Error> {
        let endpoint = ctx.cmd().endpoint_id;
        self.set_on(endpoint, false)?;
        ctx.notify_own_attr_changed(cluster::AttributeId::OnOff as _);
        Ok(())
    }

    fn handle_on(&self, ctx: impl InvokeContext) -> Result<(), Error> {
        let endpoint = ctx.cmd().endpoint_id;
        let _ = self.set_on(endpoint, true)?;
        ctx.notify_own_attr_changed(cluster::AttributeId::OnOff as _);
        Ok(())
    }

    fn handle_toggle(&self, ctx: impl InvokeContext) -> Result<(), Error> {
        let endpoint = ctx.cmd().endpoint_id;
        let on = !self.endpoint(endpoint)?.on;
        let _ = self.set_on(endpoint, on)?;
        ctx.notify_own_attr_changed(cluster::AttributeId::OnOff as _);
        Ok(())
    }

    fn handle_off_with_effect(
        &self,
        _ctx: impl InvokeContext,
        _request: cluster::OffWithEffectRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::CommandNotFound.into())
    }

    fn handle_on_with_recall_global_scene(&self, _ctx: impl InvokeContext) -> Result<(), Error> {
        Err(ErrorCode::CommandNotFound.into())
    }

    fn handle_on_with_timed_off(
        &self,
        _ctx: impl InvokeContext,
        _request: cluster::OnWithTimedOffRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::CommandNotFound.into())
    }
}
