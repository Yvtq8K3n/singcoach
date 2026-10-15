#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --locked --offline
if [[ $(uname -r) == *[Mm]icrosoft* ]]; then
  helper=target/release/singcoach-player.exe
  if [[ ! -f "$helper" || platform/windows_player.cs -nt "$helper" ]]; then
    compiler=/mnt/c/Windows/Microsoft.NET/Framework64/v4.0.30319/csc.exe
    if [[ ! -f "$compiler" ]]; then
      echo 'Windows .NET Framework C# compiler not found. Cannot build direct audio helper.' >&2
      exit 1
    fi
    if ! timeout 30s "$compiler" /nologo /target:exe "/out:$(wslpath -w "$PWD/$helper")" "$(wslpath -w "$PWD/platform/windows_player.cs")"; then
      echo 'Windows helper compilation failed. Check that Windows interop works in your WSL terminal, then rerun ./run.sh. No WSLg audio fallback will be used.' >&2
      exit 1
    fi
  fi
  # Windows csc can create the PE file without Linux execute permission.
  # Repair cached helpers too, since Rust launches the file directly via WSL.
  chmod u+x "$helper"
  export WINIT_UNIX_BACKEND=x11
  unset WAYLAND_DISPLAY
fi
exec target/release/singcoach "$@"
