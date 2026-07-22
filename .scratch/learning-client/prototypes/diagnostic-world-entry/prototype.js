/*
 * Winning viewport-first Diagnostic World-entry cockpit.
 * Browser-only interaction mock. It is not Bevy or platform-build evidence.
 */

const phases = [
  { key: "offline", short: "Offline", title: "Ready to connect" },
  { key: "login", short: "Login", title: "Authenticating login socket" },
  { key: "realm", short: "Realm", title: "Reference Realm selected" },
  { key: "world", short: "World auth", title: "Authenticating world socket" },
  { key: "character", short: "Character", title: "Controlled character selected" },
  { key: "entered", short: "Entered", title: "Entered Diagnostic World" },
  { key: "moving", short: "Moving", title: "Locally predicting movement" },
  { key: "reentered", short: "Re-entry", title: "Realm re-observed moved pose" },
];

const scriptedEvents = [
  { phase: 0, kind: "ClientPhase", detail: "Disconnected · scripted transport idle" },
  { phase: 1, kind: "ClientPhase", detail: "Login challenge → SRP proof → realm list" },
  { phase: 2, kind: "RealmSelected", detail: "Miaz Reference · realm 1 · build 12340" },
  { phase: 3, kind: "ClientPhase", detail: "World challenge → AUTH_OK · encrypted headers active" },
  { phase: 4, kind: "CharacterSelected", detail: "Miaz · GUID 0x0000…07A1" },
  { phase: 5, kind: "RealmObservedPose", detail: "LOGIN_VERIFY_WORLD · map 0 · entry anchor recorded", realm: true },
  { phase: 6, kind: "MovementSubmitted", detail: "START_FORWARD → HEARTBEAT → STOP · no sender ACK" },
  { phase: 7, kind: "RealmObservedPose", detail: "Reconnect LOGIN_VERIFY_WORLD matches moved pose", realm: true },
];

const state = {
  phase: 0,
  playing: false,
  timer: null,
  eventClock: 0,
  events: [],
  local: { x: 0, z: 0 },
  submitted: { x: 0, z: 0 },
  observed: { x: 0, z: 0 },
  trail: [{ x: 0, z: 0 }],
  correctionFrom: null,
  correctionTo: null,
  correctionActive: false,
  correctionElapsed: 0,
  correctionDuration: 0.3,
  correctionLabel: "none",
  yaw: -0.42,
  zoom: 1,
  drag: null,
  pressed: new Set(),
  lastFrame: performance.now(),
  lastUiUpdate: 0,
};

const app = document.querySelector("#app");

function fmt(value) {
  return value.toFixed(2).padStart(7, " ");
}

function worldPose(pose) {
  return `0 / ${fmt(-8833.3 + pose.x)} ${fmt(628.6)} ${fmt(94 + pose.z)}`;
}

function delta() {
  const dx = state.local.x - state.observed.x;
  const dz = state.local.z - state.observed.z;
  return Math.hypot(dx, dz);
}

function distanceFromEntry() {
  return Math.hypot(state.submitted.x, state.submitted.z);
}

function canVerifyMovement() {
  return state.phase >= 6
    && distanceFromEntry() >= 2
    && state.pressed.size === 0
    && !state.correctionActive;
}

function addEvent(event) {
  state.eventClock += 137;
  state.events.unshift({
    ...event,
    time: `+${(state.eventClock / 1000).toFixed(3)}s`,
  });
  state.events = state.events.slice(0, 8);
}

function applyPhase(index) {
  state.phase = Math.max(0, Math.min(phases.length - 1, index));
  const event = scriptedEvents[state.phase];
  if (event) addEvent(event);

  if (state.phase === 5) {
    state.local = { x: 0, z: 0 };
    state.submitted = { x: 0, z: 0 };
    state.observed = { x: 0, z: 0 };
    state.trail = [{ x: 0, z: 0 }];
    state.correctionFrom = null;
    state.correctionTo = null;
    state.correctionActive = false;
    state.correctionElapsed = 0;
    state.correctionLabel = "none";
  }

  if (state.phase === 6 && Math.hypot(state.local.x, state.local.z) < 0.1) {
    state.local = { x: 3.4, z: -1.8 };
    state.submitted = { ...state.local };
    state.trail = [{ x: 0, z: 0 }, { ...state.local }];
  }

  if (state.phase === 7) {
    state.submitted = { ...state.local };
    state.observed = { ...state.submitted };
    state.correctionFrom = null;
    state.correctionTo = null;
    state.correctionActive = false;
    state.correctionElapsed = 0;
    state.correctionLabel = "none";
  }

  if (state.phase === phases.length - 1) stopPlayback();

  updateBindings();
}

