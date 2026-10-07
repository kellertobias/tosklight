# Public site legal pages

`npm run pages:generate` renders the Impressum (`/impressum/`) and the Datenschutzerklärung
(`/datenschutz/`) of the public site, links both from every published page, and fails when a page
lacks the links or loads anything from a third-party host.

## Contact data

The operator's contact data is never committed. It comes from the environment variable
`TOSKLIGHT_IMPRINT_CONTACT`, which the "Documentation and Pages" workflow fills from the repository
secret of the same name. The value is a JSON object whose keys must all be non-empty strings:

`operator`, `name`, `street`, `postalCode`, `city`, `country`, `email`, `phone`,
`responsibleForContent`

The build stops with an error naming the missing or empty keys; it never prints the values.

For a local build, use the fictitious example data:

```sh
TOSKLIGHT_IMPRINT_CONTACT="$(cat tools/site-legal/fixtures/imprint-contact.example.json)" \
  npm run pages:generate
npm run pages:serve -- 8080
```

A site built from the example data must never be deployed; only CI publishes the site.

## Commands

```sh
node tools/site-legal/cli.mjs validate        # check TOSKLIGHT_IMPRINT_CONTACT only
node tools/site-legal/cli.mjs write <site>    # render both pages, link every page, then check
node tools/site-legal/cli.mjs inject <site>   # link every page (no contact data needed)
node tools/site-legal/cli.mjs check <site>    # links on every page, no third-party requests
node --test tools/site-legal/site-legal.test.mjs
```

## Third-party requests

Everything the site loads is served from the site itself. `ALLOWED_RESOURCE_HOSTS` in `links.mjs`
is empty on purpose: adding a host there also requires disclosing it in the Datenschutzerklärung
(`pages.mjs`). CodeSafari's syntax highlighting would otherwise download grammars from
`lighter.codehike.org`; `tools/codesafari-self-host-highlighting.mjs` serves them locally instead.
