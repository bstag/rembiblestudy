# Maintaining reMarkable Bible Study

The tested target is the reMarkable Paper Pro (Ferrari, ARM64), firmware `3.29.1.0`, with the tablet's existing XOVI installation. Treat support for a different firmware or device as a new compatibility check. The notebook action is a patch to reMarkable's `xochitl` UI and uses firmware-specific behavior for selection, recognition, placement, and pen input.

## How it works today

| Component | Purpose and dependency |
| --- | --- |
| [Reference parser and API.Bible client](../src/lib.rs) | Resolves verse references and searches the NIV. The desktop `lookup` command also accepts an enabled translation ID. |
| [Quote lookup](../src/quotes.rs) and [command handlers](../src/main.rs) | A bare phrase searches the NIV and English Wikiquote in parallel. Wikiquote checks up to four pages for matching wording. `Author — words`, `Author - words`, or `Author: words` searches an English Wikiquote author page. Results can include a work and a Wikiquote revision link. A phrase result labels the Wikiquote page, whose title might be a topic. The scraper skips sections such as Misattributed and Disputed. Both the desktop `quote` command and the tablet use this logic. **As heard** keeps the text in the input field locally. |
| [Selection and placement patch](../tablet/rembiblestudy-selection.qmd) | QMLDiff adds a selection action and review panel using the internal `SceneSelectionHandler.qml` and `DocumentView.qml` resources. The recognizer uses reMarkable sign-in and Wi-Fi. Typed input bypasses recognition. The command executor calls the Rust binary and reads JSON. The scene controller helps check for existing writing under the placement outline. |
| [Ink layout](../src/ink.rs) and [pen writer](../src/pen.rs) | Lays out Hershey lettering and writes through the tablet's `Elan marker input` device. It assumes the Paper Pro's portrait screen geometry. Xochitl receives pen events and saves the resulting native strokes. A page or tool change during writing can leave partial strokes. |

The installed binary is `/home/root/xovi/exthome/rembiblestudy/entry`. The active selection patch is `/home/root/xovi/exthome/qt-resource-rebuilder/rembiblestudy-selection.qmd`. The API.Bible key lives separately at `/home/root/.config/rembiblestudy/api-key` with file mode `0600`. Provision or replace it with `-PromptApiKey` during installation, or use `-ApiKeyFile` with a private file outside the repository. Omit both options during an upgrade to retain the configured key. Keep credentials and notebook content out of project copies and release archives.

## Upgrade Bible Study on the same firmware

