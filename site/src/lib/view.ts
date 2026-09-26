// The three.js view of the two exemplars, loaded when the section comes near the viewport.
import * as THREE from 'three';
import glm from '../data/battle-hymn-glm-5.3.notes.json';
import kimi from '../data/battle-hymn-kimi-k3.notes.json';
import sections from '../data/sections.json';
import { alignment, bounds, difference, stats, RATE, type NotesFile, type SectionsFile } from './compare';

export function start(root: HTMLElement): void {
  const sec = sections as unknown as SectionsFile;
  const files = [glm, kimi] as unknown as NotesFile[];
  const ids = files.map((f) => f.piece);
  const al = alignment(sec, ids);
  const st = files.map((f) => stats(f, sec));
  const diffs = st[0].map((a, i) => difference(a, st[1][i], 'glm-5.3', 'kimi-k3'));
  const stage = root.querySelector<HTMLElement>('.sj-stage')!;
  const canvas = stage.querySelector('canvas')!;
  const hud = stage.querySelector<HTMLElement>('.sj-hud')!;
  const chips = [...root.querySelectorAll<HTMLButtonElement>('.sj-chips button')];
  const modes = [...root.querySelectorAll<HTMLButtonElement>('.sj-modes button')];
  const rows = [...root.querySelectorAll<HTMLTableRowElement>('.sj-diff tbody tr')];
  const audios = [...root.querySelectorAll<HTMLAudioElement>('audio')];
  const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;

  let renderer: THREE.WebGLRenderer;
  try {
    renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
  } catch {
    stage.classList.add('sj-nogl');
    return;
  }
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));

  // Scene units: XS per second of the shared axis, PS per semitone.
  const XS = 3;
  const PS = 0.22;
  const GAP = 2.4;
  const pitches = files.flatMap((f) => f.notes.map((n) => n[2]));
  const pmin = Math.min(...pitches) - 2;
  const laneH = (Math.max(...pitches) + 2 - pmin + 1) * PS;
  const laneY = [laneH + GAP, 0];
  const totalH = laneH * 2 + GAP;
  const totalW = al.total * XS;
  const bright = [new THREE.Color('#4a9eff'), new THREE.Color('#ff6b8a')];
  const dim = bright.map((c) => c.clone().lerp(new THREE.Color('#09090b'), 0.7));

  const scene = new THREE.Scene();
  scene.background = new THREE.Color('#09090b');
  scene.add(new THREE.AmbientLight(0xffffff, 0.6));
  const sun = new THREE.DirectionalLight(0xffffff, 1.2);
  sun.position.set(-30, 50, 80);
  scene.add(sun);

  al.x0.forEach((x, i) => {
    const w = al.widths[i] * XS;
    const band = new THREE.Mesh(
      new THREE.PlaneGeometry(w, totalH + 1.4),
      new THREE.MeshBasicMaterial({ color: i % 2 ? '#101014' : '#16161c' }),
    );
    band.position.set(x * XS + w / 2, totalH / 2, -0.05);
    scene.add(band);
    const d = diffs[i];
    const strip = new THREE.Mesh(
      new THREE.PlaneGeometry(Math.max(0.5, w - 0.6), Math.max(0.15, d.score * (GAP - 0.5))),
      new THREE.MeshBasicMaterial({ color: '#ffc857', transparent: true, opacity: 0.3 + 0.6 * d.score }),
    );
    strip.position.set(x * XS + w / 2, laneH + GAP / 2, 0);
    scene.add(strip);
  });

  const lanes = files.map((f, li) => {
    const notes = [...f.notes].sort((a, b) => a[0] - b[0]);
    const mesh = new THREE.InstancedMesh(new THREE.BoxGeometry(1, 1, 1), new THREE.MeshLambertMaterial(), notes.length);
    const m = new THREE.Matrix4();
    const q = new THREE.Quaternion();
    const p = new THREE.Vector3();
    const s = new THREE.Vector3();
    const xs = notes.map((n, k) => {
      const x0 = al.at(f.piece, n[0]) * XS;
      const w = Math.max(0.08, al.at(f.piece, n[0] + n[1]) * XS - x0 - 0.04);
      const depth = 0.2 + (n[3] / 127) ** 2 * 1.6;
      p.set(x0 + w / 2, laneY[li] + (n[2] - pmin) * PS, depth / 2);
      s.set(w, PS * 0.8, depth);
      mesh.setMatrixAt(k, m.compose(p, q, s));
      mesh.setColorAt(k, bright[li]);
      return x0;
    });
    mesh.instanceMatrix.needsUpdate = true;
    scene.add(mesh);
    return { mesh, xs, li, lit: notes.length };
  });

  const head = new THREE.Mesh(
    new THREE.BoxGeometry(0.1, totalH + 1.8, 2.4),
    new THREE.MeshBasicMaterial({ color: '#ffc857', transparent: true, opacity: 0.9 }),
  );
  head.position.set(0, totalH / 2, 1.2);
  scene.add(head);

  const camera = new THREE.PerspectiveCamera(32, 2, 0.1, 10_000);
  const pos = new THREE.Vector3();
  const look = new THREE.Vector3();
  const tilt = new THREE.Vector2();
  let mode: 'follow' | 'whole' = 'follow';
  let active: HTMLAudioElement | null = null;
  let manual = 0;
  let shown = -1;
  let first = true;

  function playhead(): number {
    if (active) return al.at(active.dataset.piece!, active.currentTime * RATE) * XS;
    return manual;
  }

  function goal(x: number) {
    const t = Math.tan(THREE.MathUtils.degToRad(camera.fov / 2));
    if (mode === 'whole') {
      // The whole piece is some thirty times wider than tall, so it is seen from near its start,
      // receding along the time axis, rather than flat and thin.
      return [
        new THREE.Vector3(-totalW * 0.15 + tilt.x * 20, totalH * 1.3 + tilt.y * 12, totalW * 0.11),
        new THREE.Vector3(totalW * 0.3, totalH / 2, 0),
      ];
    }
    const d = (totalH * 1.3) / 2 / t;
    // Keep the view inside the piece at its start and end; the playhead itself still moves freely.
    const half = (totalH * 1.3 * camera.aspect) / 2;
    const cx = Math.min(Math.max(x, half - 10), Math.max(half - 10, totalW - half - 10));
    return [
      new THREE.Vector3(cx - 8 + tilt.x * 6, totalH / 2 + 5 + tilt.y * 4, d),
      new THREE.Vector3(cx + 10, totalH / 2, 0),
    ];
  }

  function light(x: number) {
    for (const lane of lanes) {
      let n = lane.lit;
      while (n < lane.xs.length && lane.xs[n] <= x) n++;
      while (n > 0 && lane.xs[n - 1] > x) n--;
      if (n === lane.lit) continue;
      const [from, to, c] = n > lane.lit ? [lane.lit, n, bright[lane.li]] : [n, lane.lit, dim[lane.li]];
      for (let k = from; k < to; k++) lane.mesh.setColorAt(k, c);
      lane.mesh.instanceColor!.needsUpdate = true;
      lane.lit = n;
    }
  }

  function show(i: number) {
    if (i === shown) return;
    shown = i;
    chips.forEach((c, k) => c.setAttribute('aria-pressed', String(k === i)));
    rows.forEach((r, k) => r.classList.toggle('sj-current', k === i));
    hud.textContent = `${sec.sections[i]}: ${diffs[i].text}.`;
  }

  function resize() {
    const w = stage.clientWidth;
    const h = stage.clientHeight;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
  }

  let visible = true;
  function frame() {
    if (!visible) return;
    const x = playhead();
    head.position.x = x;
    // Before anything has played every note is lit; once a recording plays, notes light as it reaches them.
    light(active ? x : Infinity);
    show(al.section(x / XS));
    const [p, l] = goal(x);
    const k = reduce || first ? 1 : 0.08;
    first = false;
    pos.lerp(p, k);
    look.lerp(l, k);
    camera.position.copy(pos);
    camera.lookAt(look);
    renderer.render(scene, camera);
    requestAnimationFrame(frame);
  }

  audios.forEach((a) =>
    a.addEventListener('play', () => {
      active = a;
      audios.forEach((o) => o !== a && o.pause());
      setMode('follow');
    }),
  );

  chips.forEach((c) =>
    c.addEventListener('click', () => {
      const i = Number(c.dataset.section);
      if (active) active.currentTime = bounds(sec, active.dataset.piece!)[i][0] / RATE;
      else manual = al.x0[i] * XS;
      setMode('follow');
    }),
  );

  function setMode(m: 'follow' | 'whole') {
    mode = m;
    modes.forEach((b) => b.setAttribute('aria-pressed', String(b.dataset.mode === m)));
  }
  modes.forEach((b) => b.addEventListener('click', () => setMode(b.dataset.mode as 'follow' | 'whole')));

  if (!reduce) {
    stage.addEventListener('pointermove', (e) => {
      const r = stage.getBoundingClientRect();
      tilt.set((e.clientX - r.left) / r.width - 0.5, 0.5 - (e.clientY - r.top) / r.height);
    });
    stage.addEventListener('pointerleave', () => tilt.set(0, 0));
  }

  new ResizeObserver(resize).observe(stage);
  resize();
  new IntersectionObserver(([e]) => {
    const was = visible;
    visible = e.isIntersecting;
    if (visible && !was) requestAnimationFrame(frame);
  }).observe(stage);
  requestAnimationFrame(frame);
}
