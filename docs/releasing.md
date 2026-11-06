# Releasing

Releases are built by GitHub Actions from the public code, never on a
developer's PC. That way anyone can check what went into a download, and it
is what free code signing for open-source projects requires.

## Making a release

1. Set the new version in `Cargo.toml` (`[workspace.package]`) and in
   `crates/knob-settings/tauri.conf.json`, run `cargo build` so `Cargo.lock`
   follows, and commit.
2. Tag the commit and push the tag:

   ```
   git tag v1.2.0
   git push origin v1.2.0
   ```

3. The **Release** workflow builds `ak820-custom-knob-setup.exe` and
   `ak820-custom-knob-portable.zip` and attaches them to a **draft** release.
   It stops if the tag doesn't match the version in `Cargo.toml`.
4. Open the draft on the Releases page, write the notes, and publish it. The
   README's download button always points at the newest published release.

To try the build without releasing, open *Actions → Release → Run workflow*.
The two files are attached to that run.

The release file names have no version number on purpose: the README links
to `releases/latest/download/ak820-custom-knob-setup.exe`, which only works
when the name stays the same.

## Checking a download

Every file the workflow builds gets a build provenance attestation. To check
that a downloaded file was built by this repository's workflow:

```
gh attestation verify ak820-custom-knob-setup.exe --repo Enceladus00/ak820-custom-knob
```

## Code signing (planned)

Browsers and Windows warn about unsigned downloads from unknown publishers.
The plan is free signing through the [SignPath Foundation](https://signpath.org),
which signs open-source projects whose releases are built by CI from public
code.

1. Apply on signpath.org with this repository. Requirements include an
   OSI-approved license (Apache-2.0), released and documented software, and
   CI builds; their site has the full list.
2. Once accepted, SignPath sets up the project and gives the organization ID,
   project and signing policy names, and an API token.
3. Store the token as the `SIGNPATH_API_TOKEN` repository secret and the IDs
   as repository variables, then add the SignPath signing step to
   `.github/workflows/release.yml`: sign the two exe files, build the
   installer from the signed files, then sign the installer.
4. Publish the code signing policy SignPath asks for in the README.
