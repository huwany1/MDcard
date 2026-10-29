# SPDX-License-Identifier: AGPL-3.0-only
# Embedded in package hooks. Uses only Debian Essential shell/coreutils/findutils.
mdcard_backup() {
    mdcard_backup_root=/var/lib/mdcard/installer-backup
    case "$1" in
        prepare)
            if [ -e "$mdcard_backup_root" ]; then
                echo "MDcard: an earlier backup exists. No files changed. Save/close MDcard, then run: sudo sh $mdcard_backup_root/restore.sh --mdcard-restore ; retry apt install." >&2
                return 1
            fi
            [ -d /opt/mdcard ] || return 0
            umask 077
            mkdir -p "$mdcard_backup_root/app"
            cp "$0" "$mdcard_backup_root/restore.sh"
            printf '%s\n' 'Save and close MDcard. Restore with: sudo sh /var/lib/mdcard/installer-backup/restore.sh --mdcard-restore' 'Then repair with: sudo dpkg --configure -a ; sudo apt install ./mdcard_0.1.5_amd64.deb' 'Only a complete, SHA-verified backup is restorable. An incomplete backup means preinst never permitted app overwrites; inspect the old app before removing the incomplete backup.' > "$mdcard_backup_root/RECOVERY.txt"
            cp -a /opt/mdcard/. "$mdcard_backup_root/app/"
            (cd /opt/mdcard && find . -type f -exec sha256sum '{}' +) > "$mdcard_backup_root/old.sha256"
            (cd "$mdcard_backup_root/app" && sha256sum --quiet -c ../old.sha256)
            mdcard_new_files > "$mdcard_backup_root/new-files"
            for mdcard_aux in /usr/bin/mdcard /usr/share/applications/mdcard.desktop /usr/share/icons/hicolor/scalable/apps/mdcard.svg /usr/share/doc/mdcard/copyright /usr/share/mdcard/payload.sha256; do
                if [ -f "$mdcard_aux" ]; then
                    mkdir -p "$mdcard_backup_root/aux$(dirname "$mdcard_aux")"
                    cp -a "$mdcard_aux" "$mdcard_backup_root/aux$mdcard_aux"
                    printf '%s\n' "$mdcard_aux" >> "$mdcard_backup_root/aux-files"
                fi
            done
            printf '%s\n' 'MDcard /opt/mdcard complete' > "$mdcard_backup_root/complete"
            echo "MDcard: complete previous application backup verified at $mdcard_backup_root."
            ;;
        restore)
            [ -d "$mdcard_backup_root" ] || return 0
            if [ ! -f "$mdcard_backup_root/complete" ] || [ "$(cat "$mdcard_backup_root/complete")" != 'MDcard /opt/mdcard complete' ]; then
                echo "MDcard: incomplete backup retained at $mdcard_backup_root; preinst did not permit any application overwrite. Inspect RECOVERY.txt." >&2
                return 1
            fi
            (cd "$mdcard_backup_root/app" && sha256sum --quiet -c ../old.sha256)
            while IFS= read -r mdcard_file; do
                case "$mdcard_file" in ''|/*|../*|*/../*|*/..) echo 'Unsafe product backup member' >&2; return 1 ;; esac
                if [ ! -e "$mdcard_backup_root/app/$mdcard_file" ] && [ ! -L "$mdcard_backup_root/app/$mdcard_file" ]; then
                    rm -f -- "/opt/mdcard/$mdcard_file"
                fi
            done < "$mdcard_backup_root/new-files"
            mkdir -p /opt/mdcard
            cp -a "$mdcard_backup_root/app/." /opt/mdcard/
            if [ -f "$mdcard_backup_root/aux-files" ]; then
                while IFS= read -r mdcard_aux; do cp -a "$mdcard_backup_root/aux$mdcard_aux" "$mdcard_aux"; done < "$mdcard_backup_root/aux-files"
            fi
            (cd /opt/mdcard && sha256sum --quiet -c "$mdcard_backup_root/old.sha256")
            echo 'MDcard: complete previous application files restored; user documents/profile data untouched.'
            # Fixed root-only application backup, identity verified above.
            rm -rf -- "$mdcard_backup_root"
            ;;
        commit)
            # Verify every immutable final file, including manifest itself,
            # before releasing the known usable previous application backup.
            (cd /opt/mdcard && sha256sum --quiet -c /usr/share/mdcard/payload.sha256)
            if [ -d "$mdcard_backup_root" ]; then
                [ -f "$mdcard_backup_root/complete" ] && [ "$(cat "$mdcard_backup_root/complete")" = 'MDcard /opt/mdcard complete' ] || return 1
                rm -rf -- "$mdcard_backup_root"
            fi
            ;;
        *) return 1 ;;
    esac
}
