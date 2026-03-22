#!/bin/zsh

set -euo pipefail

# Bring Simulator to the foreground and send the Home shortcut.
open -a Simulator

osascript <<'APPLESCRIPT'
tell application "Simulator" to activate
delay 0.2
tell application "System Events"
    key code 4 using {command down, shift down}
end tell
APPLESCRIPT
