use super::BridgeLights;
use rs_matter::dm::clusters::decl::bridged_device_basic_information as cluster;
use rs_matter::dm::{Cluster, InvokeContext, ReadContext};
use rs_matter::error::{Error, ErrorCode};
use rs_matter::tlv::{TLVBuilderParent, Utf8StrBuilder};
use rs_matter::with;

impl cluster::ClusterHandler for BridgeLights {
    const CLUSTER: Cluster<'static> = cluster::FULL_CLUSTER
        .with_features(0)
        .with_attrs(with!(required; cluster::AttributeId::ProductName))
        .with_cmds(with!());

    fn dataver(&self) -> u32 {
        self.bridged_info_dataver.get()
    }

    fn dataver_changed(&self) {
        self.bridged_info_dataver.changed();
    }

    fn product_name<P: TLVBuilderParent>(
        &self,
        ctx: impl ReadContext,
        builder: Utf8StrBuilder<P>,
    ) -> Result<P, Error> {
        builder.set(&self.endpoint(ctx.attr().endpoint_id)?.name)
    }

    fn reachable(&self, ctx: impl ReadContext) -> Result<bool, Error> {
        self.endpoint(ctx.attr().endpoint_id).map(|_| true)
    }

    fn unique_id<P: TLVBuilderParent>(
        &self,
        ctx: impl ReadContext,
        builder: Utf8StrBuilder<P>,
    ) -> Result<P, Error> {
        builder.set(&format!("tosklight-{}", ctx.attr().endpoint_id))
    }

    fn handle_keep_active(
        &self,
        _ctx: impl InvokeContext,
        _request: cluster::KeepActiveRequest<'_>,
    ) -> Result<(), Error> {
        Err(ErrorCode::CommandNotFound.into())
    }
}
