(() => {
  const BOARD_RE = /^\s*\d+\s+([.@$as]+)\s*$/;
  const ANFIELD_RE = /^Anfield\s+(\d+)\s+(\d+):/i;
  const PIECE_RE = /^Piece\s+(\d+)\s+(\d+):/i;
  const EXEC_RE = /^\$\$\$\s+exec\s+p([12])\s*:\s*\[([^\]]+)\]/i;

  function scoreBoard(rows) {
    let p1 = 0;
    let p2 = 0;
    for (const row of rows) {
      for (const cell of row) {
        if (cell === '@' || cell === 'a') p1++;
        if (cell === '$' || cell === 's') p2++;
      }
    }
    return { p1, p2 };
  }

  function inferLastPlayer(rows) {
    let a = 0;
    let s = 0;
    for (const row of rows) {
      for (const cell of row) {
        if (cell === 'a') a++;
        if (cell === 's') s++;
      }
    }
    if (a && !s) return 1;
    if (s && !a) return 2;
    return null;
  }

  function parseMoveNearby(lines, index) {
    for (let i = index - 1; i >= Math.max(0, index - 8); i--) {
      let match = lines[i].match(/(?:player|p)\s*([12]).*?(-?\d+)\s+(-?\d+)\s*$/i);
      if (match) return { player: Number(match[1]), x: Number(match[2]), y: Number(match[3]) };
      match = lines[i].match(/^\s*(-?\d+)\s+(-?\d+)\s*$/);
      if (match) return { player: null, x: Number(match[1]), y: Number(match[2]) };
    }
    return null;
  }

  function parseResult(lines, frames) {
    const tail = lines.slice(Math.max(0, lines.length - 160)).join('\n');
    let match = tail.match(/Player([12])\s+won!/i);
    if (!match) match = tail.match(/player\s*([12])[^\n]*(?:won|wins|winner)/i);
    if (match) return { winner: Number(match[1]), reason: 'game_engine result' };
    const last = frames.at(-1);
    if (!last) return { winner: null, reason: 'unknown' };
    const winner = last.score.p1 === last.score.p2 ? null : (last.score.p1 > last.score.p2 ? 1 : 2);
    return { winner, reason: 'final territory score' };
  }

  function parseRawLog(text, sourceName = 'game_engine replay') {
    const lines = text.replace(/\r/g, '').split('\n');
    const players = {
      1: { id: 1, name: 'Player 1' },
      2: { id: 2, name: 'Player 2' },
    };

    for (const line of lines) {
      const match = line.trim().match(EXEC_RE);
      if (!match) continue;
      const id = Number(match[1]);
      players[id].name = match[2].split(/[\\/]/).filter(Boolean).at(-1) || `Player ${id}`;
    }

    const frames = [];
    for (let i = 0; i < lines.length; i++) {
      const header = lines[i].trim().match(ANFIELD_RE);
      if (!header) continue;
      const width = Number(header[1]);
      const height = Number(header[2]);
      const rows = [];
      let cursor = i + 1;
      while (cursor < lines.length && rows.length < height) {
        const match = lines[cursor].match(BOARD_RE);
        if (match) rows.push(match[1]);
        cursor++;
      }
      if (rows.length !== height || rows.some((row) => row.length !== width)) continue;

      let piece = null;
      for (let j = cursor; j < Math.min(lines.length, cursor + 24); j++) {
        if (ANFIELD_RE.test(lines[j].trim())) break;
        const pieceHeader = lines[j].trim().match(PIECE_RE);
        if (!pieceHeader) continue;
        const pieceWidth = Number(pieceHeader[1]);
        const pieceHeight = Number(pieceHeader[2]);
        const pieceRows = [];
        for (let k = 0; k < pieceHeight && j + 1 + k < lines.length; k++) {
          pieceRows.push(lines[j + 1 + k].trim());
        }
        if (pieceRows.length === pieceHeight && pieceRows.every((row) => row.length === pieceWidth)) {
          piece = { width: pieceWidth, height: pieceHeight, rows: pieceRows };
        }
        break;
      }

      const lastPlayer = inferLastPlayer(rows);
      const move = parseMoveNearby(lines, i);
      if (move && move.player == null) move.player = lastPlayer;
      frames.push({ index: frames.length, width, height, board: rows, piece, score: scoreBoard(rows), lastPlayer, move });
      i = cursor - 1;
    }

    if (!frames.length) throw new Error('No Anfield frames were found in this file.');
    return normalizeReplay({
      version: 1,
      name: sourceName.replace(/\.(log|txt|json)$/i, '') || 'Replay',
      createdAt: new Date().toISOString(),
      players: [players[1], players[2]],
      frames,
      result: parseResult(lines, frames),
    });
  }

  function normalizeReplay(value) {
    if (!value || typeof value !== 'object') throw new Error('Replay JSON must be an object.');
    if (!Array.isArray(value.frames) || !value.frames.length) throw new Error('Replay JSON has no frames.');
    const frames = value.frames.map((frame, index) => {
      if (!Array.isArray(frame.board) || !frame.board.length) throw new Error(`Frame ${index + 1} has no board.`);
      const height = Number(frame.height || frame.board.length);
      const width = Number(frame.width || frame.board[0].length);
      if (frame.board.length !== height || frame.board.some((row) => typeof row !== 'string' || row.length !== width)) {
        throw new Error(`Frame ${index + 1} has inconsistent board dimensions.`);
      }
      return {
        index,
        width,
        height,
        board: frame.board.slice(),
        piece: frame.piece && Array.isArray(frame.piece.rows) ? {
          width: Number(frame.piece.width || frame.piece.rows[0]?.length || 0),
          height: Number(frame.piece.height || frame.piece.rows.length),
          rows: frame.piece.rows.slice(),
        } : null,
        score: frame.score || scoreBoard(frame.board),
        lastPlayer: frame.lastPlayer ?? inferLastPlayer(frame.board),
        move: frame.move && Number.isFinite(Number(frame.move.x)) && Number.isFinite(Number(frame.move.y)) ? {
          player: frame.move.player == null ? null : Number(frame.move.player),
          x: Number(frame.move.x), y: Number(frame.move.y),
        } : null,
      };
    });
    const sourcePlayers = Array.isArray(value.players) && value.players.length >= 2 ? value.players : [];
    return {
      version: 1,
      id: value.id || makeID(),
      name: String(value.name || 'Replay'),
      createdAt: value.createdAt || new Date().toISOString(),
      players: [
        { id: 1, name: String(sourcePlayers[0]?.name || 'Player 1') },
        { id: 2, name: String(sourcePlayers[1]?.name || 'Player 2') },
      ],
      width: frames[0].width,
      height: frames[0].height,
      frames,
      result: value.result && typeof value.result === 'object' ? {
        winner: value.result.winner == null ? null : Number(value.result.winner),
        reason: String(value.result.reason || 'unknown'),
      } : { winner: null, reason: 'unknown' },
    };
  }

  function parseFileText(text, fileName) {
    const trimmed = text.trim();
    if (!trimmed) throw new Error('The selected replay file is empty.');
    if (trimmed.startsWith('{')) {
      try { return normalizeReplay(JSON.parse(trimmed)); } catch (error) { if (!(error instanceof SyntaxError)) throw error; }
    }
    return parseRawLog(text, fileName);
  }

  function makeID() {
    if (globalThis.crypto?.randomUUID) return globalThis.crypto.randomUUID();
    return `replay-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  }

  function makeDemoReplay() {
    const boards = [
      ['..........','..@.......','..........','..........','..........','..........','.......$..','..........'],
      ['..........','..@@a.....','..........','..........','..........','..........','......s$..','..........'],
      ['..........','..@@@.....','....a.....','..........','..........','.....ss...','......$$..','..........'],
      ['..........','..@@@.....','....@@a...','..........','....ss....','.....$$...','......$$..','..........'],
      ['..........','..@@@.....','....@@@...','.....a....','...ss$....','.....$$...','......$$..','..........'],
    ];
    const pieces = [['.OO','.O.'],['OO'],['.O','OO'],['OO','.O'],['OOO']];
    const frames = boards.map((board, index) => ({
      index, width: 10, height: 8, board,
      piece: { width: pieces[index][0].length, height: pieces[index].length, rows: pieces[index] },
      score: scoreBoard(board), lastPlayer: inferLastPlayer(board), move: null,
    }));
    return normalizeReplay({ name: 'Built-in demo', players: [{name:'filler'},{name:'bender'}], frames, result: { winner: 1, reason: 'demo result' } });
  }

  window.FillerReplay = { parseFileText, normalizeReplay, makeDemoReplay, scoreBoard };
})();