function nextPhase() {
  if (state.phase >= phases.length - 1) {
    stopPlayback();
    return;
  }
  applyPhase(state.phase + 1);
}

function previousPhase() {
  stopPlayback();
  applyPhase(state.phase - 1);
}

function resetScenario() {
  stopPlayback();
  state.eventClock = 0;
  state.events = [];
  state.local = { x: 0, z: 0 };
  state.submitted = { x: 0, z: 0 };
  state.observed = { x: 0, z: 0 };
  state.trail = [{ x: 0, z: 0 }];
  state.correctionFrom = null;
  state.correctionTo = null;
  state.correctionActive = false;
  state.correctionElapsed = 0;
  state.correctionLabel = "none";
  applyPhase(0);
}

function togglePlayback() {
  if (state.playing) {
    stopPlayback();
    updateBindings();
    return;
  }

  if (state.phase >= phases.length - 1) resetScenario();
  state.playing = true;
  state.timer = window.setInterval(nextPhase, 1150);
  updateBindings();
}

function stopPlayback() {
  state.playing = false;
  if (state.timer) window.clearInterval(state.timer);
  state.timer = null;
}

function injectCorrection() {
  if (state.phase < 5) applyPhase(5);
  stopPlayback();
  state.correctionFrom = { ...state.local };
  state.correctionTo = { x: state.local.x - 1.25, z: state.local.z + 0.75 };
  state.correctionActive = true;
  state.correctionElapsed = 0;
  state.correctionLabel = "scripted · 300 ms interp";
  addEvent({
    kind: "PoseCorrection*",
    detail: "Injected client event · 1.46 m · 300 ms interpolation · source undecided",
    future: true,
  });
  updateBindings();
}

function runReconnectProof() {
  if (!canVerifyMovement()) return;
  stopPlayback();
  state.submitted = { ...state.local };
  window.setTimeout(() => {
    applyPhase(7);
  }, 420);
}

function sessionSteps() {
  return phases.map((phase, index) => {
    const className = index < state.phase ? "done" : index === state.phase ? "active" : "";
    return `
      <li class="session-step ${className}">
        <span class="status-dot ${className}"></span>
        <span>${phase.short}</span>
      </li>`;
  }).join("");
}

function metrics() {
  const definitions = {
    realm: ["Realm", "Miaz Reference / 1", ""],
    map: ["Map identity", "0 · Eastern Kingdoms", ""],
    character: ["Controlled character", "Miaz · 0x…07A1", ""],
    observed: ["Realm-observed pose", worldPose(state.observed), "observed"],
    submitted: ["Last submitted pose", worldPose(state.submitted), "emphasis"],
    rendered: ["Rendered/local pose", worldPose(state.local), "emphasis"],
    divergence: ["Rendered ↔ observed", `${delta().toFixed(2)} m`, state.correctionTo ? "correction" : ""],
    correction: ["Correction source", state.correctionLabel, state.correctionTo ? "correction" : ""],
    evidence: ["Observed by", state.phase >= 7 ? "Reconnect LOGIN_VERIFY_WORLD" : state.phase >= 5 ? "Entry LOGIN_VERIFY_WORLD" : "—", "observed"],
  };
  const fields = ["realm", "map", "character", "observed", "submitted", "rendered", "divergence", "correction", "evidence"];
  return fields.map((key) => {
    const [label, value, className] = definitions[key];
    return `<div class="metric"><dt>${label}</dt><dd class="${className}" data-field="${key}">${value}</dd></div>`;
  }).join("");
}

function eventsMarkup(limit = 5) {
  return state.events.slice(0, limit).map((event) => `
    <li class="event-row">
      <span class="event-time">${event.time}</span>
      <span class="event-kind ${event.realm ? "realm" : ""} ${event.future ? "future" : ""}">${event.kind}</span>
      <span>${event.detail}</span>
    </li>
  `).join("") || `
    <li class="event-row">
      <span class="event-time">—</span>
      <span class="event-kind">Idle</span>
      <span>Play or step the scripted client scenario.</span>
    </li>`;
}

function controls() {
  return `
    <div class="scenario-controls">
      <button type="button" data-action="previous">PREV</button>
      <button type="button" class="primary" data-action="play">${state.playing ? "PAUSE" : "CONNECT & ENTER"}</button>
      <button type="button" data-action="next">NEXT</button>
      <button type="button" data-action="correction">INJECT CORRECTION*</button>
      <button type="button" data-action="reconnect" ${canVerifyMovement() ? "" : "disabled"}>VERIFY PERSISTED MOVEMENT</button>
      <button type="button" data-action="reset">RESET</button>
    </div>`;
}

