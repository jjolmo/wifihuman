<script lang="ts">
  import type { RadarDot } from '$lib/types';

  let { dots = [], isScanning = false }: { dots: RadarDot[]; isScanning: boolean } = $props();

  let sweepAngle = $state(0);
  let animFrame: number;

  $effect(() => {
    if (isScanning) {
      const animate = () => {
        sweepAngle = (sweepAngle + 2) % 360;
        animFrame = requestAnimationFrame(animate);
      };
      animFrame = requestAnimationFrame(animate);
      return () => cancelAnimationFrame(animFrame);
    }
  });

  let hoveredDot = $state<RadarDot | null>(null);

  function rssiLabel(rssi: number): string {
    if (rssi > -40) return 'Very close';
    if (rssi > -55) return 'Close';
    if (rssi > -70) return 'Medium';
    return 'Far';
  }
</script>

<div class="radar-container">
  <svg viewBox="0 0 400 400" class="radar">
    <defs>
      <radialGradient id="radarBg" cx="50%" cy="50%" r="50%">
        <stop offset="0%" stop-color="#00e5ff05" />
        <stop offset="100%" stop-color="#00e5ff00" />
      </radialGradient>
      <filter id="glow">
        <feGaussianBlur stdDeviation="3" result="blur" />
        <feMerge><feMergeNode in="blur" /><feMergeNode in="SourceGraphic" /></feMerge>
      </filter>
      <filter id="dotGlow">
        <feGaussianBlur stdDeviation="4" result="blur" />
        <feMerge><feMergeNode in="blur" /><feMergeNode in="SourceGraphic" /></feMerge>
      </filter>
    </defs>

    <!-- Background -->
    <circle cx="200" cy="200" r="190" fill="url(#radarBg)" />

    <!-- Range rings -->
    {#each [0.25, 0.5, 0.75, 1.0] as ring}
      <circle cx="200" cy="200" r={190 * ring} fill="none" stroke="#00e5ff" stroke-opacity="0.1" stroke-width="1" />
    {/each}

    <!-- Cross lines -->
    <line x1="200" y1="10" x2="200" y2="390" stroke="#00e5ff" stroke-opacity="0.08" stroke-width="1" />
    <line x1="10" y1="200" x2="390" y2="200" stroke="#00e5ff" stroke-opacity="0.08" stroke-width="1" />
    <line x1="65" y1="65" x2="335" y2="335" stroke="#00e5ff" stroke-opacity="0.05" stroke-width="1" />
    <line x1="335" y1="65" x2="65" y2="335" stroke="#00e5ff" stroke-opacity="0.05" stroke-width="1" />

    <!-- Range labels -->
    <text x="200" y={200 - 190 * 0.25 - 4} text-anchor="middle" class="range-label">~2m</text>
    <text x="200" y={200 - 190 * 0.5 - 4} text-anchor="middle" class="range-label">~5m</text>
    <text x="200" y={200 - 190 * 0.75 - 4} text-anchor="middle" class="range-label">~10m</text>
    <text x="200" y={200 - 190 * 1.0 - 4} text-anchor="middle" class="range-label">~20m+</text>

    <!-- Sweep line (when scanning) -->
    {#if isScanning}
      <line
        x1="200" y1="200"
        x2={200 + 190 * Math.cos((sweepAngle * Math.PI) / 180)}
        y2={200 + 190 * Math.sin((sweepAngle * Math.PI) / 180)}
        stroke="#00e5ff"
        stroke-opacity="0.6"
        stroke-width="2"
        filter="url(#glow)"
      />
      <!-- Sweep trail -->
      <path
        d="M 200 200 L {200 + 190 * Math.cos((sweepAngle * Math.PI) / 180)} {200 + 190 * Math.sin((sweepAngle * Math.PI) / 180)} A 190 190 0 0 0 {200 + 190 * Math.cos(((sweepAngle - 30) * Math.PI) / 180)} {200 + 190 * Math.sin(((sweepAngle - 30) * Math.PI) / 180)} Z"
        fill="#00e5ff"
        fill-opacity="0.06"
      />
    {/if}

    <!-- Device dots -->
    {#each dots as dot}
      {@const cx = dot.x * 400}
      {@const cy = dot.y * 400}
      {@const color = dot.device.is_randomized ? '#ff6b35' : '#00e5ff'}
      <circle
        cx={cx} cy={cy} r="8"
        fill={color} fill-opacity="0.3"
        filter="url(#dotGlow)"
      />
      <circle
        cx={cx} cy={cy} r="4"
        fill={color}
        class="dot"
        onmouseenter={() => hoveredDot = dot}
        onmouseleave={() => hoveredDot = null}
      />
    {/each}

    <!-- Center dot -->
    <circle cx="200" cy="200" r="5" fill="#00e5ff" filter="url(#glow)" />
    <circle cx="200" cy="200" r="2" fill="#fff" />
  </svg>

  <!-- Tooltip -->
  {#if hoveredDot}
    <div class="tooltip" style="left: {hoveredDot.x * 100}%; top: {hoveredDot.y * 100}%;">
      <div class="tt-mac">{hoveredDot.device.mac}</div>
      <div class="tt-info">
        <span>{hoveredDot.device.vendor}</span>
        <span>{hoveredDot.device.rssi} dBm ({rssiLabel(hoveredDot.device.rssi)})</span>
      </div>
      {#if hoveredDot.device.ssid_probed}
        <div class="tt-ssid">Probing: {hoveredDot.device.ssid_probed}</div>
      {/if}
      <div class="tt-type" class:random={hoveredDot.device.is_randomized}>
        {hoveredDot.device.is_randomized ? 'Randomized MAC' : 'Real MAC'}
      </div>
    </div>
  {/if}
</div>

<style>
  .radar-container {
    position: relative;
    width: 100%;
    max-width: 500px;
    aspect-ratio: 1;
    margin: 0 auto;
  }
  .radar {
    width: 100%;
    height: 100%;
  }
  .range-label {
    fill: #00e5ff;
    fill-opacity: 0.3;
    font-size: 10px;
    font-family: var(--font-mono);
  }
  .dot { cursor: pointer; transition: r 0.15s; }
  .dot:hover { r: 6; }

  .tooltip {
    position: absolute;
    transform: translate(-50%, -120%);
    background: #111820ee;
    border: 1px solid #1e2d3d;
    border-radius: 8px;
    padding: 10px 14px;
    pointer-events: none;
    white-space: nowrap;
    z-index: 10;
    backdrop-filter: blur(8px);
  }
  .tt-mac { font-family: var(--font-mono); font-size: 13px; color: #00e5ff; font-weight: 600; }
  .tt-info { display: flex; gap: 12px; font-size: 12px; color: #708090; margin-top: 4px; }
  .tt-ssid { font-size: 12px; color: #708090; margin-top: 2px; }
  .tt-type { font-size: 11px; font-weight: 600; margin-top: 4px; color: #00e5ff; }
  .tt-type.random { color: #ff6b35; }
</style>
