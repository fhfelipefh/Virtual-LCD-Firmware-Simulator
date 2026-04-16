const canvas = document.getElementById("lcdCanvas");
const ctx = canvas.getContext("2d");
const sceneSelect = document.getElementById("sceneSelect");
const applySceneBtn = document.getElementById("applySceneBtn");
const scriptEditor = document.getElementById("scriptEditor");
const targetFpsInput = document.getElementById("targetFpsInput");
const applyFpsBtn = document.getElementById("applyFpsBtn");
const runScriptBtn = document.getElementById("runScriptBtn");
const resetBtn = document.getElementById("resetBtn");
const toggleRunBtn = document.getElementById("toggleRunBtn");
const stepBtn = document.getElementById("stepBtn");
const statusLine = document.getElementById("statusLine");
const metaLine = document.getElementById("metaLine");
const fpsPill = document.getElementById("fpsPill");

let simulator;
let wasmInit;
let WebSimulatorCtor;
let running = true;
let pointerDown = false;
let lastFpsTick = performance.now();
let framesSinceFps = 0;
let stepIntervalMs = 1000 / 60;
let lastStepTick = performance.now();

function wait(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

async function reportInitProgress(percent, message) {
  setStatus(`Inicializando ${percent}% - ${message}`);
  await wait(20);
}

function setStatus(message, isError = false) {
  statusLine.classList.remove("is-hidden");
  statusLine.textContent = message;
  statusLine.dataset.error = isError ? "true" : "false";
}

function clearStatus() {
  statusLine.textContent = "";
  statusLine.dataset.error = "false";
  statusLine.classList.add("is-hidden");
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
  metaLine.textContent = `Modo: ${mode} | ${simulator.width()}x${simulator.height()} | ${controller} | ${simulator.fps()}Hz`;
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
  const shouldIgnoreLcdInput = () => {
    const active = document.activeElement;
    if (!(active instanceof HTMLElement)) {
      return false;
    }

    const tag = active.tagName;
    return (
      tag === "TEXTAREA" ||
      tag === "INPUT" ||
      tag === "SELECT" ||
      active.isContentEditable
    );
  };

  globalThis.addEventListener("keydown", (event) => {
    if (shouldIgnoreLcdInput()) {
      return;
    }

    const button = mapKeyToButton(event.key);
    if (!button) {
      return;
    }
    simulator.set_button(button, true);
    event.preventDefault();
  });

  globalThis.addEventListener("keyup", (event) => {
    if (shouldIgnoreLcdInput()) {
      return;
    }

    const button = mapKeyToButton(event.key);
    if (!button) {
      return;
    }
    simulator.set_button(button, false);
    event.preventDefault();
  });
}

function bindControls() {
  applyFpsBtn.addEventListener("click", () => {
    try {
      const nextFps = Number.parseInt(targetFpsInput.value, 10);
      if (!Number.isFinite(nextFps)) {
        throw new TypeError("FPS inválido");
      }

      simulator.set_fps(nextFps);
      const appliedFps = simulator.fps();
      targetFpsInput.value = String(appliedFps);
      stepIntervalMs = 1000 / appliedFps;
      lastStepTick = performance.now();
      updateMeta();
      setStatus(`FPS aplicado: ${appliedFps} Hz`);
    } catch (error) {
      setStatus(String(error), true);
    }
  });

  applySceneBtn.addEventListener("click", () => {
    try {
      simulator.set_scene(sceneSelect.value);
      syncCanvasSize();
      targetFpsInput.value = String(simulator.fps());
      stepIntervalMs = 1000 / simulator.fps();
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
      targetFpsInput.value = String(simulator.fps());
      stepIntervalMs = 1000 / simulator.fps();
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
      targetFpsInput.value = String(simulator.fps());
      stepIntervalMs = 1000 / simulator.fps();
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
        if (now - lastStepTick >= stepIntervalMs) {
          simulator.step();
          lastStepTick = now;
        }
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
  await reportInitProgress(10, "preparando runtime");
  const cacheBust = `${Date.now()}`;
  const wasmModule = await import(`./pkg/virtual_lcd_web.js?v=${cacheBust}`);
  wasmInit = wasmModule.default;
  WebSimulatorCtor = wasmModule.WebSimulator;

  const wasmUrl = new URL(`./pkg/virtual_lcd_web_bg.wasm?v=${cacheBust}`, import.meta.url);
  await reportInitProgress(35, "carregando módulo wasm");
  await wasmInit(wasmUrl);
  await reportInitProgress(62, "criando simulador");
  simulator = new WebSimulatorCtor();
  stepIntervalMs = 1000 / simulator.fps();
  targetFpsInput.value = String(simulator.fps());
  await reportInitProgress(74, "carregando script padrão");
  scriptEditor.value = simulator.default_script();
  await reportInitProgress(82, "configurando canvas");
  syncCanvasSize();
  await reportInitProgress(89, "registrando interações");
  bindPointer();
  bindKeyboard();
  bindControls();
  await reportInitProgress(96, "sincronizando metadados");
  updateMeta();
  setStatus("Inicializando 100% - Runtime wasm carregado. Viewer pronto.");
  setTimeout(() => {
    clearStatus();
  }, 1200);
  startLoop();
} catch (error) {
  const details = error instanceof Error ? `${error.message}\n${error.stack ?? ""}` : String(error);
  console.error("Falha ao inicializar wasm", error);
  setStatus(`Falha ao inicializar wasm: ${details}`, true);
}
