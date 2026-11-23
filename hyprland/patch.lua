-- HACK TEMPORAL — no es una reserva real de layer-shell.
-- Ajusta SOLO el gap superior para el topbar de dinamic-ui.
-- Cargar con dofile(...) en hyprland.lua, ANTES de los binds.
-- Quitar la línea dofile y recargar Hyprland para deshacerlo.
--
-- Lo correcto sería exclusive_zone en un surface auxiliar anclado arriba.
-- Esto es solo un parche de sesión: el gap es global y afecta a todas las
-- workspaces, y solo a ventanas tiled.

local BASE = 8
local TOP_RESERVED = 40   -- topbar.height (34) + margen

local top = BASE
local topbar_on = false

local function apply()
  hl.config({
    general = {
      gaps_out = { top = top, right = BASE, bottom = BASE, left = BASE },
    },
  })
end

--- Reservar (o no) la franja superior para el topbar.
function patch_topbar(on)
  topbar_on = on and true or false
  top = topbar_on and TOP_RESERVED or BASE
  apply()
end

function patch_gaps_reset()
  top = BASE
  topbar_on = false
  apply()
end

-- Monkey-patch: el bind que conmute `--topbar` arrastra el gap superior.
-- OJO: el estado es local al patch; si el topbar cambia por config o por otro
-- camino, el gap puede desincronizarse. Es un hack, no una fuente de verdad.
local original_exec = hl.dsp.exec_cmd
hl.dsp.exec_cmd = function(...)
  local cmd = select(1, ...)
  if type(cmd) == "string"
      and cmd:find("dinamic%-ui", 1, false)
      and cmd:find("%-%-topbar", 1, false) then
    patch_topbar(not topbar_on)
  end
  return original_exec(...)
end
