// Dithered noise cloud: fbm Perlin noise, pixelated, quantised through an 8x8 Bayer
// matrix. The technique armory.in and React Bits' Dither use, written against raw
// WebGL1 so the page does not ship three.js for one full-screen quad.
//
// Classic 2D Perlin noise after Stefan Gustavson (MIT); Bayer recursion after the
// widely used GLSL one-liner (bayer2 composed into 4 and 8).
// https://github.com/stegu/webgl-noise/blob/1ea3a0b/src/classicnoise2D.glsl

const FRAG = `
precision highp float;
uniform vec2 res;
uniform float time;
uniform float px;
uniform float reveal;
uniform vec3 tint;

vec4 mod289(vec4 x) { return x - floor(x * (1.0 / 289.0)) * 289.0; }
vec4 permute(vec4 x) { return mod289(((x * 34.0) + 1.0) * x); }
vec4 taylorInvSqrt(vec4 r) { return 1.79284291400159 - 0.85373472095314 * r; }
vec2 fade(vec2 t) { return t * t * t * (t * (t * 6.0 - 15.0) + 10.0); }
float cnoise(vec2 P) {
  vec4 Pi = floor(P.xyxy) + vec4(0.0, 0.0, 1.0, 1.0);
  vec4 Pf = fract(P.xyxy) - vec4(0.0, 0.0, 1.0, 1.0);
  Pi = mod289(Pi);
  vec4 ix = Pi.xzxz, iy = Pi.yyww, fx = Pf.xzxz, fy = Pf.yyww;
  vec4 i = permute(permute(ix) + iy);
  vec4 gx = fract(i * (1.0 / 41.0)) * 2.0 - 1.0;
  vec4 gy = abs(gx) - 0.5;
  gx = gx - floor(gx + 0.5);
  vec2 g00 = vec2(gx.x, gy.x), g10 = vec2(gx.y, gy.y), g01 = vec2(gx.z, gy.z), g11 = vec2(gx.w, gy.w);
  vec4 norm = taylorInvSqrt(vec4(dot(g00, g00), dot(g01, g01), dot(g10, g10), dot(g11, g11)));
  g00 *= norm.x; g01 *= norm.y; g10 *= norm.z; g11 *= norm.w;
  float n00 = dot(g00, vec2(fx.x, fy.x)), n10 = dot(g10, vec2(fx.y, fy.y));
  float n01 = dot(g01, vec2(fx.z, fy.z)), n11 = dot(g11, vec2(fx.w, fy.w));
  vec2 f = fade(Pf.xy);
  vec2 nx = mix(vec2(n00, n01), vec2(n10, n11), f.x);
  return 2.3 * mix(nx.x, nx.y, f.y);
}
float fbm(vec2 p) {
  float v = 0.0, a = 1.0;
  for (int i = 0; i < 3; i++) { v += a * abs(cnoise(p)); p *= 3.0; a *= 0.4; }
  return v;
}
float bayer2(vec2 a) { a = floor(a); return fract(a.x / 2.0 + a.y * a.y * 0.75); }
float bayer4(vec2 a) { return bayer2(0.5 * a) * 0.25 + bayer2(a); }
float bayer8(vec2 a) { return bayer4(0.5 * a) * 0.25 + bayer2(a); }

void main() {
  vec2 cell = floor(gl_FragCoord.xy / px);
  vec2 uv = cell * px / res - 0.5;
  uv.x *= res.x / res.y;
  vec2 drift = vec2(time * 0.0075, time * 0.015);
  float base = fbm(uv);
  float f = fbm(uv - drift + base * 0.08);
  // reveal lifts the cloud out of the black, which is how a section transition
  // uses it: 0 is empty, 1 is the full cloud.
  f = smoothstep(0.25, 1.05, f) * reveal;
  float levels = 4.0;
  float q = floor(f * (levels - 1.0) + bayer8(cell)) / (levels - 1.0);
  gl_FragColor = vec4(tint * q, 1.0);
}`;

const VERT = `attribute vec2 p; void main() { gl_Position = vec4(p, 0.0, 1.0); }`;

export type Dither = { setReveal: (r: number) => void; destroy: () => void };

/** Mounts the cloud into a canvas that fills its parent. Renders only while on screen. */
export function mountDither(
  canvas: HTMLCanvasElement,
  tint: [number, number, number] = [0.42, 0.42, 0.45],
): Dither {
  const gl = canvas.getContext("webgl", { antialias: false, alpha: false });
  if (!gl) return { setReveal: () => {}, destroy: () => {} };
  const sh = (type: number, src: string) => {
    const s = gl.createShader(type)!;
    gl.shaderSource(s, src);
    gl.compileShader(s);
    return s;
  };
  const prog = gl.createProgram()!;
  gl.attachShader(prog, sh(gl.VERTEX_SHADER, VERT));
  gl.attachShader(prog, sh(gl.FRAGMENT_SHADER, FRAG));
  gl.linkProgram(prog);
  gl.useProgram(prog);
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  const loc = gl.getAttribLocation(prog, "p");
  gl.enableVertexAttribArray(loc);
  gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
  const u = (n: string) => gl.getUniformLocation(prog, n);
  const uRes = u("res"),
    uTime = u("time"),
    uPx = u("px"),
    uReveal = u("reveal");
  gl.uniform3f(u("tint"), ...tint);

  // The cloud is texture, not detail: render at half resolution on phones and at
  // 1x everywhere else, and let the browser upscale the pixelated result.
  const scale = innerWidth < 900 ? 0.5 : 1;
  const fit = () => {
    canvas.width = Math.round(canvas.clientWidth * scale);
    canvas.height = Math.round(canvas.clientHeight * scale);
    gl.viewport(0, 0, canvas.width, canvas.height);
    gl.uniform2f(uRes, canvas.width, canvas.height);
    gl.uniform1f(uPx, 3 * scale * Math.min(devicePixelRatio, 2));
  };
  const ro = new ResizeObserver(fit);
  ro.observe(canvas);
  fit();

  let reveal = 0;
  let visible = false;
  let raf = 0;
  const t0 = performance.now();
  // Fixed stages are always "in the viewport", so the observer below cannot tell
  // us the scene is off; the stage's own visibility can. setReveal() kicks the
  // loop again when the scene comes back.
  const stage = canvas.closest<HTMLElement>(".stage");
  const frame = () => {
    if (stage && stage.style.visibility === "hidden") {
      raf = 0;
      return;
    }
    gl.uniform1f(uTime, (performance.now() - t0) / 1000);
    gl.uniform1f(uReveal, reveal);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    raf = visible && reveal > 0 ? requestAnimationFrame(frame) : 0;
  };
  const kick = () => {
    if (!raf && visible) raf = requestAnimationFrame(frame);
  };
  const io = new IntersectionObserver(([e]) => {
    visible = e.isIntersecting;
    kick();
  });
  io.observe(canvas);

  return {
    setReveal: (r) => {
      reveal = r;
      kick();
    },
    destroy: () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      io.disconnect();
    },
  };
}
