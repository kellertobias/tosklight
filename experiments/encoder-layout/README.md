# Encoder Layout Lab

Open `index.html` in a browser. The experiment has no build step or external dependencies.

All group membership, labels, descriptions, pairings, and width-specific ordering live in
`encoder-layout-data.js`. The renderer in `app.js` automatically packs those blocks into pages for
four, five, or six physical encoders.

To move an attribute, move its `encoder(...)` entry to another group's `blocks`. Put controls that
must stay on the same page in one block. Use a group's optional `orders` map only when a hardware
width needs a meaningfully different block order.