function acceptance() {
  const passed = state.phase >= 7 && delta() < 0.05;
  return `
    <div class="acceptance ${passed ? "pass" : ""}">
      <strong>${passed ? "Acceptance passed" : "Acceptance pending"}</strong>
      <span>${passed
        ? "Same map; reconnect LOGIN_VERIFY_WORLD is within 0.25 m of the last submitted stop pose."
        : "Stop at least 2 m from entry, then verify the persisted pose through saving reconnect."}</span>
    </div>`;
}

function scene() {
  return `
    <canvas class="world-canvas" data-world aria-label="Interactive placeholder Diagnostic World"></canvas>
    <div class="scene-badge"><span class="status-dot ${state.phase >= 5 ? "done" : ""}"></span><span>MAP 0 / project-owned grid / WASD + RMB orbit</span></div>
    <div class="scene-legend">
      <span class="legend-item"><i class="legend-swatch"></i>rendered</span>
      <span class="legend-item"><i class="legend-swatch submitted"></i>submitted</span>
      <span class="legend-item"><i class="legend-swatch observed"></i>realm observed</span>
      <span class="legend-item"><i class="legend-swatch correction"></i>scripted correction*</span>
    </div>`;
}

function stamp() {
  return `<div class="prototype-stamp">browser mock · engine-free events · not Bevy proof</div>`;
}

function renderDiagnosticCockpit() {
  return `
    <main class="shell diagnostic-cockpit">
      <header class="a-header">
        <div class="a-header-group">
          ${stamp()}
          <div><div class="eyebrow">Client phase</div><h1 class="phase-title" data-field="phaseTitle">${phases[state.phase].title}</h1></div>
        </div>
        <div class="a-identity"><span>MIAZ REFERENCE</span><span>MAP 0</span><span>Miaz / 0x…07A1</span></div>
      </header>
      <aside class="a-sidebar">
        <div class="section-label">Session ladder</div>
        <ol class="session-ladder" data-field="sessionSteps">${sessionSteps()}</ol>
        <div style="height: 18px"></div>
        <div class="section-label">Scenario</div>
        <p class="mono-note">One compact vertical truth: where the client is, what it knows, and which evidence established it.</p>
      </aside>
      <section class="scene-frame a-scene">${scene()}</section>
      <aside class="a-inspector">
        <div class="section-label">Identity & poses</div>
        <dl class="metric-grid" data-field="metrics">${metrics()}</dl>
      </aside>
      <section class="a-events">
        <div>
          <div class="section-label">Recent semantic events</div>
          <ol class="event-list" data-field="events">${eventsMarkup(3)}</ol>
        </div>
        <div class="a-acceptance">
          ${acceptance()}
          <div style="height: 9px"></div>
          ${controls()}
        </div>
      </section>
    </main>`;
}

function render() {
  app.innerHTML = renderDiagnosticCockpit();
  bindActions();
  setupCanvas();
}

function bindActions() {
  document.querySelectorAll("[data-action]").forEach((button) => {
    button.addEventListener("click", () => {
      const action = button.dataset.action;
      if (action === "play") togglePlayback();
      if (action === "next") nextPhase();
      if (action === "previous") previousPhase();
      if (action === "reset") resetScenario();
      if (action === "correction") injectCorrection();
      if (action === "reconnect") runReconnectProof();
    });
  });
}

function updateBindings() {
  document.querySelectorAll('[data-field="phaseTitle"]').forEach((node) => { node.textContent = phases[state.phase].title; });
  document.querySelectorAll('[data-field="sessionSteps"]').forEach((node) => {
    node.innerHTML = sessionSteps();
  });
  document.querySelectorAll('[data-field="metrics"]').forEach((node) => {
    node.innerHTML = metrics();
  });
  document.querySelectorAll('[data-field="events"]').forEach((node) => {
    node.innerHTML = eventsMarkup(3);
  });
  document.querySelectorAll(".acceptance").forEach((node) => {
    node.outerHTML = acceptance();
  });
  document.querySelectorAll('[data-action="play"]').forEach((button) => {
    button.textContent = state.playing ? "PAUSE" : "CONNECT & ENTER";
  });
  updateControlAvailability();
}

function updateControlAvailability() {
  document.querySelectorAll('[data-action="reconnect"]').forEach((button) => {
    button.disabled = !canVerifyMovement();
  });
}

