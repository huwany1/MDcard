# SPDX-License-Identifier: AGPL-3.0-only
# Embedded in preinst/prerm. No process is killed and no user file is touched.
if [ ! -d /proc/self ]; then
    echo 'MDcard: /proc is unavailable; cannot verify that the editor is closed. Retry from a normal installed system.' >&2
    exit 1
fi
for mdcard_process in /proc/[0-9]*; do
    [ -d "$mdcard_process" ] || continue
    mdcard_exe=$(readlink "$mdcard_process/exe" 2>/dev/null || :)
    case "$mdcard_exe" in
        /opt/mdcard/mdcard|'/opt/mdcard/mdcard (deleted)')
            echo "MDcard is running (PID ${mdcard_process##*/}). Save your documents and close MDcard, then retry this package operation. No process was stopped." >&2
            exit 1
            ;;
    esac
    # Restricted /proc policies may deny readlink even to a container root.
    # Do not interpret an inaccessible MDcard process as an idle installation.
    if [ -z "$mdcard_exe" ]; then
        mdcard_name=''
        if IFS= read -r mdcard_name < "$mdcard_process/comm" 2>/dev/null; then
            case "$mdcard_name" in
                mdcard|MDcard)
                    # A zombie no longer owns an executable image or any open
                    # runtime file. Container PID 1 may retain its comm entry.
                    mdcard_state=''
                    if IFS=' ' read -r mdcard_pid mdcard_comm mdcard_state mdcard_rest < "$mdcard_process/stat" 2>/dev/null; then
                        case "$mdcard_state" in Z|X) continue ;; esac
                    fi
                    echo "MDcard process ${mdcard_process##*/} is running and its executable path is not readable. Close it and retry; existing files were preserved." >&2
                    exit 1
                    ;;
            esac
        elif [ -d "$mdcard_process" ]; then
            echo "MDcard: cannot inspect $mdcard_process under the current /proc permissions. No files changed; retry with normal package-manager permissions." >&2
            exit 1
        fi
    fi
done
exit 0
