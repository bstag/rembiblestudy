# reMarkable Bible Study

reMarkable Bible Study looks up Scripture and quotes from handwritten notes on the Paper Pro and can insert the selected result as ink. The desktop command line accepts English Bible book names and common abbreviations, single verses, and ranges within a book. This is an independent project for reMarkable hardware.

See [Upgrading Bible Study](docs/maintenance.md#upgrade-bible-study-on-the-same-firmware) for app releases and the [firmware checklist](docs/maintenance.md#before-and-after-a-firmware-update) for reMarkable OS updates. [Long-term options](docs/long-term-options.md) covers book lookup and edition-specific citations.

## Build and run on Windows

Install Rust and Cargo with rustup, the Visual Studio C++ build tools required by the Windows Rust toolchain, and PowerShell 7. From the project directory, run:

```powershell
cargo test --locked
cargo build --locked --release
```

Use your own API.Bible key with NIV access for Scripture. Keep the key in the current shell's environment only for the duration of the command. Wikiquote works without an API.Bible key.

```powershell
try {
    $env:API_BIBLE_KEY = Read-Host "API.Bible key" -MaskInput
    cargo run --locked -- lookup "Ephesians 4:13"
    cargo run --locked -- quote "until we all reach unity"
} finally {
    Remove-Item Env:API_BIBLE_KEY -ErrorAction SilentlyContinue
}
cargo run --locked -- quote "Augustine - our hearts are restless"
```

To use a different translation, list the translations enabled for your account and pass the desired ID.

```powershell
try {
    $env:API_BIBLE_KEY = Read-Host "API.Bible key" -MaskInput
    cargo run --locked -- translations
    $translationId = Read-Host "Translation ID"
    cargo run --locked -- lookup $translationId "Ephesians 4:13"
} finally {
    Remove-Item Env:API_BIBLE_KEY -ErrorAction SilentlyContinue
}
```

## Tablet integration

The original preview caused xochitl to restart because the tablet rejected the selection button's accessibility declaration. The installer blocks that revision. The revised patch loaded on the Paper Pro with XOVI and LucidKith. A hands-on test confirmed that lassoing `Eph 4:13` and tapping the magnifying glass opened the NIV passage preview.

Lasso a handwritten reference and tap the magnifying-glass button to preview the NIV passage. Tap the reference field to open the on-screen keyboard. The review panel stays above the keyboard so you can correct the reference and tap **Look up**. Handwriting recognition uses your reMarkable sign-in and Wi-Fi.

To find a quote, lasso a few distinctive words, open the same panel, and tap **Find quote**. A phrase searches the NIV and English Wikiquote. Enter `Author — words`, `Author - words`, or `Author: words` to search that author's English Wikiquote page. The desktop `quote` command uses the same search and can use Wikiquote without an API.Bible key. Review the wording and source before inserting a match. Tap **As heard** to keep the current wording without looking it up.

To insert a passage, hold the notebook upright and choose a writing pen as your primary tool. Lasso the reference, open the preview, and tap **Insert as ink**. The outline shows the area the passage will use. If ink or text is detected there, tap a blank spot to move the outline. When the tablet cannot verify the area, check the outline yourself. Tap **Write here** and lift the Marker while it writes. If the page, zoom, layer, or tool changes during insertion, some strokes may already be on the page.

## Build and deploy to the tablet

The installer targets reMarkable Paper Pro (Ferrari, ARM64) on firmware `3.29.1.0`. It requires Developer Mode, working USB SSH, and an existing XOVI installation with `qt-command-executor`, `qt-resource-rebuilder`, and the matching resource hash table. Back up your notebooks before enabling Developer Mode because it resets the device. Follow XOVI's setup guide for starting the extensions. [reMarkable Developer Mode](https://support.remarkable.com/s/article/How-to-enable-and-disable-developer-mode-on-reMarkable) · [XOVI setup and resource hash table](https://github.com/asivery/rm-xovi-extensions)

Use PowerShell 7, Docker with Linux containers, and the Windows OpenSSH client. Connect the tablet over USB. Set up SSH authentication for `root@10.11.99.1` and verify and register your tablet's SSH host key before running the installer. The installer requires a known host key. Pass `-SshKey` if you use a private key other than `~/.ssh/id_rsa`. Keep the private key outside the repository.

```powershell
./tablet/build.ps1
./tablet/install.ps1
./tablet/install.ps1 -Reload -PromptApiKey
```

Close any open notebook before the last command. The middle command runs preflight. The last command provisions the key through a masked prompt and starts a guarded upgrade. The installer restores the previous binary and UI patch if startup checks fail. Omit `-PromptApiKey` for later upgrades to retain the configured key. See the [maintenance guide](docs/maintenance.md) for compatibility, recovery, and post-install checks.

Bible Study was tested alongside LucidKith. rePly's ink writer was a reference for native pen input, and lettering uses the Hershey Futural stroke font. [rePly](https://github.com/vudh-dev/rePly/blob/4851e8d6c9e400957f3afa94070a2d000783971e/tablet/src/pathwriter.inc) · [Hershey font license](assets/Hershey-LICENSE.txt)

## Credentials and notebook data

- The installer sends the API.Bible key over SSH standard input and stores it in the tablet's root-only app configuration directory. The key file is mode `0600`. Keep any optional `-ApiKeyFile` outside the source tree. The source and build do not include API keys, private SSH keys, notebook files, or provider content.
- Handwriting conversion uses reMarkable's recognizer. API.Bible receives the verse reference or phrase searched. Wikiquote receives the phrase or author depending on the search. The API.Bible key is sent to API.Bible only.
- **As heard** keeps the current wording locally. Inserting a result writes ink into the open notebook, which uses your ordinary reMarkable sync settings. The local preview and ink commands pass text as process arguments, which can be inspected by privileged software on the tablet.
- Use your own credential and translation entitlements. For rules on handling keys and licensed text, see [API.Bible app and key management](https://care.api.bible/article/414-managing-apps) and [API.Bible licensing](https://care.api.bible/article/369-understanding-api-bible-licensing).

To disable the action, move `rembiblestudy-selection.qmd` out of `/home/root/xovi/exthome/qt-resource-rebuilder/` and reload XOVI.
