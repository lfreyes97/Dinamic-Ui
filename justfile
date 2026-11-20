# Dinamic UI — tareas de construcción y ejecución.
#
#   just              lista las recetas
#   just dev-fast     máquina de pocos núcleos (R3): compila rápido, UI más lenta
#   just dev-full     máquina potente (R7): perfil normal del repo
#
# Las recetas `*-fast` solo cambian flags de cargo (`--config`): no tocan
# Cargo.toml, así que la misma rama sirve en las dos máquinas. La primera vez
# recompilan las dependencias; las iteraciones siguientes son rápidas.

set shell := ["bash", "-euo", "pipefail", "-c"]

bin       := "dinamic-ui"
prefix    := env_var_or_default("PREFIX", env_var("HOME") / ".local/bin")
debug_bin := "target/debug/" + bin
rel_bin   := "target/release/" + bin

fast := "--config 'profile.dev.package.\"*\".opt-level=0' --config 'profile.dev.debug=0'"

alias df := dev-fast
alias cs := check-fast
alias bf := build-fast

default:
    @just --list --unsorted

# Estado de la máquina antes de decidir qué receta usar
info:
    @echo "CPUs: $(nproc)"
    @free -h | head -2
    @df -h . | tail -1

# ── Construcción ────────────────────────────────────────────────────────

# Compilación de desarrollo (rápida, con símbolos de depuración)
build:
    cargo build

# Compilación release (opt-level 2 + LTO thin, perfil del Cargo.toml)
release:
    cargo build --release

# Release con CPU nativa (más rápido en este equipo, no portable)
release-native:
    RUSTFLAGS="-C target-cpu=native" cargo build --release

# Solo comprobar tipos, sin generar binarios
check:
    cargo check --all-targets

# ── Perfil rápido (pocos núcleos): dependencias sin optimizar, sin debuginfo ──

# Arrancar con el perfil rápido (isla)
dev-fast *args:
    cargo {{fast}} run --bin {{bin}} -- {{args}}

# Arrancar con el perfil normal del repo (equivale a `run`)
dev-full *args:
    cargo run --bin {{bin}} -- {{args}}

# Solo comprobar, sin codegen ni link: lo más rápido para iterar
check-fast:
    cargo {{fast}} check --all-targets

# Compilar sin arrancar
build-fast:
    cargo {{fast}} build

# Tests con el perfil rápido
test-fast:
    cargo {{fast}} test

# Clippy tratando los avisos como errores
lint:
    cargo clippy --all-targets -- -D warnings

# Comprobar formato / aplicarlo
fmt-check:
    cargo fmt --check

fmt:
    cargo fmt

# Tests
test:
    cargo test

# Todo lo que debe pasar antes de un commit
ci: fmt-check check lint test

# Limpiar artefactos
clean:
    cargo clean

# ── Ejecución (debug) ───────────────────────────────────────────────────

# Arrancar como isla (modo por defecto)
run *args:
    cargo run --bin {{bin}} -- {{args}}

# Arrancar ya expandido como barra
run-bar:
    DINAMIC_UI_EXPANDED=1 cargo run --bin {{bin}}

# Arrancar con el panel abierto (pestaña 0..4: workspace, clock, system, wallpaper, alertas)
run-panel tab="0":
    DINAMIC_UI_PANEL=1 DINAMIC_UI_PANEL_TAB={{tab}} cargo run --bin {{bin}}

# Arrancar con la paleta indicada (black-metal | bathory)
run-theme theme="bathory":
    DINAMIC_UI_THEME={{theme}} DINAMIC_UI_EXPANDED=1 cargo run --bin {{bin}}

# Barra compacta (24..48 px) y panel bajo (80..320 px)
run-compact:
    DINAMIC_UI_BAR_H=24 DINAMIC_UI_PANEL_H=200 DINAMIC_UI_EXPANDED=1 cargo run --bin {{bin}}

# Barra con backtrace completo si hay un panic
run-debug:
    RUST_BACKTRACE=full DINAMIC_UI_EXPANDED=1 cargo run --bin {{bin}}

# ── Ejecución (release) ─────────────────────────────────────────────────

# Binario release como isla
run-release: release
    ./{{rel_bin}}

# Binario release con el panel abierto
run-release-panel tab="0": release
    DINAMIC_UI_PANEL=1 DINAMIC_UI_PANEL_TAB={{tab}} ./{{rel_bin}}

# Fondo de color sólido con awww (para probar la transparencia de la UI)
wallpaper color="dddddd":
    awww clear {{color}}

# ── Control de una instancia en marcha (socket de control) ──────────────

# Comprueba el binario sin compilar (para las recetas que solo lo ejecutan)
need-release:
    @test -x {{rel_bin}} || { echo "No hay release; corre just release"; exit 1; }

toggle: need-release
    ./{{rel_bin}} --toggle

expand: need-release
    ./{{rel_bin}} --expand

collapse: need-release
    ./{{rel_bin}} --collapse

panel: need-release
    ./{{rel_bin}} --panel

compact: need-release
    ./{{rel_bin}} --compact

notch: need-release
    ./{{rel_bin}} --notch

overview: need-release
    ./{{rel_bin}} --overview

menu: need-release
    ./{{rel_bin}} --menu

# Recargar ~/.config/dinamic-ui/config.toml sin reiniciar
reload: need-release
    ./{{rel_bin}} --reload

quit: need-release
    ./{{rel_bin}} --quit

# Cerrar la instancia actual y arrancar la release recién compilada
restart: release
    -./{{rel_bin}} --quit
    sleep 0.3
    setsid ./{{rel_bin}} >/dev/null 2>&1 &

# ── Instalación ─────────────────────────────────────────────────────────

# Instalar en PREFIX (por defecto ~/.local/bin)
install: release
    install -Dm755 target/release/{{bin}} "{{prefix}}/{{bin}}"
    @echo "Instalado en {{prefix}}"

# Copiar la config de ejemplo si todavía no existe
install-config:
    mkdir -p ~/.config/dinamic-ui
    cp -n config.example.toml ~/.config/dinamic-ui/config.toml
    @echo "Config en ~/.config/dinamic-ui/config.toml"

uninstall:
    rm -f "{{prefix}}/{{bin}}"

# Tamaño del binario release (sin compilar)
size: need-release
    @ls -lh {{rel_bin}}

# ── Línea base visual (Fase 0 del plan de modularización) ────────────────

# Capturas de referencia. Requiere Hyprland, `grim` y una instancia en marcha.
# El dock se captura aparte: su hover depende de env vars al arrancar.
snapshots: need-release
    @command -v grim >/dev/null || { echo "grim no instalado"; exit 1; }
    @mkdir -p docs/baseline
    ./{{rel_bin}} --notch
    sleep 0.4
    grim docs/baseline/island.png
    ./{{rel_bin}} --expand
    sleep 0.4
    grim docs/baseline/bar.png
    ./{{rel_bin}} --panel
    sleep 0.4
    grim docs/baseline/panel.png
    ./{{rel_bin}} --overview
    sleep 0.4
    grim docs/baseline/overview.png
    ./{{rel_bin}} --menu
    sleep 0.4
    grim docs/baseline/menu.png
    ./{{rel_bin}} --collapse
    @echo "Capturas en docs/baseline/"
