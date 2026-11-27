#!/bin/sh
set -eu

trial=/run/rembiblestudy-preview
app=/home/root/xovi/exthome/rembiblestudy
entry=$app/entry
active=/home/root/xovi/exthome/qt-resource-rebuilder/rembiblestudy-selection.qmd
candidate=$trial/candidate.qmd
guard=rembiblestudy-preview-rollback
pending=$app/pending-upgrade

lock_trial() {
    exec 9>/run/rembiblestudy-preview.lock
    flock -x 9
}

is_bound() {
    awk -v path="$1" '$5 == path { found = 1 } END { exit !found }' /proc/self/mountinfo
}

unmount_pair() {
    if is_bound "$active"; then umount -l "$active"; fi
    if is_bound "$entry"; then umount -l "$entry"; fi
}

clean_trial() {
    rm -f "$entry.new" "$active.new" "$candidate" "$trial/previous-pid" \
        "$pending.new" "$trial/backup" "$trial/committed" "$trial/preview-trial.sh"
    rmdir "$trial"
}

restore_file() {
    if [ -f "$1" ]; then
        cp -p "$1" "$2.new"
        mv "$2.new" "$2"
    else
        rm -f "$2"
    fi
}

restart_xovi() {
    systemctl reset-failed xochitl
    /home/root/xovi/start
}

case "${1:-}" in
begin)
    stage=$2
    lock_trial
    if [ -e "$pending" ]; then
        echo "Incomplete upgrade. See the upgrade guide and $pending." >&2
        exit 1
    fi
    test -x "$stage/entry"
    test -s "$stage/rembiblestudy-selection.qmd"
    for file in "$entry" "$active"; do
        test ! -L "$file"
        if is_bound "$file" || { [ -e "$file" ] && [ ! -f "$file" ]; }; then
            exit 1
        fi
    done
    systemctl is-active --quiet xochitl
    test -x /home/root/xovi/start
    mkdir -m 700 "$trial"
    trap clean_trial EXIT
    cp "$stage/preview-trial.sh" "$trial/preview-trial.sh"
    cp "$stage/rembiblestudy-selection.qmd" "$candidate"
    cp -p "$stage/entry" "$entry.new"
    backup=$(mktemp -d "$app/backup.XXXXXX")
    if [ -f "$entry" ]; then cp -p "$entry" "$backup/entry"; fi
    if [ -f "$active" ]; then cp -p "$active" "$backup/rembiblestudy-selection.qmd"; fi
    printf '%s\n' "$backup" > "$trial/backup"
    systemctl show xochitl -p MainPID --value > "$trial/previous-pid"
    systemd-run --unit="$guard" --on-active=90s --timer-property=AccuracySec=1s \
        /bin/sh "$trial/preview-trial.sh" rollback
    systemctl is-active --quiet "$guard.timer"
    trap - EXIT
    # Placeholders are only needed on a first install or after disabling the patch.
    if [ ! -f "$entry" ]; then touch "$entry"; fi
    if [ ! -f "$active" ]; then printf '%s\n' '; Inactive Bible Study trial' > "$active"; fi
    mount --bind "$entry.new" "$entry"
    mount --bind "$candidate" "$active"
    echo "Rollback armed for both files. Previous release saved in $backup."
    restart_xovi
    ;;
verify)
    sleep 5
    pid=$(systemctl show xochitl -p MainPID --value)
    test "$pid" -gt 1
    test "$pid" != "$(cat "$trial/previous-pid")"
    sleep 18
    logs=$(journalctl -b "_PID=$pid" --no-pager -o cat)
    printf '%s\n' "$logs" | grep -aq 'Loading file rembiblestudy-selection.qmd'
    if printf '%s\n' "$logs" | grep -aqE 'Cannot not open the startup window|Non-existent attached object'; then
        exit 1
    fi
    exec /bin/sh "$trial/preview-trial.sh" commit "$pid"
    ;;
commit)
    pid=$2
    lock_trial
    test "$pid" -gt 1
    systemctl is-active --quiet "$guard.timer"
    systemctl is-active --quiet xochitl
    test "$(systemctl show xochitl -p MainPID --value)" = "$pid"
    test "$(systemctl show xochitl -p NRestarts --value)" = 0
    grep -q '/xovi[.]so' "/proc/$pid/maps"
    is_bound "$active"
    is_bound "$entry"
    cp "$candidate" "$active.new"
    chmod 0644 "$active.new"
    # Keep the recovery path on disk even if the tablet loses power during commit.
    cp "$trial/backup" "$pending.new"
    mv "$pending.new" "$pending"
    sync
    unmount_pair
    mv "$entry.new" "$entry"
    mv "$active.new" "$active"
    sync
    rm "$pending"
    touch "$trial/committed"
    systemctl stop "$guard.timer"
    clean_trial
    echo 'Bible Study upgrade committed.'
    ;;
rollback)
    lock_trial
    # A timer job queued during commit may reach the lock after the trial is gone.
    if [ ! -f "$trial/backup" ] || [ -f "$trial/committed" ]; then exit 0; fi
    backup=$(cat "$trial/backup")
    unmount_pair
    restore_file "$backup/entry" "$entry"
    restore_file "$backup/rembiblestudy-selection.qmd" "$active"
    sync
    rm -f "$pending"
    clean_trial
    restart_xovi
    echo "Bible Study rolled back to $backup."
    ;;
*)
    echo 'Usage: preview-trial.sh begin STAGE | verify | commit PID | rollback' >&2
    exit 2
    ;;
esac