function project(point, canvas) {
  const followedX = point.x - state.local.x;
  const followedZ = point.z - state.local.z;
  const cos = Math.cos(state.yaw);
  const sin = Math.sin(state.yaw);
  const rx = followedX * cos - followedZ * sin;
  const rz = followedX * sin + followedZ * cos;
  const scale = Math.min(canvas.width, canvas.height) * 0.055 * state.zoom;
  return {
    x: canvas.width * 0.5 + rx * scale,
    y: canvas.height * 0.54 + rz * scale * 0.48,
  };
}

function drawWorld(canvas) {
  const ctx = canvas.getContext("2d");
  const dpr = window.devicePixelRatio || 1;
  const width = Math.floor(canvas.clientWidth * dpr);
  const height = Math.floor(canvas.clientHeight * dpr);
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }

  ctx.clearRect(0, 0, width, height);
  const backdrop = ctx.createLinearGradient(0, 0, 0, height);
  backdrop.addColorStop(0, "#14221b");
  backdrop.addColorStop(0.48, "#0c1511");
  backdrop.addColorStop(1, "#080d0a");
  ctx.fillStyle = backdrop;
  ctx.fillRect(0, 0, width, height);

  ctx.lineWidth = dpr;
  for (let i = -12; i <= 12; i += 1) {
    const a = project({ x: i, z: -12 }, canvas);
    const b = project({ x: i, z: 12 }, canvas);
    const c = project({ x: -12, z: i }, canvas);
    const d = project({ x: 12, z: i }, canvas);
    ctx.strokeStyle = i === 0 ? "rgba(182,243,107,0.22)" : "rgba(222,235,225,0.075)";
    ctx.beginPath(); ctx.moveTo(a.x, a.y); ctx.lineTo(b.x, b.y); ctx.stroke();
    ctx.beginPath(); ctx.moveTo(c.x, c.y); ctx.lineTo(d.x, d.y); ctx.stroke();
  }

  const origin = project({ x: 0, z: 0 }, canvas);
  const observed = project(state.observed, canvas);
  const submitted = project(state.submitted, canvas);
  const local = project(state.local, canvas);

  if (state.trail.length > 1) {
    ctx.strokeStyle = "rgba(104,216,220,0.72)";
    ctx.lineWidth = 2 * dpr;
    ctx.beginPath();
    state.trail.forEach((pose, index) => {
      const point = project(pose, canvas);
      if (index === 0) ctx.moveTo(point.x, point.y);
      else ctx.lineTo(point.x, point.y);
    });
    ctx.stroke();
  }

  ctx.setLineDash([5 * dpr, 5 * dpr]);
  ctx.strokeStyle = "rgba(240,189,104,0.5)";
  ctx.beginPath(); ctx.moveTo(origin.x, origin.y); ctx.lineTo(observed.x, observed.y); ctx.stroke();
  ctx.strokeStyle = "rgba(104,216,220,0.38)";
  ctx.beginPath(); ctx.moveTo(observed.x, observed.y); ctx.lineTo(submitted.x, submitted.y); ctx.stroke();
  ctx.setLineDash([]);

  marker(ctx, observed, 8 * dpr, "#f0bd68", "REALM OBSERVED", dpr);
  marker(ctx, submitted, 5 * dpr, "#68d8dc", "SUBMITTED", dpr, -25);

  if (state.correctionFrom && state.correctionTo) {
    const from = project(state.correctionFrom, canvas);
    const to = project(state.correctionTo, canvas);
    ctx.strokeStyle = "#ef7ec6";
    ctx.lineWidth = 2 * dpr;
    ctx.beginPath(); ctx.moveTo(from.x, from.y); ctx.lineTo(to.x, to.y); ctx.stroke();
    marker(ctx, to, 7 * dpr, "#ef7ec6", "SCRIPTED CORRECTION*", dpr, 30);
  }

  ctx.save();
  ctx.translate(local.x, local.y);
  ctx.fillStyle = "rgba(0,0,0,0.38)";
  ctx.beginPath(); ctx.ellipse(0, 10 * dpr, 13 * dpr, 5 * dpr, 0, 0, Math.PI * 2); ctx.fill();
  ctx.fillStyle = state.phase >= 5 ? "#68d8dc" : "#536158";
  ctx.strokeStyle = "rgba(255,255,255,0.45)";
  ctx.lineWidth = dpr;
  roundRect(ctx, -9 * dpr, -25 * dpr, 18 * dpr, 38 * dpr, 8 * dpr);
  ctx.fill(); ctx.stroke();
  ctx.fillStyle = "#0b120e";
  ctx.beginPath(); ctx.arc(0, -17 * dpr, 2.4 * dpr, 0, Math.PI * 2); ctx.fill();
  ctx.restore();

  ctx.fillStyle = "rgba(238,244,237,0.42)";
  ctx.font = `${9 * dpr}px ui-monospace, monospace`;
  ctx.fillText("DIAGNOSTIC PLANE / NO AZEROTH ASSETS", 18 * dpr, height - 20 * dpr);
}

