import init, { WebSimulator } from "./pkg/virtual_lcd_web.js";

const canvas = document.getElementById("lcdCanvas");
const ctx = canvas.getContext("2d");
const sceneSelect = document.getElementById("sceneSelect");
const applySceneBtn = document.getElementById("applySceneBtn");
const scriptEditor = document.getElementById("scriptEditor");
const runScriptBtn = document.getElementById("runScriptBtn");
const resetBtn = document.getElementById("resetBtn");
const toggleRunBtn = document.getElementById("toggleRunBtn");
const stepBtn = document.getElementById("stepBtn");
const statusLine = document.getElementById("statusLine");
const metaLine = document.getElementById("metaLine");
const fpsPill = document.getElementById("fpsPill");

let simulator;
let running = true;
let pointerDown = false;
let lastFpsTick = performance.now();
let framesSinceFps = 0;

function setStatus(message, isError = false) {
  statusLine.textContent = message;
  statusLine.style.color = isError ? "#ff9a9a" : "#b6dce0";
}

function syncCanvasSize() {
  const width = simulator.width();
  const height = simulator.height();
  canvas.width = width;
  canvas.height = height;
}

function updateMeta() {
  const mode = simulator.mode_name();
  const controller = simulator.controller_name();
  metaLine.textContent = `Modo: ${mode} | ${simulator.width()}x${simulator.height()} | ${controller}`;
}

function renderFrame() {
  const rgba = simulator.frame_rgba();
  const imageData = new ImageData(new Uint8ClampedArray(rgba), simulator.width(), simulator.height());
  ctx.putImageData(imageData, 0, 0);
}

function clientToLcdPoint(event) {
  const rect = canvas.getBoundingClientRect();
  const x = Math.floor(((event.clientX - rect.left) / rect.width) * simulator.width());
  const y = Math.floor(((event.clientY - rect.top) / rect.height) * simulator.height());
  return {
    x: Math.max(0, Math.min(simulator.width() - 1, x)),
    y: Math.max(0, Math.min(simulator.height() - 1, y)),
  };
}

function bindPointer() {
  canvas.addEventListener("pointerdown", (event) => {
    pointerDown = true;
    const point = clientToLcdPoint(event);
    simulator.set_pointer(point.x, point.y, true);
  });

  globalThis.addEventListener("pointerup", () => {
    pointerDown = false;
    simulator.set_pointer(0, 0, false);
  });

  canvas.addEventListener("pointermove", (event) => {
    const point = clientToLcdPoint(event);
    simulator.set_pointer(point.x, point.y, pointerDown);
  });
}

function mapKeyToButton(key) {
  switch (key) {
    case "ArrowUp":
      return "up";
    case "ArrowDown":
      return "down";
    case "ArrowLeft":
      return "left";
    case "ArrowRight":
      return "right";
    case "a":
    case "A":
      return "a";
    case "b":
    case "B":
      return "b";
    case "Enter":
      return "start";
    case "Shift":
      return "select";
    default:
      return undefined;
  }
}

function bindKeyboard() {
  globalThis.addEventListener("keydown", (event) => {
    const button = mapKeyToButton(event.key);
    if (!button) {
      return;
    }
    simulator.set_button(button, true);
    event.preventDefault();
  });

  globalThis.addEventListener("keyup", (event) => {
    const button = mapKeyToButton(event.key);
    if (!button) {
      return;
    }
    simulator.set_button(button, false);
    event.preventDefault();
  });
}

function bindControls() {
  applySceneBtn.addEventListener("click", () => {
    try {
      simulator.set_scene(sceneSelect.value);
      syncCanvasSize();
      updateMeta();
      renderFrame();
      setStatus(`Cena '${sceneSelect.value}' aplicada.`);
    } catch (error) {
      setStatus(String(error), true);
    }
  });

  runScriptBtn.addEventListener("click", () => {
    try {
      simulator.load_script(scriptEditor.value);
      syncCanvasSize();
      updateMeta();
      renderFrame();
      setStatus("Script executado com sucesso.");
    } catch (error) {
      setStatus(String(error), true);
    }
  });

  resetBtn.addEventListener("click", () => {
    try {
      simulator.reset();
      syncCanvasSize();
      updateMeta();
      renderFrame();
      setStatus("Simulador resetado.");
    } catch (error) {
      setStatus(String(error), true);
    }
  });

  toggleRunBtn.addEventListener("click", () => {
    running = !running;
    toggleRunBtn.textContent = running ? "Pausar" : "Rodar";
  });

  stepBtn.addEventListener("click", () => {
    try {
      simulator.step();
      renderFrame();
    } catch (error) {
      setStatus(String(error), true);
    }
  });
}

function tickFps(now) {
  framesSinceFps += 1;
  const elapsed = now - lastFpsTick;
  if (elapsed >= 1000) {
    const fps = Math.round((framesSinceFps * 1000) / elapsed);
    fpsPill.textContent = `${fps} FPS`;
    framesSinceFps = 0;
    lastFpsTick = now;
  }
}

function startLoop() {
  const loop = (now) => {
    try {
      if (running) {
        simulator.step();
      }
      renderFrame();
      tickFps(now);
    } catch (error) {
      setStatus(String(error), true);
      running = false;
      toggleRunBtn.textContent = "Rodar";
    }
    requestAnimationFrame(loop);
  };

  requestAnimationFrame(loop);
}

try {
  await init();
  simulator = new WebSimulator();
  scriptEditor.value = simulator.default_script();
  syncCanvasSize();
  bindPointer();
  bindKeyboard();
  bindControls();
  updateMeta();
  setStatus("Runtime wasm carregado. Viewer pronto.");
  startLoop();
} catch (error) {
  setStatus(`Falha ao inicializar wasm: ${String(error)}`, true);
}
