// Courbe en escalier, en SVG : une valeur ne change qu'a un event, elle reste
// constante entre deux. Pas de bibliotheque de graphiques pour une courbe.

export type Point = { t: number; v: number };

const W = 640;
const H = 180;
const PAD = { top: 12, right: 12, bottom: 24, left: 64 };

function day(t: number) {
  return new Date(t).toISOString().slice(0, 10);
}

// Echelle honnete : sans plancher, l'axe zoome sur l'ecart le plus faible et
// un mouvement de 0,05 % se dessine comme un effondrement. `scale` fixe le
// plancher : "zero" part de 0 (un encours), un nombre impose une amplitude
// minimale relative a la valeur (0.01 = 1 %, pour un prix par part).
export function StepChart({
  points,
  format,
  label,
  scale,
}: {
  points: Point[];
  format: (v: number) => string;
  label: string;
  scale?: "zero" | number;
}) {
  if (points.length === 0) {
    return <div className="muted-line">No data point yet.</div>;
  }

  // La courbe se prolonge jusqu'a maintenant : la derniere valeur vaut encore.
  const series = [...points, { t: Date.now(), v: points[points.length - 1].v }];
  const t0 = series[0].t;
  const t1 = Math.max(series[series.length - 1].t, t0 + 1);
  let lo = Math.min(...series.map((p) => p.v));
  let hi = Math.max(...series.map((p) => p.v));
  if (scale === "zero") {
    lo = Math.min(0, lo);
    if (hi === lo) hi = lo + 1;
    hi += (hi - lo) * 0.08;
  } else {
    const mid = (hi + lo) / 2;
    const minHalf = (Math.abs(mid) * (scale ?? 0.01)) / 2 || 0.5;
    const half = Math.max((hi - lo) / 2, minHalf);
    lo = mid - half;
    hi = mid + half;
  }
  const midValue = (hi + lo) / 2;

  const x = (t: number) => PAD.left + ((t - t0) / (t1 - t0)) * (W - PAD.left - PAD.right);
  const y = (v: number) => PAD.top + ((hi - v) / (hi - lo)) * (H - PAD.top - PAD.bottom);

  let path = `M ${x(series[0].t)} ${y(series[0].v)}`;
  for (let i = 1; i < series.length; i++) {
    path += ` H ${x(series[i].t)} V ${y(series[i].v)}`;
  }

  const baseline = y(lo);
  const area = `${path} V ${baseline} H ${x(series[0].t)} Z`;

  return (
    <svg className="chart" viewBox={`0 0 ${W} ${H}`} role="img" aria-label={label}>
      <line className="axis" x1={PAD.left} x2={W - PAD.right} y1={y(hi)} y2={y(hi)} />
      <line className="axis" x1={PAD.left} x2={W - PAD.right} y1={y(lo)} y2={y(lo)} />
      <text className="tick" x={PAD.left - 8} y={y(hi) + 4} textAnchor="end">
        {format(hi)}
      </text>
      <line className="axis" x1={PAD.left} x2={W - PAD.right} y1={y(midValue)} y2={y(midValue)} />
      <text className="tick" x={PAD.left - 8} y={y(midValue) + 4} textAnchor="end">
        {format(midValue)}
      </text>
      <text className="tick" x={PAD.left - 8} y={y(lo) + 4} textAnchor="end">
        {format(lo)}
      </text>
      <text className="tick" x={PAD.left} y={H - 6}>
        {day(t0)}
      </text>
      <text className="tick" x={W - PAD.right} y={H - 6} textAnchor="end">
        {day(t1)}
      </text>
      <path className="area" d={area} />
      <path className="line" d={path} pathLength={1} />
      {points.map((p, i) => (
        <circle key={i} className="dot-point" cx={x(p.t)} cy={y(p.v)} r={3} />
      ))}
    </svg>
  );
}
