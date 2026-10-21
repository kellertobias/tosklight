# Media Surfaces

Use Architect's **Media** workspace to place projection screens, televisions, LED walls and
projectors. A surface can have several separately positioned and cropped sections. For an LED
section, choose its module type and edit the occupancy grid; empty cells remain physical holes.

## Choose a source

1. Under **Servers & outputs**, use **Discover servers**, or enter the server's host and port.
2. Choose **Enumerate outputs** and select the exact numeric output.
3. Set the surface's size, transform and source crop. Check its preview for orientation.

A manual source remains useful when planning offline. Select a server to edit its name, address
and DMX patch in the sidebar. Deleting it removes its outputs and patched Media Server fixture;
surfaces using it retain only their fallback picture.

## Keep a fallback picture

Use **Import image** for PNG, JPEG or WebP. The picture travels inside the show without an external
file. It may be the only source or the fallback for a live output.

If live input stops, the standalone Visualizer holds the last frame for two seconds, then uses
the fallback or black. Returning input resumes automatically. Control's embedded Stage draws the
physical surfaces but does not receive live media pictures.

## Check the physical appearance

Screen settings control reflective colour and edge softness. TVs include their bezel and glow;
LED walls retain module gaps and pitch. Projectors have their own placement, throw ratio, lens
shift and cone. Use the source crop preview to check which part of the picture each surface shows.