function marker(ctx, point, radius, color, label, dpr, yOffset = 20) {
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.5 * dpr;
  ctx.beginPath(); ctx.arc(point.x, point.y, radius, 0, Math.PI * 2); ctx.stroke();
  ctx.fillStyle = color;
  ctx.font = `${8 * dpr}px ui-monospace, monospace`;
  ctx.textAlign = "center";
  ctx.fillText(label, point.x, point.y + yOffset * dpr);
  ctx.textAlign = "start";
}

function roundRect(ctx, x, y, width, height, radius) {
  ctx.beginPath();
  ctx.roundRect(x, y, width, height, radius);
}

function setupCanvas() {
  const canvas = document.querySelector("[data-world]");
  if (!canvas) return;

  canvas.addEventListener("pointerdown", (event) => {
    if (event.button !== 2) return;
    state.drag = { x: event.clientX, yaw: state.yaw };
    canvas.setPointerCapture(event.pointerId);
  });
  canvas.addEventListener("pointermove", (event) => {
    if (!state.drag) return;
    state.yaw = state.drag.yaw + (event.clientX - state.drag.x) * 0.008;
  });
  canvas.addEventListener("pointerup", () => { state.drag = null; });
  canvas.addEventListener("contextmenu", (event) => event.preventDefault());
  canvas.addEventListener("wheel", (event) => {
    event.preventDefault();
    state.zoom = Math.max(0.65, Math.min(1.7, state.zoom - event.deltaY * 0.001));
  }, { passive: false });
}

function updateMovement(deltaSeconds) {
  if (state.phase < 5) return;
  const speed = 2.6;
  let dx = 0;
  let dz = 0;
  const step = speed * deltaSeconds;
  const cos = Math.cos(state.yaw);
  const sin = Math.sin(state.yaw);
  if (state.pressed.has("w")) { dx -= sin * step; dz -= cos * step; }
  if (state.pressed.has("s")) { dx += sin * step; dz += cos * step; }
  if (state.pressed.has("a")) { dx -= cos * step; dz += sin * step; }
  if (state.pressed.has("d")) { dx += cos * step; dz -= sin * step; }
  if (dx === 0 && dz === 0) return;
  state.local.x = Math.max(-9, Math.min(9, state.local.x + dx));
  state.local.z = Math.max(-9, Math.min(9, state.local.z + dz));
  state.submitted = { ...state.local };
  const lastTrailPoint = state.trail[state.trail.length - 1];
  if (Math.hypot(state.local.x - lastTrailPoint.x, state.local.z - lastTrailPoint.z) >= 0.1) {
    state.trail.push({ ...state.local });
    state.trail = state.trail.slice(-120);
  }
  if (state.phase < 6) {
    state.phase = 6;
    addEvent(scriptedEvents[6]);
  }
}

function frame(now) {
  const deltaSeconds = Math.min(0.04, (now - state.lastFrame) / 1000);
  state.lastFrame = now;
  updateMovement(deltaSeconds);

  if (state.correctionActive && state.correctionTo) {
    const correctionDistance = Math.hypot(
      state.correctionTo.x - state.correctionFrom.x,
      state.correctionTo.z - state.correctionFrom.z,
    );
    state.correctionElapsed += deltaSeconds;
    const amount = correctionDistance > 5
      ? 1
      : Math.min(1, state.correctionElapsed / state.correctionDuration);
    state.local.x = state.correctionFrom.x + (state.correctionTo.x - state.correctionFrom.x) * amount;
    state.local.z = state.correctionFrom.z + (state.correctionTo.z - state.correctionFrom.z) * amount;
    if (amount >= 1) {
      state.local = { ...state.correctionTo };
      state.correctionActive = false;
    }
  }

  const canvas = document.querySelector("[data-world]");
  if (canvas) drawWorld(canvas);
  if (now - state.lastUiUpdate >= 100) {
    state.lastUiUpdate = now;
    updateBindings();
  } else {
    updateControlAvailability();
  }
  window.requestAnimationFrame(frame);
}

window.addEventListener("keydown", (event) => {
  if (["w", "a", "s", "d"].includes(event.key.toLowerCase())) state.pressed.add(event.key.toLowerCase());
});

window.addEventListener("keyup", (event) => {
  state.pressed.delete(event.key.toLowerCase());
});

resetScenario();
render();
window.requestAnimationFrame(frame);
