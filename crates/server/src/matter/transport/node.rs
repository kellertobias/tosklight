use super::bridge::BridgeLights;
use super::model::{AGGREGATOR_ENDPOINT_ID, EndpointShape};
use rs_matter::dm::Endpoint;
use rs_matter::dm::clusters::decl::{
    bridged_device_basic_information as bridged_info, level_control, on_off,
};
use rs_matter::dm::clusters::desc::{self, ClusterHandler as _};
use rs_matter::dm::clusters::groups::{self, ClusterHandler as _};
use rs_matter::dm::devices::{DEV_TYPE_AGGREGATOR, DEV_TYPE_BRIDGED_NODE, DEV_TYPE_DIMMABLE_LIGHT};
use rs_matter::{clusters, devices, root_endpoint};

const ROOT_ENDPOINT: Endpoint<'static> = root_endpoint!(eth);

pub(super) fn build_endpoints(shape: &[EndpointShape]) -> Vec<Endpoint<'static>> {
    let mut endpoints_meta = Vec::with_capacity(shape.len() + 2);
    endpoints_meta.push(ROOT_ENDPOINT);
    for endpoint in shape {
        endpoints_meta.push(Endpoint::new(
            endpoint.endpoint_id,
            devices!(DEV_TYPE_DIMMABLE_LIGHT, DEV_TYPE_BRIDGED_NODE),
            clusters!(
                desc::DescHandler::CLUSTER,
                groups::GroupsHandler::CLUSTER,
                <BridgeLights as bridged_info::ClusterHandler>::CLUSTER,
                <BridgeLights as on_off::ClusterHandler>::CLUSTER,
                <BridgeLights as level_control::ClusterHandler>::CLUSTER,
            ),
        ));
    }
    endpoints_meta.push(Endpoint::new(
        AGGREGATOR_ENDPOINT_ID,
        devices!(DEV_TYPE_AGGREGATOR),
        clusters!(desc::DescHandler::CLUSTER),
    ));
    endpoints_meta
}
