# Contributing to ToskLight

Thank you for contributing to ToskLight.

ToskLight is professional lighting-control software. Contributions should
preserve the operator model, show-file compatibility, and parity between the
software UI, command line, OSC, and hardware-control surfaces where applicable.

## Contribution License

By submitting a contribution to ToskLight, you agree to the contribution terms
in the ToskLight Community License.

In short: you keep ownership of your contribution, but you grant Tobias S.
Keller and any later designated ToskLight copyright holder broad rights to use,
modify, publish, sublicense, relicense, sell, and include your contribution in
ToskLight and related products, including commercial hardware products.

Accepted contributions are published under the ToskLight Community License
unless otherwise agreed in writing.

## Published Modifications

If you modify ToskLight and provide, distribute, install, or otherwise make that
modified version available to someone else, you must visibly and publicly
publish the complete source code of that modified version under the ToskLight
Community License, at no charge.

The preferred way to publish modified source is a public fork of the ToskLight
repository.

## Not a Contribution

If you send something that is not intended as a contribution, mark it clearly:

```text
Not a Contribution
```

Do this before or at the time of submission.

## Third-Party Material

Do not submit third-party code, generated code, fixture data, manuals, media,
icons, models, fonts, or other assets unless you clearly identify their source
and license.

Do not submit material under GPL, AGPL, LGPL-only, non-commercial,
no-derivatives, or proprietary terms unless it has been explicitly approved in
writing.

## Engineering Expectations

Before submitting a change, run the smallest relevant checks for the area you
changed. Common checks include:

```sh
npm run test:unit
npm run test:e2e-api
npm run test:e2e-ui
npm run test:e2e -- tests/<focused-spec>.spec.ts
```

Keep changes focused, preserve existing show compatibility unless a migration
is explicitly agreed, and update documentation or tests when behavior changes.
