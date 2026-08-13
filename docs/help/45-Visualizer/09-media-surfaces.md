# Media Surfaces

The Viz Editor's **Media** workspace authors screens, televisions, LED walls, and projectors as
portable show objects. A surface can contain several separately positioned and cropped sections;
each section keeps its physical size and full 3D transform. LED sections additionally use a named
module type and an editable occupancy grid, so deliberate holes remain holes while the picture
continues across the complete wall.

## Choose a live output

Under **Servers & outputs**, use **Discover servers** for running CITP peers, or add the media
server's host and port manually. **Enumerate outputs** asks
that server for its current numeric output list and stores the selected output identity, name,
resolution, and aspect ratio in the show. A manually entered output remains available for servers
that cannot be reached while planning. The standalone Visualizer reconnects and renews its preview
subscription automatically; it never silently changes a missing numeric output to another one.

ToskLight Media publishes each enabled logical output as its own stable CITP preview source. More
than one authored surface can use the same source without decoding or uploading the same frame more
than once.

## Portable fallback pictures

**Import image** copies a PNG, JPEG, or WebP into the show. The original path is not retained. The
bytes, dimensions, media type, and digest therefore travel through ordinary show backup, restore,
and selective import. A surface may use the picture on its own, or as the fallback for a live
source.

When live input stops, the standalone Visualizer holds the last good frame for two seconds and
then shows the imported fallback. With no fallback it becomes black. A returning source resumes on
its next valid frame. The desk's embedded Stage and helper renderer keep the authored physical
geometry but open no CITP sockets and decode no media content.

## What is rendered

Projection screens retain their authored reflective colour, gain, roughness, and edge feathering.
TVs retain a physical bezel and emissive spill. LED walls retain module gaps, occupied cells, and
pixel-pitch character. Projectors keep their body, transform, throw ratio, lens shift, cone length,
and spill. Normalized top-left crop controls use the same orientation on every surface type; the
crop preview in the editor shows exactly which part of the source remains.
