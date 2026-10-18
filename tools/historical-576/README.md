# Historical 576-fixture Linux comparison

This CI-only experiment compares three cases sequentially on one Ubuntu22.04 runner with four allowed logical CPUs, two guest-reported cores and one package. It preserves the primary checkout. It does not replace the unchanged production Control40Hz acceptance proof.

| Case | Application source | Programming |
| --- | --- | --- |
| A | Pre-intention `f67c84e48baf96a6b8bc667edca7fb859e852ac5` | Legacy channel lanes |
| B | Current `7a5468514967fca9066c2d00025a3ddb83446d39`, explicit compatibility build | The same legacy channel lanes |
| C | Ordinary current `7a5468514967fca9066c2d00025a3ddb83446d39` | Typed Color and Position lanes |

A versus B includes all intervening application changes. B versus C compares the current legacy and intention paths. Typed and legacy output are not asserted equivalent: native White differs from semantic WhiteBlend, and normalized Pan/Tilt differs from physical angle programming. This is the user-authorized translated comparison.

All cases use identical current fixture archives,576 fixtures,8064 occupied slots,16 universes,20 controller partitions and5408 animated assignments. The used profiles are LEDBeam224,Sunstrip96 andDimmer256. DLS is unused. The common input SHA256 is `0034aca0703cf144f23ee094f0f5d645eacee08711df7b835ddbb47bae785f3f`.

The canonical plan is frozen before actual server logical-head IDs are substituted. Ordered target membership, lane IDs, speed and phase settings remain fixed. Legacy translation is explicit and rejects unknown forms. Before and after every measured window, the runtime must report exactly20 live instances/controllers and5408 assignments with expected IDs,targets and parameters.

Fixture Sheet remains active and Stage stays absent. The old app receives a hash-pinned benchmark-surface-only shim to activate its Sheet without starting Stage. The current compatibility app receives a hash-pinned startup-only contract0 overlay; Engine,Dynamics,schemas and production constants remain unchanged. This overlay is not a shipping switch. Active-show create/start actions are supported; raw/inactive repository writes are outside this diagnostic.

The workflow builds each arm in a separate Cargo target and saves AppImage plus actual packaged ELF hashes. It verifies source manifests before/after and again after all builds. Targets are deleted only after receipts and packaged outputs are saved. Toolchain/environment observations remain private. These checks do not attest every external build input or all transitive dependency freshness.

Measurements follow A/B/C,C/B/A,A/B/C: three one-minute windows per case,61 observations each. All use the same external monotonic request brackets, reporting lower and upper rate bounds rather than fabricating old server timestamps. The experiment claims neither schema5 acceptance nor exact start alignment. Whole application CPU and memory are qualified by process-census completeness. UDP evidence covers16 universes,decoded-ring continuity and two post-window static fades; it does not prove dynamic waveform equivalence.

Only `comparison-numeric.json` and `comparison-provenance.json` are uploaded as `historical-576-numeric-RUN-ATTEMPT`. Both have closed schemas. Raw observations,logs,environment values,process identities and extracted files stay private. Failed clean cases remain failed and later independent cases continue; missing receipts or unverified cleanup stop subsequent launches.

The workflow is `.github/workflows/historical-576-comparison.yml`. Its push trigger is confined to the isolated topic branch and these comparison tools. It creates no release,main integration or pull request.
