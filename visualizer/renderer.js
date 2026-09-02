(() => {
  const palette = {
    background: '#0b1017', empty: '#151c27', grid: '#253042',
    p1: '#4fd6a1', p2: '#ff7f95', last: '#ffd37a', text: '#8e9bae', axis: '#344154',
  };

  function prepareCanvas(canvas) {
    const rect = canvas.getBoundingClientRect();
    const dpr = Math.max(1, Math.min(2, window.devicePixelRatio || 1));
    const width = Math.max(1, Math.floor(rect.width * dpr));
    const height = Math.max(1, Math.floor(rect.height * dpr));
    if (canvas.width !== width || canvas.height !== height) { canvas.width = width; canvas.height = height; }
    const ctx = canvas.getContext('2d');
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    return { ctx, width: rect.width, height: rect.height };
  }

  function drawBoard(canvas, frame) {
    const { ctx, width, height } = prepareCanvas(canvas);
    ctx.clearRect(0, 0, width, height);
    ctx.fillStyle = palette.background;
    ctx.fillRect(0, 0, width, height);
    if (!frame) return;
    const gap = 1;
    const cell = Math.max(2, Math.min((width - 12) / frame.width, (height - 12) / frame.height));
    const boardWidth = cell * frame.width;
    const boardHeight = cell * frame.height;
    const left = (width - boardWidth) / 2;
    const top = (height - boardHeight) / 2;
    for (let y = 0; y < frame.height; y++) {
      for (let x = 0; x < frame.width; x++) {
        const value = frame.board[y][x];
        let color = palette.empty;
        if (value === '@') color = palette.p1;
        else if (value === '$') color = palette.p2;
        else if (value === 'a' || value === 's') color = palette.last;
        ctx.fillStyle = color;
        ctx.fillRect(left + x * cell + gap / 2, top + y * cell + gap / 2, Math.max(1, cell - gap), Math.max(1, cell - gap));
      }
    }
    ctx.strokeStyle = palette.grid;
    ctx.strokeRect(left, top, boardWidth, boardHeight);
  }

  function drawPiece(canvas, piece) {
    const { ctx, width, height } = prepareCanvas(canvas);
    ctx.clearRect(0, 0, width, height);
    if (!piece?.rows?.length) {
      ctx.fillStyle = palette.text; ctx.font = '12px system-ui'; ctx.textAlign = 'center';
      ctx.fillText('No piece data', width / 2, height / 2); return;
    }
    const cell = Math.max(8, Math.min(36, (width - 20) / piece.width, (height - 20) / piece.height));
    const left = (width - cell * piece.width) / 2;
    const top = (height - cell * piece.height) / 2;
    for (let y = 0; y < piece.height; y++) {
      for (let x = 0; x < piece.width; x++) {
        ctx.fillStyle = piece.rows[y]?.[x] && piece.rows[y][x] !== '.' ? palette.last : palette.empty;
        ctx.fillRect(left + x * cell + 1, top + y * cell + 1, cell - 2, cell - 2);
      }
    }
  }

  function drawChart(canvas, replay, currentIndex) {
    const { ctx, width, height } = prepareCanvas(canvas);
    ctx.clearRect(0, 0, width, height);
    if (!replay?.frames?.length) return;
    const pad = { left: 38, right: 14, top: 14, bottom: 23 };
    const plotW = Math.max(1, width - pad.left - pad.right);
    const plotH = Math.max(1, height - pad.top - pad.bottom);
    const frames = replay.frames;
    const maxScore = Math.max(1, ...frames.flatMap((frame) => [frame.score.p1, frame.score.p2]));
    ctx.strokeStyle = palette.axis; ctx.lineWidth = 1; ctx.beginPath();
    ctx.moveTo(pad.left, pad.top); ctx.lineTo(pad.left, pad.top + plotH); ctx.lineTo(pad.left + plotW, pad.top + plotH); ctx.stroke();
    ctx.fillStyle = palette.text; ctx.font = '10px system-ui'; ctx.textAlign = 'right';
    ctx.fillText(String(maxScore), pad.left - 6, pad.top + 3); ctx.fillText('0', pad.left - 6, pad.top + plotH + 3);
    const xFor = (i) => pad.left + (frames.length <= 1 ? 0 : i / (frames.length - 1)) * plotW;
    const yFor = (v) => pad.top + plotH - (v / maxScore) * plotH;
    const drawLine = (key, color) => {
      ctx.strokeStyle = color; ctx.lineWidth = 2; ctx.beginPath();
      frames.forEach((frame, i) => { const x = xFor(i), y = yFor(frame.score[key]); i ? ctx.lineTo(x, y) : ctx.moveTo(x, y); });
      ctx.stroke();
    };
    drawLine('p1', palette.p1); drawLine('p2', palette.p2);
    if (Number.isInteger(currentIndex)) {
      const x = xFor(currentIndex); ctx.strokeStyle = 'rgba(255,255,255,.22)'; ctx.beginPath();
      ctx.moveTo(x, pad.top); ctx.lineTo(x, pad.top + plotH); ctx.stroke();
    }
  }

  function drawPreview(canvas, replay) { drawBoard(canvas, replay?.frames?.at(-1) || null); }
  window.FillerRenderer = { drawBoard, drawPiece, drawChart, drawPreview };
})();
