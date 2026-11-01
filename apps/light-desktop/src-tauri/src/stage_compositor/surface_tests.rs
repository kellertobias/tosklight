use super::configure_surface;

#[test]
fn refused_configuration_returns_an_error_and_does_not_poison_the_next_operation() {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::NOOP;
    descriptor.backend_options.noop = wgpu::NoopBackendOptions {
        enable: true,
        ..Default::default()
    };
    let instance = wgpu::Instance::new(descriptor);
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    let (device, _queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    // Exercise wgpu's actual validation sink without depending on a window-system failure.
    // The packaged Windows smoke covers the refused second swapchain on a real HWND.
    let error = configure_surface(&device, || {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("invalid surface operation regression"),
            size: 16,
            usage: wgpu::BufferUsages::empty(),
            mapped_at_creation: false,
        });
    })
    .unwrap_err();
    assert!(
        error.contains("could not configure its Stage surface"),
        "{error}"
    );
    assert!(error.contains("Invalid usage flags"), "{error}");
    configure_surface(&device, || {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("valid operation after refusal"),
            size: 16,
            usage: wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
    })
    .unwrap();
}
