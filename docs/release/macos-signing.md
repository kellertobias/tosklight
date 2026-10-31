# macOS release signing

GitHub Actions builds the macOS downloads from the Forgejo mirror. The build uses ad-hoc signing
when all Apple credentials are absent. A complete credential set enables Developer ID signing,
notarization, and ticket stapling. A partial set or a signing/notarization failure stops the build.

The Developer ID setup below is a draft procedure until exercised with a real distribution
certificate. Local ad-hoc signing and downloaded-archive startup are tested separately. No signing
credential is needed for that branch.

## GitHub configuration

Repository maintainers configure these values in the GitHub mirror repository's Actions settings.
Certificate and account credentials belong in **Secrets**, not plaintext Variables.

| Name | Storage | Value |
| --- | --- | --- |
| `APPLE_CERTIFICATE_BASE64` | Secret | Base64 of a `.p12` export containing the Developer ID Application certificate and its private key |
| `APPLE_CERTIFICATE_PASSWORD` | Secret | Password protecting that `.p12` export |
| `APPLE_SIGNING_IDENTITY` | Variable or Secret | Full certificate name, beginning `Developer ID Application: ` |
| `APPLE_ID` | Secret | Apple account used for notarization |
| `APPLE_TEAM_ID` | Variable or Secret | Developer Program team ID for that certificate and account |
| `APPLE_APP_SPECIFIC_PASSWORD` | Secret | App-specific password for the notarization account |

1. The Apple Developer account holder creates a Developer ID Application certificate and exports
   its private key and certificate together as a password-protected `.p12` file.
2. A repository maintainer adds the six values above to GitHub Actions settings.
3. A maintainer starts the existing main-branch review build or pushes an authorized release change.
4. The maintainer checks the macOS platform build, including extracted-download startup validation,
   before distributing its artifacts.

The workflow exposes these credentials only to the trusted main-branch macOS build. It imports
the certificate into a temporary keychain under the canonical temporary-artifact directory and
deletes that keychain in an `always()` cleanup step. It signs each nested Mach-O before its app,
enables hardened runtime, and requests secure timestamps. The audio-input entitlement permits
the suite's audio capture paths without disabling library validation.

The complete suite ZIP is submitted to Apple using `notarytool`. An accepted response is required.
Every app-bearing component ZIP and the complete ZIP are then extracted, stapled, assessed with
Gatekeeper, and repacked before publication. The standalone headless executable is signed and
included in the notarization submission; command-line executables do not receive stapled app tickets.

To roll back credential configuration, remove all six values. Subsequent builds return to ad-hoc
testing downloads; already published downloads retain their existing signatures.

## Download verification

The macOS build runs `tools/ci-smoke-macos-download.mjs` against the final suite ZIP. It checks
extracted app signatures and helpers, rejects absolute third-party dynamic-library dependencies,
and starts Control, Architect, and Pixel from their extracted paths. Control and Pixel use private
test directories. On GitHub-hosted runners, Architect uses a disposable account whose current and
legacy application-data directories must be absent before launch. Local validation uses its
private capture mode on the built-in display. Control and Architect must report their frontend
mounted; Pixel must serve its healthy administration interface.
Developer ID builds additionally require valid stapled tickets and Gatekeeper assessment.

Ad-hoc startup tests execute packaged binaries directly. They do not prove the Finder approval
dialog: a clean Mac must still download the ZIP in a browser, expand it without clearing quarantine,
move the apps to Applications, and exercise **Privacy & Security > Open Anyway**. Record that human
check for each supported macOS version before claiming that the approval flow was accepted.

The optional standalone `sign-macos-apps-locally.sh` is included in the suite archive. See
[the offline first-start instructions](macos-first-start.txt) for its use. A valid Developer ID
signature is preserved. Neither CI nor the local script removes download quarantine.

FFmpeg and ffprobe remain external dependencies. These signing and startup checks do not install
or bundle them and do not claim that Pixel media import works without them.

## Apple and GitHub references

- [Apple notarization requirements](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
- [Apple's current Gatekeeper approval instructions](https://support.apple.com/en-us/102445)
- [GitHub's temporary signing-keychain guidance](https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications)
