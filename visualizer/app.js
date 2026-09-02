(() => {
  const STORAGE_KEY = 'filler.replays.rust.v1';
  const MAX_REPLAYS = 12;
  const MAX_STORAGE_CHARS = 4_000_000;
  const state = {
    replay: null, index: 0, playing: false, speed: 1, timer: null,
    serverAvailable: false, liveRunning: false, liveEnabled: false, currentMatch: 0,
    eventSource: null, liveHeader: [], liveFrameLines: [], livePieceRemaining: -1, liveReplay: null,
  };
  const els = {};
  const presets = {
    wall_e: { map: 'map00', opponent: 'wall_e', games: 5, side: 'alternate' },
    h2_d2: { map: 'map01', opponent: 'h2_d2', games: 5, side: 'alternate' },
    bender: { map: 'map02', opponent: 'bender', games: 5, side: 'alternate' },
    terminator: { map: 'map01', opponent: 'terminator', games: 5, side: 'alternate' },
  };

  document.addEventListener('DOMContentLoaded', () => {
    bindElements(); bindEvents(); renderAll(); renderLibrary();
    const resize = new ResizeObserver(renderVisuals);
    [els.boardCanvas, els.pieceCanvas, els.chartCanvas].forEach((canvas) => resize.observe(canvas));
    setupServer();
  });

  function bindElements() {
    for (const id of [
      'fileInput','demoButton','exportButton','dropZone','boardCanvas','pieceCanvas','chartCanvas',
      'matchTitle','turnBadge','p1Name','p2Name','p1Score','p2Score','lastMove','lastPlayer','boardSize',
      'resultText','pieceSize','firstButton','prevButton','playButton','nextButton','lastButton','timeline',
      'timelineText','speedSelect','replayList','matchTab','replaysTab','toast','serverBadge','setupText',
      'mapSelect','opponentSelect','sideSelect','gamesInput','seedInput','liveCheckbox','startButton','stopButton',
      'seriesSummary','seriesResults','liveBadge',
    ]) els[id] = document.getElementById(id);
  }

  function bindEvents() {
    els.fileInput.addEventListener('change', async () => {
      const file = els.fileInput.files?.[0]; if (file) await loadFile(file); els.fileInput.value = '';
    });
    els.demoButton.addEventListener('click', () => loadReplay(FillerReplay.makeDemoReplay(), true));
    els.exportButton.addEventListener('click', exportCurrent);
    ['dragenter','dragover'].forEach((name) => els.dropZone.addEventListener(name, (event) => {
      event.preventDefault(); els.dropZone.classList.add('dragging');
    }));
    ['dragleave','drop'].forEach((name) => els.dropZone.addEventListener(name, (event) => {
      event.preventDefault(); els.dropZone.classList.remove('dragging');
    }));
    els.dropZone.addEventListener('drop', async (event) => { const file = event.dataTransfer?.files?.[0]; if (file) await loadFile(file); });
    els.dropZone.addEventListener('click', () => els.fileInput.click());
    document.querySelectorAll('.tab').forEach((button) => button.addEventListener('click', () => setTab(button.dataset.tab)));
    document.querySelectorAll('[data-preset]').forEach((button) => button.addEventListener('click', () => applyPreset(button.dataset.preset)));
    els.startButton.addEventListener('click', startSeries);
    els.stopButton.addEventListener('click', stopSeries);
    els.firstButton.addEventListener('click', () => seek(0));
    els.prevButton.addEventListener('click', () => seek(state.index - 1));
    els.playButton.addEventListener('click', togglePlay);
    els.nextButton.addEventListener('click', () => seek(state.index + 1));
    els.lastButton.addEventListener('click', () => seek(lastIndex()));
    els.timeline.addEventListener('input', () => seek(Number(els.timeline.value)));
    els.speedSelect.addEventListener('change', () => { state.speed = Number(els.speedSelect.value) || 1; if (state.playing) restartTimer(); });
    window.addEventListener('keydown', (event) => {
      if (!state.replay || event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement) return;
      if (event.code === 'Space') { event.preventDefault(); togglePlay(); }
      else if (event.key === 'ArrowLeft') { event.preventDefault(); seek(state.index - 1); }
      else if (event.key === 'ArrowRight') { event.preventDefault(); seek(state.index + 1); }
      else if (event.key === 'Home') { event.preventDefault(); seek(0); }
      else if (event.key === 'End') { event.preventDefault(); seek(lastIndex()); }
    });
  }

  async function setupServer() {
    if (location.protocol === 'file:') {
      setServerState(false, 'Replay-only', 'Run `make visualizer` for Docker match launching and live mode.');
      disableLauncher(true); return;
    }
    try {
      const status = await fetchJSON('/api/status'); state.serverAvailable = true;
      if (!status.engineReady) { setServerState(false, 'Engine missing', status.engineError || `Expected ${status.engineDir}`); disableLauncher(true); return; }
      if (!status.dockerReady) { setServerState(false, 'Docker missing', 'Docker Desktop / Docker CLI is required.'); disableLauncher(true); return; }
      const options = await fetchJSON('/api/options');
      fillSelect(els.mapSelect, options.maps); fillSelect(els.opponentSelect, options.opponents); applyPreset('wall_e', false); connectEvents();
      setServerState(true, 'Ready', `${status.engineDir} · ${status.imageReady ? 'Docker image ready.' : 'Docker image will build on first Start.'}`);
      disableLauncher(false);
    } catch (error) { setServerState(false, 'Server error', error.message || String(error)); disableLauncher(true); }
  }

  function connectEvents() {
    state.eventSource?.close();
    const source = new EventSource('/api/events'); state.eventSource = source;
    source.onmessage = (event) => { try { handleServerEvent(JSON.parse(event.data)); } catch (error) { console.error(error); } };
    source.onopen = () => { if (!state.liveRunning) badge(els.serverBadge, 'Ready', 'ready'); };
    source.onerror = () => { if (!state.liveRunning) badge(els.serverBadge, 'Reconnecting…', 'busy'); };
  }

  function handleServerEvent(event) {
    switch (event.type) {
      case 'setup':
        els.setupText.textContent = event.message || 'Preparing…'; badge(els.serverBadge, 'Preparing…', 'busy'); break;
      case 'series_start':
        state.liveRunning = true; state.liveEnabled = Boolean(event.live); stopPlay(); setTab('match'); disableLauncher(true, true);
        els.stopButton.disabled = false; badge(els.serverBadge, 'Running', 'busy'); els.seriesResults.replaceChildren();
        els.seriesSummary.textContent = `0 / ${event.games} complete`; els.setupText.textContent = `${event.map} · ${event.opponent} · ${event.games} game${event.games === 1 ? '' : 's'}`; renderAll(); break;
      case 'match_start':
        state.currentMatch = Number(event.index) || 0; state.liveEnabled = Boolean(event.live); resetLiveParser(); addRunningSeriesRow(event);
        els.seriesSummary.textContent = `Game ${event.index} / ${event.games}`; els.setupText.textContent = `${event.map} vs ${event.opponent} · ${String(event.side).toUpperCase()}`; renderAll(); break;
      case 'log':
        if (Number(event.index) === state.currentMatch && state.liveEnabled) consumeLiveLine(String(event.line ?? '')); break;
      case 'match_end': handleMatchEnd(event.result); break;
      case 'series_end':
        state.liveRunning = false; state.liveEnabled = false; els.stopButton.disabled = true; disableLauncher(false); badge(els.serverBadge, 'Ready', 'ready');
        badge(els.liveBadge, 'Replay', 'neutral'); els.seriesSummary.textContent = `${event.wins} / ${event.completed} wins${event.stopped ? ' · stopped' : ''}`;
        els.setupText.textContent = event.stopped ? 'Series stopped.' : `Series complete: ${event.wins}/${event.completed} wins.`; toast(`Series complete: ${event.wins}/${event.completed} wins`); renderAll(); break;
      case 'error':
        state.liveRunning = false; state.liveEnabled = false; disableLauncher(false); els.stopButton.disabled = true; badge(els.serverBadge, 'Error', 'error');
        els.setupText.textContent = event.message || 'Match runner failed.'; toast(event.message || 'Match runner failed.', true); renderAll(); break;
    }
  }

  function resetLiveParser() { state.liveHeader = []; state.liveFrameLines = []; state.livePieceRemaining = -1; state.liveReplay = null; }

  function consumeLiveLine(line) {
    if (/^\$\$\$\s+exec\s+p[12]\s*:/i.test(line.trim())) state.liveHeader.push(line);
    if (/^\s*Anfield\s+\d+\s+\d+:/i.test(line)) {
      if (state.liveFrameLines.length) finalizeLiveFrame();
      state.liveFrameLines = [line]; state.livePieceRemaining = -1; return;
    }
    if (!state.liveFrameLines.length) return;
    state.liveFrameLines.push(line);
    const piece = line.trim().match(/^Piece\s+\d+\s+(\d+):/i);
    if (piece) { state.livePieceRemaining = Number(piece[1]); return; }
    if (state.livePieceRemaining > 0 && --state.livePieceRemaining === 0) finalizeLiveFrame();
  }

  function finalizeLiveFrame() {
    if (!state.liveFrameLines.length) return;
    const text = [...state.liveHeader, ...state.liveFrameLines].join('\n'); state.liveFrameLines = []; state.livePieceRemaining = -1;
    try {
      const parsed = FillerReplay.parseFileText(text, `Live game ${state.currentMatch}`); const frame = parsed.frames[0]; if (!frame) return;
      if (!state.liveReplay) state.liveReplay = { version:1, id:`live-${Date.now()}`, name:`Live game ${state.currentMatch}`, createdAt:new Date().toISOString(), players:parsed.players, frames:[], result:{winner:null,reason:'match running'} };
      state.liveReplay.players = parsed.players; frame.index = state.liveReplay.frames.length; state.liveReplay.frames.push(frame);
      state.replay = state.liveReplay; state.index = state.liveReplay.frames.length - 1; renderAll();
    } catch (_) { /* incomplete live frame */ }
  }

  async function handleMatchEnd(result) {
    updateSeriesRow(result);
    try {
      const response = await fetch(`/api/replays/raw?name=${encodeURIComponent(result.file)}`); if (!response.ok) throw new Error((await response.text()).trim());
      const replay = FillerReplay.parseFileText(await response.text(), result.file);
      replay.name = `${result.map} · ${result.opponent} · ${String(result.side).toUpperCase()} · seed ${result.seed}`;
      loadReplay(replay, true, true);
    } catch (error) { toast(`Replay saved on disk, browser load failed: ${error.message || error}`, true); }
  }

  async function startSeries() {
    if (!state.serverAvailable || state.liveRunning) return;
    const payload = { map:els.mapSelect.value, opponent:els.opponentSelect.value, side:els.sideSelect.value, games:Number(els.gamesInput.value), seed:els.seedInput.value.trim(), live:els.liveCheckbox.checked };
    els.startButton.disabled = true; els.setupText.textContent = 'Starting series…';
    try { await fetchJSON('/api/matches/start', { method:'POST', headers:{'Content-Type':'application/json'}, body:JSON.stringify(payload) }); }
    catch (error) { els.startButton.disabled = false; toast(error.message || String(error), true); }
  }

  async function stopSeries() {
    if (!state.serverAvailable || !state.liveRunning) return; els.stopButton.disabled = true; els.setupText.textContent = 'Stopping current match…';
    try { await fetchJSON('/api/matches/stop', { method:'POST' }); } catch (error) { toast(error.message || String(error), true); }
  }

  function applyPreset(name, announce = true) {
    const preset = presets[name]; if (!preset) return; setSelect(els.mapSelect, preset.map); setSelect(els.opponentSelect, preset.opponent);
    els.gamesInput.value = preset.games; els.sideSelect.value = preset.side; if (announce) toast(`${name} preset loaded`);
  }
  function setSelect(select, value) { if ([...select.options].some((option) => option.value === value)) select.value = value; }
  function fillSelect(select, values) { select.replaceChildren(...(values || []).map((value) => Object.assign(document.createElement('option'), { value, textContent:value }))); }
  function disableLauncher(disabled, keepStop = false) {
    [els.mapSelect,els.opponentSelect,els.sideSelect,els.gamesInput,els.seedInput,els.liveCheckbox].forEach((element) => { element.disabled = disabled; });
    document.querySelectorAll('[data-preset]').forEach((button) => { button.disabled = disabled; }); els.startButton.disabled = disabled; if (!keepStop) els.stopButton.disabled = true;
  }
  function setServerState(ok, label, message) { badge(els.serverBadge, label, ok ? 'ready' : 'error'); els.setupText.textContent = message; }
  function badge(element, text, stateName) { element.textContent = text; element.className = `badge ${stateName}`; }

  function addRunningSeriesRow(event) {
    const row = document.createElement('div'); row.className = 'series-row'; row.id = `series-${event.index}`;
    row.innerHTML = `<span>${event.index}</span><span class="series-meta">${escapeHTML(event.map)} · ${escapeHTML(event.opponent)} · ${String(event.side).toUpperCase()}</span><span class="result-run">RUN</span>`;
    els.seriesResults.appendChild(row);
  }
  function updateSeriesRow(result) {
    const row = document.getElementById(`series-${result.index}`); if (!row) return;
    const label = result.error ? 'ERR' : (result.studentWon ? 'WIN' : 'LOSS'); const cls = result.studentWon ? 'result-win' : 'result-loss';
    row.innerHTML = `<span>${result.index}</span><span class="series-meta">${result.p1Score}:${result.p2Score} · seed ${result.seed}</span><span class="${cls}">${label}</span>`;
    if (result.error) row.title = result.error;
  }

  async function loadFile(file) {
    try { const replay = FillerReplay.parseFileText(await file.text(), file.name); loadReplay(replay, true); toast(`Loaded ${replay.frames.length} frames`); }
    catch (error) { toast(error.message || String(error), true); }
  }
  function loadReplay(replay, persist, seekLast = false) {
    stopPlay(); state.replay = FillerReplay.normalizeReplay(replay); state.index = seekLast ? state.replay.frames.length - 1 : 0;
    if (persist) saveReplay(state.replay); setTab('match'); renderAll(); renderLibrary();
  }

  function renderAll() {
    const replay = state.replay; const frame = replay?.frames?.[state.index] || null; const count = replay?.frames?.length || 0;
    els.exportButton.disabled = !replay; els.timeline.disabled = !replay; els.timeline.max = Math.max(0, count - 1); els.timeline.value = Math.min(state.index, Math.max(0, count - 1));
    els.timelineText.textContent = replay ? `${state.index + 1} / ${count}` : '0 / 0'; els.turnBadge.textContent = replay ? `Turn ${state.index + 1} / ${count}` : 'Turn — / —';
    els.matchTitle.textContent = replay?.name || 'No replay loaded'; els.p1Name.textContent = replay?.players?.[0]?.name || 'Player 1'; els.p2Name.textContent = replay?.players?.[1]?.name || 'Player 2';
    els.p1Score.textContent = frame?.score?.p1 ?? 0; els.p2Score.textContent = frame?.score?.p2 ?? 0; els.boardSize.textContent = frame ? `${frame.width} × ${frame.height}` : '—';
    els.lastPlayer.textContent = frame?.lastPlayer ? playerName(frame.lastPlayer) : '—'; els.lastMove.textContent = frame?.move ? `${frame.move.x} ${frame.move.y}` : '—';
    els.pieceSize.textContent = frame?.piece ? `${frame.piece.width} × ${frame.piece.height}` : '—'; els.resultText.textContent = formatResult(replay); els.playButton.textContent = state.playing ? 'Ⅱ Pause' : '▶ Play';
    badge(els.liveBadge, state.liveRunning && state.liveEnabled ? 'Live' : 'Replay', state.liveRunning && state.liveEnabled ? 'busy' : 'neutral'); renderVisuals();
  }
  function renderVisuals() { const frame = state.replay?.frames?.[state.index] || null; FillerRenderer.drawBoard(els.boardCanvas, frame); FillerRenderer.drawPiece(els.pieceCanvas, frame?.piece || null); FillerRenderer.drawChart(els.chartCanvas, state.replay, state.index); }
  function formatResult(replay) { if (!replay?.result) return '—'; if (replay.result.winner == null) return replay.result.reason || 'Draw / unknown'; return `${playerName(replay.result.winner)} · ${replay.result.reason}`; }
  function playerName(id) { return state.replay?.players?.[id - 1]?.name || `Player ${id}`; }

  function seek(index) { if (!state.replay) return; state.index = Math.max(0, Math.min(lastIndex(), index)); if (state.index >= lastIndex() && state.playing) stopPlay(); renderAll(); }
  function lastIndex() { return Math.max(0, (state.replay?.frames?.length || 1) - 1); }
  function togglePlay() { if (!state.replay) return; if (state.playing) { stopPlay(); renderAll(); return; } if (state.index >= lastIndex()) state.index = 0; state.playing = true; restartTimer(); renderAll(); }
  function restartTimer() { if (state.timer) clearInterval(state.timer); state.timer = setInterval(() => { if (!state.replay || state.index >= lastIndex()) { stopPlay(); renderAll(); return; } state.index++; renderAll(); }, Math.max(45, 700 / state.speed)); }
  function stopPlay() { state.playing = false; if (state.timer) clearInterval(state.timer); state.timer = null; }
  function setTab(tab) { document.querySelectorAll('.tab').forEach((button) => button.classList.toggle('active', button.dataset.tab === tab)); els.matchTab.classList.toggle('active', tab === 'match'); els.replaysTab.classList.toggle('active', tab === 'replays'); if (tab === 'replays') renderLibrary(); requestAnimationFrame(renderVisuals); }

  function readLibrary() { try { const value = JSON.parse(localStorage.getItem(STORAGE_KEY) || '[]'); return Array.isArray(value) ? value : []; } catch (_) { return []; } }
  function saveReplay(replay) {
    let items = readLibrary().filter((item) => item.id !== replay.id); items.unshift(replay); items = items.slice(0, MAX_REPLAYS);
    while (items.length > 1 && JSON.stringify(items).length > MAX_STORAGE_CHARS) items.pop();
    try { localStorage.setItem(STORAGE_KEY, JSON.stringify(items)); } catch (_) { toast('Replay is on disk, but browser storage is full.', true); }
  }
  function deleteReplay(id) { localStorage.setItem(STORAGE_KEY, JSON.stringify(readLibrary().filter((item) => item.id !== id))); renderLibrary(); }
  function renderLibrary() {
    const items = readLibrary(); els.replayList.replaceChildren();
    if (!items.length) { const empty = document.createElement('div'); empty.className = 'replay-empty'; empty.textContent = 'No saved replays yet.'; els.replayList.appendChild(empty); return; }
    items.forEach((item) => {
      let replay; try { replay = FillerReplay.normalizeReplay(item); } catch (_) { return; }
      const card = document.createElement('article'); card.className = 'replay-item'; card.innerHTML = `<div class="replay-preview"><canvas></canvas></div><div class="replay-body"><div class="replay-title"></div><div class="replay-meta"></div><div class="replay-actions"><button class="open">Open</button><button class="danger">Delete</button></div></div>`;
      card.querySelector('.replay-title').textContent = replay.name; card.querySelector('.replay-meta').textContent = `${replay.frames.length} turns · ${replay.width}×${replay.height}`;
      card.querySelector('.open').addEventListener('click', () => loadReplay(replay, false)); card.querySelector('.danger').addEventListener('click', () => deleteReplay(replay.id));
      els.replayList.appendChild(card); requestAnimationFrame(() => FillerRenderer.drawPreview(card.querySelector('canvas'), replay));
    });
  }

  function exportCurrent() {
    if (!state.replay) return; const blob = new Blob([JSON.stringify(state.replay, null, 2)], { type:'application/json' }); const url = URL.createObjectURL(blob);
    const link = document.createElement('a'); link.href = url; link.download = `${safeFile(state.replay.name)}.json`; link.click(); setTimeout(() => URL.revokeObjectURL(url), 500);
  }
  function safeFile(value) { return String(value || 'replay').replace(/[^a-z0-9._-]+/gi, '-').replace(/^-+|-+$/g, '') || 'replay'; }
  async function fetchJSON(url, options) { const response = await fetch(url, options); if (!response.ok) throw new Error((await response.text()).trim() || `${response.status} ${response.statusText}`); return response.json(); }
  function toast(message, error = false) { els.toast.textContent = message; els.toast.className = `toast show${error ? ' error' : ''}`; clearTimeout(toast.timer); toast.timer = setTimeout(() => { els.toast.className = 'toast'; }, 3200); }
  function escapeHTML(value) { const span = document.createElement('span'); span.textContent = String(value); return span.innerHTML; }
})();