1. Keep the previous release's ARM64 binary and QMD together. For each release, keep the source revision, supported firmware, binary, QMD, and SHA-256 hashes together in a versioned folder outside the build directory.
2. From the new release's source, run `cargo test --locked` and `./tablet/build.ps1` in PowerShell 7 with Docker running. To install a previously built release, use `-Binary` and `-Patch` with the files from the same release folder. Check the hashes against that release's record with `Get-FileHash`.
3. Close the notebook, connect the tablet over USB, and optionally run `./tablet/install.ps1` to check the tablet and candidate files without installing. This check uploads temporary files and runs the candidate's help command. For an existing installation, leave out `-PromptApiKey` and `-ApiKeyFile` to retain the configured key.
4. Run `./tablet/install.ps1 -Reload`. For archived files, run the command below from the current source checkout. The installer reports the previous release's backup path and restarts XOVI for the trial. Wait for **Bible Study upgrade committed** before opening the test notebook.
5. Run the [hands-on checks](#hands-on-checks), including a phrase without an author. To revert a completed installation, use the same `-Binary` and `-Patch` command with the previous release. To recover a trial that fails before committing, see [Disable or recover the patch](#disable-or-recover-the-patch).

```powershell
$releaseDir = 'C:/path/to/your/release'
./tablet/install.ps1 -Reload `
    -Binary "$releaseDir/rembiblestudy" `
    -Patch "$releaseDir/rembiblestudy-selection.qmd"
```

## Before and after a firmware update

1. Verify a notebook backup or export of anything important. Test restoration into a spare notebook when native editability matters. A PDF is useful as a readable copy. Archive the working Bible Study release and record the tablet model, `IMG_VERSION`, and the versions or hashes of XOVI, the two required extensions, and the matching QML resource artifacts when available. [reMarkable import and export](https://support.remarkable.com/s/article/Importing-and-exporting-files)
2. Close any open notebook and disable this project's patch using the procedure below before updating. Record the prior and new firmware versions. Confirm normal tablet startup and that the existing LucidKith and RemarkSync setup works before enabling Bible Study on the new version. [reMarkable update instructions](https://support.remarkable.com/s/article/How-to-update-your-reMarkable-and-apps)
3. Check the new firmware's XOVI support and follow upstream's `hashtab` rebuild instructions. Review the QMLDiff anchors, recognizer properties, keyboard focus, page and layer guards, overlap checks, and pen input assumptions against the new version. Update the firmware guard in the installer and the QMD `VERSION` declaration as part of the same port. [XOVI reMarkable extensions](https://github.com/asivery/rm-xovi-extensions#to-update-hashtab-required-for-ui-mods) · [QMLDiff version declaration](https://github.com/asivery/qmldiff#version-allowed_version)
4. Run `cargo test --locked` on the desktop. Run `./tablet/build.ps1` from PowerShell 7 with Docker running and check that it produces the ARM64 binary. The build locks Rust dependencies, and the Docker tag and system packages currently float. Record the toolchain and image digest for a release you expect to rebuild.
5. On a backed-up test notebook, use `./tablet/install.ps1 -Reload` over USB. Reuse the existing key when it is still present. As part of the trial, the installer checks that `xochitl` starts a new process, stays up, and logs loading the patch. Follow that startup check with the hands-on checks below.

### Hands-on checks

- Lasso `Eph 4:13` and confirm that the NIV verse opens. Correct the reference with the tablet keyboard and confirm the review panel stays open. Try a typed reference when handwriting recognition is unavailable.
- Search `until we all reach unity`, `our hearts are restless`, and `Augustine - our hearts are restless`. Check the displayed source, select a result, and test **As heard** with a different line. Keep test notes in the test notebook.
- Place an insertion on a blank spot, then test relocation around existing ink. Tap **Write here**, close and reopen the notebook, and confirm the full result persists. The earlier broken-letter display cleared on reopening. Check changes to the page, zoom, layer, and pen tool before confirming placement.
- Check LucidKith's selection action and, if using RemarkSync, check that the normal notebook pull includes the inserted ink. Record the firmware, artifact versions and results with the known-good release.

On `3.29.1.0`, hands-on testing of the original release confirmed verse lookup, keyboard editing, and placement. Version `0.2.0` completed a guarded upgrade on the same tablet. The installed files matched the release hashes, XOVI loaded Bible Study and LucidKith, and the tablet returned results for NIV phrase search, Wikiquote phrase search, and author-qualified lookup. The paired rollback was exercised with bind mounts and simulated service failures in Linux. A hands-on pass through lasso, quote review, ink insertion, and RemarkSync is still pending for `0.2.0`.

## Disable or recover the patch

The [installer](../tablet/install.ps1) checks the model, firmware, XOVI prerequisites, and `flock`. It blocks the QMD revision that previously caused a restart loop. The [trial script](../tablet/preview-trial.sh) saves the prior pair in an installation-specific `backup.*` directory, arms a 90-second systemd rollback timer, and temporarily bind-mounts both candidates before restarting XOVI. If the trial fails while the tablet is running, let the timer restore the previous pair and restart XOVI. The stored API.Bible key is managed independently.

If power is lost while the verified files are being committed, the installer stops on the next run because it finds `pending-upgrade`. After rebooting and reconnecting over USB, run the following in the tablet shell before starting XOVI. The marker names the saved pair. An absent backup file means the corresponding active file was absent before the trial. Keep backups in a separate location if you plan to reset the tablet.

```sh
(
    set -eu
    app=/home/root/xovi/exthome/rembiblestudy
    active=/home/root/xovi/exthome/qt-resource-rebuilder/rembiblestudy-selection.qmd
    backup=$(cat "$app/pending-upgrade")
    case "$backup" in "$app"/backup.*) test -d "$backup" ;; *) exit 1 ;; esac
    restore() {
        if [ -f "$1" ]; then
            cp -p "$1" "$2.recover"
            mv "$2.recover" "$2"
        else
            rm -f "$2"
        fi
    }
    restore "$backup/entry" "$app/entry"
    restore "$backup/rembiblestudy-selection.qmd" "$active"
    sync
    rm "$app/pending-upgrade"
)
```

If a committed patch later causes trouble and USB SSH still works, move only this project's active QMD outside the resource rebuilder's directory, then restart XOVI. These commands run in the tablet shell. Close any open notebook first when possible.

```sh
active=/home/root/xovi/exthome/qt-resource-rebuilder/rembiblestudy-selection.qmd
disabled=$(mktemp -d /home/root/xovi/exthome/rembiblestudy/disabled.XXXXXX)
mv "$active" "$disabled/"
systemctl reset-failed xochitl
/home/root/xovi/start
```

Inspect `systemctl status xochitl` and `journalctl -b -u xochitl --no-pager` for startup errors. If startup recovers, leave the patch disabled until the matching build passes the guarded trial. The desktop commands remain available for verse and quote lookup while the tablet integration is disabled. See the [desktop setup](../README.md) for the API.Bible key prompt.

If the tablet is still unresponsive and local patch recovery is unavailable, follow reMarkable's software recovery instructions. Check the recovery and data-loss guidance for your model first. [reMarkable software recovery](https://support.remarkable.com/articles/Knowledge/Software-recovery)
